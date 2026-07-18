//! VibeProxy — menubar app to switch between multiple Claude Code accounts.
//! Phase 2: adopt existing logins, read identity, and switch the active profile by real path.

mod keychain;
mod platform;
mod profile;
mod switch;
mod tray;

use std::path::Path;
use tauri::AppHandle;

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

/// Id of the active profile, if any.
#[tauri::command]
fn get_active_profile_id() -> Option<String> {
    store::load().active_profile_id
}

/// Adopt an existing Claude Code login at `config_dir` as a new profile. Reads identity via
/// `claude auth status`; rejects a dir with no logged-in account, and de-dupes by org.
#[tauri::command]
fn adopt_profile(
    app: AppHandle,
    label: String,
    config_dir: String,
) -> Result<profile::Profile, String> {
    let status = profile::account_meta::fetch(Path::new(&config_dir))?;
    if !status.logged_in {
        return Err("no logged-in Claude account at that config dir".to_string());
    }
    let cfg = store::load();
    if let Some(existing) = cfg
        .profiles
        .iter()
        .find(|p| p.org_id.is_some() && p.org_id == status.org_id)
    {
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

/// Make a profile active (next `claude` launch uses it).
#[tauri::command]
fn set_active_profile(app: AppHandle, id: String) -> Result<(), String> {
    activate(&app, &id)
}

/// Remove a profile from VibeProxy (does not touch its Keychain item or config dir).
#[tauri::command]
fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    store::remove_profile(&id).map_err(|e| e.to_string())?;
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
    store::remove_profile(&id).map_err(|e| e.to_string())?;
    store::add_profile(p.clone()).map_err(|e| e.to_string())?;
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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            get_settings,
            get_active_profile_id,
            adopt_profile,
            set_active_profile,
            delete_profile,
            reorder_profiles,
            refresh_profile_meta,
        ])
        .setup(|app| {
            platform::hide_dock_icon(app);
            store::ensure_initialized()?;
            let handle = app.handle().clone();
            tray::build_tray(&handle)?;
            bootstrap_default_profile(&handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
