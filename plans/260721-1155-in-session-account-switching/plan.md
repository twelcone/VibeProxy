---
title: "In-Session Account Switching (hot-swap + attribution boundaries)"
description: "Switch the account of a RUNNING Claude Code session by swapping credentials in place, and keep per-account analytics correct by recording swap boundaries and attributing transcripts by timestamp."
status: pending
priority: P1
effort: "~1 week"
tags: [vibeproxy, switching, keychain, analytics]
created: 2026-07-21
source: github.com/realiti4/claude-swap (MIT) — mechanism reference only, no code copied
---

# In-Session Account Switching

## Problem

VibeProxy's headline feature is auto-switch on quota exhaustion. It is currently half-broken: the
switch writes `~/.vibeproxy/active-path`, which only affects **newly launched** terminals. The
session that just hit the limit keeps burning the exhausted account until the user opens a new
terminal — precisely the moment they least want friction.

## Mechanism (verified in research)

Claude Code resolves `CLAUDE_CONFIG_DIR` at process start, so a running session cannot be moved to a
different directory. The only lever is mutating the credentials of the dir it is already using:

- **macOS**: overwrite the Keychain item for that dir's service name, then rewrite
  `<dir>/.credentials.json` with the same fresh values purely to bump its mtime, which triggers
  Claude Code's cache invalidation. Without the mtime bump it still applies, but only after the
  ~30s Keychain TTL.
- Claude Code refreshes its own OAuth under `~/.claude.lock` / `~/.claude.json.lock`. A swap must
  hold those same advisory locks or it can be stomped mid-refresh.

Reference: `plans/reports/research-260721-1139-claude-swap-in-session.md`.

## The cost, and how this plan pays it

Hot-swapping writes account B's token into dir A. From that moment, B's transcripts are written
into `A/projects/`, and our analytics — which attribute by directory — would silently credit them
to A. Silently-wrong data is worse than absent data.

**Fix: attribute by time, not by directory.** Every swap appends a boundary record. Scanning then
resolves each message's account by looking its timestamp up against that directory's timeline.

```
dir ~/.vibeproxy/profiles/work
  |----------- Work -----------|-------- Personal --------|---- Work ----|
                             swap                       swap
message at t  ->  binary-search the timeline  ->  correct account
```

With no swap records, the timeline is a single interval and behaviour is identical to today.

## Phases

| # | Phase | Status | Depends on |
|---|-------|--------|-----------|
| 1 | [Swap journal + timeline attribution](./phase-01-swap-journal-attribution.md) | Pending | — |
| 2 | [Credential swap (macOS Keychain + lockfiles)](./phase-02-credential-swap.md) | Pending | 1 |
| 3 | [Wire auto-switch + UI affordance](./phase-03-wire-autoswitch-ui.md) | Pending | 1, 2 |

Phase 1 first and deliberately: attribution must be correct **before** anything can corrupt it. It
ships as a no-op on today's data, so it can be verified against known-good numbers.

## Non-goals

- Linux/Windows credential swapping (macOS-only app today)
- Moving a running session to a different `CLAUDE_CONFIG_DIR` — not possible
- Replacing the existing path-file switch; hot-swap is additive and stays opt-in

## Risks

1. **We would write credentials for the first time.** Today VibeProxy only ever reads them. This is
   a new failure class: a failed Keychain delete can resurrect the wrong account. Mitigate with
   write-then-verify-readback, and abort the swap if the readback disagrees.
2. **Racing Claude Code's own OAuth refresh.** Mitigate by holding its advisory locks; abort rather
   than block indefinitely if the lock is held.
3. **MCP OAuth clobber.** The upstream project has an open issue where switching overwrites live
   `mcpOAuth` state. Preserve any account-independent OAuth keys explicitly rather than writing the
   whole credential blob.
4. **Journal loss degrades attribution silently.** Append-only file, fsync on write, and surface a
   visible warning in the Usage window when a scanned range predates the earliest journal entry.
5. **Secrets discipline.** Tokens must never be logged, never appear in errors, never cross the
   Tauri IPC boundary. Existing `keychain::Secret` already enforces this at the type level — extend
   it rather than working around it.

## Success criteria

- [ ] A running `claude` session picks up a switched account without restarting the terminal
- [ ] Auto-switch on quota rescues the *current* session, not just the next one
- [ ] Per-account analytics stay correct across swaps, verified against a fixture with known
      boundaries
- [ ] With no swaps recorded, analytics output is byte-identical to today
- [ ] A swap that cannot take the lock fails loudly and changes nothing
- [ ] No token value ever reaches a log, an error string, or the frontend

## Open questions

- Should hot-swap be default-on or opt-in? (Leaning opt-in for one release, given it writes creds.)
- Does the running session need the *transcript* to continue in the same file, or is a new session
  file on the same account acceptable?
- How should the Usage window present a day that spans a swap — split the row, or annotate it?
