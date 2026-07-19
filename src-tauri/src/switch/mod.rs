//! The switch broker: which profile the next `claude` launch uses.
//!
//! Phase 0 established the switch must target each profile's REAL config-dir path (a fixed symlink
//! would collide all profiles onto one Keychain item). So "active" is a real path written to
//! `~/.vibeproxy/active-path`, which the user's shell reads into CLAUDE_CONFIG_DIR.

use crate::profile::paths;
use std::{fs, io::Write, path::Path};

/// The active profile's real config-dir path, if set.
#[allow(dead_code)]
pub fn active_config_dir() -> Option<String> {
    fs::read_to_string(paths::active_path_file())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Atomically point the active profile at `config_dir` (temp file + rename, so a crash can't
/// leave a half-written path). Takes effect on the next `claude` launch; running sessions are unaffected.
pub fn set_active_config_dir(config_dir: &str) -> std::io::Result<()> {
    fs::create_dir_all(paths::vibeproxy_dir())?;
    let path = paths::active_path_file();
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(config_dir.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Open Terminal running `claude` scoped to a profile (macOS). Powers the "relaunch" action.
/// The default account must run with `CLAUDE_CONFIG_DIR` UNSET (setting it to `~/.claude` breaks it).
pub fn launch_claude(config_dir: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let cmd = if crate::profile::paths::is_default(config_dir) {
            "claude".to_string()
        } else {
            format!("export CLAUDE_CONFIG_DIR={} && claude", config_dir.display())
        };
        let script = format!("tell application \"Terminal\" to do script \"{cmd}\"");
        Command::new("osascript").args(["-e", &script]).spawn()?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = config_dir;
    Ok(())
}
