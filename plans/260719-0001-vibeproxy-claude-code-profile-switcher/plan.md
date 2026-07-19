---
title: "VibeProxy - Claude Code profile switcher"
description: "Native macOS (Tauri) menubar app to switch between multiple Claude Code Pro/Max accounts, show live usage, and auto-switch on quota exhaustion — via config-dir isolation + usage polling, with no inference-token relay."
status: pending
priority: P1
effort: "~3-5 weeks solo"
tags: [macos, tauri, rust, claude-code, menubar, oss]
created: 2026-07-19
blockedBy: []
blocks: []
---

# VibeProxy — Claude Code profile switcher

## Overview

VibeProxy is an open-source **Tauri v2 (Rust + web UI)** macOS menubar app that lets a user
switch between multiple **Claude Code Pro/Max accounts** in one click, see each account's live
usage in the menubar, and **auto-switch** to a fresh account when the active one runs out of quota.

**Architecture (locked after research + user decision):** Switching uses the ToS-accepted pattern —
each profile is an isolated `CLAUDE_CONFIG_DIR` with its own real `/login`, and VibeProxy atomically
repoints which profile is "active" for the next `claude` launch. Usage/quota comes from polling
Anthropic's `GET /api/oauth/usage` endpoint per profile (the same call this repo's existing
`.claude/hooks/lib/usage-limits-cache.cjs` already makes). **No OAuth token is ever relayed to the
inference API** — that relay/pooling pattern is banned and server-side-enforced by Anthropic (Jan 2026)
and would risk flagging the user's real paid accounts.

> **Honest risk framing (not "100% ToS-safe").** The research is split: the primary constraint Anthropic
> enforces is OAuth-token use for *inference* outside the official client — which VibeProxy never does.
> But polling `/api/oauth/usage` with an extracted token is still "using the token in a non-official
> tool" and one research doc flags it as residual risk. It's read-only, low-signal, and mirrors this
> repo's own hook — but a *public* app with many users polling an undocumented endpoint is a different
> enforcement profile than one developer's script. Mitigations: poll conservatively, poll inactive
> profiles lazily, keep a JSONL/statusline degrade path, and state this plainly in the README rather
> than claiming "ToS-safe." **This residual risk is a user-accepted trade-off (see Open questions).**

> **Naming note:** despite the name, VibeProxy does **not** run an inference proxy. The "proxy-like"
> role is the local usage poller / active-profile broker. This was a deliberate, evidence-driven
> pivot away from a token-rewriting reverse proxy — see `reports/` research.

## Goals

| # | Goal | Priority |
|---|------|----------|
| 1 | One-click switch between multiple Claude Code Pro/Max accounts (minimal steps) | P1 |
| 2 | Native-feeling macOS menubar app (Tauri v2, no dock icon) | P1 |
| 3 | Live per-account usage (5h + weekly %) in the menubar | P1 |
| 4 | Auto-switch active profile when it crosses a quota threshold | P1 |
| 5 | Never risk the user's real accounts (no OAuth-token relay) | P1 |
| 6 | Ship as unsigned open-source build (no Apple Developer Program) | P2 |
| 7 | Keep the Rust core Windows-portable for later | P3 |

## Architecture at a glance

```
┌─────────────────────────── VibeProxy (Tauri v2, one Rust process) ───────────────────────────┐
│                                                                                               │
│  Tray (TrayIconBuilder, ActivationPolicy::Accessory)   Web UI window (profile mgmt, settings) │
│        │ live title "P1 · 37%"   ▲ click-to-switch            ▲ invoke() commands             │
│        ▼                         │                            │                               │
│  ┌───────────────┐   ┌───────────────────┐   ┌────────────────────────┐   ┌────────────────┐  │
│  │ Poller (tokio)│   │ Profile store     │   │ Switch broker          │   │ Auto-switch    │  │
│  │ GET /api/     │──▶│ ~/.vibeproxy/     │◀──│ write active-path file │◀──│ engine (thresh │  │
│  │ oauth/usage   │   │  profiles/<id>/   │   │ (real path, NO symlink)│   │ + cooldown)    │  │
│  │ per profile   │   │  config.json      │   └────────────────────────┘   └────────────────┘  │
│  └───────────────┘   └───────────────────┘                                                    │
└───────────────────────────────────────────────────────────────────────────────────────────────┘
        │ Bearer token from Keychain svc                      │ shell reads active-path →
        ▼ "Claude Code-credentials-<sha256(path)[:8]>"        ▼ CLAUDE_CONFIG_DIR=<real path>
   api.anthropic.com/api/oauth/usage  (read-only)        `claude` (official client, real login)
```

