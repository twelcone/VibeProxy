---
phase: 5
title: "Auto-Switch Engine"
status: completed
priority: P1
effort: "2-3d"
dependencies: [3, 4]
---

# Phase 5: Auto-Switch Engine

## Overview

When the active profile crosses a quota threshold, automatically repoint the active profile to the
best eligible alternative and notify the user. This is the "auto-switch when one profile is out of
quota" requirement, built on Phase 4's usage signal and Phase 2's switch broker.

## Requirements

- Functional: monitor the active profile's usage; when it crosses the configured threshold (or its
  binding limit is exhausted), select the next eligible profile by priority + remaining quota,
  repoint active, and notify. If no profile is eligible, alert with the earliest reset time instead
  of looping. A cooldown prevents rapid flip-flopping.
- Non-functional: the engine only ever calls Phase 2's `set_active` — it does not touch tokens, run
  inference, or restart the user's running session.

## Architecture

**Trigger (pre-emptive, numeric-only):** on each `usage-updated` for the active profile, if
`five_hour_pct >= threshold` (default 95) OR `weekly_pct >= threshold` → fire a switch evaluation.
Use **numeric utilization only** — do NOT depend on `limits[].severity` strings or an exhaustion
`error.type`, whose values near 100% were never observed (plan open questions); if a real exhaustion
shape is captured in Phase 0, it can be added as an *additional* trigger later. (Reactive 429 detection
is out of scope: without a proxy VibeProxy can't see Claude Code's live 429s; the pre-emptive threshold
fires *before* the user hits a failed turn — that is the whole value.)

**Selection:** among profiles ≠ active that are **eligible** — `status == Ok` and not stale/`needs_reauth`
(Phase 4), `five_hour_pct < threshold - margin` (hysteresis), weekly not exhausted — pick lowest
`priority`, tie-break by lowest `five_hour_pct`. A profile with stale or errored usage data is NOT a
candidate (switching to an account we can't confirm has quota is worse than not switching). None
eligible → alert "all accounts near limit; earliest reset <label> in <time>" and do not switch.

**Cooldown / anti-flap:** after a switch, suppress further auto-switches for `cooldown` (default 5min,
per `claude-swap` precedent). Manual switches bypass cooldown. Don't switch back to a profile whose
usage hasn't materially dropped.

**Notify + make it real:** native notification (`tauri-plugin-notification`) + tray title flash:
"Switched to <label> (Work Max at 96%)". Because the switch only affects the *next* `claude` launch,
the notification includes a **"Relaunch Claude Code with new profile"** action (reuses Phase 2's launch
broker) so the switch has immediate effect instead of waiting for the user to restart manually. Log the
decision to an in-memory ring buffer for a UI activity view (Phase 6).

**Rust module:** `autoswitch/engine.rs` — subscribes to usage state, holds threshold/cooldown/enabled
from settings, calls `switch::broker::set_active`, emits notifications + activity events.

## Related Code Files

- Create: `src-tauri/src/autoswitch/{mod.rs,engine.rs}`
- Modify: `src-tauri/src/lib.rs` (spawn engine, subscribe to `usage-updated`)
- Modify: `src-tauri/src/switch/broker.rs` (expose a `set_active` that returns the prior id for logging)
- Modify: settings model (threshold, cooldown, enabled) — consumed here, edited in Phase 6

## Implementation Steps

1. `autoswitch/engine.rs`: subscribe to usage updates; gate on `settings.auto_switch_enabled`.
2. Threshold evaluation for the active profile (5h + weekly + binding limit exhausted).
3. Eligible-profile selection (priority, then remaining quota); handle none-eligible.
4. Call `set_active`, start cooldown timer, emit notification + activity-log event.
5. Anti-flap guards: cooldown window, and a "don't re-select a profile still ≥ threshold" rule.
6. Manual-override interaction: user manual switch resets/ignores cooldown and is logged distinctly.

## Success Criteria

- [ ] Driving the active profile's reported 5h% past the threshold triggers exactly one switch to the best eligible profile
- [ ] With all profiles near limit, the user gets a clear "earliest reset" alert and no switch loop
- [ ] Cooldown prevents repeated switches within the window; manual switch bypasses it
- [ ] Every auto-switch produces a user-visible notification and an activity-log entry
- [ ] Engine never calls anything except `set_active` + notify (no token/inference access) — verified by module boundaries
- [ ] Unit tests cover the state machine: threshold trigger, hysteresis, cooldown, none-eligible, and stale/`needs_reauth` exclusion

## Risk Assessment

- **Pre-emptive-only (no live 429)** means a switch takes effect on the *next* `claude` launch; a mid-turn exhaustion still fails that turn. Mitigation: conservative threshold (90–95%) so switches happen before exhaustion; the notification's "relaunch" action makes it immediate; document the behavior honestly (no overclaiming "seamless").
- **Switching to a profile with stale/expired data** — mitigated by the eligibility rule (only `Ok`/fresh profiles), backed by Phase 4 keep-fresh; re-poll the target immediately post-switch to confirm before relying on it.
- **Weekly-threshold switching strands the last few % of a week's quota.** Mitigation: separate (higher) weekly threshold, and let the user disable weekly-triggered auto-switch; document the trade-off.
- **Exhaustion signal shape unknown near 100%** (open question) — rely on numeric threshold, not `severity` strings.
- Flip-flop / thrash — cooldown + hysteresis (switch out at ≥threshold, only consider a profile eligible below `threshold - margin`).
