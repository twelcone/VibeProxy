---
title: "VibeProxy Usage Analytics"
description: "A Usage Analytics view inside VibeProxy that parses Claude Code's local JSONL logs to show total + per-model tokens, cost estimates, over-time trends, per-project breakdown, and cache efficiency."
status: pending
priority: P2
effort: "~1.5-2 weeks"
tags: [vibeproxy, analytics, tauri, svelte, charts]
created: 2026-07-19
---

# VibeProxy Usage Analytics

## Overview

Add a **Usage Analytics** view to VibeProxy that answers "how many tokens have I used, and on which
models?" — plus cost, trends, per-project breakdown, and cache efficiency. It reads Claude Code's own
local usage logs (no API, no token relay), aggregates them in Rust, and renders a data-dense dashboard
in the existing Svelte UI.

**Data source (verified in the VibeProxy research):** Claude Code writes one JSONL file per session
under `<CLAUDE_CONFIG_DIR>/projects/<slug>/<session-uuid>.jsonl`. `assistant`-type lines carry
`message.usage`: `input_tokens`, `output_tokens`, `cache_creation_input_tokens`,
`cache_read_input_tokens`, plus `message.model`, `timestamp`, `requestId`, and `cwd`/project. Same
source `ccusage` uses. **No `costUSD` is written** — cost is computed client-side.

> **Per-account logs (validation correction).** `CLAUDE_CONFIG_DIR` isolates *everything, including
> `projects/` history* — so each VibeProxy account writes its logs to **its own** dir
> (`~/.vibeproxy/profiles/<id>/projects/…`), and the default account to `~/.claude/projects/`. The
> analytics therefore scans **every configured profile's config dir** (from VibeProxy's `config.json`)
> — not just `~/.claude` — and adds a **per-account** dimension. Scanning only `~/.claude` would show
> just the Main account.

