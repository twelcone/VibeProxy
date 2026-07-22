//! macOS implementations: credentials in the login Keychain via `/usr/bin/security`, terminal
//! launch via AppleScript. Moved verbatim from the old `keychain` module and `switch::launch_claude`.

use super::{Blob, CredentialStore, Secret, TerminalLauncher};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

/// Credentials stored in the macOS login Keychain.
pub struct KeychainStore;

/// The Keychain service name that holds the token for a given profile config dir. Per-profile logins
/// use `Claude Code-credentials-<hash8>` (hash8 = SHA-256(path)[:8]); the default `~/.claude` login
/// uses the bare name.
pub(crate) fn service_name(config_dir: &Path) -> String {
    if crate::profile::paths::is_default(config_dir) {
        return "Claude Code-credentials".to_string();
    }
    let digest = Sha256::digest(config_dir.to_string_lossy().as_bytes());
    let hash8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("Claude Code-credentials-{hash8}")
}

/// Where the pre-swap original is preserved — a VibeProxy-owned Keychain item, never a plaintext file.
pub(crate) fn backup_service_name(config_dir: &Path) -> String {
    format!("VibeProxy-backup-{}", service_name(config_dir).trim_start_matches("Claude Code-credentials"))
        .replace("VibeProxy-backup--", "VibeProxy-backup-")
}

fn read_service(service: &str) -> Result<Blob, String> {
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", service, "-w"])
        .output()
        .map_err(|e| format!("could not run security: {e}"))?;
    if !out.status.success() {
        return Err(format!("keychain read failed for service {service}"));
    }
    Ok(Blob::new(String::from_utf8_lossy(&out.stdout).to_string()))
}

fn write_service(service: &str, acct: &str, blob: &Blob) -> Result<(), String> {
    use std::io::Write as _;
    use std::process::Stdio;

    let mut child = Command::new("/usr/bin/security")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run security: {e}"))?;

    // `-U` updates in place (a failed delete-then-add can resurrect the wrong account); the command
    // is fed via stdin, never argv, so the secret never appears in `ps`.
    let quote = |v: &str| format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""));
    let cmd = format!(
        "add-generic-password -U -a {} -s {} -w {}\n",
        quote(acct),
        quote(service),
        quote(blob.as_str().trim()),
    );
    child
        .stdin
        .as_mut()
        .ok_or("could not open security stdin")?
        .write_all(cmd.as_bytes())
        .map_err(|e| format!("could not write to security: {e}"))?;
    drop(cmd); // holds the secret — release immediately

    let out = child.wait_with_output().map_err(|e| format!("security failed: {e}"))?;
    if !out.status.success() {
        return Err(format!("keychain write failed for service {service}"));
    }
    Ok(())
}

impl CredentialStore for KeychainStore {
    fn read_token(&self, config_dir: &Path) -> Result<Secret, String> {
        let blob = read_service(&service_name(config_dir))?;
        let parsed = blob.parse()?;
        parsed["claudeAiOauth"]["accessToken"]
            .as_str()
            .map(|t| Secret::new(t.to_string()))
            .ok_or_else(|| "keychain item has no claudeAiOauth.accessToken".to_string())
    }

    fn read_blob(&self, config_dir: &Path) -> Result<Blob, String> {
        read_service(&service_name(config_dir))
    }

    fn item_account(&self, config_dir: &Path) -> Result<String, String> {
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
                let rest = l.trim().strip_prefix("\"acct\"<blob>=")?;
                Some(rest.trim().trim_matches('"').to_string())
            })
            .ok_or_else(|| format!("could not read acct attribute for service {service}"))
    }

    fn write_blob(&self, config_dir: &Path, acct: &str, blob: &Blob) -> Result<(), String> {
        write_service(&service_name(config_dir), acct, blob)
    }

    fn backup_once(&self, config_dir: &Path, acct: &str, blob: &Blob) -> Result<(), String> {
        let service = backup_service_name(config_dir);
        if read_service(&service).is_ok() {
            return Ok(()); // original already preserved
        }
        write_service(&service, acct, blob)
    }

    fn read_backup(&self, config_dir: &Path) -> Result<Blob, String> {
        read_service(&backup_service_name(config_dir))
    }
}

/// Opens a Terminal running `claude` scoped to a profile. The default account runs with
/// `CLAUDE_CONFIG_DIR` UNSET (setting it to `~/.claude` breaks it).
pub struct AppleScriptLauncher;

