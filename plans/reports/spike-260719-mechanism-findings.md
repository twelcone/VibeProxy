# Phase 0 Mechanism Spike — Findings

Date: 2026-07-19 · Claude Code `2.1.214` · macOS · read-only except where noted.

## Verdict: **GO** — fully resolved via a real second-account login. One switch-mechanism correction.

`CLAUDE_CONFIG_DIR` isolates accounts (CONFIRMED with two real logins). Credentials live in the
**Keychain under a deterministic per-path service name**. **Correction:** the planned symlink switch is
invalid — Claude hashes the *literal* config-dir path, so the switch must target each profile's **real
path**, not a swapped symlink. No change to the overall architecture; Phase 2 switch mechanism updated.

## Confirmed (autonomous, this session)

| # | Question | Result |
|---|----------|--------|
| Q1 | Config-dir honored | ✅ `CLAUDE_CONFIG_DIR=<dir> claude auth status` uses that dir (writes `.claude.json`, `backups/` there); does **not** read the real account |
| Q1 | Symlinked config dir | ✅ `CLAUDE_CONFIG_DIR=<symlink→dir>` honored, no canonicalization break, no crash — **atomic symlink swap is viable** |
| — | **Credential isolation (crux)** | ✅ Empty config dir → `{"loggedIn": false, "authMethod": "none"}` **even though the global Keychain item `Claude Code-credentials`/`twel` exists**. So a custom config dir is a fully isolated auth context — it does NOT just read the one global Keychain item |
| Q5 | Keep-fresh / identity command | ✅ `claude auth status --json` per config dir returns `loggedIn, authMethod, email, orgId, orgName, subscriptionType` (e.g. `"max"`). Non-interactive, per-dir. This is the keep-fresh trigger **and** the integration self-check **and** the profile-identity source (better than parsing `.claude.json`) |
| P3 | Onboarding command | ✅ `claude auth login --claudeai --email <e>` — interactive browser login, scoped to the config dir. Phase 3 uses this (not bare `claude`). `--console`/`--sso` variants exist |
| Q6 | Usage endpoint | ✅ `GET /api/oauth/usage` returns HTTP 200 with a real token (verified in research report); response shape known. Re-check *with a per-profile token* depends on Q2/Q3 |
| Q7 | GUI env mechanics | Partial: `launchctl getenv CLAUDE_CONFIG_DIR` is currently unset → GUI/VS Code-launched `claude` inherits nothing today. `launchctl setenv` is the lever; full inheritance test needs the user |

Notes:
- The real default account has **no** `~/.claude/.credentials.json` → macOS uses the **Keychain** store
  (service `Claude Code-credentials`, account `twel`). Confirmed via metadata lookup (no secret read).
- `--bare` mode help confirms macOS normally does "keychain reads" for OAuth.

## RESOLVED via real 2nd-account login (`~/vp-spike`, `toanpt.developer@gmail.com`, max)

- **Q2/Q3 — where creds land:** the config dir got only `.claude.json` + `backups/` — **no
  `.credentials.json`**. The token went to the **macOS Keychain**, under service
  **`Claude Code-credentials-e30f4f07`** (default account uses plain `Claude Code-credentials`).
- **Hash scheme (reverse-engineered):** the suffix = **first 8 hex of `SHA-256(absolute config-dir
  path)`**. `SHA-256("/Users/twel/vp-spike")[:8] == e30f4f07` — exact match. So VibeProxy can compute
  any profile's Keychain service name deterministically from its dir path and read the token (via
  `/usr/bin/security` to match Keychain ACL / avoid repeat prompts, like this repo's own hook).
- **Isolation confirmed with real logins:** `~/vp-spike` reports `toanpt…` while the default `claude`
  reports `adeo…` — two live accounts, fully separate.

### ⚠️ Switch-mechanism correction (the important one)

Pointing a **symlink** at the real logged-in dir and querying through it returns `loggedIn: false`:
Claude hashes the **literal** `CLAUDE_CONFIG_DIR` string (`sha256(symlink path)=d308e658`), not the
resolved realpath (`e30f4f07`). Therefore:

