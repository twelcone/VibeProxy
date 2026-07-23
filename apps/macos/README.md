# VibeProxy — native macOS menubar app

A SwiftUI menubar app (`MenuBarExtra` + a full analytics `Window`) that drives the shared Rust
core (`vibeproxy-core`) directly through uniffi — no webview, no Tauri. It's a peer of the CLI and
the Tauri app; all three sit on the same core.

## What it shows

The headline is **quota**, not cost — how much of your Claude Code Pro/Max limits you've used:

- **Menu bar:** a gauge + the active account's live 5-hour quota % (e.g. `33%`), the glanceable
  "am I about to run out" number.
- **Popover:** the active account's 5-hour and weekly limits (percent + bar + reset countdown), and
  the account switcher — every account with its own live 5-hour %, one click to switch.
- **Analytics window:** the historical, secondary view — stat cards, a daily-tokens trend chart
  (native Swift Charts), per-model/per-account breakdowns, and a per-model table. "API value" here is
  API-equivalent cost, not your subscription price.

On first run it adopts the default `~/.claude` login as "Main" so there's always an account to show.

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

The FFI surface lives in `crates/vibeproxy-ffi/src/lib.rs`: `coreVersion`, `bootstrapDefaultProfile`,
`listProfilesJson`, `activeProfileId`, `usageAllJson` (live 5h/weekly quota per account),
`usageJson(range:)` (historical analytics), `switchProfile(target:)`, `shellSnippet`. Rich types cross
as JSON strings — the same shapes `vibeproxy … --json` emits — decoded here with Codable. The live
quota poll is async (reqwest); the FFI blocks on a small runtime so the Swift call stays synchronous.
