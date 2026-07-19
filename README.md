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
_vp="$(cat ~/.vibeproxy/active-path 2>/dev/null)"; [ -n "$_vp" ] && export CLAUDE_CONFIG_DIR="$_vp" || unset CLAUDE_CONFIG_DIR
```

Open a new terminal after switching accounts (or use the **Relaunch** button). Switching only affects
**new** `claude` launches — a running session keeps its account.

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
pnpm tauri build   # produce an unsigned .app/.dmg in src-tauri/target/release/bundle
```

Requires Rust (stable), Node 20+, pnpm, and Xcode Command Line Tools.

## Privacy & security

- Your OAuth tokens stay in the macOS Keychain; VibeProxy reads them only to query your own usage and
  never writes them to disk or logs.
- VibeProxy never sends your token to any inference endpoint or third party.
- Removing an account from VibeProxy leaves its Claude login untouched.

## License

[MIT](LICENSE)
