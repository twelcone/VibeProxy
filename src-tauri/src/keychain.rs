//! Reading a profile's OAuth token from the macOS Keychain.
//!
//! Phase 0 established: a per-profile login stores its token in the Keychain under service
//! `Claude Code-credentials-<hash8>`, where `hash8 = SHA-256(absolute config-dir path)[:8]`.
//! The default `~/.claude` login uses the bare service name (no suffix).

use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

/// An OAuth access token. Never printed — `Debug` and any logging show a placeholder.
pub struct Secret(String);

impl Secret {
    #[allow(dead_code)] // consumed by the usage poller in Phase 4
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// The Keychain service name that holds the token for a given profile config dir.
pub fn service_name(config_dir: &Path) -> String {
    if crate::profile::paths::is_default(config_dir) {
        return "Claude Code-credentials".to_string();
    }
    let digest = Sha256::digest(config_dir.to_string_lossy().as_bytes());
    let hash8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("Claude Code-credentials-{hash8}")
}

/// Read the access token for a profile via `/usr/bin/security` (matches Keychain ACL, so the user
/// grants "Always Allow" once instead of being prompted every read). Never logs the value.
#[allow(dead_code)] // used by the usage poller in Phase 4
pub fn read_token(config_dir: &Path) -> Result<Secret, String> {
    let service = service_name(config_dir);
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", &service, "-w"])
        .output()
        .map_err(|e| format!("could not run security: {e}"))?;
    if !out.status.success() {
        return Err(format!("keychain read failed for service {service}"));
    }
    let blob = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(blob.trim()).map_err(|e| format!("keychain blob parse: {e}"))?;
    parsed["claudeAiOauth"]["accessToken"]
        .as_str()
        .map(|t| Secret(t.to_string()))
        .ok_or_else(|| "keychain item has no claudeAiOauth.accessToken".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn hashed_service_matches_phase0_fixture() {
        // Phase 0 empirical fixture: SHA-256("/Users/twel/vp-spike")[:8] == e30f4f07
        let s = service_name(&PathBuf::from("/Users/twel/vp-spike"));
        assert_eq!(s, "Claude Code-credentials-e30f4f07");
    }

    #[test]
    fn secret_never_leaks_in_debug() {
        let s = Secret("super-secret-token".to_string());
        assert_eq!(format!("{s:?}"), "Secret(***)");
    }
}
