---
phase: 1
title: "Foundation & Profile Model"
status: pending
priority: P1
effort: "3-4d"
dependencies: [0]
---

# Phase 1: Foundation & Profile Model

## Overview

Stand up the Tauri v2 (Rust + web) skeleton as a menubar-only app (no dock icon), define the
on-disk profile layout under `~/.vibeproxy/`, and render a tray that lists profiles from a stored
config. No live usage or switching yet — this is the scaffold everything else hangs off.

> **Gated by Phase 0.** Do not start until the mechanism spike returns GO (or a recorded adaptation).
> If Phase 0 changes the credential-store or keep-fresh design, reflect it in the module boundaries below.

## Requirements

- Functional: Tauri v2 app builds and runs on macOS; a tray icon appears with a static menu; a
  hidden main window exists but is not shown at launch; profiles are read from and written to a
  versioned `config.json`.
- Non-functional: no dock icon (agent app); single Rust process; the Rust core has zero macOS-only
  code paths outside clearly `#[cfg(target_os = "macos")]` modules (Windows portability preserved).

## Architecture

**On-disk layout (VibeProxy-owned, never inside `~/.claude`):**

```
~/.vibeproxy/
  config.json                 # { schemaVersion, activeProfileId, profiles:[...], settings:{...} }
  profiles/
    <profile-id>/             # this dir IS a CLAUDE_CONFIG_DIR for that account
      .credentials.json       # file-based OAuth creds (mode 0600) — populated in Phase 3
      .claude.json            # per-profile Claude Code config incl. oauthAccount block
      ...                     # settings.json, history, projects (isolated per profile)
```

**Profile record (in `config.json`):**

```jsonc
{
  "id": "p_ab12cd",            // stable random id, used as the dir name
  "label": "Work Max",         // user-facing name
  "email": null,               // filled from oauthAccount after login (Phase 3)
  "org": null,
  "priority": 0,               // auto-switch order (lower = preferred)
  "createdAt": "2026-07-19T..."
}
```

**Rust module boundaries (src-tauri/src/):**

- `profile/store.rs` — load/save `config.json`, CRUD on profile records (serde).
- `profile/paths.rs` — resolve `~/.vibeproxy`, per-profile dirs; the ONLY place that knows the layout.
- `tray/mod.rs` — build tray, render menu from store, `ActivationPolicy::Accessory`.
- `platform/macos.rs` (+ `platform/mod.rs` trait) — OS-specific bits behind a trait so Windows can stub.

## Related Code Files

- Create: `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `Cargo.toml`
- Create: `src-tauri/src/profile/{mod.rs,store.rs,paths.rs}`
- Create: `src-tauri/src/tray/mod.rs`, `src-tauri/src/platform/{mod.rs,macos.rs}`
- Create: web frontend scaffold (`index.html`, framework of choice — keep minimal; Vanilla/Svelte/React)
- Create: `.gitignore`, `README.md` (stub)

## Implementation Steps

1. `npm create tauri-app@latest` (Tauri v2) → pick a light frontend; confirm `cargo tauri dev` runs.
2. In `.setup()`, call `#[cfg(target_os = "macos")] app.set_activation_policy(ActivationPolicy::Accessory)` to drop the dock icon; do not auto-show the main window.
3. Build the tray via `TrayIconBuilder` with a placeholder icon + a menu (`Quit`, `Open VibeProxy`, and a `Profiles` section rendered from the store — empty-state = "No profiles").
4. Implement `profile/paths.rs` + `profile/store.rs`: create `~/.vibeproxy/` on first run, read/write `config.json` with `schemaVersion` and atomic write (temp file + rename).
5. Define the `platform` trait (`read_credentials`, `active_dir_pointer`, etc. — stubs for now) with a macOS impl file and a `#[cfg(not(macos))]` fallback that returns `unimplemented`/errors.
6. Wire two Tauri commands: `list_profiles()` and `get_settings()` returning store data to the UI (UI itself is Phase 6; here just verify the bridge).

## Success Criteria

- [ ] `cargo tauri dev` launches a tray-only app with no dock icon on macOS
- [ ] `~/.vibeproxy/config.json` is created and round-trips profile records
- [ ] Tray menu renders the (empty) profile list from the store
- [ ] `cargo build` succeeds targeting Windows (stubs compile) — verified in CI or locally with `--target x86_64-pc-windows-msvc` if toolchain present, else deferred to Phase 7 CI

## Risk Assessment

- Tauri v2 tray/dock APIs are young — prototype the tray + `Accessory` policy first (highest-uncertainty bit). Mitigation: multiple community write-ups confirm the exact pattern (see `reports/research-260718-2353-quota-menubar.md` §B.1).
- Keep the `platform` trait honest from day one so Windows portability doesn't silently rot.
