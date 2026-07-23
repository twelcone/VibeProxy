---
phase: 3
title: "Analytics UI (KPIs, per-model, per-project)"
status: done
priority: P1
effort: "3-4d"
dependencies: [1, 2]
---

# Phase 3: Analytics UI (KPIs, per-model, per-project)

## Overview

The first render of the Usage view: KPI cards (total tokens, total cost, cache-hit %, tokens today) +
a per-model breakdown (bar) + a per-project breakdown (top-N bar) + a detail table. Data-dense
dashboard styling, reusing the VibeProxy design system. Charts arrive in Phase 4; this phase nails the
layout, KPIs, bars, table, and states.

## Requirements

- Functional: a **Usage** view reachable from the app; loads `get_usage_analytics`; renders KPI cards,
  a **per-account** breakdown, per-model bars (tokens + API-value), per-project top-N bars, and a
  sortable table with an "unpriced models" disclaimer + "priced as of <date>". Per account, an inline
  field to enter **monthly subscription cost** → shows **effective $/Mtok** and savings-vs-list. All
  $ figures labeled "equivalent API value / estimate", never "spent".
- Non-functional: reuses `docs/design-system.md` (system SF, SF Mono + `tabular-nums`, coral accent);
  loading skeleton, empty state ("No usage yet"), and error state; light + dark; keyboard + focus.

## Architecture

**View placement (decide here):** the main window is ~420px — charts want width. Options: (a) a
**tabbed** main window (Accounts | Usage) that widens on the Usage tab, or (b) a **separate, larger
window** opened from the tray/menu ("Usage Analytics…"). Recommend (b) — a dedicated resizable window —
so the accounts popover stays compact. Add a tray/menu item + a Tauri command to open it.

**Series palette (important):** the quota green/amber/red scale is reserved elsewhere, so per-model
series use a **separate categorical palette** — 6 hues derived to sit beside the coral accent, each
distinguishable in light + dark, and paired with **direct labels** so meaning never rests on color
alone. Define as tokens (`--series-1..6`).

**Components (Svelte):**
- `KpiCard` — label, big `tabular-nums` value, sublabel/delta.
- `BarRow` — a labeled horizontal bar (value + optional cost) with hover highlight; used for per-model
  and per-project. Hand-rolled (div/SVG) — no chart lib needed for bars.
- `UsageTable` — sortable (aria-sort) rows: model/project, input/output/cache, total, cost.
- `Disclaimer` — "Estimates. Priced as of <date>. N models unpriced."

Number formatting: locale-aware, compact for big counts (e.g. `1.2M`), full value in tooltip/table.

## Related Code Files

- Create: `src/routes/usage/+page.svelte` (or a component in a new window entry) + `src/lib/usage/{KpiCard,BarRow,UsageTable}.svelte`
- Create: `src/lib/format.ts` (compact numbers, currency), `src/lib/series-palette.ts`
- Modify: Rust — a command/tray item to open the Usage window; register the window in `tauri.conf.json`
- Modify: `docs/design-system.md` — add the categorical series palette + KPI/bar/table patterns

## Implementation Steps

1. Add the Usage window (config + open command + tray/menu entry) and route.
2. `format.ts` + `series-palette.ts`; extend the design system doc with the series palette + dashboard components.
3. KPI row: total tokens, total API-equivalent value, cache-hit % (`cache_read / (input + cache_read)`), tokens today.
4. **Per-account** section: a `BarRow` per account (tokens + value), each with an inline monthly-cost field → **effective $/Mtok** + savings-vs-list; persist the monthly cost in settings.
5. Per-model `BarRow` list (sorted desc), showing tokens + value, with the series color + direct label.
6. Per-project top-N `BarRow` list (+ "show all" affordance).
7. `UsageTable` with sortable columns and the disclaimer.
8. Loading skeleton, empty state, error state; verify light/dark, keyboard nav, `prefers-reduced-motion`.

## Success Criteria

- [ ] Usage view opens (resizable) and shows KPIs, **per-account**, per-model bars, per-project bars, and a sortable table from real data (across all accounts)
- [ ] Entering an account's monthly cost shows a plausible **effective $/Mtok** + savings-vs-list; persists across restarts
- [ ] Numbers use `tabular-nums`; big counts compact with full value available; $ shows the "equivalent API value / as-of" disclaimer (never "spent")
- [ ] Per-model/per-account series are distinguishable without color (direct labels); contrast ≥4.5:1 in both themes
- [ ] Loading / empty / error states all render; view is keyboard-navigable
- [ ] Styling matches `docs/design-system.md` (no generic-blue dashboard, no Fira)

## Risk Assessment

- **Small window vs wide charts** — the dedicated Usage window resolves this; keep the accounts popover unchanged.
- **Palette collision with the quota scale** — the separate categorical `--series-*` tokens keep quota semantics intact; document both.
- **Big-number readability** — compact format + tabular figures + table for exact values.
