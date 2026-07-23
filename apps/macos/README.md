# VibeProxy — native macOS menubar app

A SwiftUI menubar app (`MenuBarExtra` + a full analytics `Window`) that drives the shared Rust
core (`vibeproxy-core`) directly through uniffi — no webview, no Tauri. It's a peer of the CLI and
the Tauri app; all three sit on the same core.

## What it shows

- **Menu bar:** a gauge icon + live API-equivalent spend for the selected range.
- **Popover:** active account, range picker (7d / 30d / 90d / All), spend, message/token counts, a
  stacked token-class bar, and one-click account switching.
- **Analytics window:** stat cards, a daily-tokens trend chart (native Swift Charts), per-model and
  per-account breakdowns, and a per-model table.

## Build

```sh
./apps/macos/build.sh
open apps/macos/build/VibeProxy.app
```

`build.sh` needs only the Swift toolchain from **Command Line Tools** — no full Xcode, no
`xcodebuild`. It:

1. builds `crates/vibeproxy-ffi` (the uniffi adapter over the core) as a release dylib,
2. regenerates the Swift bindings (`crates/vibeproxy-ffi/generate-swift.sh`),
3. compiles `Sources/*.swift` + the bindings with `swiftc`, linking the dylib via `@rpath`,
4. assembles `VibeProxy.app` (an `LSUIElement` agent bundle) and ad-hoc signs it.

`build/` and the generated bindings are git-ignored; both are reproduced by `build.sh`.

## Layout

| File | Role |
|------|------|
| `Sources/App.swift` | `@main`; `MenuBarExtra` popover + analytics `Window` |
| `Sources/AppState.swift` | observable state; runs FFI calls off the main actor |
| `Sources/Core.swift` | typed layer over the uniffi bindings + formatting |
| `Sources/Models.swift` | Codable models mirroring the core's JSON shapes |
| `Sources/PanelView.swift` | the menubar popover |
| `Sources/AnalyticsView.swift` | the analytics window (Swift Charts) |

The FFI surface (six functions) lives in `crates/vibeproxy-ffi/src/lib.rs`. Rich types cross as JSON
strings — the same shapes `vibeproxy … --json` emits — and are decoded here with Codable.
