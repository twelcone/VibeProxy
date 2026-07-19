//! Account identity for a profile, from the official `claude auth status --json`.
//!
//! ToS-safe: the official client reads its own credentials; VibeProxy never touches the token here.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    #[serde(default)]
    pub logged_in: bool,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub org_id: Option<String>,
    #[serde(default)]
    pub subscription_type: Option<String>,
}

/// Ask `claude auth status --json` for the account bound to `config_dir`.
/// The default `~/.claude` account must be queried with `CLAUDE_CONFIG_DIR` UNSET — setting it to
/// that path makes Claude hash it and report "not logged in" (verified in the spike).
pub fn fetch(config_dir: &Path) -> Result<AuthStatus, String> {
    let mut cmd = Command::new("claude");
    cmd.args(["auth", "status", "--json"]);
    if !super::paths::is_default(config_dir) {
        cmd.env("CLAUDE_CONFIG_DIR", config_dir);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("could not run `claude` (is it on PATH?): {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`claude auth status` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("parse auth status json: {e}"))
}
