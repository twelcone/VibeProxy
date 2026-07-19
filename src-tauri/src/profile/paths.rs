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
