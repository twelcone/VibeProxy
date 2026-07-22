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

## Track A — Linux credential/terminal backend

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

## Track C — native macOS app (the "macOS-exclusive")

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
