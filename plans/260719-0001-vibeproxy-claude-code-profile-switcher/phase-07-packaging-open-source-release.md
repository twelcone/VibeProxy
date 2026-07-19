---
phase: 7
title: "Packaging & Open-Source Release"
status: completed
priority: P2
effort: "2-3d"
dependencies: [1, 2, 3, 4, 5, 6]
---

# Phase 7: Packaging & Open-Source Release

## Overview

Ship VibeProxy as an **unsigned, open-source** macOS build (no Apple Developer Program), with a clean
first-run experience, a GitHub Actions build producing a `.dmg`/`.app`, and the repo hygiene an
open-source project needs. Keep the Rust core Windows-buildable.

## Requirements

- Functional: `cargo tauri build` produces a runnable macOS app; a documented first-run path for an
  unsigned app; GitHub release with downloadable artifact built by CI; README with install + Claude
  Code integration steps.
- Non-functional: no paid signing/notarization; reproducible CI build; MIT (or chosen) license.

## Architecture

**Unsigned distribution (user declined the $99/yr program).** Options, documented for users:

- Ad-hoc / no Developer ID signing. **First-run on modern macOS (15 Sequoia+): the old
  right-click → Open trick no longer works for unsigned/unnotarized apps** — users must go to
  System Settings → Privacy & Security → "Open Anyway" after the first blocked launch. Document that
  as the primary path; `xattr -dr com.apple.quarantine /Applications/VibeProxy.app` as a secondary
  (works but scary for non-technical users, and re-triggers on every update).
- **Recommended install path: a Homebrew cask with `--no-quarantine`** (`brew install --cask <tap>/vibeproxy`)
  — the only decent unsigned UX (no per-update Gatekeeper friction). Promote this from "optional" to the
  primary recommended install; the raw `.dmg` is the fallback for non-brew users.

**CI (`.github/workflows/build.yml`):** on tag, `macos-latest` runner runs `cargo tauri build`,
uploads the `.dmg`/`.app` to the GitHub release. Optionally add a `windows-latest` job to prove the
Rust core still builds (tray/keychain branches may be `#[cfg]`-gated / stubbed) — portability guard.

**Repo hygiene:** `README.md` (what it is, the honest design/risk note — "no inference-token relay;
usage polling carries residual accepted risk", install, first-run, Claude Code
`CLAUDE_CONFIG_DIR` setup, screenshots), `LICENSE`, `CONTRIBUTING.md` (optional), issue templates
(optional), and a clear **security/ToS statement**: VibeProxy never relays OAuth tokens to inference
and only reads usage via the same endpoint Claude Code's own tooling uses.

Use `gh` (authenticated as `twelcone`) to create the repo/release.

## Related Code Files

- Create: `.github/workflows/build.yml`
- Create: `README.md` (full), `LICENSE`, optionally `CONTRIBUTING.md`, `.github/ISSUE_TEMPLATE/`
- Modify: `tauri.conf.json` (bundle config: identifier, icon, targets `dmg`/`app`; no signingIdentity)
- Create: `docs/` screenshots/gifs (optional)

## Implementation Steps

1. Configure `tauri.conf.json` bundle (identifier e.g. `com.github.twelcone.vibeproxy`, icons, dmg+app targets, no signing identity).
2. Local `cargo tauri build`; verify the produced app runs after `xattr` quarantine removal.
3. Write `README.md`: positioning, the honest architecture/risk note (no inference-token relay; usage-polling residual risk), install (Homebrew cask `--no-quarantine` recommended) + first-run (macOS 15+: System Settings → "Open Anyway"; `xattr` secondary), and the `export CLAUDE_CONFIG_DIR="$HOME/.vibeproxy/active"` integration step.
4. Add `LICENSE` (confirm license choice with user) and the security/ToS statement.
5. `build.yml`: tag-triggered macOS build → upload artifact to release; optional Windows build-only job as a portability guard.
6. `gh repo create` (if not already) + first tagged release; verify the downloaded artifact runs on a clean-ish account following only the README.

## Success Criteria

- [ ] `cargo tauri build` produces a `.dmg`/`.app` that runs after the documented first-run step
- [ ] Tagging a release triggers CI that builds and attaches the macOS artifact
- [ ] README lets a new user install, complete first-run, wire up `CLAUDE_CONFIG_DIR`, and add a profile without prior context
- [ ] Repo has a license and an explicit ToS/security statement about not relaying tokens
- [ ] (Guard) Windows build job compiles the Rust core, or portability limitations are documented

## Risk Assessment

- **Unsigned app friction** — non-technical users may be wary of the Gatekeeper bypass. Mitigation: clear README + screenshots; revisit notarization only if the user later opts into the program.
- **CI signing secrets** — none needed (unsigned), which simplifies CI; just ensure artifacts are attached correctly.
- License choice is a user decision — confirm before first public release (open question).
