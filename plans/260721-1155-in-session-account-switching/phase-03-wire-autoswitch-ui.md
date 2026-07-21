---
phase: 3
title: "Wire auto-switch + UI affordance"
status: pending
priority: P2
effort: "1-2d"
dependencies: [1, 2]
---

# Phase 3: Wire auto-switch + UI

## Overview

Connect hot-swap to the quota auto-switch engine so the *current* session is rescued, and make the
behaviour visible and reversible.

## Requirements

- Auto-switch attempts a hot-swap on dirs with a live session, then falls back to the path file
- Opt-in setting, default off for the first release (it writes credentials)
- Panel shows when a session was hot-swapped and to what
- Usage window annotates ranges that span a swap

## Related code files

- Modify: `src-tauri/src/autoswitch/mod.rs` — call hot-swap before/alongside `activate`
- Modify: `src-tauri/src/profile/store.rs` — `hot_swap_enabled: bool` in Settings
- Modify: `src/routes/+page.svelte` — Settings toggle, activity entry
- Modify: `src/routes/usage/+page.svelte` — swap annotation on affected buckets

## Implementation steps

1. Settings flag plus a plain-language explanation of the trade-off in the Settings tab.
2. Auto-switch: when enabled, hot-swap the active dir; always also write the path file so new
   terminals agree with running ones.
3. Activity log entry: "Hot-swapped Work → Personal (quota 96%)".
4. Usage window: mark buckets containing a boundary so a split day is not read as a single account's.
5. Failure path: if hot-swap fails, fall back to today's behaviour and say so, rather than silently
   doing nothing.

## Success criteria

- [ ] Quota exhaustion mid-session moves the running session to a fresh account
- [ ] Disabled by default; enabling states the trade-off plainly
- [ ] Running terminals and new terminals never disagree about the active account
- [ ] A failed hot-swap degrades to the existing mechanism with a visible notice

## Risks

- **Two mechanisms disagreeing** is the worst outcome. Always write the path file too, so a new
  terminal matches a hot-swapped session.
- Auto-switch firing repeatedly could thrash credentials; reuse the existing cooldown.
