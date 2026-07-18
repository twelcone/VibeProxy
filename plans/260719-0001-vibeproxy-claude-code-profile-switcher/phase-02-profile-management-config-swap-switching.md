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

**How a bare `claude` picks the active profile.** Two candidate mechanisms — pick during a spike:

- **(Preferred) Managed default config dir via symlink.** VibeProxy owns
  `~/.vibeproxy/active` as a symlink → `~/.vibeproxy/profiles/<active-id>`. The user is instructed
  (or a shell snippet is installed) to export `CLAUDE_CONFIG_DIR="$HOME/.vibeproxy/active"`. Switching =
  atomically re-point the symlink (`symlink to tmp + rename`). Any new `claude` uses the new profile;
  running sessions are unaffected (documented caveat).
- **(Fallback) Launch broker.** VibeProxy launches `claude` itself (or opens Terminal) with
  `CLAUDE_CONFIG_DIR` set to the active profile dir. No shell setup needed, but only helps sessions
  VibeProxy starts.

Decision recorded in `switch/broker.rs`; default to the symlink approach, keep the launch helper as a
convenience used again in Phase 3.

**Credential access — file-based per profile.** Each profile dir uses
`.credentials.json` (mode 0600) rather than the shared macOS Keychain item, so VibeProxy can read
*every* profile's token without Keychain per-path-hash ambiguity or entitlement prompts. Reading =
parse `.claudeAiOauth.{accessToken,refreshToken,expiresAt,subscriptionType}`. The account identity
lives in the profile dir's own `.claude.json` `oauthAccount` block (email, org, tier).

> **Migration note (danger).** `security find-generic-password -s "Claude Code-credentials" -w` reads
> the **default** Keychain item — i.e. whichever account is currently default. Writing that blob into
> an arbitrary profile dir would register the *wrong account's token* under that profile's identity,
> and Phase 3's `accountUuid` dedupe won't catch it (token and `oauthAccount` come from different
> sources). **Rule: never write an exported Keychain token into a profile dir unless the token's own
> account matches that dir's `oauthAccount.accountUuid`.** Whether this migration is even needed depends
> on Phase 0 (Q2/Q3): if fresh logins write file-based creds directly, prefer a clean re-login over
> Keychain export entirely. The previously-drafted "set a documented env so Claude writes the file"
> approach was removed — no such env var exists in the research; the only verified lever is
> pre-creating the file / a real `/login` (Phase 0 confirms which works).

**Rust modules:**

- `switch/broker.rs` — `set_active(profile_id)` (atomic symlink swap), `active()`, launch helper.
- `credentials/file_store.rs` — read/write `.credentials.json`; token accessor (never logged).
- `profile/account_meta.rs` — parse `oauthAccount` from a profile's `.claude.json`.

## Related Code Files

- Create: `src-tauri/src/switch/broker.rs`
- Create: `src-tauri/src/credentials/{mod.rs,file_store.rs}`
- Create: `src-tauri/src/profile/account_meta.rs`
- Modify: `src-tauri/src/profile/store.rs` (CRUD + priority), `src-tauri/src/platform/macos.rs` (symlink swap, atomic rename)
- Modify: `src-tauri/src/tray/mod.rs` (click a profile → `set_active`)

## Implementation Steps

1. Spike both switch mechanisms against a throwaway profile dir; confirm a new `claude` process
   honors `CLAUDE_CONFIG_DIR` pointing at the symlink and that swapping mid-idle takes effect on next launch.
2. Implement `credentials/file_store.rs`: read/parse `.credentials.json`; expose `access_token()` behind a type that redacts on `Debug`.
3. Implement `profile/account_meta.rs`: read `oauthAccount` → populate/refresh the profile record's `email`/`org`/`tier`.
4. Implement `switch/broker.rs::set_active`: write `~/.vibeproxy/active` symlink atomically (create temp symlink, `rename`), update `config.json.activeProfileId`.
5. Tauri commands: `set_active_profile(id)`, `delete_profile(id)`, `reorder_profiles(order)`; tray click on a profile calls `set_active_profile`.
6. Handle the running-session caveat: after switch, surface a non-blocking "active on next Claude launch" hint (full UX in Phase 6).

## Success Criteria

- [ ] Selecting a profile flips `~/.vibeproxy/active` and a freshly launched `claude` uses that account (verified via `claude` `/status` showing the expected email/org)
- [ ] VibeProxy reads the correct token + account metadata for each profile without a Keychain prompt
- [ ] Switch is atomic (kill VibeProxy mid-switch → store + symlink remain consistent) — covered by a unit test on the broker
- [ ] Deleting a profile removes its dir and record; reordering persists priority
- [ ] Keychain-export migration refuses to write when the exported token's account ≠ the target dir's `oauthAccount`

## Risk Assessment

- **`CLAUDE_CONFIG_DIR` semantics could differ from community reports** (e.g. per-path Keychain hashing). Mitigation: the file-based-creds design avoids relying on Keychain hashing entirely; spike in step 1 before building on it.
- **Migrating the existing default account to file creds may force a re-login.** Mitigation: treat re-login as acceptable; document it; don't destroy the Keychain item until the file path is verified.
- Secrets in logs — enforce a redacting newtype for tokens; add a grep check in CI.
- **GUI-launched Claude Code won't see a shell `export`.** Claude Code started by the VS Code/Cursor extension or under launchd won't inherit `CLAUDE_CONFIG_DIR` from a shell profile and will silently keep using the default `~/.claude` account while the menubar claims a switch happened. Mitigation: document the limitation; add an integration self-check (run `claude`, confirm the reported active email matches the expected profile); consider `launchctl setenv CLAUDE_CONFIG_DIR …` as a best-effort GUI-context path (verify in Phase 0 Q7).
