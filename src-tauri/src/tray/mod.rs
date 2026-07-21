//! Menubar tray: renders the profile list from the store, switches on click, hosts the app menu.

use crate::profile;
use crate::usage::{ProfileUsage, UsageStatus};
use std::collections::HashMap;
use tauri::{
    image::Image,
    tray::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, LogicalPosition, Manager, Rect,
};

const TRAY_ID: &str = "main";

/// Gap between the menubar and the panel, in logical pixels.
const PANEL_GAP: f64 = 6.0;

/// Build the tray icon + menu and attach it. Called once in `setup()`.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let cfg = profile::store::load();
    // Deliberately no `.menu()`. A native NSMenu can only draw plain text rows, and while macOS is
    // free to pop an attached menu on any click, suppressing that per-button proved unreliable.
    // With no menu attached the click always reaches `on_tray_icon_event`, which opens the panel.
    // Everything the menu used to offer lives in the panel's own toolbar.
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .tooltip("VibeProxy")
        .on_tray_icon_event(on_tray_event)
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
    if let Some(state) = app.try_state::<crate::usage::UsageState>() {
        if let Ok(map) = state.try_read() {
            update_icon_and_title(app, &tray, &cfg, &map);
            return;
        }
    }
    reset_icon(app, &tray);
    apply_title(&tray, &cfg);
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

/// Left-click the menubar icon → toggle the panel, anchored under the icon.
fn on_tray_event(tray: &TrayIcon, event: TrayIconEvent) {
    // Match on the button only, not the up/down state — see `toggle_debounced`.
    let TrayIconEvent::Click { button: MouseButton::Left, rect, .. } = event else {
        return;
    };
    let app = tray.app_handle();
    let Some(win) = app.get_webview_window("main") else { return };

    // The panel hides itself when it loses focus, and clicking the tray icon is what takes focus
    // away. Without this guard the hide-then-click sequence would immediately reopen it.
    if crate::panel_recently_hidden() || !crate::toggle_debounced() {
        return;
    }

    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
        return;
    }
    anchor_under_tray(&win, rect);
    let _ = win.show();
    let _ = win.set_focus();
}

/// Centre the window horizontally on the tray icon, just below the menubar. Clamped to the icon's
/// monitor so a panel anchored near a screen edge stays fully on screen.
fn anchor_under_tray(win: &tauri::WebviewWindow, rect: Rect) {
    let scale = win.scale_factor().unwrap_or(1.0);
    let icon = rect.position.to_logical::<f64>(scale);
    let icon_sz = rect.size.to_logical::<f64>(scale);
    let Ok(win_sz) = win.outer_size() else { return };
    let win_w = win_sz.to_logical::<f64>(scale).width;

    let mut x = icon.x + icon_sz.width / 2.0 - win_w / 2.0;
    let y = icon.y + icon_sz.height + PANEL_GAP;

    if let Ok(Some(mon)) = win.monitor_from_point(icon.x, icon.y) {
        let m_pos = mon.position().to_logical::<f64>(scale);
        let m_size = mon.size().to_logical::<f64>(scale);
        let min_x = m_pos.x + PANEL_GAP;
        let max_x = m_pos.x + m_size.width - win_w - PANEL_GAP;
        x = x.clamp(min_x, max_x.max(min_x));
    }

    let _ = win.set_position(LogicalPosition::new(x, y));
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