impl TerminalLauncher for AppleScriptLauncher {
    fn launch_claude(&self, config_dir: &Path) -> Result<(), String> {
        let cmd = if crate::profile::paths::is_default(config_dir) {
            "claude".to_string()
        } else {
            format!("export CLAUDE_CONFIG_DIR={} && claude", config_dir.display())
        };
        let script = format!("tell application \"Terminal\" to do script \"{cmd}\"");
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("could not launch Terminal: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::blob_from_value;
    use std::path::PathBuf;

    #[test]
    fn hashed_service_matches_phase0_fixture() {
        // Phase 0 empirical fixture: SHA-256("/Users/twel/vp-spike")[:8] == e30f4f07
        assert_eq!(service_name(&PathBuf::from("/Users/twel/vp-spike")), "Claude Code-credentials-e30f4f07");
    }

    /// Real round-trip against the macOS Keychain, throwaway service so it can't collide with a real
    /// account. Run with `cargo test keychain_roundtrip -- --ignored --nocapture`.
    #[test]
    #[ignore = "writes to the real macOS login keychain"]
    fn keychain_roundtrip_writes_reads_and_preserves_siblings() {
        let store = KeychainStore;
        let dir = PathBuf::from(format!("/tmp/vp-keychain-test-{}", std::process::id()));
        let service = service_name(&dir);
        assert!(service.starts_with("Claude Code-credentials-"), "hashed, not the bare default");
        let cleanup = || {
            let _ = Command::new("/usr/bin/security").args(["delete-generic-password", "-s", &service]).output();
        };
        cleanup();

        let original = serde_json::json!({
            "claudeAiOauth": {"accessToken": "token-A", "subscriptionType": "max"},
            "mcpOAuth": {"srv": {"token": "machine-bound"}}
        });
        store.write_blob(&dir, "vibeproxy-test@example.invalid", &blob_from_value(&original).unwrap()).expect("write");
        assert_eq!(store.item_account(&dir).unwrap(), "vibeproxy-test@example.invalid");
        let back = store.read_blob(&dir).unwrap().parse().unwrap();
        assert_eq!(back["claudeAiOauth"]["accessToken"], "token-A");
        assert_eq!(back["mcpOAuth"]["srv"]["token"], "machine-bound");

        let updated = serde_json::json!({
            "claudeAiOauth": {"accessToken": "token-B", "subscriptionType": "pro"},
            "mcpOAuth": {"srv": {"token": "machine-bound"}}
        });
        store.write_blob(&dir, "vibeproxy-test@example.invalid", &blob_from_value(&updated).unwrap()).expect("update");
        let after = store.read_blob(&dir).unwrap().parse().unwrap();
        assert_eq!(after["claudeAiOauth"]["accessToken"], "token-B", "update took effect");
        assert_eq!(after["mcpOAuth"]["srv"]["token"], "machine-bound", "siblings survived");

        cleanup();
        assert!(store.read_blob(&dir).is_err(), "cleanup removed the item");
        eprintln!("keychain round-trip OK for {service}");
    }

    /// Real-Keychain coverage of the backup path (the fake-store test covers the logic; this proves
    /// the actual `security` calls for the backup service work and are idempotent).
    /// Run with `cargo test keychain_backup_roundtrip -- --ignored --nocapture`.
    #[test]
    #[ignore = "writes to the real macOS login keychain"]
    fn keychain_backup_roundtrip_is_idempotent() {
        let store = KeychainStore;
        let dir = PathBuf::from(format!("/tmp/vp-backup-test-{}", std::process::id()));
        let del = |svc: &str| { let _ = Command::new("/usr/bin/security").args(["delete-generic-password", "-s", svc]).output(); };
        let cleanup = || { del(&service_name(&dir)); del(&backup_service_name(&dir)); };
        cleanup();

        let original = serde_json::json!({"claudeAiOauth": {"accessToken": "ORIGINAL"}});
        store.write_blob(&dir, "acct@example.invalid", &blob_from_value(&original).unwrap()).unwrap();

        // First backup preserves the original; a later one must NOT overwrite it.
        let before = store.read_blob(&dir).unwrap();
        store.backup_once(&dir, "acct@example.invalid", &before).unwrap();
        store.write_blob(&dir, "acct@example.invalid", &blob_from_value(&serde_json::json!({"claudeAiOauth":{"accessToken":"SWAPPED"}})).unwrap()).unwrap();
        let now = store.read_blob(&dir).unwrap();
        store.backup_once(&dir, "acct@example.invalid", &now).unwrap();

        assert_eq!(
            store.read_backup(&dir).unwrap().parse().unwrap()["claudeAiOauth"]["accessToken"],
            "ORIGINAL", "backup still holds the true owner after a second backup_once"
        );
        cleanup();
        eprintln!("keychain backup round-trip OK");
    }

    #[test]
    fn blob_never_leaks_in_debug() {
        assert_eq!(format!("{:?}", Blob::new("{\"accessToken\":\"secret\"}".into())), "Blob(***)");
    }
    #[test]
    fn secret_never_leaks_in_debug() {
        assert_eq!(format!("{:?}", Secret::new("secret".into())), "Secret(***)");
    }
}
