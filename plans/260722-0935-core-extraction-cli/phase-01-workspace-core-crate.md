---
phase: 1
title: "Workspace + vibeproxy-core crate"
status: pending
priority: P1
effort: "2d"
dependencies: []
---

# Phase 1: Workspace + `vibeproxy-core`

## Overview

Introduce a Cargo workspace and move the Tauri-free modules into a new `vibeproxy-core` library
crate. The Tauri app depends on it. Pure mechanical relocation — no logic changes, all tests move
with their modules and stay green.

## What moves into `vibeproxy-core`

Verified Tauri-free (grep: 0 tauri refs):

- `keychain.rs`
- `usage_analytics/` (mod, scan, cost, model, export)
- `switch/` (mod, hotswap, journal, locks)
- `profile/` (mod, store, paths, account_meta)
- `usage/client.rs`, `usage/model.rs`
- `autoswitch/mod.rs` — but only `decide()` + its pure helpers and tests. `maybe_switch`/`notify`
  (which take `AppHandle`, emit events, call `crate::activate`) stay in the app; they move to a
  small `src-tauri` module in phase 2. For phase 1, split the file: pure half → core, acting half →
  app, keeping both compiling.

## What stays in `src-tauri`

- `lib.rs` (the `#[tauri::command]` adapters — rewritten to call `vibeproxy_core::…`)
- `usage/poller.rs`, `usage/mod.rs` (timer + emit + tray; phase 2 thins it)
- `tray/mod.rs`, `platform/mod.rs`, `onboarding/mod.rs`, `main.rs`
- the acting half of `autoswitch`

## Layout

```
Cargo.toml                     # NEW: [workspace] members = ["src-tauri", "crates/*"]
crates/vibeproxy-core/
  Cargo.toml                   # serde, serde_json, chrono, sha2, dirs — NO tauri
  src/lib.rs                   # pub mod keychain; pub mod usage_analytics; …
  src/… (moved modules)
src-tauri/
  Cargo.toml                   # + vibeproxy-core = { path = "../crates/vibeproxy-core" }
  src/… (glue only)
```

`crate::` paths inside moved modules stay valid because their cross-references are all core-internal
(verified: `profile::paths`, `profile::store`, `switch::journal`, `usage_analytics::model`). In
`src-tauri`, references to moved code become `vibeproxy_core::…`.

## Implementation steps

1. Create the workspace root `Cargo.toml`; add `crates/vibeproxy-core` with its manifest (copy the
   non-Tauri deps from `src-tauri/Cargo.toml`).
2. `git mv` the modules above into `crates/vibeproxy-core/src/`. Write its `lib.rs` re-exporting them
   `pub`. Keep every `#[cfg(test)]` block with its module.
3. Split `autoswitch/mod.rs`: pure `decide`/`Decision`/helpers/tests → core; `maybe_switch`/`notify`
   → a new `src-tauri/src/autoswitch.rs` that imports `vibeproxy_core::autoswitch::decide`.
4. Add the path dependency to `src-tauri/Cargo.toml`; rewrite `use crate::…` → `use vibeproxy_core::…`
   across the glue files.
5. Fix visibility: anything the app or CLI calls must be `pub` in core (e.g. `scan`, `to_csv`,
   `read_token`, `hotswap::swap_into`, `store::*`, `paths::*`, `account_meta::fetch`,
   `client::fetch`, `journal::append`).
6. `VIBEPROXY_DIR` / test env guard (`paths::ENV_SERIAL`) move into core and stay the isolation point
   for core tests.

## Tests / validation

- `cargo test -p vibeproxy-core` — every migrated unit test green.
- `cargo test -p vibeproxy-core -- --include-ignored` — the real-Keychain and real-log E2E green,
  same numbers as today.
- `cargo build` (whole workspace) + `pnpm tauri build` — app compiles and bundles.
- `cargo tree -p vibeproxy-core | grep -c tauri` == 0.

## Success criteria

- [ ] Workspace builds all three targets
- [ ] Core has no Tauri in its dependency tree
- [ ] All previously-passing tests still pass, including `--ignored` E2E, unchanged
- [ ] The Tauri app runs and behaves exactly as before

## Risks

- **Hidden `crate::` refs to glue.** Grep each moved file for `crate::tray|activate|show_usage|
  AppHandle` before moving; the earlier scout found none, but re-check per file.
- **Test env isolation.** The `ENV_SERIAL` guard must remain a single process-wide lock in core;
  do not duplicate it (this already bit us once).
