---
phase: 5
title: "Future: per-OS backends + native frontends"
status: pending
priority: P2
effort: "sized later"
dependencies: [4]
---

# Phase 5: Future tracks (documented, not executed here)

Recorded now so the phase-3 seams are shaped correctly. Each is a separate effort with its own plan.

## Track A — Linux credential/terminal backend  ✅ credential store DONE

**Implemented** (`platform/file_store.rs`): a `FileStore` reading/writing
`<config_dir>/.credentials.json` (atomic, 0600), with the pre-swap backup in a sibling file. It is
the wired default on non-macOS via `platform::credentials()`, and — because it is pure file I/O —
its full CRUD + once-only-backup + missing-file behaviour is unit-tested on macOS (4 tests). The
hot-swap flow already works against any `CredentialStore`, so it needs no per-OS branching. What
remains for Track A: the terminal launcher (below) and a real Linux CI run to exercise the cfg path.

Original notes:

- **Credentials:** confirm Claude Code on Linux uses a plaintext `.credentials.json` (research says
  yes). If so, `CredentialStore` for Linux is atomic file read/replace with `0600` — *simpler* than
  macOS, and hot-swap becomes a file swap with no keychain, no `security`, no lock CLI.
- **Terminal:** `x-terminal-emulator` / `$TERMINAL` / a sensible fallback list.
- **Config dir:** `~/.claude` (verify) with XDG awareness.
- Likely the **first** backend — simplest, and where much dev happens (incl. WSL).

## Track B — Windows credential/terminal backend

- **Credentials:** `.credentials.json` file (verify) or Credential Manager. WSL is really Track A
  (Linux) reached from Windows; native Windows is separate.
- **Terminal:** Windows Terminal / `cmd`.
- **Cross-boundary caveat:** a native-Windows GUI managing *WSL* credentials must reach `\\wsl$\…` and
  the WSL-side store — messy. The clean answer for WSL users is the Linux CLI *inside* WSL, not the
  Windows GUI reaching in. Document this explicitly.

## Track C — native macOS app (the "macOS-exclusive")  ✅ DONE (builds and runs)

**FFI layer** (`crates/vibeproxy-ffi`): a uniffi adapter over the core — a peer of the CLI and the
Tauri app, so the core stays FFI-free. Rich types cross as JSON strings (the same shapes the CLI's
`--json` emits), decoded on the Swift side with Codable; actions return `Result`, which becomes a
throwing Swift function. Exposed: `coreVersion`, `listProfilesJson`, `activeProfileId`,
`usageJson(range:)`, `switchProfile(target:)`, `shellSnippet`. Three hermetic Rust tests cover it in CI.

**The app** (`apps/macos`): a SwiftUI `MenuBarExtra` popover + a full analytics `Window`, driving the
core directly through the FFI — no webview. Menu bar shows a gauge + live spend; popover has the active
account, range picker, token-class bar, and one-click account switching; the window has stat cards, a
daily-tokens trend chart (native Swift Charts), and per-model/per-account breakdowns.

**Built without Xcode.** The macOS SDK shipped with Command Line Tools contains `SwiftUI`, `AppKit`,
and `Charts`, so `apps/macos/build.sh` compiles the app with `swiftc` and hand-assembles the `.app`
bundle (no `xcodebuild`). CI builds it on every push.

**Complete + E2E tested.** The app leads with live quota (5-hour + weekly %, the product's headline),
account switching (Active badge / Switch ›, plus "Open Claude" to make a switch take effect), add /
remove accounts, the analytics window, and a shell-integration setup hint. Every feature is E2E
tested — hermetic Rust tests for the FFI/core glue, live verification for the endpoint/GUI paths —
see `plans/reports/test-260723-1232-macos-app-e2e.md`. Three real bugs were found and fixed in the
process (config-corruption data loss, quota-blanking on a transient 429, self-inflicted rate limiting).

Original notes:

- **How it attaches:** `uniffi` generates Swift bindings from `vibeproxy-core`; SwiftUI calls the
  Rust core directly — no logic rewritten. The menubar becomes stock `MenuBarExtra` (auto-sized,
  native positioning — none of the Tauri panel-chrome pain), Charts via SwiftUI Charts.
- **Why it's cheap once the core is extracted:** it's a frontend, not a fork. It shares credential
  logic, scan, hot-swap, journal with the CLI and the Tauri app.
- **Product framing:** a premium native Mac experience alongside the cross-platform Tauri/CLI — the
  original motivation. Sequence it *after* the CLI proves the core boundary.

## Track D — Windows/Linux GUI

- Keep **Tauri**, reuse the existing Svelte UI, on the extracted core. This is the desktop GUI for
  the platforms SwiftUI can't reach. Little new work beyond the phase-3 backends.

## Guidance the seams must honour

- `CredentialStore` must express **both** a keychain-style store (macOS) and a file-style store
  (Linux/Windows) without leaking either into shared code.
- Hot-swap's backup/restore is defined on `CredentialStore`, so it works for a file store (copy the
  file aside) as cleanly as for a keychain item.
- Nothing in core assumes a GUI, an event bus, or a single running instance.
