//! Menubar tray: renders the profile list from the store and hosts the app menu.

use crate::profile;
use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, Wry,
};

/// Build the tray icon + menu and attach it to the app. Called once in `setup()`.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let cfg = profile::store::load();
    let menu = build_menu(app, &cfg)?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("bundled icon").clone())
        // macOS shows this string next to the tray icon; updated live in Phase 4.
        .title("VibeProxy")
        .tooltip("VibeProxy")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "open" => show_main_window(app),
            // Per-profile clicks become "switch active profile" in Phase 2.
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Render the dropdown menu from current config: profile rows, then Open / Quit.
fn build_menu(app: &AppHandle, cfg: &profile::Config) -> tauri::Result<Menu<Wry>> {
    let mut builder = MenuBuilder::new(app);

    if cfg.profiles.is_empty() {
        let empty = MenuItemBuilder::with_id("noop_empty", "No profiles yet")
            .enabled(false)
            .build(app)?;
        builder = builder.item(&empty);
    } else {
        for p in &cfg.profiles {
            let is_active = cfg.active_profile_id.as_deref() == Some(p.id.as_str());
            let label = format!("{}{}", if is_active { "● " } else { "   " }, p.label);
            let item = MenuItemBuilder::with_id(p.id.clone(), label).build(app)?;
            builder = builder.item(&item);
        }
    }

    let sep = PredefinedMenuItem::separator(app)?;
    let open = MenuItemBuilder::with_id("open", "Open VibeProxy").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit VibeProxy").build(app)?;

    builder.item(&sep).item(&open).item(&quit).build()
}

fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}
