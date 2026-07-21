---
phase: 2
title: "Credential swap (macOS Keychain + lockfiles)"
status: pending
priority: P1
effort: "2-3d"
dependencies: [1]
---

# Phase 2: Credential swap

## Overview

Move an account's credentials into a target config dir so a **running** Claude Code session picks
them up, without racing Claude Code's own OAuth refresh.

## Requirements

- Copy account B's credentials into dir A's Keychain service name
- Bump `<A>/.credentials.json` mtime to trigger cache invalidation, only if the file already exists
- Hold Claude Code's advisory locks for the duration; abort rather than block
- Append a journal boundary (Phase 1) **only after** the swap verifies
- No token value logged, surfaced in an error, or sent over IPC

## Architecture

```
swap(from_dir, to_account)
  ├─ acquire ~/.claude.lock and ~/.claude.json.lock  (bounded wait, else abort)
  ├─ read to_account creds        (keychain::read_secret — existing Secret type)
  ├─ write into service_name(from_dir)
  ├─ read back and verify identity matches           (else roll back, abort)
  ├─ if <from_dir>/.credentials.json exists → rewrite same content (mtime bump)
  ├─ journal::append(boundary)
  └─ release locks
```

The verify-readback exists because a failed Keychain delete can leave the previous account's item
in place, which would silently resurrect the wrong account.

`mcpOAuth` and any other account-independent keys are preserved explicitly: read the existing blob,
replace only the OAuth account fields, write back. Never write a wholesale replacement.

## Related code files

- Create: `src-tauri/src/switch/hotswap.rs` — the swap routine
- Create: `src-tauri/src/switch/locks.rs` — mkdir-as-mutex compatible with `proper-lockfile`
- Modify: `src-tauri/src/keychain.rs` — add a guarded write; keep `Secret` non-printable
- Modify: `src-tauri/src/lib.rs` — `hot_swap_account` command

## Implementation steps

1. `locks.rs`: acquire via `mkdir` (atomic), 10s staleness with mtime touch, always release on drop
   (RAII guard, so a panic cannot strand the lock).
2. `keychain.rs`: `write_token(service, &Secret)` via `/usr/bin/security` with the value passed
   through stdin or a non-echoing argument path — never in an argv that could be seen in `ps`.
3. `hotswap.rs`: the sequence above, returning a typed error per failure mode.
4. Command layer: map errors to user-facing strings that describe the failure without any value.
5. Tests: lock contention aborts cleanly; verify-readback mismatch rolls back; `.credentials.json`
   absent means no file is created.

## Success criteria

- [ ] A running session switches account without a terminal restart (manual verification required —
      this cannot be asserted in a unit test)
- [ ] A held lock aborts the swap and leaves all state untouched
- [ ] A readback mismatch rolls back and reports failure
- [ ] `mcpOAuth` survives a swap
- [ ] No test, log, or error string contains a token

## Risks

- **Manual verification is unavoidable** for the core claim. Write down the exact steps used.
- Keychain prompts may appear on first write; the app already asks for "Always Allow" on read.
