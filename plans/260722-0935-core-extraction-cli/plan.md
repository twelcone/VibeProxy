---
title: "Core Extraction + CLI (the everywhere layer)"
description: "Lift VibeProxy's Tauri-free logic into a standalone `vibeproxy-core` crate and a `vibeproxy` CLI, so the app works headless — WSL, SSH, servers — where no GUI can run, and so future frontends (a native macOS app, a Windows/Linux GUI) become thin layers on one core rather than rewrites."
status: pending
priority: P1
effort: "~1 week"
tags: [vibeproxy, architecture, workspace, cli, cross-platform]
created: 2026-07-22
---

# Core Extraction + CLI

## Why

A GUI cannot exist in WSL, over SSH, or on a headless box — but `claude` runs in all of them. The
surface that is genuinely "used everywhere" is therefore a **CLI**, not a menubar app. The GUI is a
desktop nicety on top.

Today the logic is reachable only through the Tauri app. The good news, measured directly: the core
is **already Tauri-free**. Every valuable module — `keychain`, `usage_analytics`, `switch`,
`profile`, `usage::client`, and `autoswitch::decide` — has **zero** Tauri references. The coupling
lives in three glue files (`lib.rs`, `usage::poller`, `tray`). So this is extraction, not a rewrite.

Extracting the core unlocks, in one move:
- **Headless / WSL / SSH usage** via the CLI — the actual cross-platform goal.
- **A future native macOS app** (SwiftUI via uniffi) as a *frontend on the same core*, not a fork.
- **A Windows/Linux GUI** (keep Tauri, reuse the Svelte UI) on the same core.
- Independently testable, side-effect-free credential logic — the part where regressions are most
  dangerous.

## Target shape

```
Cargo workspace (root)
├── crates/vibeproxy-core     library — no Tauri, no I/O side effects
│     keychain · usage_analytics · switch · profile · usage::client · autoswitch::decide
│     + platform seams (CredentialStore, TerminalLauncher, ClaudeConfig) — macOS impl now
├── crates/vibeproxy-cli      bin `vibeproxy` — headless, works in WSL/SSH
└── src-tauri                 the desktop app — thin adapter: commands, poller, tray, events
                              depends on vibeproxy-core
```

The rule that keeps this honest: **core computes and reads; frontends do side effects.** Core never
emits an event, draws a tray, or spawns a window. It returns values; the app (or CLI) acts on them.

## Phases

| # | Phase | Status | Depends on |
|---|-------|--------|-----------|
| 1 | [Workspace + `vibeproxy-core` crate](./phase-01-workspace-core-crate.md) | ✅ Done | — |
| 2 | [Decouple side effects from the core](./phase-02-decouple-side-effects.md) | ✅ Done | 1 |
| 3 | [Platform seams behind traits (macOS impl)](./phase-03-platform-seams.md) | Pending | 1 |
| 4 | [`vibeproxy` CLI (headless / WSL)](./phase-04-cli.md) | Pending | 1, 2, 3 |
| 5 | [Future: per-OS backends + native frontends](./phase-05-future-tracks.md) | Pending | 4 |

Phases 1–4 are the deliverable: a working CLI on a clean core, with the Tauri app still working
unchanged on top. Phase 5 is documented, not executed here — it's the map for Windows/Linux
credential backends and the SwiftUI-via-uniffi track, so the seams built in phase 3 are shaped right.

## Non-goals (this plan)

- Rewriting any logic. Modules move; their internals do not change.
- Windows/Linux credential/terminal *implementations* — only the *seams* for them (phase 3).
- Building the SwiftUI app. Phase 5 records how it attaches; it is a separate future effort.
- Changing on-disk formats (`config.json`, `swaps.jsonl`, the Keychain service scheme).

## Risks

1. **Behaviour drift during the move.** Mitigate: move modules verbatim, keep every existing test,
   run the full suite (incl. the `--ignored` real-Keychain / real-log E2E) after each phase and
   require byte-identical results.
2. **The side-effect decoupling changes control flow** (poller, auto-switch). Mitigate: core returns
   a decision/aggregate; the app keeps the exact emit/tray/activate calls it has now, just moved to
   the call site. Covered by the existing `autoswitch` decision tests.
3. **Workspace layout breaks the Tauri build.** Mitigate: Tauri keeps its own crate at `src-tauri`;
   the workspace only adds sibling members. Verify `pnpm tauri build` after phase 1.
4. **Config/path resolution differs between CLI and app.** Both already resolve via `profile::paths`
   with the `VIBEPROXY_DIR` override; the CLI reuses it unchanged.

## Success criteria

- [ ] `cargo build` produces `vibeproxy-core`, `vibeproxy` (CLI), and the Tauri app from one workspace
- [ ] `vibeproxy status` / `switch` / `usage` work in a plain terminal with no GUI running
- [ ] The Tauri app behaves identically to today — same tests green, same real-data E2E numbers
- [ ] `vibeproxy-core` has zero Tauri dependency (a `cargo tree` check)
- [ ] Every OS-specific call (Keychain, terminal launch, config-dir) sits behind a trait with a
      macOS implementation, so a Linux/Windows impl is an addition, not an edit
- [ ] No on-disk format changed; an app built before and after reads the same `config.json`

## Open questions

- **Where do Windows/Linux keep Claude credentials?** Prior research says a plaintext
  `.credentials.json`, not a keychain — confirm before shaping `CredentialStore` (phase 3).
- **CLI output contract:** human-readable by default, `--json` for scripting? (Assume yes.)
- **Does the CLI share the running app's state live, or read the same files?** Files — both read
  `config.json` / logs; no IPC between CLI and app. Note any write-write races (switch from both).
