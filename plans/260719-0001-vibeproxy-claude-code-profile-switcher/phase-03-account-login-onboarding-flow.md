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
  real browser OAuth login scoped to that dir, detects when `.credentials.json` + `oauthAccount`
  appear, and registers the profile with its email/org. Duplicate-account detection (same
  `accountUuid`) warns the user.
- Non-functional: never automate away Anthropic's consent screen; the browser OAuth is user-driven.

## Architecture

**Login mechanics.** Claude Code subscription login is interactive (opens a browser; the CLI prints
a URL / starts a local callback). VibeProxy cannot silently mint a subscription token — so onboarding
= spawn `claude` with `CLAUDE_CONFIG_DIR=<new profile dir>` and let the user complete the real flow:

- **Only stated path:** open Terminal.app running `CLAUDE_CONFIG_DIR=<dir> claude` (first run triggers
  the interactive `/login` browser flow). Do **not** use `claude setup-token` / a headless "printed
  URL" flow — that yields a long-lived `CLAUDE_CODE_OAUTH_TOKEN` (a *different*, model-requests-only
  auth path, precedence #5) that likely can't drive `/api/oauth/usage` and isn't the subscription
  credential this design needs.
- Ensure the token lands as file-based creds in the new dir (exact lever — pre-created placeholder vs
  nothing needed — is settled by **Phase 0** Q2/Q3, not assumed here). If Phase 0 shows login writes to
  Keychain for that dir, the fallback is a post-login, account-matched export (see Phase 2 migration rule).

**Completion detection.** Watch the new profile dir (`notify` crate / FSEvents) for
`.credentials.json` creation + a populated `oauthAccount` in `.claude.json`; on detection, read
account meta (Phase 2), fill the profile record, and mark onboarding done. Timeout + manual "I've
finished logging in" fallback button.

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

1. `add_profile(label)` command: allocate id + dir via `profile/paths.rs`, create the dir, seed shared non-secret config (settings/plugins/MCP) via `shared_config.rs`, configure for file-based creds.
2. Spawn the interactive login: open Terminal with `CLAUDE_CONFIG_DIR` set (macOS: `osascript`/`open -a Terminal`), or run `claude` and relay the login URL to the UI.
3. Start `dir_watcher` on the new dir; on `.credentials.json` + `oauthAccount` present, call `account_meta` and finalize the record.
4. Dedupe: if `accountUuid` matches an existing profile, warn and offer to cancel or relabel.
5. Failure/timeout handling: "finish in the terminal, then click Done"; allow retry; clean up empty dirs on cancel.

## Success Criteria

- [ ] "Add profile" walks a user through a real `claude` login into an isolated dir and registers the account with correct email/org
- [ ] Adding the same account twice is detected and warned
- [ ] Cancelling/failing cleans up the half-created profile dir
- [ ] After onboarding, the new profile is immediately switchable (Phase 2) and pollable (Phase 4)

## Risk Assessment

- **Interactive OAuth can't be fully scripted** — by design. Mitigation: make the manual step obvious; provide the exact command as copy-paste fallback.
- **File-based-creds override behavior on macOS** (does Claude write the file when told to, vs Keychain?) is the key unknown. Mitigation: verify in step 1; if Claude insists on Keychain for a given dir, fall back to a post-login `security ... -w` export into the dir.
- FS-watch races (partial writes) — debounce + require both files present before finalizing.