**Delivery:** a new view **inside VibeProxy** (user decision) — reuses the app's file access, Tauri
backend, and design system. Not a separate web app (a browser can't read `~/.claude`).

## Goals

| # | Goal | Priority |
|---|------|----------|
| 1 | Total tokens used + per-model breakdown, **across all accounts** | P1 |
| 2 | **Per-account** breakdown (which account used what) | P1 |
| 3 | Cost as **equivalent API value** + effective $/Mtok vs your flat subscription | P1 |
| 4 | Over-time trends (daily/weekly tokens + value) | P1 |
| 5 | Per-project breakdown | P2 |
| 6 | Cache efficiency (read vs write vs fresh input) | P2 |
| 7 | Fast + correct on large logs (dedup, aggregate in Rust) | P1 |

## Architecture

```
For each configured account (from VibeProxy config.json) + default ~/.claude:
   <account config dir>/projects/**/*.jsonl        (Claude Code usage logs — read-only)
        │  scan + parse assistant lines, dedupe by requestId/message id, tag with account
        ▼
Rust  usage_analytics::scan(accounts)  →  Aggregates { totals, per_account, per_model,
                                            per_day, per_project, cache }
        │  + API-equivalent value = tokens × pricing.json; + effective $/Mtok vs each
        │    account's (optional) monthly subscription cost
        ▼  Tauri command  get_usage_analytics(range, filters) -> Analytics
Svelte  "Usage" view:  KPI cards · per-account + per-model bars · per-project bars ·
                       trend line/area · cache stack · table
```

**Reuse, don't reinvent:** this view follows [`docs/design-system.md`](../../docs/design-system.md) —
system SF + SF Mono (`tabular-nums` for all numbers), warm neutrals, **coral accent**. The usage
green/amber/red scale is reserved for *quota* elsewhere, so per-model chart series get a **separate
categorical palette** (see Phase 3). No generic-blue dashboard, no Fira font (the design database's
defaults are overridden by the project's own system).

## Phases

| # | Phase | Status | Depends on |
|---|-------|--------|-----------|
| 1 | [Usage Log Parser & Aggregator](./phase-01-start.md) | ✅ Done | — |
| 2 | [Cost Estimation](./phase-02-cost-estimation.md) | ✅ Done | 1 |
| 3 | [Analytics UI (KPIs, per-model, per-project)](./phase-03-analytics-ui-kpis-per-model-per-project.md) | Pending | 1, 2 |
| 4 | [Time-Series Charts and Filters](./phase-04-time-series-charts-and-filters.md) | Pending | 3 |
| 5 | [Polish, Export, Accessibility](./phase-05-polish-export-accessibility.md) | Pending | 3, 4 |

## Design direction (from the ak:ui-ux-pro-max pass, adapted)

| DB recommendation | Verdict | Why |
|---|---|---|
| **Data-Dense Dashboard** style (KPI cards + charts + tables, minimal padding, grid) | **Adopt** | Correct pattern for this view |
| Chart types: line/area for trends, bar for per-model/per-project, stacked for cache | **Adopt** | Matches the data (see per-chart notes in phases) |
| "Real-Time Landing" page pattern | **Ignore** | This is an in-app dashboard, not a landing page |
| Blue `#2563EB` + amber palette | **Reject** → reuse VibeProxy coral + a categorical series palette | Consistency with the host app |
| Fira Code / Fira Sans | **Reject** → system SF + SF Mono | Native-mac app, already in the design system |
| No-emoji SVG icons, cursor-pointer, 150–300ms hover, contrast, reduced-motion, filtering | **Adopt** | Baseline dashboard quality |

## Charts (chart-domain guidance)

- **Tokens/cost over time** → line or smooth-area chart; per-model as multiple series (≤6). Distinguish series by more than color (line style / direct labels) for colorblind safety; provide a data-table fallback.
- **Per-model breakdown** → horizontal bar (precise comparison) with tokens + cost. Donut only if ≤5 models and proportion is the point.
- **Per-project** → top-N horizontal bar (JSONL is organized by project).
- **Cache efficiency** → stacked bar / ratio: cache-read vs cache-write vs fresh input; a "cache hit %" KPI.

## Success Criteria

- [ ] Usage view shows correct total tokens + per-model split (cross-check against `ccusage` for the same window)
- [ ] Per-model **cost** estimate from a bundled, versioned pricing table (with an "estimate" disclaimer)
- [ ] Daily/weekly **trend** charts + a **per-project** breakdown + **cache** read/write/fresh split
- [ ] Parses a large `~/.claude` (100s of sessions) in well under a second, deduped, without blocking the UI
- [ ] Reuses the VibeProxy design system; charts are keyboard/screen-reader accessible with a table fallback; light + dark
- [ ] Reads logs only — never sends usage data anywhere

## Risks

1. **JSONL schema drift** — Claude Code could change field names/shape. Mitigate: schema-tolerant parsing (unknown fields ignored, missing → 0), isolate parsing in one module, fixture tests. Confirm current shape in Phase 1.
2. **Pricing table staleness** — model rates change; there's no local price feed. Mitigate: a versioned `pricing.json` with a "last updated" date shown in the UI; label figures as estimates. (User accepted this maintenance burden.)
3. **Large-log performance** — many/large JSONL files. Mitigate: stream line-by-line in Rust, aggregate incrementally, optionally cache results keyed by file mtime.
4. **Dedup correctness** — the same request can appear multiple times; ccusage dedupes by `requestId` (+ message id). Mitigate: replicate that dedup; test with a duplicated fixture.

## Validation outcomes (resolved)

- **Scope = all accounts + per-account breakdown.** Scan every configured profile's config dir, not
  just `~/.claude`. Adds a per-account dimension. (Corrects a data-source bug in the first draft.)
- **Cost = equivalent API value + subscription effectiveness.** Show what the tokens would cost on the
  pay-per-token API, AND let the user enter each account's flat monthly plan cost to compute effective
  $/Mtok vs list price. Labeled clearly (Max is a flat fee, not per-token spend).

## Open questions

- **Time zone / "today"** — compute day buckets in local time (matches user expectation)? Confirm in Phase 4.
- **Unknown model ids** — bundle rates for the current Claude family; unknown id → tokens shown, value "—". Confirm the id list from real logs in Phase 1/2.
- **View placement** — dedicated resizable Usage window (recommended) vs a tab in the main window. Confirm in Phase 3.
- **Subscription cost input** — where the user enters each account's monthly plan cost (Settings, or inline on the per-account card). Decide in Phase 3.
- **Log-root robustness** — a profile's `config_dir` may be `~/.claude` (default) or a custom dir; scan `<dir>/projects/` for each. Confirm no profile shares a dir (dedupe by resolved path).

<!-- slug: vibeproxy-usage-analytics -->
