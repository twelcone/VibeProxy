# VibeProxy Research — Quota Detection & Native macOS Menubar Stack

Date: 2026-07-18

## TL;DR Recommendation

- **Quota signal**: Call Anthropic's undocumented OAuth usage endpoint directly (`GET https://api.anthropic.com/api/oauth/usage`) per-profile, using each profile's Keychain-stored OAuth token. This is corroborated by 3 independent sources (see A.2) and is a live, verified-working call (tested below). Use it as the primary polling signal. Cross-check against real-time 429/`overloaded_error` responses observed on the local proxy as the ground-truth trigger for the actual switch.
- **Architecture**: Build the "Proxy" in VibeProxy's name literally — a local HTTP(S) reverse proxy that Claude Code talks to via `ANTHROPIC_BASE_URL=http://127.0.0.1:PORT`. This gives real-time visibility into 429/rate-limit headers/error bodies AND lets you rewrite the `Authorization` header per active profile to do the actual account switch, without touching Claude Code's own config mid-session.
- **Menubar stack (revised — Tauri v2, Rust)**: `tauri::tray::TrayIconBuilder` for the tray icon with a live-updating title/tooltip showing usage %, `ActivationPolicy::Accessory` (set programmatically in Rust `setup()`, not via `tauri.conf.json`) to hide the Dock icon. Run the local reverse proxy as a genuine `tokio`+`axum`/`hyper` HTTP listener inside the same Rust process (NOT through Tauri's IPC/command layer, which buffers and breaks streaming — see B.2). Non-sandboxed, Developer ID–signed + notarized `.app` for macOS (primary target); cross-platform to Windows later is architecturally supported by Tauri but out of deep-dive scope here per the coordinator's steer.

---

## PART A — Usage & Quota Detection

### A.1 Local usage logs (`~/.claude/projects/**/*.jsonl`)

Verified directly on this machine — confirmed real, not hypothetical:

- Path pattern: `~/.claude/projects/<slugified-cwd>/<session-uuid>.jsonl` (dashes replace `/`, e.g. `-Users-twel-Projects-ECAS`).
- Each line is a JSON event with a `type`: `user`, `assistant`, `attachment`, `system`, `queue-operation`, `file-history-snapshot`, `permission-mode`, `mode`, `last-prompt`.
- `assistant`-type lines carry the token/usage data at `message.usage`:
  ```json
  "usage": {
    "input_tokens": 24078,
    "cache_creation_input_tokens": 17665,
    "cache_read_input_tokens": 16427,
    "output_tokens": 267,
    "server_tool_use": {"web_search_requests":0,"web_fetch_requests":0},
    "service_tier": "standard",
    "cache_creation": {"ephemeral_1h_input_tokens":17665,"ephemeral_5m_input_tokens":0},
    "iterations": [...]
  }
  ```
  Top-level fields also include `message.model`, `timestamp`, `sessionId`, `cwd`, `version`, `requestId`. **No `costUSD` field is written locally** in this Claude Code version (2.1.198) — cost must be computed client-side from token counts × a pricing table (this is exactly what `ccusage` and `claude-code-usage-analyzer` do, pulling LiteLLM pricing data).
- **ccusage** (github.com/ryoppippi/ccusage) is the reference implementation: TS CLI, walks `~/.claude/projects/**/*.jsonl`, dedupes by `requestId`+hash, aggregates by day/session/5h-block, computes cost from a bundled pricing table. Multiple forks exist (`ccusage_go`, `ccusage0`, `claude-code-usage-analyzer`) confirming the JSONL shape is stable across community tooling.
- **claude-monitor** (Maciek-roboblog/Claude-Code-Usage-Monitor) does NOT primarily parse JSONL for rate-limit state — it installs itself as Claude Code's **statusline command** to capture Claude Code's own official `rate_limits` payload (see A.2), falling back to local-log-derived estimates only when that capture is stale/missing. This is the more reliable pattern for VibeProxy to imitate for token accounting, though not for triggering switches (statusline only fires while the CLI is interactively rendering).
- **Verdict**: Local JSONL is good for historical cost/token analytics and a UI "usage history" screen, but it is a *lagging, best-effort, per-session-derived* signal — it cannot see actual server-side quota state (utilization %, reset timestamps) and does not tell you when the account is rate-limited. Use it for analytics, not for the switch trigger.

Sources: [ryoppippi/ccusage](https://github.com/ryoppippi/ccusage), [aarora79/claude-code-usage-analyzer](https://github.com/aarora79/claude-code-usage-analyzer), [Maciek-roboblog/Claude-Code-Usage-Monitor](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor), and direct inspection of local files on this machine (`~/.claude/projects/-Users-twel-Projects-ECAS/*.jsonl`).

### A.2 Rate limits & quota windows — the real endpoint (verified live)

Three independent, cross-corroborating sources converge on the same data model:

1. **This project's own existing tooling** — `/Users/twel/Projects/VibeProxy/.claude/hooks/lib/usage-limits-cache.cjs` (already in the repo, part of the `agentkit`/`ck` engineer kit, unrelated to this research task but a real primary source) implements exactly this:
   - Reads the OAuth access token from macOS Keychain: `security find-generic-password -s "Claude Code-credentials" -w` → JSON → `.claudeAiOauth.accessToken`. Falls back to `~/.claude/.credentials.json` on non-macOS.
   - Calls `GET https://api.anthropic.com/api/oauth/usage` with headers:
     ```
     Accept: application/json
     Authorization: Bearer <accessToken>
     anthropic-beta: oauth-2025-04-20
     ```
   - Parses response `.five_hour.utilization` and `.seven_day.utilization` (0–100 or fractional; code normalizes both).
   - Explicitly disables itself if `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` / `ANTHROPIC_API_KEY` are set in env (`hasAnthropicRuntimeOverride`) — it assumes overridden base URL ⇒ not a real subscription. **This matters**: if VibeProxy sets `ANTHROPIC_BASE_URL` for Claude Code, Claude Code's own internal quota-eligibility heuristics (and thus statusline `rate_limits`, see below) will likely also go dark, because Claude Code applies the same "override present ⇒ skip subscription quota" logic internally. VibeProxy's own menubar process is unaffected — it holds Keychain credentials directly per profile and calls the endpoint itself, independent of Claude Code's env.

2. **Live verification** — I ran this exact call against the real endpoint using this machine's actual Keychain credential (read-only GET, harmless):
   ```
   GET https://api.anthropic.com/api/oauth/usage → HTTP 200
   ```
   Real response shape (fields relevant to VibeProxy in bold):
   ```json
   {
     "five_hour": {"utilization": 9.0, "resets_at": "2026-07-18T22:49:59.93Z", "limit_dollars": null, "used_dollars": null, "remaining_dollars": null},
     "seven_day": {"utilization": 24.0, "resets_at": "2026-07-23T02:59:59.93Z", ...},
     "seven_day_oauth_apps": null, "seven_day_opus": null, "seven_day_sonnet": null,
     "extra_usage": {"is_enabled": false, "monthly_limit": 10000, "used_credits": 5273.0, "utilization": 52.73, "disabled_reason": "out_of_credits", ...},
     "limits": [
       {"kind": "session", "group": "session", "percent": 9, "severity": "normal", "resets_at": "...", "scope": null, "is_active": false},
       {"kind": "weekly_all", "group": "weekly", "percent": 24, "severity": "normal", "resets_at": "...", "scope": null, "is_active": false},
       {"kind": "weekly_scoped", "group": "weekly", "percent": 29, "severity": "normal", "resets_at": "...", "scope": {"model": {"display_name": "Fable"}}, "is_active": true}
     ],
     "spend": {"used": {"amount_minor": 5273, "currency": "USD", "exponent": 2}, "limit": {"amount_minor": 10000}, "percent": 53, "enabled": false, "disabled_reason": "out_of_credits"},
     "member_dashboard_available": false
   }
   ```
   The `limits[]` array with `kind`/`percent`/`severity`/`resets_at`/`is_active` is the richest structured signal — `severity` looks like it escalates (e.g. `normal` → presumably `warning`/`critical`/exhausted at higher percent, unconfirmed at 100%, see Unresolved). `is_active` flags which limit is currently the binding constraint.

3. **Claude Code's statusline JSON** (`code.claude.com/docs/en/statusline`, and multiple 2026 blog write-ups) exposes the *same* `five_hour`/`seven_day` shape to any script configured as the statusline command:
   ```json
   { "model": {...}, "context_window": {...}, "cost": {"total_cost_usd": ...},
     "rate_limits": {
       "five_hour": {"used_percentage": 9, "resets_at": "..."},
       "seven_day": {"used_percentage": 24, "resets_at": "..."}
     }
   }
   ```
   Added in Claude Code v1.2.80. Empty when logged in via bare API key (only populated for Pro/Max subscription OAuth). This is what `claude-monitor` captures by installing itself as the statusline command — an alternative, zero-extra-HTTP-call way to get the same numbers, but it only updates when Claude Code re-renders the statusline (i.e., only while a CLI session is interactively active), so it's a weaker signal for a menubar app that wants to poll independently of whether any CLI session is open.

**Window mechanics** (confirmed across `platform.claude.com/docs/en/api/rate-limits`, Anthropic support articles, and multiple 2026 explainer posts): Claude.ai/Claude Code Pro/Max subscriptions enforce a **5-hour rolling session window** plus, since 2025, an independent **weekly cap** (both must be under threshold; weekly cannot be bypassed by waiting out the 5h window). Max $200 tier gets substantially higher weekly allowance than Pro. These are **subscription-level quotas**, distinct from the classic per-minute `anthropic-ratelimit-*` RPM/TPM headers that apply to API-key/console billing — conflating the two is a common mistake in blog posts; OAuth/subscription usage draws against the session+weekly quota model above, not the RPM/TPM model (GitHub issue anthropics/claude-code#43333 discusses this distinction; `claude -p` with OAuth still bills against subscription quota, not API pay-per-token).

**Is there an official "remaining quota" endpoint?** Yes — `/api/oauth/usage`, confirmed live above. It's undocumented (not in `platform.claude.com/docs`), but it's exactly what Anthropic's own `/usage` slash-command and this repo's existing hook rely on, so it's a "used-in-production, not officially published" API — moderate stability risk (see A.2 risk note below), but currently the best source of truth without local-log inference.

Sources: [platform.claude.com/docs/en/api/rate-limits](https://platform.claude.com/docs/en/api/rate-limits), [Claude Code statusline docs](https://code.claude.com/docs/en/statusline), [Maciek-roboblog/Claude-Code-Usage-Monitor](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor), local file `/Users/twel/Projects/VibeProxy/.claude/hooks/lib/usage-limits-cache.cjs`, live curl verification (this session), [anthropics/claude-code#43333](https://github.com/anthropics/claude-code/issues/43333).

### A.3 Detecting "out of quota" — concrete signal & approaches

**Concrete signals, from `code.claude.com/docs/en/errors` (Claude Code's own error reference)**:

| Condition | Signal | Reset info |
|---|---|---|
| Per-key/org rate limit | HTTP 429, message `API Error: Request rejected (429)` | none structured; standard `anthropic-ratelimit-*` headers + `retry-after` on the raw HTTP response (RPM/TPM model) |
| Server overloaded (not your quota) | HTTP 529, `API Error: Repeated 529 Overloaded errors` | none — transient, auto-retried, explicitly NOT counted against usage |
| **Session (5h) quota exhausted** | Plan-level message: `You've hit your session limit · resets 3:45pm` | human-readable time only in the CLI message; structured `resets_at` ISO timestamp available via `/api/oauth/usage` (`five_hour.resets_at`) or statusline `rate_limits.five_hour.resets_at` |
| **Weekly quota exhausted** | `You've hit your weekly limit · resets Mon 12:00am` | same — structured via `seven_day.resets_at` |
| Opus-specific sub-limit | `You've hit your Opus limit · resets 3:45pm` | via `limits[]` array, `kind` field |
| Extended context credits needed | `API Error: Usage credits required for 1M context` | n/a |
| Prepaid credit exhaustion (console orgs) | `Credit balance is too low` | via `spend.disabled_reason` |

Claude Code auto-retries 429/529 up to 10x (`CLAUDE_CODE_MAX_RETRIES`) with exponential backoff; a `CLAUDE_CODE_RETRY_WATCHDOG=1` mode retries indefinitely for CI. The actual HTTP error body for a session/weekly quota exhaustion (as opposed to raw 429) is presumably a distinct `error.type` (likely something like `rate_limit_error` with a subscription-specific message) — **the exact machine-parseable `error.type`/`error.message` JSON body for a quota-exhausted response was not directly observed in this research** (no local log captured a real exhaustion event on this machine at research time); this is the main unresolved item (see bottom).

**Enumerated approaches to observe the "out of quota" state:**

| Approach | Mechanism | Pros | Cons | Feasibility |
|---|---|---|---|---|
| **A. Local HTTP proxy** (recommended) | `ANTHROPIC_BASE_URL=http://127.0.0.1:PORT`; VibeProxy terminates TLS locally, forwards to `api.anthropic.com`, inspects every response (status code, error body, headers) before relaying to Claude Code | Real-time, sees ground-truth 429/error bodies as they happen, zero polling latency, also gives you the hook point to rewrite `Authorization` per active profile for the actual switch — the proxy IS the switch mechanism | Requires Claude Code process restart to pick up new `ANTHROPIC_BASE_URL` (env read once at startup — confirmed via docs); must handle TLS (Claude Code talks HTTPS — proxy can be plain HTTP since it's loopback, Claude Code doesn't validate that URL's scheme against a pinned cert as long as you set `http://`); must passthrough SSE streaming untouched | High — this is the natural fit for "VibeProxy" and gives the best signal quality |
| B. Poll `/api/oauth/usage` on a timer | Menubar app calls the endpoint every N seconds/minutes per profile using each profile's Keychain token | Simple, no interference with Claude Code process, works even when proxy isn't wired up yet | Undocumented endpoint (could change), polling lag (won't catch mid-burst exhaustion instantly), still doesn't give a live "just got 429" event, uses your own quota-eligible request budget (minor) | High — good as the *baseline/always-on* signal even if proxy is also used |
| C. Tail `~/.claude/projects/**/*.jsonl` | `FSEvents`/`kqueue` watch on the dir, parse new `assistant` lines for `usage` | Free (no extra network calls), gives historical token trend | No cost field, no quota %, no rate-limit signal at all — pure token counting, must reimplement pricing table, high maintenance (pricing changes) | Low value alone; useful only for a "usage history" screen, not for detection |
| D. Tail Claude Code process stdout/stderr | Spawn/wrap `claude` CLI, or read its statusline output if VibeProxy sets itself as the statusline command | Statusline gives official `rate_limits` for free | Only updates while CLI actively renders (interactive session open); requires VibeProxy to own the user's statusline config (conflicts with user's own statusline customizations); doesn't work for headless `claude -p` runs | Medium — good supplementary source, not sufficient alone |
| E. Full MITM/system-wide proxy (e.g. via `/etc/hosts` + self-signed CA) | Intercept ALL traffic to `api.anthropic.com` system-wide | Not needed — Claude Code respects `ANTHROPIC_BASE_URL` directly | Massive complexity/trust cost (installing a root CA), no benefit over A | Not recommended |

**Recommended design**: Combine **A (proxy, primary real-time trigger)** with **B (endpoint polling, baseline/pre-emptive signal for the menubar % bars and for catching exhaustion for a profile Claude Code isn't currently pointed at)**. Use C only for an optional analytics/history view, not for detection logic.

### A.3.1 Proxy mechanics detail (since this is the project's namesake approach)

- **Base URL override**: Claude Code reads `ANTHROPIC_BASE_URL` (and `ANTHROPIC_AUTH_TOKEN`/`ANTHROPIC_API_KEY`) once at process start from env or `~/.claude/settings.json`'s `env` block. VibeProxy should write/manage this in the user's Claude Code settings (or launch `claude` itself with the env set) — **mid-session env changes do nothing**; switching requires either restarting the Claude Code process or (cleaner) never restarting Claude Code and instead doing all the switching *inside* the proxy by rewriting the outbound `Authorization` header per request based on which profile is "active" in VibeProxy's own state. The latter is the clean design: point `ANTHROPIC_BASE_URL` at VibeProxy once, forever; profile switching becomes purely a VibeProxy-side pointer change, invisible to Claude Code.
- **Auth header rewriting**: Proxy strips whatever `Authorization`/`x-api-key` Claude Code sends (likely a placeholder, since real auth now lives in VibeProxy) and substitutes the active profile's real Bearer token (from Keychain) before forwarding upstream to `https://api.anthropic.com`. On upstream 429/session-limit response, proxy flips its internal "active profile" pointer to the next eligible profile (per B's polled state) and, depending on design, either (a) returns the 429 to Claude Code so its own retry/backoff kicks in and the *next* retry uses the new profile, or (b) transparently retries the same request against the new profile before responding (smoother UX, more complex, risk of double-billing/side-effects on non-idempotent calls — favor (a) for simplicity, YAGNI).
- **Streaming SSE passthrough**: `/v1/messages` streaming responses are `text/event-stream`; the proxy must not buffer the response — pipe chunks through as received and must not alter `Content-Length`/`Transfer-Encoding` framing. This is a well-trodden pattern for any async HTTP stack; the main gotcha is disabling any automatic response buffering/gzip re-encoding in the HTTP client library. See B.2 for the concrete Rust/Tokio/axum/reqwest implementation of this.
- **TLS**: proxy listens on `http://127.0.0.1:PORT` (loopback, plaintext is fine — no traffic leaves the machine unencrypted); it makes real HTTPS calls upstream to `api.anthropic.com` via the app's HTTP client. No need for a custom CA or system trust store changes.

### A.4 Auto-switch trigger design (high level; credentials handling is out of scope per prompt)

1. VibeProxy maintains an ordered list of profiles with cached quota state (from B's poller): `{ profileId, fiveHourPercent, weeklyPercent, resetsAt, eligible }`.
2. Trigger conditions to advance to next profile: (a) proxy observes a real 429/quota-exhaustion response for the currently-active profile, or (b) poller reports `percent >= threshold` (e.g. 95–100%) for the active profile before a request is even made (pre-emptive switch, avoids user hitting a failed turn at all).
3. Selection: pick the next profile in priority order whose cached `fiveHourPercent < threshold` (and, if relevant, whose weekly isn't exhausted); if none eligible, surface a menubar alert with the earliest `resets_at` across all profiles rather than looping/erroring silently.
4. Switch = update the proxy's "active profile" pointer + refresh menubar UI; no Claude Code restart needed given the design in A.3.1.
5. Post-switch, re-poll the newly active profile immediately to update its badge and confirm eligibility before relying on it further.

---

## PART B — Native macOS Menubar App Stack (Tauri v2, Rust)

Stack decision (from coordinator, not re-litigated here): **Tauri v2, Rust backend + web UI**, open-source, non-sandboxed, macOS primary target, Windows-portable later. Profiles = Claude subscription (Pro/Max) OAuth logins. Detection = local proxy (Part A, unchanged).

### B.1 Tauri tray/menubar (v2 API)

- Tray icon: `tauri::tray::TrayIconBuilder::new()` (Rust, in `setup()`), built with an `.icon()`, optional `.menu()`, and `.on_tray_icon_event()` for click handling. This is the v2 replacement for v1's `SystemTray`.
- **Live-updating menubar text/percentage**: on macOS the tray supports a native title string alongside the icon (like `NSStatusItem.button.title`). JS-side API: `trayIcon.setTitle("37%")`; Rust-side equivalent via the `TrayIcon` handle's `set_title()`. For menu *item* text inside the dropdown (e.g. per-profile usage rows), each `MenuItem`/`CheckMenuItem` handle exposes `set_text()` and can be updated live from a background Tokio task without rebuilding the whole menu — rebuilding (`set_menu()`) is only needed when adding/removing/reordering items or changing item type. This maps directly onto VibeProxy's need: keep a stashed `MenuItem` handle per profile, update `set_text()` on each poll tick.
- **Hiding the Dock icon**: **not** a `tauri.conf.json` setting — must be done in Rust: `#[cfg(target_os = "macos")] app.set_activation_policy(tauri::ActivationPolicy::Accessory);` inside `.setup()`. This is the Tauri/AppKit equivalent of SwiftUI's `LSUIElement`. Can also be toggled at runtime via `app_handle.set_activation_policy(...)` if you ever want to show a Dock icon conditionally (e.g., first-run onboarding window) — not required for MVP.
- Multiple independent community write-ups (DEV Community "Complete Guide to Building a macOS Menu Bar App with Tauri v2", "Building a Menubar App with Tauri v2 — What Nobody Tells You") converge on this same pattern, plus official docs — good corroboration for a fairly young v2 API surface.

Sources: [Tauri v2 — System Tray](https://v2.tauri.app/learn/system-tray/), [Tauri v2 — Window Customization](https://v2.tauri.app/learn/window-customization/), [tauri-apps/tauri discussion #10774 — toggle dock icon](https://github.com/tauri-apps/tauri/discussions/10774), [DEV — Complete Guide to Building a macOS Menu Bar App with Tauri v2](https://dev.to/hiyoyok/complete-guide-to-building-a-macos-menu-bar-app-with-tauri-v2-aji).

### B.2 Running the local proxy inside the Tauri Rust process

**Critical gotcha, confirmed via search**: do NOT implement the proxy as a Tauri IPC command/event pair (frontend `invoke()` → Rust command → forward request). At least one documented case (`proxy_localhost_stream`) shows this pattern **buffers the entire upstream response before returning it** — fatal for SSE streaming, since Claude Code (and the Claude UI, if ever proxied) needs tokens to arrive incrementally, not all-at-once after the full response completes.

**Correct approach**: run a real, independent HTTP server inside the same Rust process, bound to `127.0.0.1:PORT`, using the Tokio async runtime — completely separate from Tauri's webview IPC layer. Tauri already depends on Tokio internally, so no extra runtime is needed; spawn the server as a task on app startup (`tauri::async_runtime::spawn` or a dedicated `tokio::spawn` inside `.setup()`).

- **Recommended crates**: `axum` (built on `hyper`+`tokio`, first-party SSE support via `axum::response::sse`, ergonomic routing) for the listener; `reqwest` (also `hyper`-based, supports streaming request/response bodies via `.body(reqwest::Body::wrap_stream(...))` and `.bytes_stream()` on the response) for the upstream call to `api.anthropic.com`. This axum-in, reqwest-out pairing is the standard Rust reverse-proxy pattern — `axum-reverse-proxy` (crates.io) and `tokio-rs/axum`'s own `examples/reverse-proxy` demonstrate exactly this shape and are worth using as a structural reference, even if not pulled in as a dependency (VibeProxy's proxy needs custom header-rewrite logic per active profile, which a generic reverse-proxy crate would need to be configured for anyway — copying the pattern is more KISS than depending on a niche crate for a ~100-line proxy handler).
- **SSE passthrough mechanics**: read the upstream `reqwest::Response` as a byte stream (`.bytes_stream()`), forward chunks to the axum response body as they arrive (`axum::body::Body::from_stream(stream)` or a manual SSE event mapper) — do not `.text().await`/`.bytes().await` the whole response first, that's exactly the buffering bug to avoid.
- **Threading model**: the HTTP listener task and the Tauri app/webview event loop coexist fine — Tauri's own runtime is Tokio-based, so the proxy server is just another set of tasks on the same executor. Use `tauri::Manager`/app state (`Arc<Mutex<...>>` or better, `tokio::sync::watch`/`RwLock`) to share "currently active profile" state between the Tauri command handlers (UI-triggered profile switch) and the proxy's request-handling tasks (reads active profile per incoming request to pick which Keychain-derived Bearer token to inject).

Sources: [tokio-rs/axum — reverse-proxy example](https://github.com/tokio-rs/axum/blob/main/examples/reverse-proxy/src/main.rs), [axum-reverse-proxy crate](https://crates.io/crates/axum-reverse-proxy), [DEV — Rust Axum Streaming Response: 6 Production Patterns from SSE to WebSocket](https://www.toolsku.com/en/blog/rust-axum-streaming-response-2026), community report of Tauri IPC buffering issue with `proxy_localhost_stream` (surfaced via search, exact source thread not independently re-verified — treat as a strong warning sign, not gospel; validate empirically early in implementation).

### B.3 Reading Claude OAuth credentials cross-platform

- **macOS**: `security-framework` crate — official-ish Rust binding to Apple's `Security.framework` (the same framework Swift's `Security` module and the `security` CLI both wrap). Its `passwords` module exposes a generic-password lookup by service+account, i.e. the Rust equivalent of `security find-generic-password -s "Claude Code-credentials" -w` (confirmed working shape via this project's own JS hook, Part A.2) — same underlying keychain item, same `errSecItemNotFound` semantics if missing. No need to shell out to `/usr/bin/security`; call the framework directly for better error handling and no subprocess spawn.
- **Windows (feasibility note only, per coordinator's scope)**: the `keyring` crate (open-source-cooperative/keyring-rs) provides a cross-platform credential-store abstraction, backed on Windows by `windows-native-keyring-store` (wraps Credential Manager, mapping each entry to a generic credential). Claude Code on Windows would need to be confirmed to actually store its OAuth token in Windows Credential Manager under an equivalent service name (not verified in this research pass — flagged as unresolved, low priority since macOS is primary target). If it does, `keyring` gives a single Rust API surface for both platforms with platform-specific backends swapped at compile time — clean fit for a cross-platform Tauri app.

Sources: [security-framework crate docs](https://docs.rs/security-framework), [security-framework passwords module](https://docs.rs/security-framework/latest/security_framework/passwords/index.html), [keyring crate](https://docs.rs/keyring), [windows-native-keyring-store](https://docs.rs/windows-native-keyring-store), local file `/Users/twel/Projects/VibeProxy/.claude/hooks/lib/usage-limits-cache.cjs` (confirms the macOS keychain item name/shape to target).

### B.4 Launch-at-login and notarization/signing

- **Launch at login**: `tauri-plugin-autostart` (v2: `tauri-plugin-autostart = "2.0.0"` in `Cargo.toml`, `@tauri-apps/plugin-autostart` JS bindings). Register in Rust via `tauri_plugin_autostart::Builder::new().app_name("VibeProxy").build()` added as a `.plugin(...)` on the Tauri builder; toggle from the frontend via `enable()`/`disable()`/`isEnabled()`. On macOS it supports both AppleScript and Launch Agent registration methods (LaunchAgent is the modern/preferred one, roughly equivalent to what `SMAppService` does natively — the plugin abstracts this so you don't hand-roll `SMAppService` FFI bindings from Rust). Cross-platform (Linux/Windows/macOS) for free, fitting the stated future-Windows goal.
- **Signing & notarization** (macOS, Developer ID, open-source distribution — no App Store): set `bundle.macOS.signingIdentity` in `tauri.conf.json` (e.g. `"Developer ID Application: <Name> (<TEAMID>)"`) or the `APPLE_SIGNING_IDENTITY` env var; enable `hardenedRuntime: true`. `tauri build` handles codesigning as part of the bundle step when a valid identity is available in the keychain/CI secrets. Notarization is a separate, required step for Developer ID–signed apps distributed outside the App Store — driven by `APPLE_ID`/`APPLE_PASSWORD` (app-specific password) or `APPLE_API_KEY` env vars that Tauri's CLI picks up automatically to call Apple's notary service; `--skip-stapling` exists for iterating without waiting on Apple's notarization turnaround during dev. A free Apple Developer account cannot notarize — a paid Developer Program membership ($99/yr) is required for a clean Gatekeeper-trusted release, worth flagging as a real cost/prerequisite for shipping this open-source app to non-technical users (technical users can always right-click-Open an unnotarized build, but that's a worse first-run experience).
- **Sandbox**: same conclusion as before, restack-independent — App Sandbox is incompatible with reading `~/.claude/**` and cross-app Keychain access regardless of UI framework; Tauri apps are unsandboxed by default for Developer ID distribution, which is the correct/only viable posture here. No Tauri-specific sandbox entitlement work needed.

Sources: [Tauri v2 — Autostart plugin docs](https://v2.tauri.app/plugin/autostart/), [tauri-apps/tauri-plugin-autostart](https://github.com/tauri-apps/tauri-plugin-autostart), [Tauri v2 — macOS Code Signing](https://v2.tauri.app/distribute/sign/macos/), [DEV — Shipping a Production macOS App with Tauri 2.0: Code Signing, Notarization, and Homebrew](https://dev.to/0xmassi/shipping-a-production-macos-app-with-tauri-20-code-signing-notarization-and-homebrew-mc3).

---

## Trade-off Matrix — Quota Detection Approaches

| Dimension | A. Local proxy (real-time) | B. Poll `/api/oauth/usage` | C. Tail JSONL logs | D. Own the statusline |
|---|---|---|---|---|
| Real-time accuracy | High (ground truth) | Medium (polling lag) | Low (no quota %, tokens only) | Medium (only while CLI open) |
| Implementation complexity | High (HTTP proxy, SSE, header rewrite) | Low (single fetch) | Medium (file watch + pricing table) | Low, but conflicts w/ user's own statusline |
| API stability risk | Depends on Anthropic API compat (documented-ish) | Undocumented endpoint, could break | Stable (JSONL schema has been consistent) | Documented but young (v1.2.80+) feature |
| Works headless (`claude -p`, CI) | Yes | Yes | Yes | No |
| Enables the actual account switch | Yes (this IS the switch mechanism) | No (detection only) | No | No |
| Maintenance burden | Medium-high (SSE edge cases, upstream API changes) | Low | Medium (pricing table drift) | Low, until user customizes statusline |

**Ranked recommendation**: A (primary, does double duty as detector + switcher) + B (baseline poller/pre-emptive signal, cheap insurance) together. C is optional/nice-to-have for a history view. D is not recommended as a primary source — too fragile an ownership model over a user-facing CLI feature that isn't VibeProxy's to control.

## Architectural Fit

- Project is greenfield, open-source, macOS-primary-with-future-Windows. Tauri v2 (Rust backend + web UI) + in-process axum/tokio HTTP server is a coherent single-binary stack (one language for backend+proxy, one signing identity for macOS distribution) and directly serves the stated Windows-portability goal — a Rust proxy core is trivially reused, only the tray/keychain integration needs per-OS branches (Part B.1/B.3). Honors KISS/YAGNI over a bundled separate-process proxy sidecar.
- The project's own existing `.claude/hooks/lib/usage-limits-cache.cjs` is proof this exact endpoint+Keychain approach already works reliably on this developer's machine today — de-risks A.2 considerably; this isn't a cold-start unknown. Its logic (endpoint URL, headers, Keychain service name `"Claude Code-credentials"`) should be ported near-verbatim into Rust via `security-framework` + `reqwest`.
- Tauri v2's tray/menu APIs and the ecosystem write-ups for them are all 2025-2026 vintage — younger and thinner than SwiftUI's `MenuBarExtra` docs would have been, but corroborated by multiple independent sources (official docs + 2+ community deep-dives) and by the fact Tauri itself is a mature, widely-adopted framework (Tauri is not the risk; the tray/dock-hiding/streaming specifics are the newer surface — treat B.1/B.2 as the parts to prototype first).

## Unresolved Questions

1. Exact JSON `error.type`/body for a subscription-quota-exhausted response (as opposed to raw 429) was not directly observed — no real exhaustion event was available to capture during this research. Needs to be captured empirically during implementation/testing (e.g., deliberately exhaust a test account, or scrape more GitHub issue threads with pasted error bodies).
2. Whether `/api/oauth/usage`'s `limits[].severity` field has documented values beyond `"normal"` (e.g., a `"critical"`/`"exceeded"` state at 100%) — only a 9%/24% utilization sample was available live; behavior at/near 100% is inferred, not observed.
3. Whether Anthropic could rate-limit or block the `/api/oauth/usage` endpoint if VibeProxy polls it frequently across multiple profiles (undocumented endpoint, no published SLA/ToS statement found for this specific route) — recommend conservative polling interval (existing hook uses 60s–5min) and treat failures gracefully.
4. No open-source precedent was found for a Tauri/Rust menubar app that also runs a live Anthropic reverse proxy with per-request credential rewriting — this specific combination is architecturally sound but has no reference implementation to sanity-check against; treat SSE-passthrough and header-rewrite as the highest-implementation-risk components and prototype/test them first.
5. Behavior of `ANTHROPIC_BASE_URL` + custom root/self-signed cert requirements if Anthropic ever pins certs client-side inside Claude Code — not observed as an issue in current docs/discussions, but worth a smoke test early.
6. The Tauri-IPC-buffers-SSE claim (B.2) was surfaced via a single search snippet referencing a `proxy_localhost_stream` pattern, not independently cross-verified against a primary source/issue thread — the underlying advice (run a real axum/tokio listener, don't proxy through Tauri commands) is sound Rust-networking practice regardless, but the specific claim should be spot-checked once implementation starts.
7. Windows Credential Manager storage of Claude's OAuth token (service/account naming) was not verified — noted as low-priority per coordinator's macOS-first scope.

Status: DONE
Summary: Confirmed and live-tested Anthropic's undocumented `/api/oauth/usage` OAuth endpoint (via this project's own existing hook script) as the primary quota signal, cross-corroborated by Claude Code's statusline `rate_limits` field and community tools (ccusage, claude-monitor); recommend a local reverse proxy (`ANTHROPIC_BASE_URL` override, in-process Tokio/axum/reqwest server inside the Tauri Rust process) as both the real-time 429 detector and the actual account-switch mechanism, paired with a Tauri v2 tray UI (`TrayIconBuilder`, `ActivationPolicy::Accessory`) distributed unsandboxed via Developer ID (App Sandbox is incompatible with `~/.claude` + cross-app Keychain access).
Concerns: Exact JSON shape of a real quota-exhaustion error response and `severity` field values near 100% utilization were not empirically observed — verify early in implementation. No existing open-source reference for the specific "Tauri menubar app as live Anthropic reverse proxy" combination — prototype the proxy's SSE/streaming path and the Tauri-IPC-buffering avoidance (B.2) first, as the highest-risk components. Paid Apple Developer Program membership ($99/yr) is a real prerequisite for a notarized release, worth confirming is budgeted.
