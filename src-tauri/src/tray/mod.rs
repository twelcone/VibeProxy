//! Menubar tray: renders the profile list from the store, switches on click, hosts the app menu.

use crate::profile;
use crate::usage::{ProfileUsage, UsageStatus};
use std::collections::HashMap;
use tauri::{
    image::Image,
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

/// Rebuild the menu + icon + title from the current store (after a switch / add / delete).
/// Pulls the latest usage snapshot so a switch immediately shows the new profile's meter (or clears
/// the old one) instead of leaving the previous profile's colored meter until the next poll.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let cfg = profile::store::load();
    if let Ok(menu) = build_menu(app, &cfg) {
        let _ = tray.set_menu(Some(menu));
    }
    if let Some(state) = app.try_state::<crate::usage::UsageState>() {
        if let Ok(map) = state.try_read() {
            update_icon_and_title(app, &tray, &cfg, &map);
            return;
        }
    }
    reset_icon(app, &tray);
    apply_title(&tray, &cfg);
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        "quit" => app.exit(0),
        "open" => show_main_window(app),
        "usage" => {
            let _ = crate::show_usage_window(app);
        }
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
    let usage = MenuItemBuilder::with_id("usage", "Usage Analytics…").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit VibeProxy").build(app)?;
    builder
        .item(&sep)
        .item(&open)
        .item(&usage)
        .item(&quit)
        .build()
}

fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Update the tray for the active profile's latest usage (called by the poller).
pub fn apply_active_usage(app: &AppHandle, cfg: &profile::Config, usage: &HashMap<String, ProfileUsage>) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    update_icon_and_title(app, &tray, cfg, usage);
}

/// Draw the fill-meter + "<label> NN%" for the active profile, or reset to the plain icon + label
/// when usage is Ok-but-missing / needs-reauth / errored / absent. Never leaves a stale meter.
fn update_icon_and_title(
    app: &AppHandle,
    tray: &TrayIcon,
    cfg: &profile::Config,
    usage: &HashMap<String, ProfileUsage>,
) {
    let active = cfg
        .active_profile_id
        .as_deref()
        .and_then(|id| cfg.profiles.iter().find(|p| p.id == id));
    let Some(p) = active else {
        reset_icon(app, tray);
        let _ = tray.set_title(Some("VibeProxy".to_string()));
        return;
    };
    let label = p.label.clone();

    match usage.get(&p.id) {
        Some(u) if u.status == UsageStatus::Ok && u.five_hour_pct.is_some() => {
            let pct = u.five_hour_pct.unwrap();
            let _ = tray.set_icon(Some(draw_meter(pct)));
            let _ = tray.set_icon_as_template(false); // keep the severity color
            let _ = tray.set_title(Some(format!("{label} {}%", pct.round() as i32)));
        }
        Some(u) if u.status == UsageStatus::NeedsReauth => {
            reset_icon(app, tray);
            let _ = tray.set_title(Some(format!("{label} · re-login")));
        }
        _ => {
            reset_icon(app, tray);
            let _ = tray.set_title(Some(label));
        }
    }
}

/// Restore the plain app icon (template mode so macOS tints it normally).
fn reset_icon(app: &AppHandle, tray: &TrayIcon) {
    if let Some(icon) = app.default_window_icon() {
        let _ = tray.set_icon(Some(icon.clone()));
        let _ = tray.set_icon_as_template(true);
    }
}

/// Severity color for a utilization percentage (matches the design system's usage scale).
fn severity_rgb(pct: f32) -> [u8; 3] {
    if pct >= 90.0 {
        [224, 101, 78] // crit
    } else if pct >= 70.0 {
        [227, 180, 87] // warn
    } else {
        [88, 183, 118] // good
    }
}

/// Draw the fill-meter tray icon: a small battery-style gauge whose fill = 5-hour %, colored by
/// severity. Hand-drawn RGBA (no image crate); a hairline border keeps it legible on any menubar.
fn draw_meter(pct: f32) -> Image<'static> {
    let (w, h): (u32, u32) = (34, 18);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let set = |buf: &mut Vec<u8>, x: u32, y: u32, c: [u8; 4]| {
        if x < w && y < h {
            let i = ((y * w + x) * 4) as usize;
            buf[i..i + 4].copy_from_slice(&c);
        }
    };

    let (x0, y0, x1, y1) = (3u32, 4u32, w - 4, h - 5);
    let border = [150u8, 150, 150, 235];
    for x in x0..=x1 {
        set(&mut buf, x, y0, border);
        set(&mut buf, x, y1, border);
    }
    for y in y0..=y1 {
        set(&mut buf, x0, y, border);
        set(&mut buf, x1, y, border);
    }

    let inner_x0 = x0 + 1;
    let inner_w = (x1 - 1).saturating_sub(inner_x0) + 1;
    let fill_w = ((pct / 100.0).clamp(0.0, 1.0) * inner_w as f32).round() as u32;
    let c = severity_rgb(pct);
    let fill = [c[0], c[1], c[2], 255];
    for y in (y0 + 1)..y1 {
        for x in inner_x0..inner_x0 + fill_w {
            set(&mut buf, x, y, fill);
        }
    }

    Image::new_owned(buf, w, h)
}
