//! Shell integration: the one line that makes a switch actually reach the user's terminals.
//!
//! VibeProxy only writes `~/.vibeproxy/active-path`. Nothing happens until the user's shell reads
//! that file into `CLAUDE_CONFIG_DIR`. If the line is missing, every `claude` runs on the default
//! account regardless of what VibeProxy thinks is active — the core feature silently does nothing.
//! So we detect its absence and offer to install it, rather than leaving it to a manual step the
//! app's own author skipped.

use std::path::PathBuf;

/// Marker that identifies our block on re-install and detection, independent of exact formatting.
const MARKER: &str = "vibeproxy/active-path";

/// The canonical snippet — the single source of truth, surfaced to the UI so it can't drift from
/// what we actually install.
pub fn snippet() -> String {
    "_vp=\"$(cat ~/.vibeproxy/active-path 2>/dev/null)\"; \
     [ -n \"$_vp\" ] && export CLAUDE_CONFIG_DIR=\"$_vp\" || unset CLAUDE_CONFIG_DIR"
        .to_string()
}

/// Shell rc files we look in / write to, most specific first. zsh is the macOS default.
fn candidate_rc_files() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return Vec::new() };
    let uses_bash = std::env::var("SHELL").map(|s| s.contains("bash")).unwrap_or(false);
    if uses_bash {
        vec![home.join(".bashrc"), home.join(".bash_profile"), home.join(".zshrc")]
    } else {
        vec![home.join(".zshrc"), home.join(".zprofile"), home.join(".bashrc")]
    }
}

/// Is the integration present in any shell rc file?
pub fn is_installed() -> bool {
    candidate_rc_files().iter().any(|p| {
        std::fs::read_to_string(p).map(|s| s.contains(MARKER)).unwrap_or(false)
    })
}

/// Append the snippet to the primary shell rc (creating the file if needed). Idempotent: if the
/// marker is already present anywhere, do nothing and report where. Returns the file touched.
pub fn install() -> Result<String, String> {
    if let Some(existing) = candidate_rc_files()
        .into_iter()
        .find(|p| std::fs::read_to_string(p).map(|s| s.contains(MARKER)).unwrap_or(false))
    {
        return Ok(existing.to_string_lossy().to_string());
    }

    let target = candidate_rc_files()
        .into_iter()
        .next()
        .ok_or("could not resolve a shell rc file")?;

    let block = format!(
        "\n# VibeProxy — point Claude Code at the active account (added by VibeProxy)\n{}\n",
        snippet()
    );
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&target)
        .map_err(|e| format!("could not open {}: {e}", target.display()))?;
    f.write_all(block.as_bytes())
        .map_err(|e| format!("could not write {}: {e}", target.display()))?;
    Ok(target.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_and_marker_agree() {
        // The detection marker must actually appear in what we install, or install-then-detect lies.
        assert!(snippet().contains(MARKER));
    }

    #[test]
    fn install_is_idempotent_and_detected() {
        let tmp = std::env::temp_dir().join(format!("vp-shell-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // Point HOME at the temp dir for the duration (serialized: see paths::ENV_SERIAL).
        let _g = vibeproxy_core::profile::paths::ENV_SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var("HOME").ok();
        std::env::set_var("HOME", &tmp);
        std::env::set_var("SHELL", "/bin/zsh");

        assert!(!is_installed(), "clean home has no integration");
        let f1 = install().unwrap();
        assert!(is_installed(), "installed then detected");
        let after_first = std::fs::read_to_string(&f1).unwrap();
        // Second install must not append a duplicate block.
        let f2 = install().unwrap();
        assert_eq!(f1, f2);
        assert_eq!(after_first, std::fs::read_to_string(&f2).unwrap(), "idempotent");

        if let Some(h) = prev { std::env::set_var("HOME", h); } else { std::env::remove_var("HOME"); }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
