---
phase: 5
title: "Polish, Export, Accessibility"
status: done
priority: P2
effort: "2-3d"
dependencies: [3, 4]
---

# Phase 5: Polish, Export, Accessibility

## Overview

Final pass: CSV/PNG export, performance for very large logs, a full accessibility sweep, and
refinement so the Usage view feels finished and shippable alongside the rest of VibeProxy.

## Requirements

- Functional: export the current view's data (CSV) and optionally a chart image (PNG); handle very
  large logs smoothly (caching + virtualized table); auto-refresh (or a Refresh button) so numbers
  stay current as Claude Code writes new logs.
- Non-functional: WCAG-AA pass (contrast, keyboard, screen-reader summaries, table fallbacks, reduced
  motion); no UI jank on large data.

## Architecture

- **Export:** a Rust command `export_usage_csv(range, filters) -> path` (writes to a user-chosen
  location via a save dialog) for the per-model/per-day/per-project rows; front-end offers "Export
  CSV". PNG export of a chart via canvas snapshot is optional/nice-to-have.
- **Performance / caching:** cache the parsed aggregate keyed by the set of files + their mtimes;
  re-scan only changed/new files (incremental). Virtualize `UsageTable` when rows are many (rare for
  per-model, possible for per-project/per-day). Keep the initial scan under the Phase 1 budget.
- **Freshness:** a Refresh button + optional light polling (e.g. re-scan on window focus) so the view
  reflects recent sessions without a manual restart.
- **Accessibility:** each chart gets an `aria-label`/summary describing its key insight; a togglable
  data table; legend items toggle series and are keyboard-reachable; focus-visible everywhere; verify
  with reduced-motion and large text.

## Related Code Files

- Create: `src-tauri/src/usage_analytics/export.rs` (CSV writer + save dialog), `src/lib/usage/ExportMenu.svelte`
- Modify: `usage_analytics/mod.rs` (mtime-keyed cache / incremental rescan), `UsageTable.svelte` (virtualization), chart components (aria summaries, legend toggles)

## Implementation Steps

1. CSV export command + save dialog + front-end trigger; verify output opens cleanly in a spreadsheet.
2. mtime-keyed aggregate cache + incremental rescan of changed files; measure on the real log set.
3. Virtualize the table for long lists; keep sorting working with virtualization.
4. Refresh button + re-scan on window focus.
5. Accessibility sweep: chart `aria` summaries + data-table fallbacks, keyboard legend toggles, focus states, contrast in both themes, reduced-motion, large-text.
6. Optional: PNG chart export via canvas snapshot.

## Success Criteria

- [ ] "Export CSV" produces correct per-model/day/project rows for the current filters
- [ ] Re-opening / refreshing reflects new sessions; repeat scans are fast via the mtime cache
- [ ] Large logs: no visible jank; table virtualized; initial scan within budget
- [ ] WCAG-AA: charts have text summaries + table fallbacks; fully keyboard-navigable; passes contrast in light + dark; reduced-motion respected
- [ ] Feature feels consistent with the rest of VibeProxy (design system, states, polish)

## Risk Assessment

- **Cache invalidation** — key strictly on file set + mtimes; a changed file forces a rescan of just that file; a full "hard refresh" escape hatch.
- **Export path/permissions** — use the OS save dialog (no silent writes); handle cancel/failure with a clear message.
- **A11y for custom SVG charts** — the data-table fallback + aria summary is the reliable path; don't rely on SVG semantics alone.
