---
phase: 3
title: "Platform seams behind traits (macOS impl)"
status: done
priority: P1
effort: "1-2d"
dependencies: [1]
---

# Phase 3: Platform seams

## Overview

Put every OS-specific operation behind a trait, with a macOS implementation now. No new-platform
code yet — the point is to shape the seams so a Linux/Windows backend is an *addition*, never an edit
to shared logic. This is also what a future SwiftUI/uniffi frontend and the CLI both rely on.

## The OS-specific surface (measured)

| Operation | Today (macOS) | Trait |
|---|---|---|
| Read/write credentials | `/usr/bin/security` (8 sites, `keychain.rs`) | `CredentialStore` |
| Launch a terminal on a profile | `osascript` Terminal (2 sites, `switch/mod.rs`) | `TerminalLauncher` |
| Locate Claude's config dir + "is default" | `~/.claude`, `is_default` (`profile/paths.rs`) | `ClaudeConfig` |
| Account identity | `claude auth status --json` (`account_meta.rs`) | cross-platform already (claude CLI) |

## Trait sketch

```rust
// crates/vibeproxy-core/src/platform/mod.rs
pub trait CredentialStore {
    fn read_blob(&self, dir: &Path) -> Result<Blob, String>;
    fn write_blob(&self, dir: &Path, acct: &str, blob: &Blob) -> Result<(), String>;
    fn item_account(&self, dir: &Path) -> Result<String, String>;
    // backup/restore for hot-swap build on read/write, so they stay platform-agnostic
}

pub trait TerminalLauncher { fn launch_claude(&self, dir: &Path) -> Result<(), String>; }
pub trait ClaudeConfig    { fn default_dir(&self) -> Option<PathBuf>; fn is_default(&self, d: &Path) -> bool; }

pub fn host() -> Host;   // returns the impls for the current OS via cfg(target_os)
```

macOS impls wrap the exact code that exists today, moved verbatim. Hot-swap's backup/restore and the
lock dance are expressed in terms of `CredentialStore`, so they need no per-OS branching beyond the
store itself.

## Implementation steps

1. Define the traits + a `Host` bundle in `core::platform`.
2. Move current `keychain.rs` bodies into `platform::macos::KeychainStore` implementing
   `CredentialStore`; keep the SHA-256 service-name scheme and the `Secret`/`Blob` non-printable
   wrappers.
3. Move `switch::launch_claude` into `platform::macos::AppleScriptLauncher`.
4. Move `paths::default_config_dir`/`is_default` behind `ClaudeConfig` (macOS impl unchanged).
5. Call sites (`hotswap`, `scan`'s account resolution, `poller`, CLI) take a `&Host` (or use
   `platform::host()`), rather than calling `security`/`osascript` directly.
6. Keep `#[cfg(target_os = "macos")]` on the macOS impls; a non-macOS build compiles the traits with a
   `todo!()`/`unimplemented!()` host so the workspace builds on Linux CI while backends are pending.

## Tests / validation

- The real-Keychain E2E now runs through `KeychainStore` — same asserts, still `--ignored`.
- A trait-level unit test with an in-memory `CredentialStore` fake exercises `hotswap` backup/restore
  logic without touching the real Keychain — this is a new, valuable test the current design can't have.

## Success criteria

- [ ] No `usr/bin/security` or `osascript` string outside `platform::macos`
- [ ] `hotswap` and `scan` depend only on the traits, not on macOS specifics
- [ ] Workspace compiles on Linux (traits present, macOS impls cfg'd out, host = unimplemented)
- [ ] An in-memory credential fake lets hot-swap backup/restore be unit-tested off-device

## Risks

- **Over-abstraction.** Keep traits to the four operations that are genuinely OS-specific; don't
  trait-ify pure logic. If only one method varies, it's one method, not a framework.
- **The default-account special case** (`~/.claude` needs `CLAUDE_CONFIG_DIR` unset) is macOS-verified
  behaviour; carry it into `ClaudeConfig::is_default` with the existing comment, and re-verify per OS
  in phase 5.
