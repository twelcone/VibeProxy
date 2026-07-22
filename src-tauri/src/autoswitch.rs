//! App-side auto-switch actor. The pure decision lives in `vibeproxy_core::autoswitch::decide`;
//! this interprets the `Decision` and performs the side effects a GUI owns: activate the profile,
//! notify, emit events, and (opt-in) hot-swap the running session's credentials.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use vibeproxy_core::autoswitch::{decide, Decision};
use vibeproxy_core::profile::store::Config;
use vibeproxy_core::usage::ProfileUsage;

/// Evaluate and act: switch + notify, or warn. `cooldown_until` guards against rapid re-switching.
pub fn maybe_switch(
    app: &AppHandle,
    cfg: &Config,
    usage: &HashMap<String, ProfileUsage>,
    cooldown_until: &mut Option<Instant>,
) {
    let cooling = matches!(cooldown_until, Some(t) if Instant::now() < *t);
    match decide(cfg, usage, cooling) {
        Decision::None => {}
        Decision::Switch { target_id, target_label, from_label, pct } => {
            if crate::activate(app, &target_id).is_ok() {
                *cooldown_until = Some(Instant::now() + Duration::from_secs(cfg.settings.cooldown_secs));

                // The path file only affects newly launched terminals, which is no help to the
                // session that just hit the limit. When enabled, also move the credentials into
                // the dir that session is already using. Always write the path file too, so a new
                // terminal and a hot-swapped session never disagree about the active account.
                let hot = if cfg.settings.hot_swap_enabled {
                    hot_swap_active_session(cfg, &target_id, &target_label)
                } else {
                    None
                };
                let tail = match hot {
                    Some(true) => "Your running session switched too.",
                    Some(false) => "Couldn't switch the running session — relaunch Claude Code to use it.",
                    None => "Relaunch Claude Code to use it.",
                };
                notify(
                    app,
                    &format!("Switched to {target_label}"),
                    &format!("{from_label} reached {pct}% — moved to a fresher account. {tail}"),
                );
                let _ = app.emit(
                    "auto-switched",
                    serde_json::json!({ "from": from_label, "to": target_label, "pct": pct }),
                );
            }
        }
        Decision::Blocked { from_label, pct } => {
            notify(
                app,
                "All accounts near their limit",
                &format!("{from_label} is at {pct}% and no other account has headroom. Try again after a reset."),
            );
            *cooldown_until = Some(Instant::now() + Duration::from_secs(cfg.settings.cooldown_secs));
            let _ = app.emit("auto-switch-blocked", serde_json::json!({ "active": from_label, "pct": pct }));
        }
    }
}

/// Swap the target account's credentials into the dir the *previously active* profile uses, which
/// is the dir any running session is bound to. Returns None when there is nothing to swap.
fn hot_swap_active_session(cfg: &Config, target_id: &str, target_label: &str) -> Option<bool> {
    let previous = cfg.active_profile_id.as_deref()?;
    if previous == target_id {
        return None;
    }
    let dir_of = |id: &str| cfg.profiles.iter().find(|p| p.id == id).map(|p| p.config_dir.clone());
    let target_dir = dir_of(target_id)?;
    let session_dir = dir_of(previous)?;

    match vibeproxy_core::switch::hotswap::swap_into(
        std::path::Path::new(&session_dir),
        std::path::Path::new(&target_dir),
        target_id,
        target_label,
    ) {
        Ok(()) => Some(true),
        // Never surface the underlying error text: it can carry keychain detail.
        Err(_) => Some(false),
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}
