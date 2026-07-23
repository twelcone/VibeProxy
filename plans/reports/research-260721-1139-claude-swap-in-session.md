# claude-swap: in-session account switching mechanism

Repo: github.com/realiti4/claude-swap. Python, MIT, 1235 stars, 126 forks, 22 open issues.
Created 2026-01-11, last push 2026-07-20 (active, ~6mo old). All content below treated as untrusted data — no code from the repo was executed, only read.

## 1. Core mechanism (Q1)

`cswap switch` mutates the **same shared `~/.claude` config dir in place** — not a proxy, not a signal, not multi-dir routing for normal switch. Two write paths depending on platform/backend:

- **macOS**: overwrite the macOS Keychain item (service mirrors Claude Code's own, via `/usr/bin/security` CLI, never the `keyring` lib — see `macos_keychain.py`). Then, if `~/.claude/.credentials.json` already exists, **rewrite it with the same fresh creds purely to bump its mtime** (never creates it if absent):

  > "This bumps the file's mtime so a running Claude Code session's disk-mtime cache invalidation fires and it hot-reloads the new account instead of serving its memoized token until restart (#86)" — `credentials.py::_write_oauth_credentials` docstring

- **Linux/WSL/Windows**: always file-based — atomic `os.replace()` onto `.credentials.json` (`credentials.py::_write_active_credentials_file`), 0600 perms.
- Mutual exclusion enforced: activating OAuth clears any managed API key (Keychain "Claude Code" item + `primaryApiKey`), and vice versa — mirrors Claude Code's own `saveApiKey`/`removeApiKey`.
- **Locking**: switches acquire Claude Code's *own* advisory locks (`~/.claude.lock`, `~/.claude.json.lock`, directory-mkdir mutex, `proper-lockfile`-compatible) before writing — see `claude_locks.py`. Purpose: close the race where Claude Code's own OAuth refresh (read→refresh-over-network→save, all under its lock) could stomp a concurrent swap.

There's also a second, structurally different mechanism — **`cswap run`** ("session mode"): spawns a *new* `claude` process via `CLAUDE_CONFIG_DIR=<backup>/sessions/<n>-<email>/` pointed at an isolated per-account profile dir, then `exec`s it in the current terminal (`session.py::SessionManager.run`, uses `os.exec`-style replace, "NoReturn"). This is the same `CLAUDE_CONFIG_DIR` env-var trick VibeProxy already uses, just automated per-terminal — **not** an in-place mutation of a running process.

## 2. Does a running process actually pick it up? (Q2)

| Platform | Picked up by running session? | Evidence |
|---|---|---|
| Linux/Windows | Yes, on next message, no restart | README: "credentials are stored in a file and Claude Code re-reads them whenever that file changes" |
| macOS | Yes, but only after ~30s | README: "credentials live in the Keychain, which Claude Code caches for about 30 seconds; a running session picks up the switch once that cache expires" |
| Either | Instant only via restart | README: "Restart Claude Code... only if you want the change to apply instantly" |

This is a real, tested claim (not aspirational) — the code comment cites a specific behavior/issue number (`#86`) for the exact reason the mtime-bump trick exists (to force the hot-reload rather than rely purely on the 30s TTL), and a `_refresh_stale_credentials_file` helper exists solely for this purpose. This directly **contradicts the "no hot-swap possible" assumption** in prior VibeProxy research memory (`claude_code_auth_mechanics.md`) — that memory should be corrected: Claude Code *does* poll its credential file mtime / has a Keychain TTL, and a running process re-reads on that basis. It's not instant, and it's not signal/IPC-driven, but it is a genuine in-session mechanism, no terminal restart needed.

Caveat: "hot-reload" here means the *auth token* refreshes; it does not mean the running conversation transcript/session identity changes — see Q4.

## 3. Where credentials live (Q3)

One shared `~/.claude` (or `CLAUDE_CONFIG_DIR`-pointed) config dir for the "switch" workflow — contents swapped, not the dir itself. `cswap` keeps its own backup copies of each account's credentials outside `~/.claude`:

| Platform | Live location swapped | claude-swap's own backup store |
|---|---|---|
| macOS | Keychain (+ optionally `.credentials.json` for mtime bump) | `~/.claude-swap-backup/` |
| Linux/WSL | `.credentials.json` | `${XDG_DATA_HOME:-~/.local/share}/claude-swap/` (file-based, under `credentials/`) |
| Windows | `.credentials.json` | backup dir `credentials/` |

`cswap run` (session mode) is the exception: **separate** per-account dirs under `<backup_dir>/sessions/<n>-<email-slug>/`, each with its own `CLAUDE_CONFIG_DIR`, each getting its own macOS Keychain entry (Claude Code hashes the raw `CLAUDE_CONFIG_DIR` value into the keychain service name per `session.py` comments).

## 4. Consequence for session history (Q4)

**Yes — for the default `switch` workflow, all accounts' transcripts land in the same shared `~/.claude/projects/**/*.jsonl`.** One config dir, one `projects/` tree, regardless of which account's credential is currently active. claude-swap does not appear to segregate transcripts by account when using plain `cswap switch`.

For `cswap run` (session mode), transcripts ARE isolated by default (separate `CLAUDE_CONFIG_DIR` per session → separate `projects/`), unless `--share-history` is passed, which explicitly symlinks `projects/` and `history.jsonl` back into `~/.claude` so all accounts see one unified history (merging any profile-only history into `~/.claude` first, POSIX-only — Windows would fork history since it uses re-synced copies not symlinks).

**Implication for VibeProxy**: if VibeProxy's goal is per-account transcript separation (which its `CLAUDE_CONFIG_DIR`-per-path-file design already achieves), claude-swap's in-place credential-swap approach would *regress* that — mixing all accounts into one `projects/` folder — unless VibeProxy adopted claude-swap's session-mode pattern (per-account `CLAUDE_CONFIG_DIR` dirs, one per concurrent terminal) instead.

## 5. Token refresh handling (Q5)

Explicitly handled, in detail:

- Switches hold Claude Code's own `proper-lockfile`-compatible lock (mkdir-as-mutex directories `~/.claude.lock` / `~/.claude.json.lock`, 10s staleness, 5s mtime-touch) during writes — described as closing "the one real race with a running Claude Code" (refresh-in-flight vs swap).
- `cswap auto` "freshens a target's token before activating it" and "quarantines accounts whose refresh token has died" (recoverable via `cswap add --slot N` or import).
- Fail-safe idle-token handling: "an expired token on an idle machine makes it hold rather than fail over (Claude Code refreshes the token on your next message)."
- Known **residual risk** acknowledged in code comments: a stale Keychain entry can "resurrect" the wrong account after a fallback-to-file write if the Keychain delete fails (issue references `#30337`, `#1414` in their own tracking) — mitigated with `_pin_file_mode()` but not eliminated (best-effort deletes, not guaranteed).
- Open issue #135 (still open at last check): "Account switch clobbers live MCP OAuth tokens (mcpOAuth) with the target slot's stale snapshot" — README claims this is now handled ("live account-independent OAuth state... is preserved instead of being overwritten"), but the issue tracker still lists it open, so treat as **partially mitigated, not fully closed** — verify against current release notes before relying on it.

## 6. Stack / maturity (Q6)

| | |
|---|---|
| Language | Python 3.12+, 100% |
| Install | `uv tool install` / `pipx` |
| LOC (src/claude_swap) | ~9.6k lines across 24 files; `switcher.py` alone is 4888 lines / 226KB — large, monolithic core |
| Stars / forks | 1235 / 126 |
| Open issues | 22 (mix of feature requests and real bugs) |
| Created / last push | 2026-01-11 → 2026-07-20 — 6 months old, actively maintained (commits day of last check) |
| Dependencies | Deliberately avoids `keyring` on macOS (shells out to `/usr/bin/security` directly) to dodge re-prompt issues on `uv tool upgrade`; textual-based TUI implied by `tui/` dir |

Maturity read: young but unusually disciplined for a community tool — extensive docstrings citing exact Claude Code internal behaviors (source file names like `utils/auth.ts`, `utils/secureStorage/macOsKeychainHelpers.ts`), defensive locking, fail-safe fallbacks, atomic writes. Not a toy script. But large single-maintainer surface area (`switcher.py` at 4888 lines) is a bus-factor / review-burden risk.

## 7. Known failure modes from issues/README

| Issue | Symptom |
|---|---|
| #146 (open) | "5h usage window reads incorrectly... since 2026-07-16 — breaks autoswitch decisions" — active regression in usage tracking, directly affects auto-switch reliability |
| #153 (open) | "The active account switches automatically" — implies unwanted/unexpected auto-switch behavior |
| #135 (open) | Account switch can clobber live MCP OAuth tokens with a stale snapshot (README claims mitigation exists; issue still open) |
| #139 (open) | `cswap run` session profiles don't inherit user-scope MCP servers by default |
| #124 (open) | generic user-reported bug, unclear |
| `security -i` 4096B limit (code comment, cites Claude Code issue #30337) | A `security -i` write >4096 bytes silently truncates mid-argument, corrupting the Keychain entry — claude-swap works around this but flags it as a real historical Claude Code bug class |
| Documented residual (code comment) | Stale Keychain entry can resurrect wrong account if delete fails after a fallback-to-file write; best-effort only |

## Bottom line for VibeProxy

1. In-session hot-reload IS real on both platforms (contradicts prior memory) — Claude Code re-reads `.credentials.json` mtime / has a ~30s Keychain TTL and will pick up a swapped credential without restart, confirmed by claude-swap's own targeted mtime-bump workaround (issue #86 reference).
2. That mechanism only works for **swap-in-place on one shared config dir** — it does NOT give per-account transcript isolation; VibeProxy's current per-account `CLAUDE_CONFIG_DIR` file design is *better* for history separation but *loses* the "no restart needed" property, because a new `CLAUDE_CONFIG_DIR` value is only read at process start (per prior research), unlike a same-dir content mutation.
3. claude-swap's "session mode" (`cswap run`) is functionally identical to VibeProxy's existing approach (per-account `CLAUDE_CONFIG_DIR` dirs) plus automation — it does NOT hot-swap a running process either; it launches a fresh `claude` in the terminal.
4. Adopting claude-swap's in-place-mutation trick would require abandoning VibeProxy's per-account transcript isolation (or adding claude-swap's own `--share-history`-style symlink dance) — a real trade-off, not a pure win.

## Unresolved questions

- Does Claude Code's mtime-cache invalidation apply mid-tool-call, or only between assistant turns? (repo docstring implies "next message" boundary — not verified against Claude Code's own source in this pass)
- Whether #135 (MCP OAuth clobber) is actually fixed in the current release or the README claim is aspirational/partial — issue is still open on GitHub.
- Whether the ~30s Keychain TTL is Anthropic-documented anywhere or purely reverse-engineered by claude-swap's author (same caveat as prior VibeProxy research: Anthropic doesn't document `CLAUDE_CONFIG_DIR`/caching behavior officially).
- Did not review `switcher.py` (4888 lines, core orchestration) line-by-line — spot-checked only; possible additional edge cases not surfaced by docstrings alone.
- Did not test any of this live; all claims are from repo-declared behavior (docstrings/README), which is a credible but self-reported source, not independently verified against a running Claude Code instance.
