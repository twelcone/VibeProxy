//! The switch broker: which profile the next `claude` launch uses.
//!
//! Phase 0 established the switch must target each profile's REAL config-dir path (a fixed symlink
//! would collide all profiles onto one Keychain item). So "active" is a real path written to
//! `~/.vibeproxy/active-path`, which the user's shell reads into CLAUDE_CONFIG_DIR.

pub mod journal;
pub mod hotswap;
pub mod locks;

use crate::profile::paths;
use std::{fs, io::Write};

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

/// Make a profile active: write the right active-path value and record it as active. The default
/// `~/.claude` account clears active-path (so the shell UNSETS CLAUDE_CONFIG_DIR); any other profile
/// writes its real path. Shared by the desktop app (which then refreshes the tray) and the CLI.
pub fn activate_profile(id: &str) -> Result<(), String> {
    let p = crate::profile::store::find(id).ok_or("no such profile")?;
    let write_val = if crate::profile::paths::is_default(std::path::Path::new(&p.config_dir)) {
        ""
    } else {
        &p.config_dir
    };
    set_active_config_dir(write_val).map_err(|e| e.to_string())?;
    crate::profile::store::set_active_profile_id(id).map_err(|e| e.to_string())
}
