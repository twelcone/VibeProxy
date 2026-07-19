---
phase: 1
title: "Usage Log Parser & Aggregator"
status: completed
priority: P1
effort: "3-4d"
dependencies: []
---

# Phase 1: Usage Log Parser & Aggregator

## Overview

Scan Claude Code's local JSONL logs, parse token usage per assistant message, dedupe, and aggregate
into the structures the UI needs (totals, per-model, per-day, per-project, cache). Expose it as one
Tauri command. This is the data engine; everything else renders it.

## Requirements

- Functional: for **every configured account** (VibeProxy profiles from `config.json` + the default
  `~/.claude`), walk `<config_dir>/projects/**/*.jsonl`; parse `assistant` lines' `message.usage`;
  tag each with its account; dedupe repeated requests; aggregate totals, **per-account**, per-model,
  per-day (local date), per-project, and cache (read/write/fresh). Return via a Tauri command,
  optionally filtered by date range.
- Non-functional: schema-tolerant (unknown fields ignored, missing → 0); streams line-by-line (no
  loading whole files into memory); runs off the UI thread; sub-second on 100s of sessions.

## Architecture

**Input shape (verified in VibeProxy research — high-confidence but re-confirm current):**
```jsonc
// one JSON object per line; only type == "assistant" carries usage
{ "type": "assistant", "timestamp": "2026-07-19T...Z", "requestId": "req_...",
  "cwd": "/Users/twel/Projects/Foo",
  "message": { "model": "claude-opus-4-8", "usage": {
    "input_tokens": 24078, "output_tokens": 267,
    "cache_creation_input_tokens": 17665, "cache_read_input_tokens": 16427 } } }
```
Project = derived from the file's parent dir slug (or `cwd`). Day = `timestamp` in **local** time.

**Dedup:** the same request can appear more than once across resumed sessions. Dedup by
`requestId` (fall back to `message.id` if present); skip a line whose key was already counted — the
approach `ccusage` uses.

**Account log roots (validation correction):** `CLAUDE_CONFIG_DIR` isolates `projects/` per account,
so there is **no single log root**. Build the list of roots from VibeProxy's own `config.json`:
`<profile.config_dir>/projects/` for each profile, plus `~/.claude/projects/` for the default. Dedupe
roots by resolved absolute path (a profile whose `config_dir` *is* `~/.claude` must not be scanned
twice). Tag every parsed row with the owning account (profile id/label).

**Aggregates (`usage_analytics/model.rs`):**
```rust
struct Tokens { input: u64, output: u64, cache_write: u64, cache_read: u64 }  // + total(), billable_input()
struct Analytics {
    totals: Tokens,
    per_account: Vec<AccountRow>, // { account, tokens: Tokens, messages: u64 }
    per_model: Vec<ModelRow>,     // { model, tokens: Tokens, messages: u64 }
    per_day: Vec<DayRow>,         // { date: "YYYY-MM-DD", tokens: Tokens }  (sorted)
    per_project: Vec<ProjRow>,    // { project, tokens: Tokens }             (sorted desc by total)
    per_model_per_day: Vec<...>,  // for the multi-series trend (Phase 4)
    range: { from, to },
    message_count: u64,
}
```
Cost/value fields are added in Phase 2 (kept separate so parsing stays pure).

**Modules (`src-tauri/src/usage_analytics/`):** `scan.rs` (resolve roots + walk + stream + dedup),
`model.rs` (structs, serde camelCase), `mod.rs` (public `scan(accounts, range) -> Analytics`).

## Related Code Files

- Create: `src-tauri/src/usage_analytics/{mod.rs,scan.rs,model.rs}`
- Modify: `src-tauri/src/lib.rs` (add `mod usage_analytics;` + `get_usage_analytics` command)
- Test fixtures: `src-tauri/src/usage_analytics/fixtures/*.jsonl` (tiny sample logs incl. a duplicate)

## Implementation Steps

1. **Confirm current schema** — inspect a couple of real lines from `~/.claude/projects/**/*.jsonl`
   (field names for usage/model/timestamp/requestId) before coding; adjust structs if drifted.
2. `model.rs`: `Tokens`, `Analytics` and row structs with `#[serde(rename_all = "camelCase")]`, tolerant `#[serde(default)]`.
3. Resolve account log roots: read VibeProxy `config.json`, map each profile → `<config_dir>/projects/`, add `~/.claude/projects/`, dedupe by resolved path (each root carries its account label).
4. `scan.rs`: for each root, enumerate files under it, read line-by-line, parse only `type=="assistant"`, extract usage/model/day/project + tag account, dedup by requestId (global), fold into a `HashMap` accumulator; optional `range` filter on timestamp.
6. Sort per-account/per-model/per-project/per-day; assemble `Analytics`.
7. Tauri command `get_usage_analytics(range: Option<Range>) -> Result<Analytics, String>`, run via `spawn_blocking` (file I/O off the async runtime).
8. **Tests:** fixtures across (a) two accounts, (b) multi-model/multi-project, (c) a duplicated `requestId`, (d) missing/extra fields; assert totals, per-account split, dedup, per-model split, cache sums.

## Success Criteria

- [ ] `get_usage_analytics` returns correct totals + **per-account** + per-model/day/project/cache for the fixtures
- [ ] Usage is aggregated across **all** configured accounts' dirs (not just `~/.claude`); roots deduped by path
- [ ] Duplicate `requestId` is counted once (globally, across accounts)
- [ ] Malformed / non-assistant / unknown-field lines are skipped without error
- [ ] Totals match `ccusage` for the same window/account (±rounding)
- [ ] Sub-second on the real log set; parsing runs off the UI thread

## Risk Assessment

- **Schema drift** — isolate all field knowledge in `model.rs`; tolerant parsing; a fixture pins the shape. Step 1 re-confirms before building.
- **Dedup key differences** — if `requestId` is absent on some lines, fall back to `message.id` then a content hash; test the fallback.
- **`walkdir` dependency** — trivial add, or hand-roll recursion to avoid the dep (KISS).
