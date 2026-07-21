---
phase: 1
title: "Swap journal + timeline attribution"
status: pending
priority: P1
effort: "2d"
dependencies: []
---

# Phase 1: Swap journal + timeline attribution

## Overview

Make analytics attribute usage by **when** a message was written, not by which directory it landed
in. Ships before any credential swapping exists, so it is a provable no-op on current data.

## Requirements

- Append-only journal of account-occupancy boundaries per config dir
- Scanner resolves each message's account by timestamp lookup against that dir's timeline
- With an empty journal, output is byte-identical to today's

## Data model

`~/.vibeproxy/swaps.jsonl`, one JSON object per line, append-only:

```json
{"at":"2026-07-21T09:14:02Z","configDir":"/Users/x/.vibeproxy/profiles/work","accountId":"p_8f3a","accountLabel":"Personal"}
```

Semantics: from `at` onward, `configDir` belongs to `accountId`. The interval ends at the next
record for the same dir, or at +infinity.

A dir with no records resolves to the profile that owns it — today's behaviour.

## Related code files

- Create: `src-tauri/src/switch/journal.rs` — append, load, and build per-dir timelines
- Modify: `src-tauri/src/usage_analytics/scan.rs` — `ingest_line` takes a resolved account rather
  than a fixed label; `resolve_accounts` supplies the timeline
- Modify: `src-tauri/src/profile/paths.rs` — path to the journal file

## Implementation steps

1. `journal.rs`: `append(entry)` (create dirs, open append, write line, `sync_all`), `load()`
   tolerant of corrupt lines (skip, count them), `timeline_for(dir) -> Vec<(start, account)>`
   sorted by time.
2. Timeline lookup: binary search for the last boundary at or before a message timestamp. Timestamps
   are already parsed in `local_date`; reuse that parse rather than re-parsing.
3. Thread the resolved account through `ingest_line`. Today it receives `&str` label from
   `resolve_accounts`; it should receive the dir's timeline and resolve per line.
4. Fast path: when a dir's timeline has one entry, skip the lookup entirely — that is every dir
   until Phase 2 ships, so there must be no measurable regression.
5. Surface `journalGapWarning` on `Analytics` when the scanned range starts before the earliest
   journal entry **and** any swap exists for that dir — the one case where attribution is a guess.

## Tests

- Fixture with two boundaries in one dir: messages before, between, and after land on the right
  accounts
- Empty journal: aggregates identical to the pre-change implementation (assert against the existing
  `real_logs_aggregate_consistently` invariants)
- Corrupt/partial trailing line is skipped without failing the scan
- Timestamps exactly equal to a boundary attribute to the *new* account (boundary is inclusive)
- Out-of-order journal lines still produce a correctly sorted timeline

## Success criteria

- [ ] Per-account totals correct across a fixture containing swaps
- [ ] Empty-journal output unchanged from today
- [ ] Scan time within noise of the current 294ms release baseline
- [ ] Corrupt journal degrades gracefully and visibly, never silently

## Risks

- **Timezone drift**: the journal stores UTC, transcripts carry RFC3339 with offsets. Compare
  instants, never local date strings.
- **Clock skew** could order a boundary after a message it should precede. Accept; note in the
  warning surface.
