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

### Categorical series palette (charts only)

Chart series (per-model, per-account, per-project) need colors that are **neither** the accent (brand/
active) **nor** the usage scale (quota severity) — reusing either would make a chart color read as
state. Six hues, each ≥4.5:1 on `--panel` in its own theme, cycled if a chart has more than six series.

| Token | Light | Dark |
|-------|-------|------|
| `--series-1` | `#2a716d` | `#5cb4ae` |
| `--series-2` | `#4b5bb5` | `#8b97e0` |
| `--series-3` | `#8a4a86` | `#c187bd` |
| `--series-4` | `#5f6b28` | `#a8b95c` |
| `--series-5` | `#b03a5b` | `#e0748f` |
| `--series-6` | `#46647d` | `#86a6bf` |

**Every series is directly labeled** — color is reinforcement, never the sole carrier of meaning
(colorblind safety + the "state reads without color" principle). Access via `seriesColor(i)` in
`src/lib/series-palette.ts`.

### Token location

All tokens live in `src/lib/styles/tokens.css`, imported once by the root `+layout.svelte` so every
window (main popover, Usage Analytics) shares one source. Don't redeclare them per component.

## Dashboard components (Usage Analytics)

- **`KpiCard`** — uppercase label, large `tabular-nums` value, optional sublabel. Exact value in `title`
  when the display value is abbreviated.
- **`BarRow`** — labeled horizontal bar; proportional to the section max, but the number is always
  printed so a row reads without the bar. Row dividers are applied by the parent panel, not the row.
- **`UsageTable`** — sortable columns with `aria-sort`; numbers right-aligned and `tabular-nums`;
  names truncate with the full value in `title`.
- **Numbers:** compact in the UI (`1.2M`), exact on hover/in tooltips. Formatters in `src/lib/format.ts`.
- **Money:** always labeled as *equivalent API value / estimate*, never "spent" — a Pro/Max plan is a
  flat fee, so presenting token value as spend would be actively misleading. Unpriced models show "—",
  never `$0`.

## Charts

Hand-rolled SVG (`src/lib/chart/svg.ts`) — no charting dependency. Daily buckets with ≤7 series and
no zoom/brush is well inside what scale + path math covers; revisit only if interactions get heavier.

- **Grid** uses `--hair`; axis ticks use `--ink-faint` at 10px with `tabular-nums`. Y ticks are
  "nice" 1/2/5×10ⁿ steps so labels read as round numbers.
- **Series color** comes from `--series-*`, **always paired with a dash pattern** (`seriesDash(i)`) so
  lines stay separable in greyscale or with colorblindness. A legend shows swatch + dash together.
- **Series cap: 6**, remainder folded into a labeled "Other (N)" series — the count is shown, never a
  silent truncation.
- **Single-series** line charts add a 18% area fill; multi-series stay stroke-only to avoid mud.
- **Cache chart** is the one place the semantic scale appears in a chart: cache-read uses `--good`
  because it *is* the good outcome, while write/fresh use neutral series hues — a low hit rate is not
  an error state, so no warn/crit.
- **Hover** snaps to a bucket via invisible hit bands (never interpolates between days). Tooltip is
  HTML, positioned by percentage, `pointer-events: none`.
- **Every chart has a "View as table" toggle** rendering `ChartTable` — an SVG shape is not readable
  by a screen reader and exact values can't be pulled off a line. This is required, not optional.
- **Legend entries are `<button>`s that mute their series** — keyboard-reachable for free, and the
  muted state uses strikethrough *plus* dimming so it isn't carried by opacity alone. Muting never
  recolors the remaining series (color/dash follow the series' original index).
- `aria-label` on each chart is a **generated sentence describing the actual data** (range, series
  count, peak and when) — not a static label. Hover tooltips are a mouse-only enhancement; the
  keyboard/screen-reader path is the table fallback, by design.
- Charts carry no essential motion, so `prefers-reduced-motion` needs no chart-specific handling
  beyond the global guard in `tokens.css`.
- **Known limitation:** SVG tick text is a fixed 10px and won't grow with macOS text-size settings.
  The table fallback is the accessible route for anyone who needs larger type.

### Encodings that real data forced

Both of these were chosen on paper, shipped, then changed after looking at actual logs. Recorded so
they don't get "simplified" back.

- **Composition charts are normalized to 100%, not stacked by magnitude.** Cache reads run ~96% of
  tokens, so an absolute stack collapsed cache-write and fresh-input into invisible slivers and read
  as a plain total-tokens bar chart. Daily magnitude is the trend chart's job; the composition chart's
  only job is the split.
- **The trend chart needs a Share mode.** One model routinely accounts for ~95% of tokens, which pins
  every other series flat to the axis — the chart is effectively single-series in absolute mode.
  Share (percent of each bucket) is what makes the others readable.
- **No flat reference lines.** A cache-hit-rate line sat at 100% across the whole chart: zero
  information, and it was drawn in `--accent`, violating the accent rule above. Removed.
- **A lone row draws no bar.** A bar at 100% of itself encodes nothing; `BarRow` takes `soloRow` to
  suppress the track when it is the only row.

## Surfaces and decorative colour

Updated after comparing against a peer app: the original outline-only treatment read as a wireframe
next to real content.

- **Cards and panels are filled** (`--panel-2`) with a hairline border and a **12px** radius. Outline
  on the raw background is reserved for nothing — an unfilled box looks unfinished.
- **`--accent` stays sparing** — active account, primary action, selected toggle. It is never used
  for decoration, chart series, or category badges.
- **Decorative and categorical colour comes from `--series-*`**, which already exists and is tuned to
  sit beside coral. KPI icon chips, plan-tier badges, and chart series all draw from it. This is how
  the UI gets colourful without diluting what coral means.
- **Icon chips**: 24px rounded square, `color-mix(… 16%, transparent)` of the tint behind a
  `currentColor` Lucide glyph. Used on KPI cards and the app mark.
- **Status dots** (7px) on account rows: `--good` active, `--warn`/`--crit` by quota, `--ink-faint`
  when unknown. Always paired with a badge or number — never the sole signal.
- **Chart series carry a vertical gradient fill**, 30% → 2% opacity. Applied to every series, not
  just a solo one: a dominant series reads as volume while near-zero series stay invisible, so the
  chart gains depth without turning to mud.
- **Absent data says so** — "no usage data yet", not an empty bar with an em dash, which reads as
  broken rather than empty.

## Numbers

- **Compact units are hand-rolled, not `Intl` compact notation.** Locale compact renders "2.8bn" and
  "116.2m" under en-AU, and a lowercase "m" reads as milli as readily as million. Suffixes stay
  uppercase `K`/`M`/`B` everywhere; only the digits are localized.
- **Currency uses `currencyDisplay: "narrowSymbol"`** — otherwise en-AU renders "US$6,149.44", and the
  country prefix is noise in a single-currency UI.
- **Percentages never round to a value they haven't reached.** 99.93% renders as "99.9%", not "100%";
  0.02% renders as "0.0%", not "0%".

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
