//! Adding a new account: create an isolated config dir, drive the real `claude auth login` into it,
//! and (once the browser OAuth completes) register it via `profile::adopt`. No GUI concerns — the
//! CLI, the Tauri app, and the macOS app all drive this same flow.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::paths;

    #[test]
    fn prepare_creates_a_dir_and_cleanup_is_guarded() {
        let _g = paths::ENV_SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = std::env::temp_dir().join(format!("vp-onb-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        std::env::set_var("VIBEPROXY_DIR", &tmp);
        assert_eq!(paths::vibeproxy_dir(), tmp, "isolation in effect");

        let dir = prepare().unwrap();
        let path = Path::new(&dir);
        assert!(path.is_dir(), "prepared dir exists");
        assert!(path.starts_with(paths::profiles_dir()), "under our profiles dir");

        // cleanup refuses a path outside our profiles dir…
        cleanup("/etc").unwrap();
        assert!(Path::new("/etc").exists(), "guard left /etc alone");
        // …and removes one inside it.
        cleanup(&dir).unwrap();
        assert!(!path.exists(), "cleaned up");

        std::env::remove_var("VIBEPROXY_DIR");
        let _ = fs::remove_dir_all(&tmp);
    }
}
