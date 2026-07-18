---
phase: 6
title: "UI/UX & Settings"
status: pending
priority: P2
effort: "4-5d"
dependencies: [3, 4, 5]
---

# Phase 6: UI/UX & Settings

## Overview

Build the main management window and settings so the app is pleasant to use (the "easy to use / good
UX" requirement): profile list with usage bars and click-to-switch, add/remove/reorder, an activity
log, and settings (threshold, poll interval, auto-switch toggle, launch-at-login). The tray stays the
primary surface; this window is for management.

## Requirements

- Functional: management window (profiles with live usage bars, active indicator, switch/add/delete/
  reorder), settings panel, activity log of switches, first-run onboarding. Launch-at-login toggle.
- Non-functional: keyboard-navigable, respects `prefers-reduced-motion`, light/dark aware, no
  horizontal scrolling; window is opened from the tray and hidden (not quit) on close.

## Architecture

**Frontend ↔ Rust bridge:** reuse the Tauri commands from earlier phases (`list_profiles`,
`set_active_profile`, `add_profile`, `delete_profile`, `reorder_profiles`, `get_settings`,
`set_settings`) + subscribe to `usage-updated` and `activity` events for live UI.

**Views:**

- **Profiles** — card/row per profile: label, email/org, active badge, 5h + weekly bars with reset
  countdown, switch button, drag-to-reorder priority, delete.
- **Add profile** — kicks off Phase 3 onboarding; shows the login-in-progress state.
- **Activity** — reverse-chron list of auto/manual switches (from the engine's ring buffer).
- **Settings** — `auto_switch_enabled`, `threshold_pct`, `poll_interval_secs`, `cooldown_secs`,
  `launch_at_login`, and a "Claude Code integration" helper (shows the `export CLAUDE_CONFIG_DIR=...`
  snippet + a "copy" button, and a "relaunch Claude Code with active profile" action).

**Settings model (`settings/model.rs`):** serde struct persisted in `config.json.settings`;
`set_settings` validates ranges (threshold 50–100, interval ≥60) and pushes changes to poller +
engine live.

**Launch-at-login:** `tauri-plugin-autostart` (`Builder::new().app_name("VibeProxy")`), toggled from
settings via `enable()`/`disable()`/`isEnabled()`.

## Related Code Files

- Create: `src-tauri/src/settings/{mod.rs,model.rs}`
- Create/expand: web frontend (`Profiles`, `AddProfile`, `Activity`, `Settings` views + styles)
- Modify: `src-tauri/src/lib.rs` (register autostart plugin, `set_settings` propagation, window show/hide)
- Modify: `src-tauri/src/tray/mod.rs` ("Open VibeProxy" shows the window)

## Implementation Steps

1. `settings/model.rs` + `get_settings`/`set_settings` commands with validation; hot-apply to poller/engine.
2. Profiles view with live usage bars + reset countdown; switch/add/delete/reorder wired to commands.
3. Add-profile view integrating Phase 3 onboarding states (in-progress, success, error, dedupe warning).
4. Activity view fed by the engine's `activity` events/ring buffer.
5. Settings view incl. launch-at-login (autostart plugin) and the Claude Code integration helper (config-dir snippet + relaunch action).
6. Window lifecycle: open from tray, hide-on-close (don't quit), remember size; first-run shows onboarding.
7. Accessibility pass: keyboard nav, focus states, reduced motion, light/dark, mobile-width safety (window is resizable).

## Success Criteria

- [ ] User can add, switch, reorder, and delete profiles entirely from the window with live usage bars
- [ ] Settings changes (threshold, interval, auto-switch, launch-at-login) take effect without restart
- [ ] Activity log lists auto + manual switches with timestamps and reasons
- [ ] Closing the window hides it (app keeps running in the menubar); "Open VibeProxy" reopens it
- [ ] UI is keyboard-navigable, dark/light aware, and honors reduced-motion

## Risk Assessment

- Scope creep in UI — keep to the four views above; usage-history charts are explicitly deferred (YAGNI).
- Settings that must propagate live (interval/threshold) — use shared state (`watch`/`RwLock`) rather than requiring a restart; test hot-apply.
- autostart plugin path differences per OS — acceptable (macOS primary); verify enable/disable idempotency.
