//! File-based credential store for Linux/Windows, where Claude Code keeps credentials in a plaintext
//! `<config_dir>/.credentials.json` rather than a keychain. Compiles on every platform (it's just
//! file I/O) so its logic is unit-tested on macOS, but it is only *wired* as the default off macOS.
//!
//! The pre-swap backup is a sibling file. On these platforms the live credentials already sit in a
//! plaintext file, so a plaintext backup adds no exposure the platform doesn't already have — unlike
//! macOS, where the backup goes into the Keychain to preserve the "never a plaintext file" posture.

// Wired as the default only off macOS; compiled here for test coverage, so its non-test uses look
// dead on a macOS build.
#![cfg_attr(target_os = "macos", allow(dead_code))]

use super::{Blob, CredentialStore, Secret};
use std::path::{Path, PathBuf};

pub struct FileStore;

fn live_path(dir: &Path) -> PathBuf {
    dir.join(".credentials.json")
}
fn backup_path(dir: &Path) -> PathBuf {
    dir.join(".vibeproxy-backup.json")
}

fn read_file(path: &Path) -> Result<Blob, String> {
    std::fs::read_to_string(path)
        .map(Blob::new)
        .map_err(|e| format!("read {}: {e}", path.display()))
}

fn write_file(path: &Path, blob: &Blob) -> Result<(), String> {
    // Atomic + 0600, matching how Claude Code protects the file on these platforms.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, blob.as_str()).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))
}

impl CredentialStore for FileStore {
    fn read_token(&self, dir: &Path) -> Result<Secret, String> {
        let parsed = read_file(&live_path(dir))?.parse()?;
        parsed["claudeAiOauth"]["accessToken"]
            .as_str()
            .map(|t| Secret::new(t.to_string()))
            .ok_or_else(|| "credentials file has no claudeAiOauth.accessToken".to_string())
    }

    fn read_blob(&self, dir: &Path) -> Result<Blob, String> {
        read_file(&live_path(dir))
    }

    fn item_account(&self, dir: &Path) -> Result<String, String> {
        // No keychain `acct` on a file; surface the email so callers have a stable label. Unused by
        // the file write path (write_blob ignores acct), but kept for trait parity.
        let parsed = read_file(&live_path(dir))?.parse()?;
        Ok(parsed["claudeAiOauth"]["account"]["email_address"]
            .as_str()
            .or_else(|| parsed["oauthAccount"]["emailAddress"].as_str())
            .unwrap_or("file")
            .to_string())
    }

    fn write_blob(&self, dir: &Path, _acct: &str, blob: &Blob) -> Result<(), String> {
        write_file(&live_path(dir), blob)
    }

    fn backup_once(&self, dir: &Path, _acct: &str, blob: &Blob) -> Result<(), String> {
        let path = backup_path(dir);
        if path.exists() {
            return Ok(()); // preserve the true owner, not the most recent swap-in
        }
        write_file(&path, blob)
    }

    fn read_backup(&self, dir: &Path) -> Result<Blob, String> {
        read_file(&backup_path(dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);

    fn tmp() -> PathBuf {
        // Atomic counter, not a timestamp: parallel tests can read the same nanosecond and collide.
        let d = std::env::temp_dir().join(format!(
            "vp-filestore-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn tok(store: &FileStore, dir: &Path) -> String {
        store.read_blob(dir).unwrap().parse().unwrap()["claudeAiOauth"]["accessToken"].as_str().unwrap().to_string()
    }

    #[test]
    fn write_then_read_round_trips() {
        let s = FileStore;
        let d = tmp();
        s.write_blob(&d, "acct", &Blob::new(json!({"claudeAiOauth":{"accessToken":"T"}}).to_string())).unwrap();
        assert_eq!(tok(&s, &d), "T");
        assert_eq!(s.read_token(&d).unwrap().expose(), "T");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn backup_is_once_and_survives_a_second_swap() {
        let s = FileStore;
        let d = tmp();
        s.write_blob(&d, "acct", &Blob::new(json!({"claudeAiOauth":{"accessToken":"OWNER"}}).to_string())).unwrap();
        // snapshot the owner, then two swaps
        s.backup_once(&d, "acct", &s.read_blob(&d).unwrap()).unwrap();
        s.write_blob(&d, "acct", &Blob::new(json!({"claudeAiOauth":{"accessToken":"SWAP1"}}).to_string())).unwrap();
        s.backup_once(&d, "acct", &s.read_blob(&d).unwrap()).unwrap(); // must NOT overwrite
        s.write_blob(&d, "acct", &Blob::new(json!({"claudeAiOauth":{"accessToken":"SWAP2"}}).to_string())).unwrap();

        assert_eq!(tok(&s, &d), "SWAP2", "live holds the latest swap");
        assert_eq!(
            s.read_backup(&d).unwrap().parse().unwrap()["claudeAiOauth"]["accessToken"], "OWNER",
            "backup still holds the original owner after two swaps"
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn missing_files_error_rather_than_panic() {
        let s = FileStore;
        let d = tmp();
        assert!(s.read_blob(&d).is_err());
        assert!(s.read_backup(&d).is_err());
        assert!(s.read_token(&d).is_err());
        std::fs::remove_dir_all(&d).ok();
    }

    #[cfg(unix)]
    #[test]
    fn written_file_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let s = FileStore;
        let d = tmp();
        s.write_blob(&d, "acct", &Blob::new(json!({"claudeAiOauth":{"accessToken":"T"}}).to_string())).unwrap();
        let mode = std::fs::metadata(live_path(&d)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "credentials file must not be world-readable");
        std::fs::remove_dir_all(&d).ok();
    }
}
