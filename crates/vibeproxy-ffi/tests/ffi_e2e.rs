//! End-to-end tests of the FFI surface the macOS app calls, over an isolated `VIBEPROXY_DIR`.
//!
//! Covers the local, deterministic paths: empty-state JSON, remove-then-re-point, the bootstrap
//! gate, and the cancel guard. The network/subprocess/GUI paths (live quota poll, adopt via a real
//! login, relaunch, the add-account browser flow) are verified live and recorded in
//! `plans/reports/` — they can't be hermetic here.

use vibeproxy_core::profile::{paths, store, Profile};
use vibeproxy_ffi::*;

/// Run `f` with a throwaway `VIBEPROXY_DIR`, serialized against every other env-mutating test.
fn with_temp(f: impl FnOnce()) {
    let _g = paths::ENV_SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let tmp = std::env::temp_dir().join(format!("vp-ffi-e2e-{}", store::new_id()));
    std::env::set_var("VIBEPROXY_DIR", &tmp);
    // Never touch the developer's real state, even if the redirect somehow isn't in effect.
    assert_eq!(paths::vibeproxy_dir(), tmp, "VIBEPROXY_DIR redirect not in effect");
    store::ensure_initialized().unwrap();
    f();
    std::env::remove_var("VIBEPROXY_DIR");
    let _ = std::fs::remove_dir_all(&tmp);
}

fn seed(id: &str) -> Profile {
    Profile {
        id: id.into(),
        label: id.into(),
        config_dir: format!("/tmp/{id}"),
        email: None,
        org_id: None,
        subscription_type: None,
        priority: 0,
        created_at: String::new(),
    }
}

#[test]
fn empty_state_is_valid_and_network_free() {
    with_temp(|| {
        assert_eq!(list_profiles_json().unwrap(), "[]");
        assert!(active_profile_id().is_none());
        assert_eq!(usage_all_json().unwrap(), "[]", "no accounts → no poll, empty array");
        // usage_json scans Claude Code's logs (not our dir), so it returns a valid aggregate.
        let usage = usage_json(None).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&usage).unwrap()["totals"].is_object());
    });
}

#[test]
fn remove_active_repoints_to_the_next_profile() {
    with_temp(|| {
        store::add_profile(seed("a")).unwrap();
        store::add_profile(seed("b")).unwrap();
        store::set_active_profile_id("a").unwrap();

        remove_profile("a".into()).unwrap();

        let cfg = store::load();
        let ids: Vec<_> = cfg.profiles.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["b"], "removed a, kept b");
        assert_eq!(cfg.active_profile_id.as_deref(), Some("b"), "active re-pointed to b");
    });
}

#[test]
fn remove_last_account_clears_active() {
    with_temp(|| {
        store::add_profile(seed("only")).unwrap();
        store::set_active_profile_id("only").unwrap();

        remove_profile("only".into()).unwrap();

        let cfg = store::load();
        assert!(cfg.profiles.is_empty());
        assert!(cfg.active_profile_id.is_none(), "no profile left → active cleared");
    });
}

#[test]
fn bootstrap_is_a_no_op_when_profiles_exist() {
    with_temp(|| {
        store::add_profile(seed("existing")).unwrap();
        // Non-empty → returns Ok without touching the network or the profile set.
        bootstrap_default_profile().unwrap();
        let ids: Vec<_> = store::load().profiles.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids, ["existing"], "bootstrap left the existing profile untouched");
    });
}

#[test]
fn cancel_add_ignores_paths_outside_our_profiles_dir() {
    with_temp(|| {
        // Guarded: cleanup only removes dirs under ~/.vibeproxy/profiles, so this is a safe no-op.
        cancel_add_profile("/etc".into()).unwrap();
        assert!(std::path::Path::new("/etc").exists());
    });
}

#[test]
fn relaunch_without_an_active_account_errors_cleanly() {
    with_temp(|| {
        assert!(relaunch_claude().is_err(), "no active account → a clean error, not a panic");
    });
}
