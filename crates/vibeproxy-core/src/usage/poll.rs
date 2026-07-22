//! One-shot usage fetch for a single profile — the unit the app poller loops over and the CLI
//! calls directly. No timer, no events; those belong to the caller.

use crate::platform::{self, CredentialStore};
use crate::profile;
use crate::usage::{client, model::ProfileUsage};
use std::path::Path;

/// Fetch one profile's usage. `is_active == false` first "touches" the dir via `claude auth status`
/// so the official client refreshes an otherwise-expiring token (ToS-safe). Blocking work (keychain,
/// subprocess) runs off the async executor via `spawn_blocking`.
pub async fn poll_profile(profile_id: &str, config_dir: &Path, is_active: bool) -> ProfileUsage {
    if !is_active {
        let dir = config_dir.to_path_buf();
        let _ = tokio::task::spawn_blocking(move || {
            let _ = profile::account_meta::fetch(&dir);
        })
        .await;
    }

    // A Keychain failure is transient (locked / not-yet-authorized) → Error, NOT NeedsReauth;
    // only a 401 from the usage endpoint means the token is actually invalid.
    let token = {
        let dir = config_dir.to_path_buf();
        match tokio::task::spawn_blocking(move || platform::credentials().read_token(&dir)).await {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => return ProfileUsage::error(profile_id, format!("credentials: {e}")),
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
