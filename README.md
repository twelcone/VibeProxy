# VibeProxy

A native macOS menubar app to switch between multiple **Claude Code** Pro/Max accounts in one
click, track each account's usage, and auto-switch before one runs out of quota.

> **Status: planning.** No app code yet — this repo currently holds the implementation plan.
> See [`plans/260719-0001-vibeproxy-claude-code-profile-switcher/`](plans/260719-0001-vibeproxy-claude-code-profile-switcher/plan.md).

## What it does

- **One-click account switching** between your Claude Code Pro/Max logins
- **Live usage** in the menubar — 5-hour and weekly utilization per account
- **Auto-switch** to a fresh account before the active one hits its limit
- **Menubar-only** (no dock icon), launch-at-login, open-source

## How it works (design summary)

- Each account is an isolated `CLAUDE_CONFIG_DIR` with its own real login. Switching atomically
  repoints which profile the next `claude` launch uses — the ToS-accepted multi-account pattern.
- Usage comes from polling Anthropic's OAuth usage endpoint per profile (read-only).
- **VibeProxy never relays your OAuth token to the inference API.** It is not an inference proxy —
  the "proxy" role is local usage polling and active-profile brokering.

> **Honest note:** polling the usage endpoint with an account token is read-only and low-signal, but
> still "using the token in a non-official tool" — a small residual risk documented in the plan.

## Tech

Tauri v2 (Rust + web UI), unsandboxed, macOS-first with a Windows-portable core.

## Roadmap

The plan is broken into phases 0–7 (starting with a shell-only mechanism spike that validates the
core assumptions before any Rust). See the [plan](plans/260719-0001-vibeproxy-claude-code-profile-switcher/plan.md).

## License

TBD (MIT proposed).