- ❌ A fixed `~/.vibeproxy/active` symlink as the exported config dir **does not work** — all profiles
  logged in through it collide on one Keychain item, and re-pointing it can't reach a profile's token.
- ✅ **Corrected design:** each profile is a fixed real dir `~/.vibeproxy/profiles/<id>`; its Keychain
  item is stable (`Claude Code-credentials-<sha256(realpath)[:8]>`). "Active" is brokered by the
  **real path**, two ways:
  1. **Shell indirection (real path, not symlink):** shell rc **sets or unsets** the var:
     `_vp="$(cat ~/.vibeproxy/active-path 2>/dev/null)"; [ -n "$_vp" ] && export CLAUDE_CONFIG_DIR="$_vp" || unset CLAUDE_CONFIG_DIR`.
     VibeProxy writes the active profile's **real path** into `~/.vibeproxy/active-path`, or empties it
     for the default account. New shells resolve to the right account. (Still next-launch only.)

     > **Default-account gotcha (found during Phase 2 implementation):** `CLAUDE_CONFIG_DIR=~/.claude`
     > set explicitly ≠ unset. Setting it makes Claude hash `/Users/<u>/.claude` → service
     > `Claude Code-credentials-72c4fc80` (0 items) → `loggedIn:false`. The default account's bare
     > Keychain item is read only when the var is **unset**. So: default profile = clear active-path
     > (shell unsets); read its identity/token with the var unset. Non-default profiles set the var.
  2. **Launch broker:** VibeProxy spawns `claude` / opens Terminal with `CLAUDE_CONFIG_DIR=<real path>`.

### Still open (low risk)

- **Q4 — refresh write-back** (soak): ~11h after login, `CLAUDE_CONFIG_DIR=~/vp-spike claude auth status
  --json` still returns `loggedIn: true` (toanpt) with no re-login — refresh token intact, risk low.
  (Within the original ~24h token life, so a full post-expiry refresh-in-place confirmation is still
  pending, but the inactive-profile concern is substantially de-risked.)
- **Q7 — GUI inheritance:** does a VS Code/Cursor-launched `claude` pick up `launchctl setenv
  CLAUDE_CONFIG_DIR …`? (test in-editor.)
- `/api/oauth/usage` `severity` near 100% still unobserved.

## Impact on the plan

- **No architecture change.** Config-dir isolation + symlink switching + `auth status` identity/keep-fresh
  + usage endpoint all hold. Phases 1–7 proceed as written.
- **Design lock-ins from this spike:**
  - Switch = broker the **real profile path** (indirection file `~/.vibeproxy/active-path` + launch
    broker). **NOT a config-dir symlink** (invalid — see correction above). Update Phase 2.
  - Credential read = **Keychain-per-path** via `/usr/bin/security -s "Claude Code-credentials-<sha256(realpath)[:8]>" -w`.
    **Drop the "force file-based creds" design** — login writes to Keychain, and per-path service names
    make multi-profile reads clean and keep tokens encrypted. Update Phase 2/4.
  - Profile identity + keep-fresh + self-check = `claude auth status --json` per dir (not `.claude.json` parsing).
  - Onboarding = `claude auth login --claudeai --email <e>` with `CLAUDE_CONFIG_DIR=<real dir>` (Phase 3).
- **Reuse:** the logged-in `~/vp-spike` (toanpt account) can become VibeProxy's first real profile —
  or `rm -rf ~/vp-spike` to discard (leaves a harmless orphan Keychain item).

## Unresolved questions

- Q2/Q3/Q4 above (need the interactive login).
- Q5 refresh-on-status: does running `auth status` near expiry actually refresh the token, or only report? (confirm during the soak).
- Q7: does a VS Code/Cursor-launched `claude` inherit `launchctl setenv CLAUDE_CONFIG_DIR`? (needs the user to test in-editor).
- `/api/oauth/usage` `limits[].severity` values near 100% still unobserved.
