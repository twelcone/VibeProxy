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

/// The whole Keychain blob for a profile. Wrapped so it cannot be printed or logged: it holds the
/// OAuth tokens verbatim.
pub struct Blob(String);

impl std::fmt::Debug for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Blob(***)")
    }
}

impl Blob {
    pub fn parse(&self) -> Result<serde_json::Value, String> {
        serde_json::from_str(self.0.trim()).map_err(|e| format!("keychain blob parse: {e}"))
    }
    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Read a profile's entire Keychain blob (not just the token) — a swap must carry every field, and
/// must leave account-independent ones such as `mcpOAuth` alone.
pub fn read_blob(config_dir: &Path) -> Result<Blob, String> {
    let service = service_name(config_dir);
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", &service, "-w"])
        .output()
        .map_err(|e| format!("could not run security: {e}"))?;
    if !out.status.success() {
        return Err(format!("keychain read failed for service {service}"));
    }
    Ok(Blob(String::from_utf8_lossy(&out.stdout).to_string()))
}

/// The `acct` attribute of an existing item. Needed to rewrite it without inventing a new one.
pub fn item_account(config_dir: &Path) -> Result<String, String> {
    let service = service_name(config_dir);
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", &service])
        .output()
        .map_err(|e| format!("could not run security: {e}"))?;
    if !out.status.success() {
        return Err(format!("no keychain item for service {service}"));
    }
    // Attribute dump line looks like: `"acct"<blob>="someone@example.com"`
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .chain(String::from_utf8_lossy(&out.stdout).lines())
        .find_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("\"acct\"<blob>=")?;
            Some(rest.trim().trim_matches('"').to_string())
        })
        .ok_or_else(|| format!("could not read acct attribute for service {service}"))
}

/// Re-wrap a merged JSON value as a writable blob.
pub fn blob_from_value(v: &serde_json::Value) -> Result<Blob, String> {
    serde_json::to_string(v).map(Blob).map_err(|e| format!("serialize keychain blob: {e}"))
}

/// Write a blob into a profile's Keychain item.
///
/// Two deliberate choices:
/// - `-U` updates in place rather than delete-then-add. A failed delete in that pattern can leave
///   the previous account's item behind and silently resurrect the wrong account.
/// - The command is fed through stdin (`security -i`), never argv, so the secret never appears in
///   `ps` output for any other process on the machine.
pub fn write_blob(config_dir: &Path, acct: &str, blob: &Blob) -> Result<(), String> {
    use std::io::Write as _;
    use std::process::Stdio;

    let service = service_name(config_dir);
    let mut child = Command::new("/usr/bin/security")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run security: {e}"))?;

    let quote = |v: &str| format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""));
    let cmd = format!(
        "add-generic-password -U -a {} -s {} -w {}\n",
        quote(acct),
        quote(&service),
        quote(blob.as_str().trim()),
    );

    child
        .stdin
        .as_mut()
        .ok_or("could not open security stdin")?
        .write_all(cmd.as_bytes())
        .map_err(|e| format!("could not write to security: {e}"))?;
    // `cmd` holds the secret; drop it as soon as it has been handed over.
    drop(cmd);

    let out = child.wait_with_output().map_err(|e| format!("security failed: {e}"))?;
    if !out.status.success() {
        // stderr from `security` echoes the command it failed on, which would include the secret.
        return Err(format!("keychain write failed for service {service}"));
    }
    Ok(())
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

    /// Real round-trip against the macOS Keychain, using a throwaway config-dir path so the
    /// derived service name cannot collide with a real Claude account. Ignored by default: it
    /// mutates the login keychain.
    /// Run with `cargo test keychain_roundtrip -- --ignored --nocapture`.
    #[test]
    #[ignore = "writes to the real macOS login keychain"]
    fn keychain_roundtrip_writes_reads_and_preserves_siblings() {
        let dir = PathBuf::from(format!("/tmp/vp-keychain-test-{}", std::process::id()));
        let service = service_name(&dir);
        assert!(service.starts_with("Claude Code-credentials-"), "hashed, not the bare default");

        let cleanup = || {
            let _ = Command::new("/usr/bin/security")
                .args(["delete-generic-password", "-s", &service])
                .output();
        };
        cleanup(); // in case an earlier run aborted

        let original = serde_json::json!({
            "claudeAiOauth": {"accessToken": "token-A", "subscriptionType": "max"},
            "mcpOAuth": {"srv": {"token": "machine-bound"}}
        });
        let blob = blob_from_value(&original).unwrap();
        write_blob(&dir, "vibeproxy-test@example.invalid", &blob).expect("initial write");

        let acct = item_account(&dir).expect("read acct attribute");
        assert_eq!(acct, "vibeproxy-test@example.invalid");

        let read_back = read_blob(&dir).expect("read back").parse().expect("parse");
        assert_eq!(read_back["claudeAiOauth"]["accessToken"], "token-A");
        assert_eq!(read_back["mcpOAuth"]["srv"]["token"], "machine-bound");

        // Update in place (-U): the item must be replaced, not duplicated, and siblings survive.
        let updated = serde_json::json!({
            "claudeAiOauth": {"accessToken": "token-B", "subscriptionType": "pro"},
            "mcpOAuth": {"srv": {"token": "machine-bound"}}
        });
        write_blob(&dir, &acct, &blob_from_value(&updated).unwrap()).expect("update write");
        let after = read_blob(&dir).expect("read after update").parse().expect("parse");
        assert_eq!(after["claudeAiOauth"]["accessToken"], "token-B", "update took effect");
        assert_eq!(after["mcpOAuth"]["srv"]["token"], "machine-bound", "siblings survived");

        cleanup();
        assert!(read_blob(&dir).is_err(), "cleanup removed the test item");
        eprintln!("keychain round-trip OK for {service}");
    }

    #[test]
    fn blob_never_leaks_in_debug() {
        let b = Blob("{\"accessToken\":\"super-secret\"}".to_string());
        assert_eq!(format!("{b:?}"), "Blob(***)");
    }

    #[test]
    fn secret_never_leaks_in_debug() {
        let s = Secret("super-secret-token".to_string());
        assert_eq!(format!("{s:?}"), "Secret(***)");
    }
}
