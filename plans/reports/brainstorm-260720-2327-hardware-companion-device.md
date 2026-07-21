# Brainstorm — VibeProxy Hardware Companion Device

**Date:** 2026-07-20
**Status:** Decided (hardware); endpoint design not yet planned
**Outcome:** Buy Pi Zero 2 W + 3.5" capacitive touch. VibeProxy needs a local read+control endpoint.

## Problem

Desk device showing Claude Code quota, **plus interaction** — tap to switch active account, multiple
views. Not display-only. Question was Pi Zero 2 W vs ESP32.

## Requirements (as clarified)

| # | Requirement |
|---|---|
| 1 | Show quota (5h + weekly) across **all** VibeProxy accounts |
| 2 | **Tap to switch** active account from the device |
| 3 | Multiple views; scope expected to grow ("view other things") |
| 4 | Always-on, unattended for weeks |
| 5 | User is a software dev, less confident w/ electronics — soldering is a cost |

## Approaches evaluated

| Approach | Verdict | Why |
|---|---|---|
| ESP32 (T-Display-S3 / CYD) | **Rejected** | Every new view = hand-written C/LVGL. Throws away existing Svelte UI. Fine for fixed scope; scope isn't fixed. |
| Pi Zero 2 W + 3.5" touch | **Chosen** | Python/pygame or Chromium kiosk. Possible Svelte reuse. User buying a Pi anyway for other projects. |
| Pi 4/5 + 5–7" touch | Rejected (this round) | Comfortable kiosk + full Svelte reuse, but bigger/pricier than warranted to start. |

### Why interactivity flipped the decision

Display-only = fixed scope → microcontroller wins (cheaper, more reliable, no SD).
Interaction + growing views = unbounded UI work → general-purpose computer wins.
VibeProxy already has a Svelte frontend + design system; a browser kiosk reuses it.

**Caveat recorded:** Zero 2 W is 512MB. Chromium kiosk = sluggish load, janky transitions — but the
workload (one light page, 60s refresh, rare taps) is unusually kiosk-tolerant. **Test kiosk first**;
fall back to pygame (~250 LOC, per fuziontech prior art) only if it's actually bad.

## Prior art (searched 2026-07-20)

Space is crowded — 7+ projects. Three architectures:

| Architecture | Example | Fits our case? |
|---|---|---|
| Device polls Anthropic directly | [fuziontech/claude-quota-display](https://github.com/fuziontech/claude-quota-display) (Pi, pygame, supports Zero 2 W) | **No** — read-only, can't switch accounts |
| Host daemon → BLE push | [Clawdmeter](https://github.com/HermannBjorgvin/Clawdmeter) (scrapes `anthropic-ratelimit-unified-5h-utilization` headers, ~1 Haiku token/poll) | Partial |
| Host bridge → LAN HTTP, device polls | [rootedlab monitor](https://github.com/rootedlab-code/claude-code-usage-monitor) (bearer token 0600, `:8787`) | **Yes** — matches our design |

Others: [claude-usage-stick](https://github.com/oauramos/claude-usage-stick), [claude-dashboard](https://github.com/eriktaveras/claude-dashboard), [ClaudeGauge](https://www.hackster.io/dorofino/claudegauge-real-time-ai-usage-monitor-on-esp32-s3-with-a-a82d4b), [CYD companion](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor/issues/198).

**Key gap:** none handle multiple accounts. rootedlab's bridge explicitly doesn't; fuziontech reads
one `~/.claude/.credentials.json`. **Multi-account aggregation is VibeProxy's actual differentiator** —
and the real justification for building an endpoint. Not "getting numbers on a screen."

## VibeProxy implication — endpoint required

Direct-poll architecture cannot satisfy requirement 2: switching writes `~/.vibeproxy/active-path`,
only the Mac can do that. So:

- `GET /quota` — all accounts aggregated (superset of existing `get_usage` / `get_usage_analytics`)
- `POST /active` — switch active profile (wraps existing `activate()`)

Read side is mostly existing Tauri command output. Write side is new and security-relevant.

## Risks

1. **Control surface on LAN.** `POST /active` changes which account every terminal uses. Needs bearer
   token + bind to LAN iface (not `0.0.0.0`) + explicit opt-in. Higher stakes than a read-only feed.
2. **README contradiction.** README currently promises VibeProxy talks to nothing external. An HTTP
   listener changes the security story — must be documented + opt-in, default off.
3. **Credential refresh race.** fuziontech's Pi app refreshes the OAuth token and writes back to
   `~/.claude/.credentials.json`. If run against a VibeProxy-managed account concurrently → possible
   token invalidation. Mitigation: point it at an unmanaged config dir, or disable its refresh path.
4. **Pi always-on failure modes.** SD corruption on power blips → **overlayfs read-only root** (single
   highest-value mitigation, a `raspi-config` toggle). Plus systemd `Restart=always`, hardware
   watchdog, volatile journald.
5. **Hardware ordering gotchas.** Zero 2 W ships **without GPIO header** (need WH variant or hammer
   header — else 40 pins of soldering). Waveshare 3.5" comes resistive *or* capacitive — resistive is
   bad for finger taps. Zero is **micro-USB** power + mini-HDMI.

## Success criteria

- [ ] Device shows all VibeProxy accounts' 5h + weekly quota, refreshed ≤60s
- [ ] Tap an account on device → Mac's active profile changes → new terminals pick it up
- [ ] Endpoint off by default; token-authed; refuses non-LAN origins
- [ ] Survives a week unattended incl. a power cycle and a WiFi drop
- [ ] README security section updated to reflect the listener

## Next steps

1. Order parts (WH variant + capacitive screen).
2. Finish Analytics Phase 4–5 (in flight) *or* branch to endpoint design — user's call.
3. Brainstorm/plan the endpoint properly before implementing — bind address, auth, opt-in UX, JSON
   shape, whether `POST /active` ships at all in v1.

## Unresolved questions

- Endpoint before or after Analytics Phase 4–5?
- Does `POST /active` ship in v1, or read-only first and control later?
- Exact Waveshare 3.5" SKU (capacitive 640×480 per fuziontech — confirm current part number).
- Is Zero 2 W "WH" (pre-soldered header) currently available, or is a hammer header needed?
- Kiosk vs pygame — decide empirically after first boot.
