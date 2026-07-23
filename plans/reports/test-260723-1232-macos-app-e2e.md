# macOS app — E2E test report

Date: 2026-07-23. Branch: `feat/swift-ffi`. Target: `apps/macos` (SwiftUI menubar app) on
`vibeproxy-core` via the uniffi FFI.

## Method

- **Hermetic**: Rust tests over an isolated `VIBEPROXY_DIR` (core + FFI). Run in CI.
- **Live**: the real `~/.claude` account on this machine — quota endpoint, log scan, switch broker,
  and the built `.app` driven via the accessibility API + screenshots. Not in CI (needs a logged-in
  `claude` + live state).

## Coverage — every feature

| Feature | How verified | Result |
|---|---|---|
| Live quota poll (5h + weekly) | `cli status --json` against the real endpoint | ✅ `5h=45% weekly=5% status=ok` |
| Menu-bar shows quota % | AX label of the status item | ✅ `45%` (gauge + %) |
| Popover: quota bars + reset countdown | screenshot | ✅ 5h/weekly bars, "resets in …" |
| Account switch (config-swap) | `cli switch Main` → active-path file | ✅ switched; default clears path (correct) |
| Switcher UI (Active badge / Switch ›) | screenshot with 2 accounts | ✅ active badged, others show "Switch ›" |
| Open Claude (relaunch) | FFI `relaunch_claude`; no-active → error | ✅ hermetic error path; mirrors shipped Tauri cmd |
| Add account | onboarding prepare/cleanup hermetic; adopt via real login | ✅ prepare+cleanup tested; adopt verified (`Added Main`) |
| Remove account (+ re-point active) | FFI `remove_profile` hermetic | ✅ re-points to next / clears when last |
| Historical analytics | `cli usage --json` | ✅ `10,440 msgs, $7,235, 4 models, 26 days` |
| Analytics window (charts/tables) | screenshot | ✅ trend chart + breakdowns render real data |
| Shell integration hint + install | core hermetic (HOME override); popover hint | ✅ not-installed → hint shows; install idempotent |
| Bootstrap default account | hermetic gate + live | ✅ adopts `~/.claude` as Main; no-op when non-empty |
| Config resilience (corrupt file) | `store::load` salvage tests | ✅ keeps valid profiles, drops only corrupt, backs up |
| Orphan GC | onboarding hermetic | ✅ removes abandoned add dirs |

## Automated suite

`cargo test`: **54 core + 6 CLI + 9 FFI (3 unit + 6 e2e)** pass. `clippy --workspace --all-targets
-D warnings` clean. `apps/macos/build.sh` builds the `.app` end to end.

## Bugs found and fixed during E2E

1. **Menu bar showed quota %, popover showed "Usage unavailable"** — a transient poll error
   wholesale-replaced the last good reading. Fixed: `refreshUsage` keeps the last `ok` value on a
   transient `error` (the core model documents this intent).
2. **Self-inflicted 429 from aggressive polling** — the app polled on launch, on *every* popover
   open, and on a timer. The plan mandates conservative polling. Fixed: a 60s min-poll throttle
   (popover re-opens reuse the cached reading; manual Refresh, switch, and the 2-min timer force).
3. **Total account loss on a single malformed profile** — `store::load` did
   `from_str(...).unwrap_or_default()`, so one bad field reset the whole config to empty (and a
   subsequent save persisted the loss). Fixed with `salvage()` + a `.corrupt.bak` backup.

## Unresolved

- Full **add-account** browser OAuth and **live multi-account switch** need a *second* real Claude
  login, which only the user can complete; the mechanics around them are covered hermetically and the
  shared `adopt`/`switch` core is production-tested.
