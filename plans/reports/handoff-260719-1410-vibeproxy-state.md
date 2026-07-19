# VibeProxy — Session Handoff / Resume Checkpoint

Written 2026-07-19 before a fresh-context session. Read this first to resume.

## What VibeProxy is

Open-source **macOS menubar app (Tauri v2 · Rust + Svelte/SvelteKit)** to switch between multiple
**Claude Code Pro/Max accounts**, show live usage, and auto-switch on quota. Repo:
`github.com/twelcone/VibeProxy` (PRIVATE; user plans to flip public at release). Git identity =
`twelcone <vuquocminhdang@gmail.com>` (set per-repo; see memory).

## Status — what's DONE (shipped, on `main`, pushed)

**Core app — all 8 phases complete**, tag `v0.1.0` cut (draft GitHub release built by CI with an
unsigned `.dmg`). Plan: `plans/260719-0001-vibeproxy-claude-code-profile-switcher/` (all phases ✅).
- Menubar-only app (no dock icon), `~/.vibeproxy/` store, first-run adopts the default `~/.claude` as "Main".
- Switch active account via a **real-path indirection file** `~/.vibeproxy/active-path` (shell reads it
  into `CLAUDE_CONFIG_DIR`). NOT a symlink. Default account = **clear** active-path (unset the var).
- Per-account token read from Keychain service `Claude Code-credentials-<sha256(config_dir)[:8]>`
  (bare `Claude Code-credentials` for the default).
- Live usage via `GET /api/oauth/usage` per account; **fill-meter** tray indicator.
- Auto-switch engine (threshold + hysteresis + cooldown + notification + relaunch) — demoed live.
- Settings UI, shell-integration snippet, activity log, launch-at-login.
- Independent code review done; 2 original + 7 review findings fixed.

**Usage Analytics feature — backend done (Phases 1–2 of 5).** Plan:
`plans/260719-1323-vibeproxy-usage-analytics/`.
- `src-tauri/src/usage_analytics/{mod,model,scan,cost}.rs` + `pricing.json`.
- Command `get_usage_analytics(range) -> Analytics`: scans **every account's**
  `<config_dir>/projects/**/*.jsonl` (NOT just `~/.claude` — logs are per-config-dir), dedupes by
  `requestId`, aggregates tokens by account/model/day(local)/project + cache; adds **API-equivalent
  value** (bundled `pricing.json`, estimate — not real spend, accounts are flat-fee Max).
- Verified vs real logs: ~2.6B tokens / 8,638 msgs (Opus 4.8 heaviest; cache-read dominates).
- **19 Rust tests pass.**

## What's NEXT (resume here)

Usage Analytics **Phases 3–5 (all frontend/Svelte)**:
- **Phase 3 — Analytics UI** (`plans/.../phase-03-...md`): a dedicated resizable **Usage window**;
  KPI cards (tokens, API-value, cache-hit %, today); **per-account** + per-model + per-project bars;
  sortable table; inline **monthly-cost input per account → effective $/Mtok** (persist in settings).
- **Phase 4 — Charts + filters**: trend (per-model/account series, day/week), cache chart; date-range +
  account + model filters (Rust re-queries by range). Prefer hand-rolled SVG (KISS) over a chart lib.
- **Phase 5 — Polish**: CSV export, mtime-keyed incremental cache, table virtualization, WCAG-AA.

## Key facts / gotchas (don't relearn the hard way)

- **`CLAUDE_CONFIG_DIR=~/.claude` set explicitly ≠ unset** — Claude hashes the path and misses the bare
  Keychain item → "not logged in". Default account works ONLY with the var unset. Centralized in
  `profile::paths::is_default()`. This governs switching, keychain, account_meta, AND log-root scanning.
- **Logs are per-account** (config-dir isolates `projects/`). Analytics scans all profiles' dirs.
- **JSONL schema** (`type=="assistant"` lines): `message.usage.{input_tokens, output_tokens,
  cache_creation_input_tokens, cache_read_input_tokens}`, `message.model`, `requestId`, ISO-Z
  `timestamp`, `cwd`. Real model ids incl. `claude-opus-4-8`, `claude-fable-5`, `claude-sonnet-5`,
  `claude-haiku-4-5-20251001`. Never print log CONTENT (real conversations) — numbers/keys only.
- **Design system**: `docs/design-system.md` — system SF + SF Mono (`tabular-nums`), warm neutrals,
  **coral accent**; quota green/amber/red is reserved → analytics model-series need a SEPARATE
  categorical palette. Tray fill-meter is the chosen menubar indicator (dot/ring rejected).
- **pricing.json rates are ESTIMATES** — verify against Anthropic's current pricing page.
- **Bash hook** blocks commands containing `build`/`target`/`node_modules`/`.svelte-kit`; `.claude/.ckignore`
  has `!build`\n`!target` to allow packaging builds. Use `pnpm check` (not `pnpm build`) for FE typecheck when needed.
- Run the app: `pnpm tauri dev` (relaunch after Rust changes). Tests: `cargo test` in `src-tauri`.
  App state lives in `~/.vibeproxy/config.json` (+ `active-path`).

## Open questions (deferred, low-risk)

- Analytics: local-time day buckets (done), unknown-model handling (value "—"), where the monthly-cost
  input lives (settings vs per-account card), HTML-report export (user said leave out for now).
- App: flip repo public + publish the `v0.1.0` draft release (user decision, not yet done).
- Spike soak: inactive-token refresh confirmed ~11h; a full >24h re-check is still nice-to-have.

## To resume

"Continue building the Usage Analytics UI (Phase 3)" — start with the Usage window + KPI cards +
per-account/model/project bars wired to `get_usage_analytics`, following `docs/design-system.md`.
