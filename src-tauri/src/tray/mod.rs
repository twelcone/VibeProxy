//! Menubar tray: renders the profile list from the store, switches on click, hosts the app menu.

use crate::profile;
use tauri::{
    menu::{Menu, MenuBuilder, MenuEvent, MenuItemBuilder, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Manager, Wry,
};

const TRAY_ID: &str = "main";

/// Build the tray icon + menu and attach it. Called once in `setup()`.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let cfg = profile::store::load();
    let menu = build_menu(app, &cfg)?;
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .tooltip("VibeProxy")
        .menu(&menu)
        .on_menu_event(on_menu_event)
        .build(app)?;
    apply_title(&tray, &cfg);
    Ok(())
}

/// Rebuild the menu + title from the current store (after a switch / add / delete).
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let cfg = profile::store::load();
    if let Ok(menu) = build_menu(app, &cfg) {
        let _ = tray.set_menu(Some(menu));
    }
    apply_title(&tray, &cfg);
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        "quit" => app.exit(0),
        "open" => show_main_window(app),
        id => {
            // A profile row was clicked → make it active.
            if profile::store::find(id).is_some() {
                let _ = crate::activate(app, id);
            }
        }
    }
}

/// macOS: show the active profile's label next to the tray icon. Phase 4 appends live usage.
fn apply_title(tray: &TrayIcon, cfg: &profile::Config) {
    let title = cfg
        .active_profile_id
        .as_ref()
        .and_then(|id| cfg.profiles.iter().find(|p| &p.id == id))
        .map(|p| p.label.clone())
        .unwrap_or_else(|| "VibeProxy".to_string());
    let _ = tray.set_title(Some(title));
}

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
