//! VibeProxy — menubar app to switch between multiple Claude Code accounts.
//! Phase 1: tray-only scaffold, profile store, and the UI ↔ Rust bridge.

mod platform;
mod profile;
mod tray;

/// Return all configured profiles (UI reads this to render the list).
#[tauri::command]
fn list_profiles() -> Vec<profile::Profile> {
    profile::store::load().profiles
}

/// Return current settings.
#[tauri::command]
fn get_settings() -> profile::Settings {
    profile::store::load().settings
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![list_profiles, get_settings])
        .setup(|app| {
            // Menubar-only: drop the Dock icon before anything else.
            platform::hide_dock_icon(app);
            // Create ~/.vibeproxy + config.json on first run.
            profile::store::ensure_initialized()?;
            // Build the tray from stored profiles.
            tray::build_tray(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
