//! Background poller: fetch each profile's usage on a timer, update shared state, notify UI + tray.
//!
//! The active profile is polled every tick; inactive profiles are polled lazily (every Nth tick)
//! to keep the undocumented-endpoint / ToS surface small. Before polling an inactive profile we
//! "touch" it via `claude auth status` so the official client refreshes its (otherwise-expiring) token.

use super::{client, model::ProfileUsage};
use crate::{keychain, profile, tray};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

/// Poll inactive profiles once every N ticks (active profile is every tick).
const INACTIVE_EVERY: u64 = 5;

/// Shared, last-known usage per profile id.
pub type UsageState = Arc<RwLock<HashMap<String, ProfileUsage>>>;

/// Spawn the poll loop on Tauri's async runtime.
pub fn spawn(app: AppHandle, state: UsageState) {
    tauri::async_runtime::spawn(async move {
        let mut tick: u64 = 0;
        loop {
            let cfg = profile::store::load();
            let interval = cfg.settings.poll_interval_secs.max(60);
            let active_id = cfg.active_profile_id.clone();
            let known: HashSet<String> = state.read().await.keys().cloned().collect();

            for p in &cfg.profiles {
                let is_active = Some(&p.id) == active_id.as_ref();
                // Poll: the active profile every tick, never-seen profiles immediately, others lazily.
                let never_polled = !known.contains(&p.id);
                if !is_active && !never_polled && tick % INACTIVE_EVERY != 0 {
                    continue;
                }
                let usage = poll_one(&p.id, PathBuf::from(&p.config_dir), is_active).await;
                state.write().await.insert(p.id.clone(), usage);
            }

            // Drop usage for profiles that no longer exist.
            {
                let ids: HashSet<&str> = cfg.profiles.iter().map(|p| p.id.as_str()).collect();
                state.write().await.retain(|k, _| ids.contains(k.as_str()));
            }

            // Notify the UI, then refresh the tray meter for the active profile.
            let snapshot: Vec<ProfileUsage> = state.read().await.values().cloned().collect();
            let _ = app.emit("usage-updated", &snapshot);
            {
                let guard = state.read().await;
                tray::apply_active_usage(&app, &cfg, &guard);
            }

            tick = tick.wrapping_add(1);
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });
}

async fn poll_one(profile_id: &str, config_dir: PathBuf, is_active: bool) -> ProfileUsage {
    // Keep inactive profiles' tokens fresh via the official client (ToS-safe refresh).
    if !is_active {
        let dir = config_dir.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let _ = profile::account_meta::fetch(&dir);
        })
        .await;
    }

    // Read the token off the async executor (Keychain access may block on a GUI prompt once).
    // A Keychain failure is transient (locked / not-yet-authorized) → Error, NOT NeedsReauth;
    // only a 401 from the usage endpoint means the token is actually invalid.
    let token = {
        let dir = config_dir.clone();
        match tauri::async_runtime::spawn_blocking(move || keychain::read_token(&dir)).await {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => return ProfileUsage::error(profile_id, format!("keychain: {e}")),
            Err(_) => return ProfileUsage::error(profile_id, "keychain read task failed".into()),
        }
    };

    match client::fetch(token.expose()).await {
        Ok(r) => ProfileUsage::ok(
            profile_id,
            r.five_hour.as_ref().and_then(|w| w.utilization),
            r.five_hour.and_then(|w| w.resets_at),
            r.seven_day.as_ref().and_then(|w| w.utilization),
            r.seven_day.and_then(|w| w.resets_at),
        ),
        Err(client::FetchError::Unauthorized) => ProfileUsage::needs_reauth(profile_id),
        Err(client::FetchError::Other(e)) => ProfileUsage::error(profile_id, e),
    }
}
