---
phase: 0
title: "Mechanism Spike (go/no-go)"
status: pending
priority: P1
effort: "1-2d + 24h refresh soak"
dependencies: []
---

# Phase 0: Mechanism Spike (go/no-go)

## Overview

Before writing a line of Rust, validate — with throwaway shell scripts and two real test logins —
the load-bearing assumptions the entire app rests on. Every later phase depends on these being true.
This is the cheapest possible de-risking and it **gates Phase 1**. Exit with an explicit go/no-go and
a short findings note in `reports/`.

## Why this exists

The plan's core mechanism (config-dir isolation + file-based creds + usage polling + pre-emptive
switch) combines several facts the research flagged as **unverified/community-sourced**:
`CLAUDE_CONFIG_DIR` Keychain-hashing behavior, whether login/refresh *writes back* to a file vs
Keychain, token expiry/refresh for inactive profiles, and symlink path canonicalization. If any is
false, the architecture changes. Find out now.

## Spike questions (each needs an empirical yes/no)

1. **Config-dir honored + symlink.** With `CLAUDE_CONFIG_DIR=$HOME/.vibeproxy/active` (a symlink to a
   real profile dir), does a fresh `claude` use that profile? Does Claude Code canonicalize the
   symlink anywhere (project registry, Keychain service name) that would break atomic re-pointing?
2. **Where do creds land on login.** Doing `/login` with `CLAUDE_CONFIG_DIR` set to an empty dir: does
   the token land in a file (`<dir>/.credentials.json`) or in macOS Keychain? If Keychain, what is the
   exact service name — is it the default `Claude Code-credentials`, or a per-path-hashed name?
3. **Can we force file-based creds.** Is there a reliable way to make Claude Code read+write creds from
   the profile dir's `.credentials.json` on macOS (pre-created file? empty placeholder? a real env)?
   Does an empty/placeholder file break login or get populated?
4. **Refresh write-back (24h soak).** After a token refresh (force expiry or wait a cycle), does the
   refreshed token land back in the file, or in Keychain? Does the file's refresh token get
   invalidated (bricking a profile that only reads the file)?
5. **Inactive-profile token freshness.** Does spawning `CLAUDE_CONFIG_DIR=<profile> claude` in a
   non-interactive auth/status mode refresh that profile's token without a full session? Identify the
   exact subcommand (research cites CCSwitcher using `claude auth status`-style refresh — verify it
   exists and refreshes). This is the keep-fresh primitive Phase 4 needs.
6. **Usage endpoint with a file token.** Does `GET /api/oauth/usage` (headers per Phase 4) succeed
   with a token read from a profile's `.credentials.json`? Capture a real response shape; if possible,
   capture a near-exhaustion payload (`limits[].severity`, exhaustion error body) — the plan's open
   questions.
7. **GUI-launch env inheritance.** Does `launchctl setenv CLAUDE_CONFIG_DIR …` reach a Claude Code
   started by the VS Code/Cursor extension, or does that path ignore it (silent wrong-account risk)?

## Go / No-Go criteria

- **GO** if: Q1 yes, Q2+Q3 give a workable file-creds path (or a clean Keychain-per-dir path), Q4
  shows refresh doesn't brick file-based profiles, Q5 yields a working keep-fresh command, Q6 works.
- **CONDITIONAL / adapt** if: creds must live in Keychain per-dir (→ switch credential module to
  Keychain-per-service reads, drop file-based design) or no keep-fresh command exists (→ redesign
  usage display around active-profile-only, demote per-profile usage from P1).
- **NO-GO / rethink** if: config-dir isn't honored via symlink, or refresh reliably bricks non-active
  profiles with no workaround (→ the whole multi-profile-usage premise needs redesign; escalate to user).

## Related Files

- Create: `plans/reports/spike-260719-mechanism-findings.md` (results + go/no-go + decisions that flow into Phases 1-4)
- Throwaway: shell scripts under a scratch dir (not committed)

## Success Criteria

- [ ] All 7 spike questions answered empirically with evidence (commands + observed output, secrets redacted)
- [ ] Explicit GO / CONDITIONAL / NO-GO recorded, with any architecture adaptations written down
- [ ] Phase 2/3/4 design decisions (credential store = file vs Keychain; keep-fresh command; poll strategy) are settled before implementation starts

## Risk Assessment

- The 24h refresh soak (Q4) adds calendar time — start it day 1 and do the other questions in parallel.
- Uses the user's real accounts for test logins — use throwaway/low-value accounts if available; all reads are read-only except real `/login`.
