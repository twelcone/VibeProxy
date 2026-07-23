//! VibeProxy core: the Tauri-free logic shared by the desktop app and the CLI.
//!
//! Everything here computes or reads; it performs no GUI side effects (no events, tray, or windows).
//! Frontends (the Tauri app, the CLI, a future native app) act on what these modules return.

pub mod autoswitch;
pub mod platform;
pub mod shell;
pub mod profile;
pub mod switch;
pub mod usage;
pub mod usage_analytics;
