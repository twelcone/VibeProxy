---
phase: 4
title: "Time-Series Charts and Filters"
status: pending
priority: P1
effort: "3-4d"
dependencies: [3]
---

# Phase 4: Time-Series Charts and Filters

## Overview

Add the trend and cache charts plus the filters that make the dashboard explorable: tokens/cost over
time (per-model series), a cache-efficiency chart, and controls for date range, granularity
(day/week), and model.

## Requirements

- Functional: a **trend chart** (tokens or cost) over time with per-model series and day/week
  granularity; a **cache-efficiency chart** (read vs write vs fresh input, and hit % over time);
  filters for date range + model that re-query/re-slice and update every panel consistently.
- Non-functional: charts respect `prefers-reduced-motion` (data readable immediately, no essential
  motion); hover tooltips with exact values; a **data-table fallback** for each chart; light + dark;
  responsive to the window width.

## Architecture

**Charting approach (decide here):** either (a) hand-rolled **SVG** line/area/stacked-bar (no dep,
full control, fine for ≤6 series and daily buckets), or (b) a lightweight Svelte-friendly lib
(`layerchart`/`d3-shape`). Recommend **(a) SVG** for KISS and to match the app's hand-drawn tray
meter; escalate to a lib only if interactions get heavy. Whichever: draw an area fill at ~20% opacity,
a faint grid, emphasized endpoints, and distinguish series by **line style + direct end labels**, not
color alone (chart-domain a11y guidance).

**Charts:**
- **Tokens/cost over time** — line or smooth area, x = day (local), one series per model (cap ~6, group
  the rest as "Other"); toggle metric (tokens ↔ cost) and granularity (day ↔ week). Uses
  `per_model_per_day` from Phase 1.
- **Cache efficiency** — stacked area/bar of `cache_read / cache_write / fresh_input` per day, plus a
  "cache hit %" line; a KPI already shows the all-time hit %.

**Filters (`src/lib/usage/Filters.svelte`):** date-range (presets: 7d / 30d / all + custom),
**account** multi-select, model multi-select, granularity. Filters drive a re-call of
`get_usage_analytics(range)` (Rust re-aggregates for the range) or client-side slicing of an
already-loaded full set — prefer **Rust re-query by range** so large sets stay fast and memory-light.
The trend chart can also be grouped **by account** (series = accounts) as an alternative to by-model.

## Related Code Files

- Create: `src/lib/usage/{TrendChart,CacheChart,Filters,ChartTable}.svelte`, `src/lib/chart/svg.ts` (scales, path builders, ticks)
- Modify: `src/routes/usage/+page.svelte` (wire filters → query → charts); Phase 1 `Range` param used by the command
- Modify: `docs/design-system.md` — chart tokens (grid, series, fills), motion rules for charts

## Implementation Steps

1. `chart/svg.ts`: linear/time scales, path/area builders, nice ticks, responsive viewBox.
2. `TrendChart`: multi-series line/area from `per_model_per_day`; metric + granularity toggles; hover tooltip; end labels; `ChartTable` fallback (toggle "View as table").
3. `CacheChart`: stacked read/write/fresh per day + hit-% line.
4. `Filters`: range presets + custom, model select, granularity; on change re-query Rust and update all panels (KPIs, bars, charts) together.
5. Empty/loading/error per chart (skeleton, not an empty axis frame); `prefers-reduced-motion` guard.
6. Verify light/dark contrast for series/grid; keyboard-reachable tooltips + table fallback.

## Success Criteria

- [ ] Trend chart shows per-model tokens/cost over time with day/week toggle and matches the table totals
- [ ] Cache chart shows read/write/fresh split and hit % over time
- [ ] Date-range + model filters update KPIs, bars, and charts consistently, staying fast on large sets
- [ ] Every chart has a data-table fallback and hover tooltips with exact values
- [ ] Charts are readable with reduced-motion and in both themes; series distinguishable without color

## Risk Assessment

- **Too many model series** — cap at ~6 + "Other"; document the cap in the UI (no silent truncation).
- **Filter/state drift across panels** — a single filter state re-queries once and all panels read the same result; avoid per-panel independent fetches.
- **SVG vs lib scope creep** — start SVG; only adopt a lib if zoom/brush/large-data interactions demand it (note the decision).
