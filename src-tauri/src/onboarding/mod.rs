//! Adding a new account: create an isolated config dir, drive the real `claude auth login` into it,
//! and (via the poll in lib.rs) register it once the browser OAuth completes.

use crate::profile::paths;
use std::{fs, path::Path};

/// Create a fresh, empty profile config dir under `~/.vibeproxy/profiles/<id>` and seed the user's
/// non-secret shared config into it (so a new account isn't a factory-reset Claude Code). Returns
/// the absolute dir path — its Keychain service name is derived from this path once login completes.
pub fn prepare() -> std::io::Result<String> {
    let id = crate::profile::store::new_id();
    let dir = paths::profiles_dir().join(&id);
    fs::create_dir_all(&dir)?;
    seed_shared_config(&dir);
    Ok(dir.to_string_lossy().to_string())
}

/// Copy non-secret user config (e.g. `settings.json`) from the default `~/.claude` into a new profile
/// dir. Best-effort — never fails onboarding. Never copies identity/secret files
/// (`.credentials.json`, `.claude.json`).
fn seed_shared_config(dir: &Path) {
    let Some(default) = paths::default_config_dir() else {
        return;
    };
    for name in ["settings.json", "CLAUDE.md"] {
        let src = default.join(name);
        if src.is_file() {
            let _ = fs::copy(&src, dir.join(name));
        }
    }
}

/// Open Terminal and run the interactive subscription login scoped to `config_dir` (macOS).
/// The user completes the browser OAuth; credentials land in the Keychain for this dir.
pub fn launch_login(config_dir: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = format!(
            "tell application \"Terminal\" to do script \"export CLAUDE_CONFIG_DIR={config_dir} && claude auth login --claudeai\"",
        );
        Command::new("osascript").args(["-e", &script]).spawn()?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = config_dir;
    Ok(())
}

/// Remove a not-yet-registered profile dir (on cancel/timeout). Guarded to VibeProxy's own dir.
pub fn cleanup(config_dir: &str) -> std::io::Result<()> {
    let dir = Path::new(config_dir);
    if dir.starts_with(paths::profiles_dir()) && dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Startup sweep: delete any `profiles/<id>` dir not referenced by a registered profile — cleans up
/// after an add that was abandoned by quitting the app (the UI's cancel handles the in-session case).
pub fn gc_orphans() {
    let Ok(entries) = fs::read_dir(paths::profiles_dir()) else {
        return;
    };
    let registered: std::collections::HashSet<String> = crate::profile::store::load()
        .profiles
        .iter()
        .map(|p| p.config_dir.clone())
        .collect();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !registered.contains(&path.to_string_lossy().to_string()) {
            let _ = fs::remove_dir_all(&path);
        }
    }
}
