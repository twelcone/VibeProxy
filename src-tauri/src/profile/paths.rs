//! Filesystem layout for VibeProxy. The ONLY place that knows where things live.

use std::path::{Path, PathBuf};

/// The default Claude Code config dir (`~/.claude`). This account is special: Claude only reads its
/// (bare) Keychain item when `CLAUDE_CONFIG_DIR` is **unset** — setting the var to this path makes
/// Claude hash it and look for a non-existent service. So the default is used by *clearing* the env,
/// never by pointing at it. (Empirically verified — see spike findings.)
pub fn default_config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Is this the default `~/.claude` config dir?
pub fn is_default(config_dir: &Path) -> bool {
    default_config_dir().as_deref() == Some(config_dir)
}

/// Root dir VibeProxy owns: `~/.vibeproxy`. Never inside `~/.claude`.
/// Honors `VIBEPROXY_DIR` when set (used by tests to isolate state; also lets a user relocate it).
pub fn vibeproxy_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("VIBEPROXY_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::home_dir()
        .expect("could not resolve home directory")
        .join(".vibeproxy")
}

/// Persistent app state: `~/.vibeproxy/config.json`.
pub fn config_path() -> PathBuf {
    vibeproxy_dir().join("config.json")
}

/// Plain file holding the active profile's real config-dir path.
/// The user's shell reads this: `export CLAUDE_CONFIG_DIR="$(cat ~/.vibeproxy/active-path ...)"`.
/// Written by the switch broker in Phase 2.
#[allow(dead_code)]
pub fn active_path_file() -> PathBuf {
    vibeproxy_dir().join("active-path")
}

/// Dir holding VibeProxy-created profile config dirs (`~/.vibeproxy/profiles/<id>`).
/// Adopted profiles (e.g. the default `~/.claude` or an existing `~/vp-spike`) keep their
/// own path and live outside this dir — a profile's `config_dir` is always absolute.
pub fn profiles_dir() -> PathBuf {
    vibeproxy_dir().join("profiles")
}

/// Append-only log of account-occupancy boundaries: `~/.vibeproxy/swaps.jsonl`.
/// Written when an account is hot-swapped into a config dir; read by the analytics scanner so
/// usage is attributed to whoever owned the dir at the time, not to whoever owns it now.
pub fn swap_journal_file() -> PathBuf {
    vibeproxy_dir().join("swaps.jsonl")
}

/// `set_var` is process-global while Rust runs tests on parallel threads, so every test that
/// touches `VIBEPROXY_DIR` must take THIS lock — one per process, not one per module. Two separate
/// mutexes guarding the same global provide no mutual exclusion at all.
#[cfg(test)]
pub(crate) static ENV_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    /// The default account is load-bearing: Claude only reads its bare Keychain item when
    /// `CLAUDE_CONFIG_DIR` is unset, so misidentifying it sends the switch down the wrong path.
    #[test]
    fn only_the_real_default_dir_is_treated_as_default() {
        let home = dirs::home_dir().expect("home dir");
        assert!(is_default(&home.join(".claude")));
        assert!(!is_default(&home.join(".claude-other")));
        assert!(!is_default(&home.join(".vibeproxy/profiles/p_1")));
        assert!(!is_default(Path::new("/tmp/.claude")), "same leaf, different parent");
    }

    #[test]
    fn vibeproxy_dir_override_and_fallback() {
        let _guard = ENV_SERIAL.lock().unwrap_or_else(|p| p.into_inner());

        let tmp = std::env::temp_dir().join("vp-paths-test");
        std::env::set_var("VIBEPROXY_DIR", &tmp);
        assert_eq!(vibeproxy_dir(), tmp);
        assert_eq!(config_path(), tmp.join("config.json"));
        assert_eq!(swap_journal_file(), tmp.join("swaps.jsonl"));

        // An empty override is treated as unset, not as the filesystem root.
        std::env::set_var("VIBEPROXY_DIR", "");
        assert!(vibeproxy_dir().ends_with(".vibeproxy"));

        std::env::remove_var("VIBEPROXY_DIR");
        assert!(vibeproxy_dir().ends_with(".vibeproxy"));
    }
}