## Phases

| # | Phase | Status | Priority | Depends on |
|---|-------|--------|----------|-----------|
| 0 | [Mechanism Spike (go/no-go)](./phase-00-mechanism-spike.md) | Pending | P1 | — |
| 1 | [Foundation & Profile Model](./phase-01-start.md) | ✅ Done | P1 | 0 |
| 2 | [Profile Management & Config-Swap Switching](./phase-02-profile-management-config-swap-switching.md) | ✅ Done | P1 | 0, 1 |
| 3 | [Account Login & Onboarding Flow](./phase-03-account-login-onboarding-flow.md) | ✅ Done | P1 | 0, 2 |
| 4 | [Usage Polling & Menubar Display](./phase-04-usage-polling-menubar-display.md) | ✅ Done | P1 | 2 |
| 5 | [Auto-Switch Engine](./phase-05-auto-switch-engine.md) | ✅ Done | P1 | 3, 4 |
| 6 | [UI/UX & Settings](./phase-06-uiux-settings.md) | ✅ Done | P2 | 3, 4, 5 |
| 7 | [Packaging & Open-Source Release](./phase-07-packaging-open-source-release.md) | Pending | P2 | 1–6 |

> **Phase 0 gates everything — now essentially GO** (see `reports/spike-260719-mechanism-findings.md`).
> Shell-only spike confirmed: config-dir isolates accounts, `claude auth status --json` gives
> identity+keep-fresh, creds live in Keychain under a deterministic per-path service name. **One design
> correction: the switch is a real-path indirection file, not a symlink.** Only a 24h refresh soak +
> the GUI-launch inheritance check remain open (low risk).

## Key decisions (evidence-backed)

| Decision | Choice | Why |
|----------|--------|-----|
| Profile type | Claude Pro/Max OAuth logins | User requirement |
| Switch mechanism | `CLAUDE_CONFIG_DIR` per profile → **real-path indirection file** (`~/.vibeproxy/active-path`) + launch broker | Only ToS-accepted pattern; **Phase 0 proved a symlink collides all profiles' Keychain items** — must target the real path; no hot-swap of a running session exists |
| Credential store | **Keychain-per-path** — read svc `Claude Code-credentials-<sha256(dir)[:8]>` via `/usr/bin/security` | Phase 0: logins write tokens to Keychain (not a file); deterministic per-path name gives clean multi-profile reads, tokens stay encrypted |
| Quota signal | Poll `GET /api/oauth/usage` per profile | Live 5h + weekly % + `resets_at`; mirrors this repo's own hook. **Not** used for inference (residual ToS risk noted above) |
| Token freshness | Keep-fresh via spawning official `claude` per profile | Inactive profiles' tokens expire in ~24h; only the official client may refresh them ToS-safely |
| Auto-switch | Pre-emptive on numeric threshold; repoints next launch + "relaunch" action | No mechanism switches a *live* session; repoint + optional relaunch is the honest UX |
| Stack | Tauri v2 (Rust + web), unsandboxed | Cross-platform-portable, native tray, no App Sandbox (needs `~/.claude` + creds access) |
| Distribution | Unsigned/ad-hoc + GitHub Actions build | User declined Apple Developer Program ($99/yr) |

## Success Criteria

