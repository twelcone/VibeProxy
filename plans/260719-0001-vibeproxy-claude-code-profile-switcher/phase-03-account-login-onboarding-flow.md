---
phase: 3
title: "Account Login & Onboarding Flow"
status: pending
priority: P1
effort: "3-4d"
dependencies: [0, 2]
---

# Phase 3: Account Login & Onboarding Flow

## Overview

The UX to add an account: create a fresh profile dir and run Claude Code's real interactive OAuth
`/login` into it, then detect completion and register the profile. This keeps VibeProxy on the
ToS-accepted path — every profile is a genuine login, no token copying between accounts.

## Requirements

- Functional: "Add profile" creates a new `CLAUDE_CONFIG_DIR`, drives the user through `claude`'s
  real browser OAuth login scoped to that dir, detects login completion (via `claude auth status
  --json` reporting `loggedIn: true`), and registers the profile with its email/org. Duplicate-account
  detection (same `orgId`/email) warns the user.
- Non-functional: never automate away Anthropic's consent screen; the browser OAuth is user-driven.

## Architecture

**Login mechanics.** Claude Code subscription login is interactive (opens a browser; the CLI prints
a URL / starts a local callback). VibeProxy cannot silently mint a subscription token — so onboarding
= spawn `claude` with `CLAUDE_CONFIG_DIR=<new profile dir>` and let the user complete the real flow:

- **Only stated path:** open Terminal.app running `CLAUDE_CONFIG_DIR=<dir> claude auth login --claudeai
  [--email <e>]` (interactive browser flow, verified in Phase 0). Do **not** use `claude setup-token` /
  a headless "printed URL" flow — that yields a long-lived `CLAUDE_CODE_OAUTH_TOKEN` (a *different*,
  model-requests-only auth path, precedence #5) that can't drive `/api/oauth/usage` and isn't the
  subscription credential this design needs.
- No cred-file handling needed: Phase 0 confirmed the token lands in the **Keychain** (service
  `Claude Code-credentials-<sha256(dir)[:8]>`), read later via Phase 2's `keychain_store`.

**Completion detection.** Poll `CLAUDE_CONFIG_DIR=<dir> claude auth status --json` (cheap, non-interactive)
until `loggedIn: true`; then read `email`/`orgId`/`subscriptionType` from that same output, fill the
profile record, and mark onboarding done. (A `notify`/FSEvents watch on `.claude.json` can trigger the
poll sooner.) Timeout + a manual "I've finished logging in" fallback button.

**Shared config (important UX — a `CLAUDE_CONFIG_DIR` is a factory-reset Claude Code).** Redirecting
the config dir isolates *everything*: `settings.json`, MCP servers, plugins, skills, history, project
trust. A power user switching profiles would otherwise lose their whole environment. On profile
creation, **symlink or copy the user's non-secret shared config** (e.g. `settings.json`, `plugins/`,
MCP config — NOT `.credentials.json`/`.claude.json` identity) from their base `~/.claude` into the new
dir. Exact shared-vs-isolated list is decided in Phase 0 (what can be shared without breaking per-account
identity); symlink for live-shared, copy for snapshot.

**Rust modules:**

- `onboarding/login_flow.rs` — orchestrate dir creation, seed shared config, spawn login, watch for completion, register.
- `onboarding/shared_config.rs` — symlink/copy the non-secret shared config into a new profile dir.
- `onboarding/dir_watcher.rs` — FS watch wrapper (`notify`), debounced.

## Related Code Files

- Create: `src-tauri/src/onboarding/{mod.rs,login_flow.rs,dir_watcher.rs}`
- Modify: `src-tauri/src/profile/store.rs` (register completed profile, dedupe by `accountUuid`)
- Modify: `src-tauri/src/switch/broker.rs` (reuse launch helper to spawn scoped `claude`)
- Modify: frontend onboarding view (basic; polished in Phase 6)

## Implementation Steps

1. `add_profile(label)` command: allocate id + dir via `profile/paths.rs`, create the dir, seed shared non-secret config (settings/plugins/MCP) via `shared_config.rs`.
2. Spawn the interactive login: open Terminal with `CLAUDE_CONFIG_DIR=<dir> claude auth login --claudeai` (macOS: `osascript`/`open -a Terminal`).
3. Poll `claude auth status --json` for the dir (optionally kicked by `dir_watcher` on `.claude.json`); on `loggedIn: true`, read email/orgId/tier and finalize the record.
4. Dedupe: if `orgId`/email matches an existing profile, warn and offer to cancel or relabel.
5. Failure/timeout handling: "finish in the terminal, then click Done"; allow retry; clean up empty dirs on cancel.

## Success Criteria

- [ ] "Add profile" walks a user through a real `claude` login into an isolated dir and registers the account with correct email/org
- [ ] Adding the same account twice is detected and warned
- [ ] Cancelling/failing cleans up the half-created profile dir
- [ ] After onboarding, the new profile is immediately switchable (Phase 2) and pollable (Phase 4)

## Risk Assessment

- **Interactive OAuth can't be fully scripted** — by design. Mitigation: make the manual step obvious; provide the exact command as copy-paste fallback.
- **Completion-detection races** — poll `auth status` rather than trusting a single FS event; require `loggedIn: true` before finalizing; debounce watcher triggers.
- **Keychain ACL prompt on first token read** (Phase 2) happens after onboarding — surface the "Always Allow" expectation in the onboarding success step.
