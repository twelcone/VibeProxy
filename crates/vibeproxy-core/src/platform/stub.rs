//! Non-macOS placeholder so the workspace compiles on Linux/Windows CI while real backends are
//! pending (plan phase 5). Every operation fails loudly rather than silently doing nothing.

use super::TerminalLauncher;
use std::path::Path;

pub struct UnsupportedLauncher;
impl TerminalLauncher for UnsupportedLauncher {
    fn launch_claude(&self, _: &Path) -> Result<(), String> {
        Err("launching a terminal is not implemented on this platform yet".into())
    }
}
