---
phase: 2
title: "Cost Estimation"
status: pending
priority: P1
effort: "1-2d"
dependencies: [1]
---

# Phase 2: Cost Estimation

## Overview

Turn token counts into **equivalent API value** — what these tokens *would* cost on pay-per-token —
using a bundled, versioned pricing table. Since the accounts are flat-fee **Max subscriptions** (not
per-token), this is framed as value/leverage, **not** actual spend. Cache tokens bill at different
rates (cache-write > input > cache-read), so value is computed per token class. Then, given each
account's (optional) **monthly subscription cost**, compute **effective $/Mtok** vs list price so the
user sees how much leverage they're getting.

## Requirements

- Functional: per-model / per-account / total **API-equivalent value** from `input / output /
  cache_write / cache_read` × per-model rates; graceful unknown-model handling (tokens shown, value
  "—"); expose a "priced as of" date. **Per-account:** accept an optional monthly subscription cost
  and compute effective $/Mtok = monthly-cost ÷ (Mtok in the period) and vs-list savings %. All added
  onto the Phase 1 aggregates.
- Non-functional: pricing lives in one data file, easy to update; no network calls; a wrong/missing
  rate never crashes or reports a fake $0 as if real; nothing is labeled "spent" — it's "value".

## Architecture

**Pricing table (`src-tauri/src/usage_analytics/pricing.json`, embedded via `include_str!`):**
```jsonc
{
  "updatedAt": "2026-07-19",
  "currency": "USD",
  "perMillionTokens": {
    "claude-opus-4-8":  { "input": 15, "output": 75, "cacheWrite": 18.75, "cacheRead": 1.5 },
    "claude-sonnet-5":  { "input": 3,  "output": 15, "cacheWrite": 3.75,  "cacheRead": 0.3 },
    "claude-haiku-4-5": { "input": 1,  "output": 5,  "cacheWrite": 1.25,  "cacheRead": 0.1 }
    // + Fable and 1M-context variants; fill with the current published rates at build time
  }
}
```
Rates above are **placeholders** — populate from the current Anthropic pricing page during
implementation. Match model ids to what actually appears in the logs (Phase 1 surfaces the real ids);
support a prefix/normalized match (e.g. `claude-opus-4-8[1m]` → the 1M-context rate, else base).

**Cost calc (`usage_analytics/cost.rs`):** pure function
`fn cost(model: &str, t: &Tokens, table: &Pricing) -> Option<f64>` = Σ(tokens_class × rate_class / 1e6).
Attach `cost: Option<f64>` to each `ModelRow`/`DayRow`/`ProjRow` and a `totalCost`. Unknown model →
`None` (UI renders "—" + a "some models unpriced" note).

## Related Code Files

- Create: `src-tauri/src/usage_analytics/cost.rs`, `src-tauri/src/usage_analytics/pricing.json`
- Modify: `usage_analytics/model.rs` (add `cost` fields), `mod.rs` (apply cost after aggregation)

## Implementation Steps

1. Author `pricing.json` with the current Claude family rates (input/output/cacheWrite/cacheRead per 1M) + `updatedAt`.
2. `cost.rs`: load embedded pricing (parse once), `cost(model, tokens)` with model-id normalization + unknown → None.
3. In `mod.rs`, after aggregation, compute cost for each per-model / per-day / per-project row and the grand total.
4. Per-account effective $/Mtok: given an optional `monthlyCostUsd` per account (stored in settings,
   entered in Phase 3), compute `effective_per_mtok = monthlyCostUsd / (account_tokens_in_period / 1e6)`
   and `savings_vs_list = 1 - effective/list_blended`; expose alongside `per_account`.
5. Surface `pricing.updatedAt` + a list of any unpriced model ids in the `Analytics` payload for the UI disclaimer.
6. **Tests:** known model → exact expected API-value from fixture tokens; unknown model → value None but tokens intact; cache-read valued lower than input; effective $/Mtok math for a sample monthly cost.

## Success Criteria

- [ ] Per-model and total cost computed with correct per-class rates (verified against a hand-calc fixture)
- [ ] Unknown model ids show tokens with cost `None` and are listed for the UI disclaimer
- [ ] `pricing.updatedAt` is exposed so the UI can show "estimate, priced as of <date>"
- [ ] Changing a rate in `pricing.json` changes the computed cost (no hard-coded numbers elsewhere)

## Risk Assessment

- **Rate accuracy/staleness** (user-accepted) — single source of truth in `pricing.json` with a visible date; label everything "estimate". Consider a follow-up that fetches rates, out of scope here.
- **Model-id mismatch** — the id in the logs may include suffixes (`[1m]`, dated variants). Normalize + prefix-match; unknown → None rather than a wrong price.
