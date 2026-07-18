//! OS-specific bits, isolated behind small functions so the future Windows port
//! only needs to fill in the `#[cfg(not(target_os = "macos"))]` branches.

/// Hide the Dock icon (macOS agent / menubar-only app). No-op on other platforms.
#[cfg(target_os = "macos")]
pub fn hide_dock_icon(app: &mut tauri::App) {
    // `Accessory` = LSUIElement equivalent: menubar presence, no Dock icon, no app menu.
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

#[cfg(not(target_os = "macos"))]
pub fn hide_dock_icon(_app: &mut tauri::App) {}
