//! Swift-facing FFI over `vibeproxy-core`, via uniffi. A thin adapter — the same role the CLI and the
//! Tauri app play. The core stays FFI-free; this crate is where Swift-callable functions live.
//!
//! Rich types (profiles, analytics) cross the boundary as JSON strings, so the Swift side decodes
//! them with Codable using the exact shapes the CLI's `--json` already emits — no need to mirror every
//! core struct as a uniffi Record. Actions return `Result`, which becomes a throwing Swift function.

use std::path::Path;
use vibeproxy_core::profile::store;
use vibeproxy_core::usage::poll_profile;
use vibeproxy_core::usage_analytics::{self, Range};

uniffi::setup_scaffolding!();

/// A single flat error the Swift side receives as a thrown exception.
#[derive(Debug, uniffi::Error)]
pub enum FfiError {
    Message(String),
}
impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let FfiError::Message(m) = self;
        f.write_str(m)
    }
}
impl std::error::Error for FfiError {}
fn err(e: impl std::fmt::Display) -> FfiError {
    FfiError::Message(e.to_string())
}

/// Version of the core, for a liveness/handshake check from Swift.
#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// The configured profiles, as a JSON array (same shape as `vibeproxy list --json`).
#[uniffi::export]
pub fn list_profiles_json() -> Result<String, FfiError> {
    serde_json::to_string(&store::load().profiles).map_err(err)
}

/// The active profile id, if any.
#[uniffi::export]
pub fn active_profile_id() -> Option<String> {
    store::load().active_profile_id
}

/// Usage analytics as JSON (same shape as `vibeproxy usage --json`). `range` is "FROM..TO" or nil.
#[uniffi::export]
pub fn usage_json(range: Option<String>) -> Result<String, FfiError> {
    let parsed = parse_range(range)?;
    serde_json::to_string(&usage_analytics::scan(&parsed)).map_err(err)
}

/// Make a profile active by id or label. New terminals pick it up via the shell integration.
#[uniffi::export]
pub fn switch_profile(target: String) -> Result<(), FfiError> {
    let cfg = store::load();
    let id = cfg
        .profiles
        .iter()
        .find(|p| p.id == target || p.label == target)
        .map(|p| p.id.clone())
        .ok_or_else(|| FfiError::Message(format!("no account matching \"{target}\"")))?;
    vibeproxy_core::switch::activate_profile(&id).map_err(err)
}

/// First run: if no profiles exist yet, adopt the default `~/.claude` login as "Main" and make it
/// active — the same bootstrap the Tauri app does, so there is always an account to show and switch.
/// Best-effort: a "not logged in / claude unavailable" error is returned for the app to surface, not
/// a failure to retry.
#[uniffi::export]
pub fn bootstrap_default_profile() -> Result<(), FfiError> {
    if !store::load().profiles.is_empty() {
        return Ok(());
    }
    vibeproxy_core::profile::adopt("Main".to_string(), "~/.claude")
        .map(|_| ())
        .map_err(FfiError::Message)
}

/// Live usage (5-hour and weekly quota %) for every configured account, as a JSON array of the
/// `ProfileUsage` shape. This — not historical token cost — is the number the menu bar shows. Polls
/// each account's usage endpoint; blocks on a small runtime so the Swift call is synchronous.
#[uniffi::export]
pub fn usage_all_json() -> Result<String, FfiError> {
    let cfg = store::load();
    let active = cfg.active_profile_id.clone();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(err)?;
    let usages = rt.block_on(async {
        // Sequential is fine at this account count and keeps the endpoint's rate limits happy.
        let mut out = Vec::with_capacity(cfg.profiles.len());
        for p in &cfg.profiles {
            let is_active = Some(&p.id) == active.as_ref();
            out.push(poll_profile(&p.id, Path::new(&p.config_dir), is_active).await);
        }
        out
    });
    serde_json::to_string(&usages).map_err(err)
}

/// Begin adding an account: create an isolated config dir and open a Terminal running the real
/// `claude auth login` scoped to it. Returns the pending config dir; the app polls `check_login_json`
/// until the browser OAuth completes, then calls `adopt_profile`.
#[uniffi::export]
pub fn begin_add_profile() -> Result<String, FfiError> {
    let dir = vibeproxy_core::onboarding::prepare().map_err(err)?;
    vibeproxy_core::onboarding::launch_login(&dir).map_err(err)?;
    Ok(dir)
}

