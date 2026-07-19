//! VibeProxy — menubar app to switch between multiple Claude Code accounts.
//! Phase 2: adopt existing logins, read identity, and switch the active profile by real path.

mod autoswitch;
mod keychain;
mod onboarding;
mod platform;
mod profile;
mod switch;
mod tray;
mod usage;
mod usage_analytics;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use profile::store;

/// All configured profiles (UI reads this to render the list).
#[tauri::command]
fn list_profiles() -> Vec<profile::Profile> {
    store::load().profiles
}

/// Current settings.
#[tauri::command]
fn get_settings() -> profile::Settings {
    store::load().settings
}

/// Update settings (validated + persisted). The poller picks up threshold/interval/cooldown on its
/// next tick; launch-at-login is applied immediately.
#[tauri::command]
fn set_settings(app: AppHandle, mut settings: profile::Settings) -> Result<profile::Settings, String> {
    settings.threshold_pct = settings.threshold_pct.clamp(50, 100);
    settings.poll_interval_secs = settings.poll_interval_secs.max(60);
    settings.cooldown_secs = settings.cooldown_secs.max(30);
    store::set_settings(settings.clone()).map_err(|e| e.to_string())?;

    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    let _ = if settings.launch_at_login {
        autostart.enable()
    } else {
        autostart.disable()
    };
    Ok(settings)
}

/// Id of the active profile, if any.
#[tauri::command]
fn get_active_profile_id() -> Option<String> {
    store::load().active_profile_id
}

/// Latest usage snapshot for all profiles (the UI also gets live `usage-updated` events).
#[tauri::command]
async fn get_usage(state: tauri::State<'_, usage::UsageState>) -> Result<Vec<usage::ProfileUsage>, ()> {
    Ok(state.read().await.values().cloned().collect())
}

/// Token-usage analytics parsed from every account's local Claude Code logs (read-only).
#[tauri::command]
async fn get_usage_analytics(
    range: Option<usage_analytics::Range>,
) -> Result<usage_analytics::Analytics, String> {
    tauri::async_runtime::spawn_blocking(move || usage_analytics::scan(&range))
        .await
        .map_err(|e| format!("usage scan task failed: {e}"))
}

/// Adopt an existing Claude Code login at `config_dir` as a new profile. Reads identity via
/// `claude auth status`; rejects a dir with no logged-in account, and de-dupes by org.
#[tauri::command]
fn adopt_profile(
    app: AppHandle,
    label: String,
    config_dir: String,
) -> Result<profile::Profile, String> {
    let config_dir = expand_tilde(config_dir);
    let status = profile::account_meta::fetch(Path::new(&config_dir))?;
    if !status.logged_in {
        return Err("no logged-in Claude account at that config dir".to_string());
    }
    let cfg = store::load();
    if let Some(existing) = cfg.profiles.iter().find(|p| {
        p.config_dir == config_dir || (p.org_id.is_some() && p.org_id == status.org_id)
    }) {
        return Err(format!("that account is already added as \"{}\"", existing.label));
    }
    let is_first = cfg.profiles.is_empty();
    let profile = profile::Profile {
        id: store::new_id(),
        label,
        config_dir,
        email: status.email,
        org_id: status.org_id,
        subscription_type: status.subscription_type,
        priority: cfg.profiles.len() as i32,
        created_at: String::new(),
    };
    store::add_profile(profile.clone()).map_err(|e| e.to_string())?;
    if is_first {
        activate(&app, &profile.id)?;
    }
    tray::refresh(&app);
    Ok(profile)
}

/// Start adding a new account: create an isolated config dir and open the browser login for it.
/// Returns the pending config dir; the UI polls `check_login_status`, then calls `adopt_profile`.
#[tauri::command]
fn begin_add_profile() -> Result<PendingAdd, String> {
    let config_dir = onboarding::prepare().map_err(|e| e.to_string())?;
    onboarding::launch_login(&config_dir).map_err(|e| e.to_string())?;
    Ok(PendingAdd { config_dir })
}

/// Poll whether the login into `config_dir` has completed (and read the account it bound to).
#[tauri::command]
fn check_login_status(config_dir: String) -> Result<profile::account_meta::AuthStatus, String> {
    profile::account_meta::fetch(Path::new(&config_dir))
}

