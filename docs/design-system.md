# VibeProxy — Design System

Source of truth for the UI. Phase 6 (UI/UX) builds against this; don't re-decide these per screen.
VibeProxy is a **native macOS menubar utility** (Tauri + Svelte), not a web page or mobile app —
so it favors information density, the system font, and macOS idioms over web/landing conventions.

Derived from the `ak-ui-ux-pro-max` pass + native-mac review. Where this diverges from that database's
generic output, the reason is recorded (the DB is web/mobile-tuned).

## Principles

1. **Native first.** Feel like it belongs in macOS: system font, vibrancy, subtle shadows — no glows, no web-app chrome.
2. **Scanned, not read.** It's a utility. Surface state (which account, how much quota) at a glance; density over hero whitespace.
3. **State reads without color.** Never rely on hue alone — pair every usage color with a number and, at the limit, a word.
4. **One accent, used sparingly.** Coral marks brand/active/primary only. Usage severity is a *separate* scale.

## Color tokens

Defined as CSS custom properties on `:root`, overridden for dark via `@media (prefers-color-scheme: dark)`
**and** `:root[data-theme="dark"|"light"]` (the in-app/OS theme toggle must win in both directions).

| Token | Light | Dark | Use |
|-------|-------|------|-----|
| `--ground` | `#e7e3dc` | `#161513` | Desktop/window backdrop |
| `--panel` | `#fbfaf8` | `#232120` | Popover / window surface |
| `--panel-2` | `#f1eee9` | `#2c2a27` | Hover / nested surface |
| `--panel-3` | `#e9e5de` | `#35322e` | Chips, code, sliders track |
| `--ink` | `#26231f` | `#f1ede6` | Primary text |
| `--ink-soft` | `#6e675e` | `#a79e92` | Secondary text |
| `--ink-faint` | `#837a6e` | `#928a7e` | Labels (kept ≥3:1 — do not lighten further) |
| `--hair` | `#e3ded5` | `#35312c` | Hairline borders/dividers |
| `--accent` | `#c4623f` | `#e0805c` | **Brand / active / primary CTA only** |
| `--accent-ink` | `#ffffff` | `#221008` | Text/icon on accent |

**Accent rationale:** deliberately a Claude-adjacent **coral**, *not* the database's generic dev-tool
blue (`#2563EB`) + orange. Blue+orange is the templated default; coral ties to Claude Code (the subject)
and stays distinctive. This is a fixed decision.

### Semantic usage scale (separate from accent)

Drives 5-hour / weekly usage bars and rings. **Not** a brand color; never reuse `--accent` for state.

| State | Light | Dark | Threshold |
|-------|-------|------|-----------|
| `--good` | `#3e9b5f` | `#58b776` | < 70% |
| `--warn` | `#cf9422` | `#e3b457` | 70–89% |
| `--crit` | `#ce4530` | `#e0654e` | ≥ 90% (also show a "Near limit" label) |

## Typography

- **UI face:** the macOS system stack — `-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`.
  Deliberately **not Inter** (the DB's suggestion): Inter reads as a web app in a window; SF is native.
- **Mono (technical bits — config paths, keychain service, shortcuts):** `ui-monospace, "SF Mono", Menlo, monospace`.
- **Numbers:** `font-variant-numeric: tabular-nums` everywhere digits appear (usage %, resets, times, activity) so columns don't jitter.
- **Scale (px):** 10 (uppercase labels) · 11–12 (secondary) · 13 (body/rows) · 14 (names) · 15 (titles). Weights: 400 body, 550–600 labels/names, 650–700 emphasis. Uppercase labels get `letter-spacing: .06em`.

## Iconography

- **Icon set: Lucide** (MIT, clean stroke, macOS-appropriate). Inline as SVG — **never emoji** (font-dependent, unthemeable, the #1 "unprofessional" tell).
- Sizes as tokens: `--icon-sm: 16px`, `--icon-md: 20px`. One stroke width (2). `stroke: currentColor` so icons theme automatically.
### Menubar (tray) indicator

The menubar item is ~22pt tall and sits on a light *or* dark background (wallpaper-dependent), so
fine detail is lost. Rules:

- **The `%` number is the primary signal** — always in the menubar's own ink color so it stays legible.
- **Tint the number only at warn/critical** (≥70% amber, ≥90% red); keep it default ink when healthy so
  the menubar stays calm and only demands attention when quota is actually running low.
- **Glyph = fill-meter (decided).** A small battery-style gauge (~20×10px, rounded, 1px inset outline,
  severity-colored fill = 5-hour %), followed by the number. Rejected alternatives: thin ring/donut
  (strokes vanish at menubar scale) and solid dot. Number-only is the acceptable fallback if the meter
  ever proves too busy at real size.
- Draw the meter as a real image (not emoji); accept color mode over macOS template mode since state is color-coded.
- Icons in use: `arrow-left-right` (brand/switch), `plus` (add), `settings-2`, `power` (quit), `zap` (auto-switch), `check` (active), `alert-triangle` + `refresh-cw` (reconnect/relaunch), `grip-vertical` (reorder), `battery`/`wifi`/`command` (menubar chrome mock only).

## Motion

- Micro-interactions **150–300ms**; nothing over ~400ms. Usage bar/ring fills = 300ms.
- Easing: `ease-out` entering, `ease-in` exiting; exits ~60–70% of enter duration.
- Popover: scale+fade from top-right (its trigger). Toast: slide-in from the tray edge.
- Always guard `@media (prefers-reduced-motion: reduce)` → disable transitions/animations.

## Layout & density

- **Menubar popover:** ~320px wide. Comfortable density — rows ~40px, 15px horizontal padding.
- **Main window:** ~560px max-width, single column, sectioned (Accounts / Automatic switching / Integration / Activity).
- **Spacing:** 4/8px rhythm. Section gaps 16–24px.
- Each surface has **one** primary action (accent); everything else is subordinate.
- No horizontal body scroll; wide content (code snippet) scrolls in its own container.

## Accessibility baseline (every screen)

- Text contrast ≥ 4.5:1 (≥ 3:1 for large labels) — **verify dark independently**, don't infer from light.
- Visible focus ring (`:focus-visible`, 2px accent) on all interactive elements; `cursor: pointer` on clickables.
- State never by color alone (usage = color + number + word at the limit).
- Respect `prefers-reduced-motion` and OS text-size where the webview honors it.
- Icon-only controls get an `aria-label`.

## From the `ak-ui-ux-pro-max` pass — decisions

| DB recommendation | Verdict | Why |
|---|---|---|
| Dark Mode (OLED) style for dev tools | **Adopt** (both themes) | Correct for a coding-adjacent tool |
| Accent blue `#2563EB` + orange | **Reject** → coral | Templated default; coral fits the subject |
| Font Inter | **Reject** → system SF | Native app, not a web page |
| "Minimal glow / text-shadow" | **Reject** | Non-native on macOS |
| "Minimal Single Column / hero" landing pattern | **Ignore** | Utility, not a landing page |
| No emoji as icons; cursor-pointer; focus states; 150–300ms; reduced-motion; contrast | **Adopt** | Baseline quality |

## Open questions

- App icon / brand mark: the `arrow-left-right`-in-rounded-square placeholder needs a real designed mark before release.
- Whether the webview honors macOS Dynamic Type / increased-contrast settings (verify in Phase 6).
