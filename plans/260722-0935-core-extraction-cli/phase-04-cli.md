---
phase: 4
title: "vibeproxy CLI (headless / WSL)"
status: pending
priority: P1
effort: "2d"
dependencies: [1, 2, 3]
---

# Phase 4: `vibeproxy` CLI

## Overview

A binary crate that drives the core from a plain terminal — the surface that actually works
everywhere `claude` does: WSL, SSH, containers, headless servers. No GUI, no Tauri.

## Command surface

| Command | Does | Core call |
|---|---|---|
| `vibeproxy list` | show profiles + active marker | `store::load` |
| `vibeproxy status [--json]` | active account, its usage %, resets | `store` + `usage::poll_profile` |
| `vibeproxy switch <id\|label>` | set the active profile (writes `active-path`) | `switch::set_active_config_dir` + `store::set_active_profile_id` |
| `vibeproxy usage [--range a..b] [--json]` | the analytics aggregate | `usage_analytics::scan` |
| `vibeproxy export <path> [--range]` | CSV | `usage_analytics::to_csv` |
| `vibeproxy adopt <label> <dir>` | register an existing login | `account_meta::fetch` + `store::add_profile` |
| `vibeproxy remove <id\|label>` | drop a profile | `store::remove_profile` |
| `vibeproxy auto` | one-shot: switch if the active is over threshold | `autoswitch::decide` |
| `vibeproxy shell-init` | print the shell snippet (for `eval`) | `shell::snippet` |

Human-readable by default; `--json` on read commands for scripting. Exit non-zero on failure with a
message to stderr; never print a token.

## Why this is the WSL answer

Inside WSL, `claude` reads `~/.claude` in the Linux filesystem and (per research) stores credentials
in a `.credentials.json` file, not a keychain. The CLI runs *in that same WSL userland*, so it
manages the right files with no cross-boundary problem a Windows-host GUI would have. `vibeproxy
switch` there just repoints `active-path`; the shell snippet (already cross-shell) does the rest.

## Implementation steps

1. `crates/vibeproxy-cli` with `clap` (derive) for arg parsing; depends on `vibeproxy-core`.
2. One thin handler per command — all logic is in core; the CLI only parses, calls, and formats.
3. `--json` via `serde_json` on the same structs the GUI serializes (`Analytics`, `ProfileUsage`,
   `Profile`), so CLI and GUI output are the same shape.
4. Respect `VIBEPROXY_DIR` (already the core's config-dir override) for isolated testing.
5. Man-page-ish `--help` text; a short section in the README on headless/WSL use.

## Tests / validation

- Integration test: against a temp `VIBEPROXY_DIR`, `adopt` a fake dir, `list` shows it, `switch`
  sets `active-path`, `remove` drops it — asserting on the CLI's stdout/exit codes.
- `--json` output parses back into the core structs.
- Manual: run `vibeproxy status` inside a WSL/Linux shell against a real `~/.claude`.

## Success criteria

- [ ] `vibeproxy` runs with no GUI/Tauri present
- [ ] `switch` / `status` / `usage` work in a plain Linux/WSL terminal
- [ ] `--json` output matches the GUI's serialized shapes
- [ ] No token or credential value ever reaches stdout/stderr/logs

## Risks

- **CLI vs app write races** (both can `switch`). Reuse `store`'s existing atomic write + config
  lock; document that running both simultaneously is fine but last-writer-wins on `active-path`.
- **Scope creep into a TUI.** Keep it a plain CLI; a TUI is not a goal.
