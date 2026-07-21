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

/// Signed distance from a point to a rounded rectangle: negative inside, positive outside.
/// Clamping the point into the rect's inner core reduces the corners to a circle of radius `r`.
fn rrect_distance(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    // A rect narrower or shorter than its own corner diameter has an empty core, and `clamp` panics
    // when min exceeds max. Collapse that axis to its midpoint instead — the shape degrades to a
    // circle, which is the correct rendering for a pill shorter than it is round.
    let core = |lo: f32, hi: f32, v: f32| if hi < lo { (lo + hi) / 2.0 } else { v.clamp(lo, hi) };
    let cx = core(x0 + r, x1 - r, px);
    let cy = core(y0 + r, y1 - r, py);
    let (dx, dy) = (px - cx, py - cy);
    (dx * dx + dy * dy).sqrt() - r
}

/// Draw the fill-meter tray icon: a pill-shaped gauge whose fill = 5-hour %, colored by severity.
///
/// Hand-drawn RGBA (no image crate). Coverage comes from a rounded-rect distance field rather than
/// plotting edges directly, which is what makes the corners round and anti-aliased — the previous
/// version stepped along four straight edges and could only ever produce a hard rectangle. Rendered
/// at 2x so it stays crisp when macOS scales it for a Retina menubar.
fn draw_meter(pct: f32) -> Image<'static> {
    const SCALE: f32 = 2.0;
    let (w, h): (u32, u32) = (30 * SCALE as u32, 16 * SCALE as u32);

    // Pill geometry in device pixels, inset so anti-aliasing has room at the edges.
    let (x0, y0) = (1.0 * SCALE, 3.0 * SCALE);
    let (x1, y1) = (w as f32 - 1.0 * SCALE, h as f32 - 3.0 * SCALE);
    let radius = (y1 - y0) / 2.0;

    let track = [255u8, 255, 255, 56]; // reads on a light or dark menubar alike
    let c = severity_rgb(pct);
    let fill_end = x0 + (x1 - x0) * (pct / 100.0).clamp(0.0, 1.0);

    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);

            // 0.5px feather either side of the edge gives a clean 1px anti-aliased boundary.
            let track_cov = (0.5 - rrect_distance(px, py, x0, y0, x1, y1, radius)).clamp(0.0, 1.0);
            if track_cov <= 0.0 {
                continue;
            }
            // The fill is its own pill so its leading edge stays round at low percentages.
            let fill_cov = if fill_end > x0 {
                (0.5 - rrect_distance(px, py, x0, y0, fill_end.max(x0 + 2.0 * radius), y1, radius))
                    .clamp(0.0, 1.0)
            } else {
                0.0
            };

            let (r, g, b) = (c[0] as f32, c[1] as f32, c[2] as f32);
            let (tr, tg, tb) = (track[0] as f32, track[1] as f32, track[2] as f32);
            // Composite fill over track, then the whole pill over transparency.
            let a_fill = fill_cov;
            let a_track = track[3] as f32 / 255.0;
            let out_a = a_fill + a_track * (1.0 - a_fill);
            let blend = |f: f32, t: f32| {
                if out_a <= 0.0 { 0.0 } else { (f * a_fill + t * a_track * (1.0 - a_fill)) / out_a }
            };

            let i = ((y * w + x) * 4) as usize;
            buf[i] = blend(r, tr).round() as u8;
            buf[i + 1] = blend(g, tg).round() as u8;
            buf[i + 2] = blend(b, tb).round() as u8;
            buf[i + 3] = (out_a * track_cov * 255.0).round() as u8;
        }
    }

    Image::new_owned(buf, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: `clamp(min, max)` aborts when min exceeds max, which happened whenever the fill
    /// pill was narrower than its own corner diameter — every quota reading under roughly 20%.
    #[test]
    fn meter_renders_across_the_whole_range_without_panicking() {
        for pct in 0..=100 {
            let img = draw_meter(pct as f32);
            assert!(img.width() > 0 && img.height() > 0, "empty image at {pct}%");
        }
        // Values outside the nominal range are clamped, not fatal.
        for pct in [-5.0f32, 0.4, 101.0, f32::MAX] {
            let _ = draw_meter(pct);
        }
    }

    #[test]
    fn meter_fill_grows_with_utilisation() {
        let opaque = |pct: f32| {
            let img = draw_meter(pct);
            img.rgba().chunks_exact(4).filter(|px| px[3] > 200).count()
        };
        let (low, mid, high) = (opaque(10.0), opaque(50.0), opaque(95.0));
        assert!(low < mid && mid < high, "fill must grow: {low} < {mid} < {high}");
    }

    #[test]
    fn rrect_distance_is_negative_inside_and_positive_outside() {
        // 20x10 pill, radius 5, centred at (10,5)
        let d = |x, y| rrect_distance(x, y, 0.0, 0.0, 20.0, 10.0, 5.0);
        assert!(d(10.0, 5.0) < 0.0, "centre is inside");
        assert!(d(-5.0, 5.0) > 0.0, "left of the shape is outside");
        assert!(d(0.5, 0.5) > 0.0, "corner is cut away by the radius");
    }

    /// The degenerate case that caused the panic: a rect narrower than its corner diameter.
    #[test]
    fn rrect_distance_survives_a_rect_smaller_than_its_radius() {
        let d = rrect_distance(5.0, 5.0, 0.0, 0.0, 4.0, 4.0, 8.0);
        assert!(d.is_finite(), "degenerate rect must not produce NaN");
    }

    #[test]
    fn severity_matches_the_design_system_thresholds() {
        assert_eq!(severity_rgb(0.0), severity_rgb(69.9), "both good");
        assert_ne!(severity_rgb(69.9), severity_rgb(70.0), "warn boundary");
        assert_ne!(severity_rgb(89.9), severity_rgb(90.0), "crit boundary");
        assert_eq!(severity_rgb(90.0), severity_rgb(100.0), "both crit");
    }
}
