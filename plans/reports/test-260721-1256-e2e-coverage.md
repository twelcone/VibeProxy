# QA Report — E2E / function coverage

Date: 2026-07-21 · Branch: `feat/usage-analytics-dashboard`

## Results

| Suite | Result |
|---|---|
| Rust (debug, incl. ignored) | **47 passed, 0 failed** |
| Rust (release, incl. ignored) | **47 passed, 0 failed** |
| Frontend (vitest) | **40 passed, 0 failed** |
| Typecheck (`svelte-check`) | 210 files, 0 errors, 0 warnings |
| Production build | success |
| Determinism | 3 consecutive full runs, identical |
| Timezone independence | passes under UTC, America/New_York, Pacific/Auckland, America/Los_Angeles |

Total **87 automated tests**, up from 36 at session start.

## What was added

### Frontend test infrastructure (did not exist)

No test runner was installed. Added `vitest` + `vitest.config.ts` (`$lib` alias, node env),
`pnpm test` / `pnpm test:watch`.

40 tests over the 15 exported pure functions in `format.ts` and `chart/svg.ts` — previously **zero
coverage**, despite being the modules where every shipped display bug this session originated.

Regression tests pin the actual defects:

| Test | Bug it pins |
|---|---|
| `pct` never rounds to a value it has not reached | 99.93% rendered as "100%" |
| fixed uppercase suffixes regardless of locale | en-AU produced "2.8bn" / "116.2m" |
| bare currency symbol, not country-qualified | en-AU produced "US$6,149.44" |
| `shortDate` parses bare dates as local | `new Date("2026-07-14")` is UTC, renders as the 13th |
| degenerate domain pins to midpoint | single-bucket chart produced NaN paths |

### Rust — previously uncovered modules

- `tray/mod.rs` (9 fns, 0 tests) — meter renders across the full 0–100 range without panicking
  (pins the `clamp(min, max)` abort that fired for every reading under ~20%), fill grows
  monotonically, `rrect_distance` sign correctness, degenerate rect stays finite, severity
  thresholds at their boundaries.
- `profile/paths.rs` (2 fns, 0 tests) — `is_default` accepts only the true default dir and rejects
  a same-leaf path under a different parent; `VIBEPROXY_DIR` override and empty-override fallback.

## Test-quality verification

Passing tests prove little on their own, so the frontend suite was **mutation-tested** — the source
was deliberately reverted to three previously-shipped bugs:

| Mutation | Detected |
|---|---|
| `pct` back to naive `Math.round` | 1 failure |
| `shortDate` back to UTC parsing | 2 failures |
| removed the degenerate-domain guard | 1 failure |

All three caught. Source restored, suite green.

## Defect found and fixed during this run

**Test isolation violation.** Two modules each declared their own `SERIAL` mutex to guard
`VIBEPROXY_DIR`. `set_var` is process-global and Rust runs tests on parallel threads, so two
independent mutexes provided no mutual exclusion — the new `paths` test raced `store::io_tests` and
failed intermittently. Hoisted to a single process-wide `paths::ENV_SERIAL`; the journal E2E now
takes it too. Verified across 3 consecutive full runs.

This was a genuine pre-existing latent flaw, exposed rather than introduced by the new tests.

## Explicitly NOT covered

Stated plainly rather than implied by a green tick:

| Area | Why not | Risk |
|---|---|---|
| **Hot-swap on a live session** | Requires swapping real credentials between real accounts | **HIGH** — the core claim of the switching feature is unverified |
| 19 Tauri commands (`lib.rs`) | Thin wrappers over tested functions; direct testing needs a Tauri harness | Low-medium |
| Svelte components | No component test framework; would need jsdom + testing-library | Medium — layout defects found this session were all visual |
| `tray` click/anchor/positioning | Needs a real mouse; AX-synthesized clicks do not reach the handler | Medium |
| `onboarding`, `usage/poller`, `account_meta` | Spawn processes / hit the network | Medium |
| Visual regression | No screenshot baseline | Medium |

"Every function" was not achieved and is not a sensible target — `notify`, `hide_dock_icon` and
similar GUI glue have no assertable behaviour. Coverage was prioritised by risk and by where defects
have actually occurred.

## Recommendations

1. **Verify hot-swap manually.** Enable Settings → "Switch running sessions too", open a `claude`
   session, force a switch, confirm the session continues on the new account. Nothing else covers it.
2. Add jsdom + `@testing-library/svelte` if component regressions become recurring — every UI defect
   this session was caught by eye, not by tests.
3. Consider a screenshot baseline for the two windows; the visual bugs found today (legend wrap,
   email wrap, axis collision) are exactly what that catches.

## Unresolved questions

- Should `pnpm test` and `cargo test` run in a pre-push hook, or stay manual?
- Is a Tauri command-layer harness worth the setup, given the commands are thin?
- Should the ignored E2E tests run in CI, given they touch the real Keychain and the real log set?