- [ ] User can register ≥2 Pro/Max accounts and switch active in ≤2 clicks from the menubar
- [ ] Menubar shows correct live 5h + weekly % for each profile (matches `claude`'s own `/usage`), and **keeps showing it for inactive profiles across multi-day runs** (keep-fresh works)
- [ ] When active profile crosses the numeric threshold, VibeProxy repoints to a fresh *eligible* (non-stale) profile, notifies, and offers relaunch
- [ ] No VibeProxy code ever sends an OAuth token to any inference endpoint or third party
- [ ] App runs with no dock icon, launches at login (opt-in), and installs from an unsigned GitHub release with documented first-run steps
- [ ] Rust core compiles for Windows (tray/keychain branches may be stubbed) — portability not regressed

## Risks (top)

1. **Inactive-profile token expiry (highest)** — access tokens die in ~24h and only the *active*
   profile ever gets refreshed by Claude Code, so per-profile usage would rot to "needs re-login".
   Mitigate: keep-fresh by spawning the official `claude` per profile (Phase 0 verifies the command);
   poll inactive profiles lazily; treat stale/`needs_reauth` profiles as ineligible for auto-switch.
2. **Keychain ACL prompts** (resolved design) — reading `claude`'s Keychain items from VibeProxy
   prompts until "Always Allow". Mitigate: `/usr/bin/security` subprocess (repo-hook precedent), one
   grant per profile service, first-run explainer. (The old file-vs-Keychain unknown is resolved: Keychain.)
3. **`CLAUDE_CONFIG_DIR` + `/api/oauth/usage` are undocumented** — Anthropic could change either.
   Mitigate: isolate each behind one module; conservative poll interval; schema-tolerant parsing;
   graceful degradation; smoke-test on each Claude Code update.
4. **Residual ToS risk of usage polling** (see framing above) — public app amplifies it. Mitigate:
   honest README, lazy polling, degrade path; **user-accepted trade-off**.
5. **Running-session switch caveat** — switching only affects the *next* `claude` launch, and a shell
   `export` won't reach GUI/VS Code/Cursor-launched Claude Code (silent wrong-account). Mitigate: UI
   expectation-setting, "relaunch" helper, `launchctl setenv` best-effort, an integration self-check.
6. **Fresh profile = factory-reset Claude Code** — new config dirs lose settings/MCP/plugins/skills.
   Mitigate: seed shared non-secret config into new profile dirs (Phase 3).

## Non-goals (explicit)

- **Seamless mid-session switching** — no mechanism hot-swaps a running `claude`; switches affect the next launch (+ optional relaunch). Not promising "instant."
- **Per-terminal / per-session different accounts** — VibeProxy manages one global active profile (unlike `claude-swap`'s env-scoped mode). Stated so users aren't surprised.
- **An inference proxy** — despite the name, VibeProxy never proxies model requests.
- **Usage-history charts / cost analytics** — deferred (YAGNI); the `/usage` % is the scope.

## Open questions (need answers before/within the noted phase)

- **[Decision] Is the residual usage-polling ToS risk acceptable for a *public* release?** User accepted it for the architecture; confirm the README framing is acceptable before Phase 7 goes public.
- ~~[Phase 0] keep-fresh command?~~ **Resolved:** `claude auth status --json` per config dir (also gives identity/tier). Refresh-on-status still needs a 24h soak confirm.
- ~~[Phase 0] where do creds land?~~ **Resolved:** macOS Keychain, svc `Claude Code-credentials-<sha256(realpath)[:8]>` (not a file). Symlink switch invalid → use real-path indirection.
- **[Phase 0 — soak]** Does token refresh write back to the same Keychain item (no re-login) over 24h+? (`~/vp-spike` soak in progress.)
- **[Phase 0/3]** Exact list of shared non-secret config safe to copy into new profile dirs without breaking per-account identity.
- **[Phase 2/7]** Does a VS Code/Cursor-launched `claude` inherit `launchctl setenv CLAUDE_CONFIG_DIR`? (GUI-launch mitigation.)
- **[Phase 5]** Exact `error.type`/body + `limits[].severity` of a real quota-exhaustion response (only low-utilization `normal` observed live).
- **[Phase 7]** License choice (MIT suggested) — confirm before first public release.

## Reference reports

- `reports/research-260718-2353-claude-code-auth-storage.md` — credential storage, switching mechanisms, ToS, prior art
- `reports/research-260718-2353-quota-menubar.md` — `/api/oauth/usage`, detection approaches, Tauri v2 tray/proxy/packaging

<!-- slug: vibeproxy-claude-code-profile-switcher -->
