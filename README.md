# VibeProxy

A native macOS menubar app to switch between multiple **Claude Code** Pro/Max accounts in one
click, watch each account's usage, and **auto-switch** before one runs out of quota.

> Design preview: open [`docs/mockups/vibeproxy-mock.html`](docs/mockups/vibeproxy-mock.html) in a browser.

## Features

- **One-click account switching** between your Claude Code Pro/Max logins (menubar or window)
- **Live usage** in the menubar — a fill-meter of the active account's 5-hour usage, plus 5-hour + weekly bars per account in the window
- **Auto-switch** to the freshest account when the active one crosses a threshold, with a notification and a one-click *Relaunch Claude Code*
- **Add accounts** via the real `claude` login, or import an existing config dir
- Menubar-only (no dock icon), optional launch-at-login, open-source

## How it works

Each account is an isolated `CLAUDE_CONFIG_DIR` with its own real login. VibeProxy switches the active
account by writing its path to `~/.vibeproxy/active-path`; a one-line shell snippet reads that into
`CLAUDE_CONFIG_DIR` so new terminals use it. Usage comes from polling Anthropic's OAuth usage endpoint
per account (read-only).

> **VibeProxy is not an inference proxy.** Despite the name, it never routes or relays your OAuth token
> to the model API — that pattern is against Anthropic's terms. The "proxy"-like role is local usage
> polling + brokering which account is active.
>
> **Honest note:** polling the usage endpoint with your account token is read-only and low-signal, but
> it is still "using the token in a non-official tool" — a small residual risk. Use at your own
> discretion.

## Requirements

- macOS 13+ (Ventura or later)
- [Claude Code](https://claude.com/claude-code) installed and on your `PATH`
- One or more Claude Pro/Max accounts

## Install

VibeProxy is distributed **unsigned** (no Apple Developer Program), so macOS Gatekeeper needs a
one-time approval.

1. Download `VibeProxy_x.y.z_aarch64.dmg` from [Releases](../../releases) and drag the app to Applications.
2. **First launch (macOS 15+):** double-click → macOS blocks it → open **System Settings → Privacy &
   Security**, scroll down, and click **Open Anyway**. (On macOS 13–14 you can right-click the app →
   **Open**.)
   - Terminal alternative: `xattr -dr com.apple.quarantine /Applications/VibeProxy.app`

## Set up Claude Code integration

Add this to your shell profile (e.g. `~/.zshrc`) so new terminals use the active account. VibeProxy
shows it under **Settings → Claude Code integration** with a copy button:

```sh
_vpd="${VIBEPROXY_DIR:-$HOME/.vibeproxy}"; _vp="$(cat "$_vpd/active-path" 2>/dev/null)"; [ -n "$_vp" ] && export CLAUDE_CONFIG_DIR="$_vp" || unset CLAUDE_CONFIG_DIR
```

Open a new terminal after switching accounts (or use the **Relaunch** button). Switching only affects
**new** `claude` launches — a running session keeps its account.

## Headless / CLI (WSL, SSH, servers)

A menubar app can't run where there's no desktop — WSL, an SSH session, a container — but `claude`
does. The `vibeproxy` CLI is the same core without the GUI, so it works anywhere:

```sh
vibeproxy list                 # accounts, * marks the active one
vibeproxy switch work          # by id or label
vibeproxy status               # active account + its 5-hour / weekly usage
vibeproxy usage                # token analytics from your local logs (--json for scripts)
vibeproxy export usage.csv     # the same analytics as CSV
vibeproxy adopt work ~/.vibeproxy/profiles/work
vibeproxy auto                 # switch once if the active account is over the threshold
eval "$(vibeproxy shell-init)" # wire switching into this shell (or --install to persist)
```

Inside WSL this is the *correct* interface, not a fallback: it manages the credentials in the same
Linux userland `claude` uses, with no reaching across the Windows boundary. Read commands take
`--json`; the shapes match the app's, so anything scripting the GUI's data works against the CLI too.

The app and CLI share one Rust core (`vibeproxy-core`) and read the same files, so you can use either
or both. The desktop GUI runs on macOS today; the CLI is the cross-platform surface.

## Usage

- Click the menubar icon → **Open VibeProxy**.
- Your current login is adopted automatically as **Main**. Add more with **Add via login** (completes a
  real browser login) or **Import an existing config dir**.
- Click **Switch** on any account, or let auto-switch handle it. Tune the threshold, poll interval, and
  launch-at-login under **Settings**.

## Build from source

```sh
pnpm install
pnpm tauri dev     # run
pnpm tauri build   # produce an unsigned .app/.dmg in target/release/bundle
```

Requires Rust (stable), Node 20+, pnpm, and Xcode Command Line Tools.

## Privacy & security

- Your OAuth tokens live in the macOS Keychain. VibeProxy reads them to query your own usage, and
  never writes them to a plaintext file or to logs.
- **Switching running sessions** (off by default) is the one feature that *writes* credentials: to
  move a live session to another account it copies that account's token into the Keychain item of
  the directory the session is using, and touches that directory's `.credentials.json` to prompt a
  reload. The displaced account's original token is first snapshotted into a VibeProxy-owned Keychain
  item so it is never lost, and can be restored. With this setting off, VibeProxy only ever *reads*
  credentials.
- VibeProxy never sends your token to any inference endpoint or third party. It talks only to
  Anthropic's own usage endpoint, with your own token, to read your own numbers.
- The shell integration and the "Switch running sessions" toggle write to files you own — your shell
  profile and the Keychain respectively — and only when you enable them.
- Removing an account from VibeProxy leaves its Claude login untouched.

## License

[MIT](LICENSE)