/// Abandon an in-progress add (removes the not-yet-registered profile dir).
#[tauri::command]
fn cancel_add_profile(config_dir: String) -> Result<(), String> {
    onboarding::cleanup(&config_dir).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingAdd {
    config_dir: String,
}

/// Make a profile active (next `claude` launch uses it).
#[tauri::command]
fn set_active_profile(app: AppHandle, id: String) -> Result<(), String> {
    activate(&app, &id)
}

/// Open a Terminal running `claude` on the active profile (so a just-switched account takes effect
/// immediately instead of waiting for the user to restart).
#[tauri::command]
fn relaunch_claude() -> Result<(), String> {
    let cfg = store::load();
    let dir = cfg
        .active_profile_id
        .and_then(|id| cfg.profiles.into_iter().find(|p| p.id == id))
        .map(|p| p.config_dir);
    match dir {
        Some(d) => switch::launch_claude(Path::new(&d)).map_err(|e| e.to_string()),
        None => Err("no active profile".to_string()),
    }
}

/// Remove a profile from VibeProxy (does not touch its Keychain item or config dir).
#[tauri::command]
fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    let was_active = store::load().active_profile_id.as_deref() == Some(id.as_str());
    store::remove_profile(&id).map_err(|e| e.to_string())?;
    if was_active {
        // Don't leave active-path pointing at a removed dir: re-point to the first remaining
        // profile, or clear it (→ shell falls back to the default account).
        let next = store::load().profiles.first().map(|p| p.id.clone());
        match next {
            Some(pid) => activate(&app, &pid)?,
            None => switch::set_active_config_dir("").map_err(|e| e.to_string())?,
        }
    }
    tray::refresh(&app);
    Ok(())
}

/// Reorder profiles by id (also sets auto-switch priority).
#[tauri::command]
fn reorder_profiles(app: AppHandle, order: Vec<String>) -> Result<(), String> {
    store::reorder(&order).map_err(|e| e.to_string())?;
    tray::refresh(&app);
    Ok(())
}

/// Refresh a profile's account identity from `claude auth status` (e.g. after a re-login).
#[tauri::command]
fn refresh_profile_meta(app: AppHandle, id: String) -> Result<profile::Profile, String> {
    let mut p = store::find(&id).ok_or("no such profile")?;
    let status = profile::account_meta::fetch(Path::new(&p.config_dir))?;
    p.email = status.email;
    p.org_id = status.org_id;
    p.subscription_type = status.subscription_type;
    store::update_profile(p.clone()).map_err(|e| e.to_string())?;
    tray::refresh(&app);
    Ok(p)
}

/// Core switch: point active-path at the profile's real config dir, persist, refresh the tray.
/// Shared by the tray click handler and the `set_active_profile` command.
pub(crate) fn activate(app: &AppHandle, id: &str) -> Result<(), String> {
    let p = store::find(id).ok_or("no such profile")?;
    // Default account → clear active-path so the shell UNSETS CLAUDE_CONFIG_DIR (see paths::is_default).
    // Any other profile → write its real path.
    let write_val = if profile::paths::is_default(Path::new(&p.config_dir)) {
        ""
    } else {
        &p.config_dir
    };
    switch::set_active_config_dir(write_val).map_err(|e| e.to_string())?;
    store::set_active_profile_id(id).map_err(|e| e.to_string())?;
    tray::refresh(app);
    Ok(())
}

/// Expand a leading `~/` to the home dir (Rust's `Command`/paths don't do shell tilde expansion).
fn expand_tilde(p: String) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    p
}

/// First run: if there are no profiles yet, adopt the default `~/.claude` login as "Main".
fn bootstrap_default_profile(app: &AppHandle) {
    if !store::load().profiles.is_empty() {
        return;
    }
    let Some(default_dir) = dirs::home_dir().map(|h| h.join(".claude")) else {
        return;
    };
    match profile::account_meta::fetch(&default_dir) {
        Ok(status) if status.logged_in => {
            let profile = profile::Profile {
                id: store::new_id(),
                label: "Main".to_string(),
                config_dir: default_dir.to_string_lossy().to_string(),
                email: status.email,
                org_id: status.org_id,
                subscription_type: status.subscription_type,
                priority: 0,
                created_at: String::new(),
            };
            if store::add_profile(profile.clone()).is_ok() {
                let _ = activate(app, &profile.id);
            }
        }
        _ => { /* not logged in / claude unavailable — leave empty, UI offers Add */ }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let usage_state: usage::UsageState = Arc::new(RwLock::new(HashMap::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(usage_state.clone())
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            get_settings,
            set_settings,
            get_active_profile_id,
            get_usage,
            get_usage_analytics,
            adopt_profile,
            begin_add_profile,
            check_login_status,
            cancel_add_profile,
            set_active_profile,
            relaunch_claude,
            delete_profile,
            reorder_profiles,
            refresh_profile_meta,
        ])
        .setup(move |app| {
            platform::hide_dock_icon(app);
            store::ensure_initialized()?;
            let handle = app.handle().clone();
            tray::build_tray(&handle)?;
            bootstrap_default_profile(&handle);
            onboarding::gc_orphans();
            usage::poller::spawn(handle, usage_state);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
