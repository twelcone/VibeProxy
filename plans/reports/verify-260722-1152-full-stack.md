# Full-Stack Verification — everything so far

Date: 2026-07-22 · Branch: `refactor/core-extraction` (33 commits ahead of main, tree clean)

Complete verification across every layer after the core-extraction refactor (phases 1–4 + phase-5
Track A) and all UI/feature work before it.

## Summary: all green (one honest gap — Linux execution)

| Layer | Check | Result |
|---|---|---|
| Rust — unit + integration | `cargo test --workspace` | **62 pass**, deterministic across 3 runs |
| Rust — real-data E2E | `--include-ignored` (real Keychain, real logs, FileStore, journal, cache) | **5 pass** → **67 total** |
| Rust — quality | `cargo clippy --all-targets` | **0 warnings** (21 fixed) |
| Frontend — types | `svelte-check` | 0 errors, 0 warnings, 197 files |
| Frontend — unit | `vitest` | **40 pass** (format + chart primitives) |
| Frontend — build | `vite build` | ✅ `build/usage/index.html` present (404 fix holds) |
| App — bundle | `tauri build` | ✅ 15M `.app` + 4.7M `.dmg`; `/usage/index.html` embedded |
| CLI — behaviour | 45 adversarial manual checks + 6 hermetic integration tests | ✅ (see cli-qa report) |
| CLI — shell loop | real bash: switch → `CLAUDE_CONFIG_DIR` → `claude` follows it | ✅ |
| Machine hygiene | `~/.vibeproxy`, `~/.zshrc`, `~/.claude` | clean / clean / intact |

Test count end to end: **67 Rust + 40 frontend + 45 manual CLI = 152 verifications.**

## What each layer proved

- **Core (`vibeproxy-core`)** — zero Tauri in its dep tree; the credential logic (hot-swap
  backup/restore) is unit-tested with an in-memory fake AND against the real Keychain; analytics
  scan/attribution verified against the real 3.2B-token log set with internal-consistency asserts.
- **CLI (`vibeproxy`)** — every command + error path + `--json` shape; standalone 5.2M binary, no
  Tauri; the switch→shell→claude redirect proven in a real shell.
- **App (Tauri)** — links against the extracted core, still builds and bundles; the Usage window's
  production asset path (`/usage/`) is embedded (the shipped-404 regression stays fixed).
- **Frontend** — the display-bug regressions (locale numbers, pct rounding, chart date parsing) stay
  pinned by vitest; the shared design tokens type-check.

## Fixed during this verification

- **21 clippy lints** (clippy had never been installed). Mechanical ones via `--fix`; four in
  `scan.rs` by hand (descending sorts → `sort_by_key(Reverse)`, a type alias). No behaviour change —
  suite green before and after.

## Not verified (unchanged honest gaps)

- **Linux/WSL execution.** `FileStore` (the non-macOS credential backend) is fully unit-tested on
  macOS, but its wiring as the platform default — `credentials()` returning it off macOS — is a
  `cfg` only a Linux runner exercises. This is the single claim that cannot be closed from this
  machine.
- **Hot-swap on a live session** (opt-in, off by default). Logic tested via the fake store; the
  actual "a running `claude` picks up the swapped account" is unverified end-to-end and needs two
  real accounts + a live session.
- **CLI ↔ app concurrent writes.** Last-writer-wins on `config.json`/`active-path`; expected, not
  stress-tested.

## Recommendation

The macOS-verifiable surface is fully green and deterministic. The one meaningful next step for
cross-platform confidence is a **Linux CI job** running `cargo test --workspace` + the CLI suite,
which is the only way to close the `FileStore` wiring gap. Everything else is ready for a PR/review.

## Unresolved questions

- Set up Linux CI now, or defer until Windows/SwiftUI tracks begin?
- Merge/PR `refactor/core-extraction` before starting phase-5 tracks B–D?