/// Poll whether the login into `config_dir` has completed. Returns the account identity as JSON
/// (`AuthStatus`: `loggedIn`, `email`, `orgId`, `subscriptionType`).
#[uniffi::export]
pub fn check_login_json(config_dir: String) -> Result<String, FfiError> {
    let status =
        vibeproxy_core::profile::account_meta::fetch(Path::new(&config_dir)).map_err(err)?;
    serde_json::to_string(&status).map_err(err)
}

/// Register a logged-in config dir as a new account. Makes it active if it is the first profile.
#[uniffi::export]
pub fn adopt_profile(label: String, config_dir: String) -> Result<(), FfiError> {
    vibeproxy_core::profile::adopt(label, &config_dir).map(|_| ()).map_err(FfiError::Message)
}

/// Abandon an in-progress add (removes the not-yet-registered profile dir).
#[uniffi::export]
pub fn cancel_add_profile(config_dir: String) -> Result<(), FfiError> {
    vibeproxy_core::onboarding::cleanup(&config_dir).map_err(err)
}

/// Remove an account from VibeProxy (leaves its Claude login / Keychain item untouched). If it was
/// active, re-point to the first remaining profile, or clear the active path so terminals fall back
/// to the default account.
#[uniffi::export]
pub fn remove_profile(id: String) -> Result<(), FfiError> {
    let was_active = store::load().active_profile_id.as_deref() == Some(id.as_str());
    store::remove_profile(&id).map_err(err)?;
    if was_active {
        match store::load().profiles.first().map(|p| p.id.clone()) {
            Some(next) => vibeproxy_core::switch::activate_profile(&next).map_err(FfiError::Message)?,
            None => vibeproxy_core::switch::set_active_config_dir("").map_err(err)?,
        }
    }
    Ok(())
}

/// The shell integration line, so the Swift app can show/copy it like the Tauri app does.
#[uniffi::export]
pub fn shell_snippet() -> String {
    vibeproxy_core::shell::snippet()
}

/// Parse a "FROM..TO" range; either side may be empty. Mirrors the CLI.
fn parse_range(range: Option<String>) -> Result<Option<Range>, FfiError> {
    let Some(r) = range else { return Ok(None) };
    let (from, to) = r
        .split_once("..")
        .ok_or_else(|| FfiError::Message("range must look like FROM..TO".into()))?;
    let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
    Ok(Some(Range { from: opt(from), to: opt(to) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vibeproxy_core::profile::paths;

    #[test]
    fn core_version_matches_the_crate() {
        assert_eq!(core_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn parse_range_forms() {
        assert!(parse_range(None).unwrap().is_none());
        let r = parse_range(Some("2026-07-01..2026-07-31".into())).unwrap().unwrap();
        assert_eq!(r.from.as_deref(), Some("2026-07-01"));
        assert_eq!(r.to.as_deref(), Some("2026-07-31"));
        // one-sided
        assert!(parse_range(Some("2026-07-01..".into())).unwrap().unwrap().to.is_none());
        // malformed
        assert!(parse_range(Some("nonsense".into())).is_err());
    }

    #[test]
    fn empty_state_gives_valid_json_and_clean_errors() {
        let _g = paths::ENV_SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = std::env::temp_dir().join(format!("vp-ffi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("VIBEPROXY_DIR", &tmp);
        assert_eq!(paths::vibeproxy_dir(), tmp, "isolation in effect");

        // list on empty → "[]"; usage → valid Analytics JSON; unknown switch → Err.
        assert_eq!(list_profiles_json().unwrap(), "[]");
        // usage_all with no profiles is a network-free empty array (no account to poll).
        assert_eq!(usage_all_json().unwrap(), "[]");
        assert!(active_profile_id().is_none());
        let usage = usage_json(None).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&usage).unwrap()["totals"].is_object());
        assert!(usage_json(Some("bad".into())).is_err());
        assert!(matches!(switch_profile("ghost".into()), Err(FfiError::Message(_))));

        std::env::remove_var("VIBEPROXY_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
