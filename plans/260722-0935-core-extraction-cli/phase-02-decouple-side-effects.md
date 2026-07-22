---
phase: 2
title: "Decouple side effects from the core"
status: pending
priority: P1
effort: "1-2d"
dependencies: [1]
---

# Phase 2: Decouple side effects

## Overview

Core must compute and read, never act. Two call sites currently mix the two — auto-switch and the
poller. Split them so core returns data and the app performs the emit / tray / activate / notify.
This is what lets the same core drive a CLI (which has no events or tray) and a GUI alike.

## The two seams

### Auto-switch
- **Core (already):** `decide(cfg, usage, cooling) -> Decision` — pure, tested.
- **App:** `maybe_switch` interprets the `Decision`: calls `activate`, sets the cooldown, emits
  `auto-switched` / `auto-switch-blocked`, posts a notification, and (opt-in) hot-swaps. Keep all of
  that in `src-tauri/src/autoswitch.rs` exactly as today — only its *location* changed in phase 1.
- **CLI:** can call `decide` and, for a `vibeproxy auto` command, perform the switch via core's
  `switch::set_active_config_dir` without any events. No notification, no tray.

### Poller
- **Core:** add `usage::poll_profile(config_dir, is_active) -> ProfileUsage` (the body of today's
  `poll_one`, minus anything Tauri) and keep `client::fetch` in core.
- **App:** `usage/poller.rs` keeps the timer, the shared `UsageState`, `app.emit("usage-updated")`,
  `tray::apply_active_usage`, and the `maybe_switch` call. It calls `core::usage::poll_profile` for
  the data.
- **Identity refresh** (`refresh_identities`) is core-worthy too: core exposes
  `profile::refresh_identity(profile) -> Option<Profile>` (pure compute from `account_meta::fetch`);
  the app persists and emits `profiles-updated`.

## Implementation steps

1. Move `poll_one`'s Tauri-free body into `core::usage::poll_profile`; the app's `poll_one` becomes a
   thin caller (it already only used `spawn_blocking` + the return).
2. Extract the identity-diff logic into `core::profile::refresh_identity`; the app keeps the persist +
   emit.
3. Confirm `autoswitch::decide` in core is the only decision path; the app's `maybe_switch` consumes
   `Decision` and owns every side effect.
4. Audit: `grep -rn 'emit\|AppHandle\|NotificationExt\|tray::' crates/vibeproxy-core` must be empty.

## Tests / validation

- Existing `autoswitch` decision tests cover the split (they already test `decide` in isolation).
- Add a core test for `refresh_identity`: unchanged identity → `None`; changed email/org → `Some`.
- App still emits the same events (manual check: switch fires `auto-switched`; a re-login fires
  `profiles-updated`).

## Success criteria

- [ ] `grep` finds no `emit`/`AppHandle`/`tray`/notification in `vibeproxy-core`
- [ ] Poller and auto-switch behave identically in the app
- [ ] Core exposes `poll_profile`, `decide`, `refresh_identity` as pure, testable functions

## Risks

- **Silent behaviour change in the poller cadence.** Keep the tick/interval logic in the app; only
  the per-profile fetch moves. Don't refactor timing here.
