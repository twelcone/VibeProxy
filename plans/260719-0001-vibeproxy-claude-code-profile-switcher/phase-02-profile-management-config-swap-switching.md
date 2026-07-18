---
phase: 2
title: "Profile Management & Config-Swap Switching"
status: pending
priority: P1
effort: "4-5d"
dependencies: [0, 1]
---

# Phase 2: Profile Management & Config-Swap Switching

## Overview

Implement the core switch: read a profile's credentials + account metadata, and atomically make a
chosen profile the "active" one so the **next** `claude` launch uses it. This is the heart of the
app and the part with the most Claude-Code-internals risk.

## Requirements

- Functional: read `oauthAccount` metadata (email, org, tier) and the OAuth token for any profile;
  set/get the active profile; switching is atomic (no window where `claude` sees half-swapped state);
  CRUD profiles (create dir, delete dir + record, reorder priority).
- Non-functional: switch operation is idempotent and reversible; never writes secrets to logs;
  credential reads never leave the machine.

## Architecture

**How a bare `claude` picks the active profile (settled by Phase 0 — NOT a symlink).** Claude hashes
the *literal* `CLAUDE_CONFIG_DIR` string to key its Keychain item, so a fixed symlink would collide all
profiles onto one item. Brokering targets each profile's **real, stable path**
`~/.vibeproxy/profiles/<id>`:

- **(Primary) Real-path indirection file.** Install a one-line shell snippet:
  `export CLAUDE_CONFIG_DIR="$(cat ~/.vibeproxy/active-path 2>/dev/null || echo "$HOME/.claude")"`.
  Switching = atomically write the active profile's **real path** into `~/.vibeproxy/active-path`
  (temp + rename). New shells resolve to that real path → correct per-profile Keychain item. Running
  sessions unaffected (documented caveat).
- **(Companion) Launch broker.** VibeProxy spawns `claude` / opens Terminal with
  `CLAUDE_CONFIG_DIR=<real profile path>` — immediate for VibeProxy-started sessions; powers the Phase 5
  "relaunch" action; reused in Phase 3 onboarding.

**Credential access — Keychain-per-path (settled by Phase 0).** A per-profile login stores its token in
the macOS Keychain under service **`Claude Code-credentials-<hash8>`**, where
`hash8 = SHA-256(absolute profile path)[:8]` (the default profile uses plain `Claude Code-credentials`).
VibeProxy computes that service name and reads the token via a `/usr/bin/security find-generic-password
-s <service> -w` subprocess — matches the Keychain ACL so the user grants "Always Allow" once (no repeat
prompts), the approach this repo's own hook uses. Account identity/tier comes from
`claude auth status --json` per dir (`email`, `orgId`, `subscriptionType`), not `.claude.json` parsing.

> The earlier "force file-based `.credentials.json` per profile" design is **dropped** (Phase 0 showed
> logins write to Keychain). The deterministic per-path service name gives clean multi-profile reads
> while keeping tokens encrypted; no Keychain-export migration is needed since each VibeProxy profile
> does its own real login.

**Rust modules:**

- `switch/broker.rs` — `set_active(profile_id)` (atomic `active-path` write), `active()`, launch helper.
- `credentials/keychain_store.rs` — compute `Claude Code-credentials-<sha256(path)[:8]>`, read token via `/usr/bin/security`; token accessor (redacting newtype, never logged).
- `profile/account_meta.rs` — profile identity/tier from `claude auth status --json` per dir.

## Related Code Files

- Create: `src-tauri/src/switch/broker.rs`
- Create: `src-tauri/src/credentials/{mod.rs,keychain_store.rs}`
- Create: `src-tauri/src/profile/account_meta.rs`
- Modify: `src-tauri/src/profile/store.rs` (CRUD + priority), `src-tauri/src/platform/macos.rs` (`active-path` atomic write, `/usr/bin/security` invocation)
- Modify: `src-tauri/src/tray/mod.rs` (click a profile → `set_active`)

## Implementation Steps

1. Implement `credentials/keychain_store.rs`: `service_name(path) = "Claude Code-credentials-" + sha256(path)[:8]`; read token via `/usr/bin/security find-generic-password -s <service> -w`; expose `access_token()` behind a redacting newtype. (Scheme verified in Phase 0.)
2. Implement `profile/account_meta.rs`: run `CLAUDE_CONFIG_DIR=<dir> claude auth status --json` → populate the profile record's `email`/`orgId`/`subscriptionType`.
3. Implement `switch/broker.rs::set_active`: atomically write the active profile's real path into `~/.vibeproxy/active-path` (temp + rename), update `config.json.activeProfileId`; provide the launch-broker helper (`claude` with `CLAUDE_CONFIG_DIR=<real path>`).
4. Install/verify the shell snippet that reads `active-path` (Phase 6 settings surfaces the copy-paste + a "done" check).
5. Tauri commands: `set_active_profile(id)`, `delete_profile(id)`, `reorder_profiles(order)`; tray click on a profile calls `set_active_profile`.
6. Handle the running-session caveat: after switch, surface a non-blocking "active on next Claude launch" hint + offer relaunch (full UX in Phase 6).

## Success Criteria

- [ ] Selecting a profile writes `~/.vibeproxy/active-path` and a freshly launched `claude` (new shell, or via the launch broker) uses that account — verified by `claude auth status --json` showing the expected email/org
- [ ] VibeProxy reads the correct token per profile via the computed Keychain service name (after a one-time "Always Allow")
- [ ] Switch is atomic (kill VibeProxy mid-switch → store + `active-path` remain consistent) — covered by a unit test on the broker
- [ ] `keychain_store::service_name` reproduces the Phase 0 fixture (`/Users/…/vp-spike` → `Claude Code-credentials-e30f4f07`) in a unit test
- [ ] Deleting a profile removes its dir and record; reordering persists priority

## Risk Assessment

- **Keychain ACL prompts.** Reading `claude`'s Keychain items from VibeProxy triggers a macOS prompt until the user clicks "Always Allow" (or the `security` binary is trusted). Mitigation: use the `/usr/bin/security` subprocess (repo hook precedent); surface a first-run explainer; one grant per profile service.
- **`CLAUDE_CONFIG_DIR`/Keychain-hash scheme is undocumented** and could change. Mitigation: isolate the hashing + `security` call in `keychain_store.rs`; a unit test pins the known fixture; smoke-test on Claude Code updates.
- Secrets in logs — enforce a redacting newtype for tokens; add a grep check in CI.
- **GUI-launched Claude Code won't see a shell `export`.** Claude Code started by the VS Code/Cursor extension or under launchd won't inherit `CLAUDE_CONFIG_DIR` from a shell profile and will silently keep using the default `~/.claude` account while the menubar claims a switch happened. Mitigation: document the limitation; add an integration self-check (run `claude`, confirm the reported active email matches the expected profile); consider `launchctl setenv CLAUDE_CONFIG_DIR …` as a best-effort GUI-context path (verify in Phase 0 Q7).
