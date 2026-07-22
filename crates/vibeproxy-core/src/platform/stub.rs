//! Non-macOS placeholder so the workspace compiles on Linux/Windows CI while real backends are
//! pending (plan phase 5). Every operation fails loudly rather than silently doing nothing.

use super::{Blob, CredentialStore, Secret, TerminalLauncher};
use std::path::Path;

const MSG: &str = "credential store is not implemented on this platform yet";

pub struct UnsupportedStore;
impl CredentialStore for UnsupportedStore {
    fn read_token(&self, _: &Path) -> Result<Secret, String> { Err(MSG.into()) }
    fn read_blob(&self, _: &Path) -> Result<Blob, String> { Err(MSG.into()) }
    fn item_account(&self, _: &Path) -> Result<String, String> { Err(MSG.into()) }
    fn write_blob(&self, _: &Path, _: &str, _: &Blob) -> Result<(), String> { Err(MSG.into()) }
    fn backup_once(&self, _: &Path, _: &str, _: &Blob) -> Result<(), String> { Err(MSG.into()) }
    fn read_backup(&self, _: &Path) -> Result<Blob, String> { Err(MSG.into()) }
}

pub struct UnsupportedLauncher;
impl TerminalLauncher for UnsupportedLauncher {
    fn launch_claude(&self, _: &Path) -> Result<(), String> {
        Err("launching a terminal is not implemented on this platform yet".into())
    }
}
