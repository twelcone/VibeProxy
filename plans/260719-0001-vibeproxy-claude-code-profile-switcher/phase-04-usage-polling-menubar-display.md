---
phase: 4
title: "Usage Polling & Menubar Display"
status: completed
priority: P1
effort: "3-4d"
dependencies: [2]
---

# Phase 4: Usage Polling & Menubar Display

## Overview

Poll Anthropic's `GET /api/oauth/usage` per profile to get live 5h + weekly utilization and reset
times, cache it, and render it in the menubar (tray title for the active profile + per-profile rows
in the dropdown). This is the "track profile's usage" requirement and the signal Phase 5 consumes.

## Requirements

- Functional: for each profile, fetch utilization on an interval; parse `five_hour`, `seven_day`,
  and `limits[]`; show the active profile's % in the tray title and all profiles' % in the menu;
  reset times shown as relative ("resets in 2h13m"). Graceful "usage unavailable" on error.
- Non-functional: conservative polling (default 120s, min 60s) to avoid hammering an undocumented
  endpoint; schema-tolerant parsing (unknown fields ignored, missing fields → None); reads are
  strictly usage-only (never inference).

## Architecture

**Endpoint (verified live in research §A.2):**

```
GET https://api.anthropic.com/api/oauth/usage
Accept: application/json
Authorization: Bearer <profile access token>
anthropic-beta: oauth-2025-04-20
```

Response fields consumed:

```jsonc
{
  "five_hour":  { "utilization": 9.0,  "resets_at": "2026-07-18T22:49:59Z" },
  "seven_day":  { "utilization": 24.0, "resets_at": "2026-07-23T02:59:59Z" },
  "limits": [ { "kind":"session|weekly_all|weekly_scoped", "percent":9,
               "severity":"normal", "resets_at":"...", "is_active":false } ]
}
```

Port logic near-verbatim from this repo's existing
`.claude/hooks/lib/usage-limits-cache.cjs` (proven working): same URL, headers, and the
`utilization` normalization (values arrive 0–100 or fractional). Note that hook disables itself when
`ANTHROPIC_BASE_URL` is set — irrelevant here since VibeProxy holds tokens directly and never sets a
base URL.

**Quota model (`usage/model.rs`):**

```rust
struct ProfileUsage {
  five_hour_pct: Option<f32>, five_hour_resets_at: Option<OffsetDateTime>,
  weekly_pct: Option<f32>,    weekly_resets_at: Option<OffsetDateTime>,
  binding_limit: Option<Limit>,  // limits[] where is_active == true
  fetched_at: OffsetDateTime,
  status: UsageStatus,           // Ok | Stale{age} | NeedsReauth | Error(String)
}
```

`status` drives both the UI ("live" vs greyed "needs re-login" vs "stale 2d") and Phase 5 eligibility
(only `Ok` and fresh-enough profiles are switch candidates).

**Poller (`usage/poller.rs`):** a `tokio` interval task; on each tick, read token (Phase 2 Keychain store)
→ `reqwest` GET → parse → update shared state (`Arc<RwLock<HashMap<id,ProfileUsage>>>`) → emit a Tauri
event so tray + UI refresh.

**Token freshness — the load-bearing problem (do not skip).** Access tokens expire ~24h after issue,
and Claude Code only refreshes the token of a profile *while it is running*. Under this architecture,
**inactive profiles never run**, so their Keychain-stored tokens expire within a day and `/api/oauth/usage`
returns 401 — which would make the per-profile usage display (a P1 goal) read "needs re-login" for
every non-active account almost all the time. Two mitigations, both required:

1. **Keep-fresh via the official client (ToS-safe refresh).** Before polling an inactive profile whose
   token is near/past `expiresAt`, spawn a cheap non-interactive `CLAUDE_CONFIG_DIR=<dir> claude`
   auth/status subcommand so the *official binary* refreshes that profile's creds (CCSwitcher
   precedent — research §4/§5.1). The exact subcommand + that it writes back to the file are verified
   in **Phase 0** (Q5). If no such command exists, fall back to (2) only.
2. **Poll strategy: active eagerly, inactive lazily.** Poll the *active* profile every interval. Poll
   *inactive* profiles infrequently (e.g. on menu-open, on demand, and pre-switch) rather than every
   tick — this both cuts the undocumented-endpoint/ToS surface and reduces refresh churn.

On a 401 that keep-fresh can't resolve, mark the profile `needs_reauth` (distinct from a transient
network error) and show it as such; auto-switch (Phase 5) must treat `needs_reauth`/stale profiles as
ineligible.

**Tray rendering:** active profile → tray title `"<label> · <5h%>"`; menu rows per profile:
`"<label>  5h 37% · wk 24%"` with a color/emoji severity hint. Update via cached `MenuItem` handles'
`set_text()` (no full menu rebuild).

## Related Code Files

- Create: `src-tauri/src/usage/{mod.rs,model.rs,poller.rs,client.rs}`
- Modify: `src-tauri/src/tray/mod.rs` (title + per-profile row updates from usage state)
- Modify: `src-tauri/src/lib.rs` (spawn poller in `.setup()`, register usage-updated event)
- Reference (do not import): `.claude/hooks/lib/usage-limits-cache.cjs`

## Implementation Steps

1. `usage/client.rs`: single-profile fetch with the exact headers above; timeouts; map 401/403/429/5xx to typed errors (401 → `NeedsReauth`).
2. `usage/model.rs`: schema-tolerant serde structs (`#[serde(default)]`, ignore unknown); `utilization` normalizer; `UsageStatus`.
3. `usage/keepfresh.rs`: spawn the Phase-0-verified `claude` auth/status refresh per profile when a token is near/past expiry; guard concurrency (one per profile).
4. `usage/poller.rs`: tokio interval — active profile eagerly, inactive profiles lazily (on-demand/menu-open/pre-switch); call keep-fresh before an inactive poll; write shared state; emit `usage-updated`.
5. Tray: stash `MenuItem` handles per profile; on `usage-updated`, update tray title + rows; relative reset formatting; render `needs re-login` / `stale` states.
6. Backoff + jitter on repeated failures; surface "usage unavailable" without spamming the endpoint; respect settings poll interval (Phase 6 wires the setting).
7. **Tests:** parser fixtures for real + fabricated-near-100% `/api/oauth/usage` payloads (schema drift is the most likely breakage); `UsageStatus` transitions (Ok→Stale→NeedsReauth).

## Success Criteria

- [ ] Menubar shows each profile's 5h + weekly % matching `claude`'s own `/usage` for that account (±rounding)
- [ ] **Inactive profiles keep showing live usage over a multi-day run** (keep-fresh works) — not perpetual "needs re-login"
- [ ] Reset times display and count down correctly across the 5h boundary
- [ ] Endpoint/network errors degrade to "unavailable"; expired-token profiles show a distinct "needs re-login" state
- [ ] Default active-poll interval ≥120s; inactive profiles polled lazily; no more than one in-flight request per profile
- [ ] Parser handles a fabricated near-100%/exhaustion payload without panicking

## Risk Assessment

- **Undocumented endpoint may change/rate-limit** — schema-tolerant parsing + conservative interval + graceful degradation; isolate all endpoint knowledge in `usage/client.rs`.
- **`severity`/exhaustion shape near 100% unobserved** — treat `>= threshold%` as the reliable signal; don't hard-depend on `severity` string values (open question for Phase 5).
- Polling multiple profiles' tokens = multiple usage reads; keep interval conservative to stay well within any unpublished limits (research §A.2 unresolved Q3).
