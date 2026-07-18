//! Filesystem layout for VibeProxy. The ONLY place that knows where things live.

use std::path::PathBuf;

/// Root dir VibeProxy owns: `~/.vibeproxy`. Never inside `~/.claude`.
pub fn vibeproxy_dir() -> PathBuf {
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
