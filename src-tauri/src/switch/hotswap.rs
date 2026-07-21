//! Move an account's credentials into a config dir so a RUNNING Claude Code session picks them up.
//!
//! Claude Code resolves `CLAUDE_CONFIG_DIR` at process start, so a live session cannot be pointed
//! at a different directory. The only lever is changing the credentials of the directory it is
//! already using. macOS caches Keychain reads for ~30s; bumping `.credentials.json`'s mtime makes
//! Claude Code invalidate sooner, which is why the file is rewritten even though the Keychain is
//! the authority.
//!
//! Every swap appends a boundary to the journal so analytics can still attribute usage correctly —
//! see `switch::journal`.

use crate::keychain;
use crate::switch::{journal, locks};
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub enum SwapError {
    /// Claude Code is mid-refresh. Aborting is correct: waiting risks writing under its save.
    Locked,
    Read(String),
    Write(String),
    /// The written item did not read back as expected — the dir is in an unknown state.
    VerifyFailed,
    Journal(String),
}

impl std::fmt::Display for SwapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwapError::Locked => write!(f, "Claude Code is busy refreshing credentials — try again in a moment"),
            SwapError::Read(e) => write!(f, "could not read source credentials: {e}"),
            SwapError::Write(e) => write!(f, "could not write credentials: {e}"),
            SwapError::VerifyFailed => write!(f, "credentials did not verify after writing; nothing was switched"),
            SwapError::Journal(e) => write!(f, "swap succeeded but could not be recorded: {e}"),
        }
    }
}

/// Merge `source`'s account identity into `target`'s blob, preserving everything else.
///
/// Only `claudeAiOauth` moves. Keys such as `mcpOAuth` describe integrations bound to the machine
/// rather than the account, and wholesale replacement is what causes them to be clobbered.
fn merge_account_into(target: &serde_json::Value, source: &serde_json::Value) -> serde_json::Value {
    let mut merged = target.clone();
    if let (Some(obj), Some(oauth)) = (merged.as_object_mut(), source.get("claudeAiOauth")) {
        obj.insert("claudeAiOauth".to_string(), oauth.clone());
    }
    merged
}

/// Extract a stable identity for verification. The access token itself is never compared directly
/// in logs or errors — only inside this process.
fn identity_of(v: &serde_json::Value) -> Option<String> {
    let oauth = v.get("claudeAiOauth")?;
    oauth
        .get("subscriptionType")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .or_else(|| oauth.get("accessToken").and_then(|t| t.as_str()).map(|t| {
            // A short digest, never the token, so a mismatch can be detected without holding it.
            use sha2::{Digest, Sha256};
            let d = Sha256::digest(t.as_bytes());
            d.iter().take(4).map(|b| format!("{b:02x}")).collect()
        }))
}

/// Put `source_dir`'s account into `target_dir`, so a session running on `target_dir` switches.
pub fn swap_into(
    target_dir: &Path,
    source_dir: &Path,
    account_id: &str,
    account_label: &str,
) -> Result<(), SwapError> {
    let _guard = locks::acquire(&locks::claude_lock_paths(), 5, Duration::from_millis(120))
        .ok_or(SwapError::Locked)?;

    let source = keychain::read_blob(source_dir).map_err(SwapError::Read)?;
    let source_json = source.parse().map_err(SwapError::Read)?;
    let target = keychain::read_blob(target_dir).map_err(SwapError::Read)?;
    let target_json = target.parse().map_err(SwapError::Read)?;

    let want = identity_of(&source_json).ok_or(SwapError::VerifyFailed)?;
    let merged = merge_account_into(&target_json, &source_json);
    let acct = keychain::item_account(target_dir).map_err(SwapError::Read)?;

    let blob = keychain::blob_from_value(&merged).map_err(SwapError::Write)?;
    keychain::write_blob(target_dir, &acct, &blob).map_err(SwapError::Write)?;

    // Read back before believing it. A silently-failed write would leave the old account live while
    // the journal claims otherwise, which is worse than a loud failure.
    let after = keychain::read_blob(target_dir).map_err(SwapError::Read)?;
    let after_json = after.parse().map_err(SwapError::Read)?;
    if identity_of(&after_json).as_deref() != Some(want.as_str()) {
        return Err(SwapError::VerifyFailed);
    }

    bump_credentials_mtime(target_dir);

    journal::append(&journal::Boundary {
        at: chrono::Utc::now().to_rfc3339(),
        config_dir: target_dir.to_string_lossy().to_string(),
        account_id: account_id.to_string(),
        account_label: account_label.to_string(),
    })
    .map_err(|e| SwapError::Journal(e.to_string()))
}

/// Rewrite `.credentials.json` with its own contents purely to move its mtime forward, which makes
/// a running session invalidate its cached credentials sooner than the ~30s Keychain TTL.
///
/// Deliberately never creates the file: its absence means Claude Code is using the Keychain alone,
/// and creating one would change which source it trusts.
fn bump_credentials_mtime(config_dir: &Path) {
    let path = config_dir.join(".credentials.json");
    if !path.exists() {
        return;
    }
    if let Ok(existing) = std::fs::read(&path) {
        let _ = std::fs::write(&path, existing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_moves_the_account_and_preserves_everything_else() {
        let target = json!({
            "claudeAiOauth": {"accessToken": "old", "subscriptionType": "max"},
            "mcpOAuth": {"some-server": {"token": "machine-bound"}},
            "unrelated": 42
        });
        let source = json!({
            "claudeAiOauth": {"accessToken": "new", "subscriptionType": "pro"}
        });
        let merged = merge_account_into(&target, &source);

        assert_eq!(merged["claudeAiOauth"]["accessToken"], "new", "account moves");
        assert_eq!(merged["claudeAiOauth"]["subscriptionType"], "pro");
        assert_eq!(
            merged["mcpOAuth"]["some-server"]["token"], "machine-bound",
            "mcpOAuth is machine-bound and must survive a swap"
        );
        assert_eq!(merged["unrelated"], 42, "unknown keys are preserved");
    }

    #[test]
    fn merge_without_source_oauth_leaves_target_untouched() {
        let target = json!({"claudeAiOauth": {"accessToken": "old"}});
        let merged = merge_account_into(&target, &json!({"somethingElse": true}));
        assert_eq!(merged["claudeAiOauth"]["accessToken"], "old");
    }

    #[test]
    fn identity_prefers_subscription_and_never_returns_the_raw_token() {
        let with_sub = json!({"claudeAiOauth": {"accessToken": "tok", "subscriptionType": "max"}});
        assert_eq!(identity_of(&with_sub).as_deref(), Some("max"));

        let no_sub = json!({"claudeAiOauth": {"accessToken": "super-secret-token"}});
        let id = identity_of(&no_sub).unwrap();
        assert_ne!(id, "super-secret-token", "identity must not be the token itself");
        assert_eq!(id.len(), 8, "digest, not the value");
    }

    #[test]
    fn identity_is_none_without_oauth() {
        assert!(identity_of(&json!({"mcpOAuth": {}})).is_none());
    }
}
