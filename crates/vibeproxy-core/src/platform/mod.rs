//! OS-specific seams behind traits, with a macOS implementation. The two operations that genuinely
//! diverge per platform are credential storage (macOS Keychain vs. a plaintext file on Linux/Windows)
//! and launching a terminal. Config-dir discovery is deliberately NOT trait-ified: `~/.claude` is the
//! path on every platform — only the credential *store* differs — so a second impl would be identical.

use std::path::Path;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod stub;

/// An OAuth access token. Never printed — `Debug` shows a placeholder.
pub struct Secret(String);
impl Secret {
    pub(crate) fn new(s: String) -> Self {
        Secret(s)
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// A whole credential blob for a config dir. Wrapped so it can't be printed: it holds tokens verbatim.
pub struct Blob(String);
impl std::fmt::Debug for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Blob(***)")
    }
}
impl Blob {
    pub(crate) fn new(s: String) -> Self {
        Blob(s)
    }
    pub fn parse(&self) -> Result<serde_json::Value, String> {
        serde_json::from_str(self.0.trim()).map_err(|e| format!("credential blob parse: {e}"))
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Re-wrap a merged JSON value as a writable blob. Pure; platform-agnostic.
pub fn blob_from_value(v: &serde_json::Value) -> Result<Blob, String> {
    serde_json::to_string(v).map(Blob).map_err(|e| format!("serialize credential blob: {e}"))
}

/// Where a profile's credentials live, and where its pre-swap original is preserved. The macOS impl
/// is the Keychain; a file-based impl on other platforms is future work (see the plan's phase 5).
pub trait CredentialStore {
    /// Just the access token, for the usage poller.
    fn read_token(&self, dir: &Path) -> Result<Secret, String>;
    /// The entire blob — a swap must carry every field (e.g. `mcpOAuth`), not just the token.
    fn read_blob(&self, dir: &Path) -> Result<Blob, String>;
    /// The store's notion of the account attached to a dir's item (macOS: the Keychain `acct`).
    fn item_account(&self, dir: &Path) -> Result<String, String>;
    /// Overwrite the live blob in place.
    fn write_blob(&self, dir: &Path, acct: &str, blob: &Blob) -> Result<(), String>;
    /// Snapshot the current blob ONCE, so the account that truly owns the dir is never lost.
    fn backup_once(&self, dir: &Path, acct: &str, blob: &Blob) -> Result<(), String>;
    /// Read a previously preserved snapshot, if any.
    fn read_backup(&self, dir: &Path) -> Result<Blob, String>;
}

/// Launch a terminal running `claude` on a profile.
pub trait TerminalLauncher {
    fn launch_claude(&self, dir: &Path) -> Result<(), String>;
}

/// The credential store for the current OS.
#[cfg(target_os = "macos")]
pub fn credentials() -> impl CredentialStore {
    macos::KeychainStore
}
#[cfg(not(target_os = "macos"))]
pub fn credentials() -> impl CredentialStore {
    stub::UnsupportedStore
}

/// The terminal launcher for the current OS.
#[cfg(target_os = "macos")]
pub fn launcher() -> impl TerminalLauncher {
    macos::AppleScriptLauncher
}
#[cfg(not(target_os = "macos"))]
pub fn launcher() -> impl TerminalLauncher {
    stub::UnsupportedLauncher
}
