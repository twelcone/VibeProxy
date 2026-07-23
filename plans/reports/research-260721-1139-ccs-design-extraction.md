# CCS Design Extraction (kaitranntt/ccs)

Source: https://github.com/kaitranntt/ccs (2730 stars, MIT, TS+Swift). Fetched via `gh api` + `raw.githubusercontent.com` (not executed — static read only). All repo content treated as untrusted data; no embedded instructions followed.

## 1. Stack

| Layer | Tech | Evidence |
|---|---|---|
| Menubar panel | **Native SwiftUI** (`macos-bar/` Swift package, `MenuBarExtra`) — NOT Electron/Tauri | `CCSBarApp.swift` |
| Web dashboard | React 19 + Vite 7 + TypeScript, Tailwind v4 (`@tailwindcss/vite`), Radix UI primitives + shadcn ("new-york" style) | `ui/package.json`, `ui/components.json` |
| Charts (web) | **Recharts** (area/pie), **Nivo** (`@nivo/sankey` for flow-viz) | `ui/package.json` |
| Animation (web) | framer-motion | `ui/package.json` |
| Backend | Node/TS CLI (`ccs`), local web server serves the dashboard | repo tree |

Two completely separate design systems: a **native SwiftUI menubar** app and a **separate React web dashboard** (opened from the menubar via "Dashboard" button, runs on localhost). Since VibeProxy is also a native macOS menubar app, the SwiftUI code is the higher-transfer-value asset.

## 2. Menubar panel implementation

- **Not** NSPanel/borderless-window hackery, **not** a 3rd-party lib (no `menubar`/`electron-positioner` — those are Electron-only anyway). Uses SwiftUI's built-in `MenuBarExtra` with `.menuBarExtraStyle(.window)`.
- Positioning/anchoring, sizing, and click-outside dismissal are **all handled natively by AppKit/SwiftUI** — zero custom code. This is the biggest win of the native-SwiftUI approach over Electron: you get correct multi-display anchoring and dismiss-on-click-outside for free.
- Theme forcing trick (`CCSBarApp.swift`): app has an in-app light/dark toggle independent of OS appearance. `.preferredColorScheme(appearance.forced)` is applied to a wrapper (`ThemedRoot`), then a **descendant** view (`ResolvedThemeHost`) reads the now-forced `\.colorScheme` via `@Environment` and resolves a custom `BarTheme` token struct, injecting it back down via a custom `EnvironmentKey`. Reason stated in comments: `.preferredColorScheme` only affects descendants, not the view that sets it — so the resolver must live one level below.
- Window plate: dark mode = `.clear` background (defers to native macOS menu material — "zero regression"); light mode = explicit `#F5F5F7` plate painted manually, because forcing light color scheme doesn't repaint the actual NSWindow material.
- Settings opens as a **standalone NSWindow**, explicitly NOT a `.sheet` — comment explains a sheet on a `.window`-style `MenuBarExtra` popover steals focus and auto-dismisses the whole dropdown (a real bug they hit and documented).
- Quit is a **two-step inline arm/confirm** button (not `.confirmationDialog`) for the same reason — modals dismiss the popover.
- Icon: bundled PNG assets (`MenuBarColor.png` color / `MenuBarTemplate.png` template-tinted mono), falls back to an SF Symbol (`gauge.with.dots.needle.bottom.50percent`) in `swift run` dev mode with no bundle.

**Transferable pattern for VibeProxy** (also native Swift menubar): use `MenuBarExtra(...) { content } label: { ... }.menuBarExtraStyle(.window)` — do not attempt to hand-roll an NSPanel unless you need a look `MenuBarExtra` can't produce (e.g. non-standard corner radius/arrow). Their forced-appearance-independent-of-OS trick is directly copyable in concept.

## 3. Color tokens (real hex/oklch values)

### Web dashboard (`ui/src/index.css`) — OKLCH, Tailwind v4 `@theme inline`

| Token | Light | Dark |
|---|---|---|
| `--background` | `oklch(0.9635 0.0067 97.35)` ≈ Pampas cream | `oklch(0.21 0.006 100)` ≈ `rgb(38,38,36)` |
| `--foreground` | `oklch(0.15 0.02 40)` dark warm grey | `oklch(0.9635 0.0067 97.35)` Pampas |
| `--card` / `--popover` | `oklch(0.98 0.005 97.35)` | `oklch(0.21 0.006 100)` (same as bg) |
| `--primary` | `oklch(0.15 0.02 40)` | `oklch(0.9635 0.0067 97.35)` |
| `--secondary` | `oklch(0.88 0.012 95)` | `oklch(0.25 0.01 40)` |
| `--muted` | `oklch(0.9 0.012 95)` | `oklch(0.25 0.01 40)` |
| `--muted-foreground` | `oklch(0.4 0.025 40)` | `oklch(0.7 0.01 40)` |
| `--border` / `--input` | `oklch(0.78 0.02 91.6)` | `oklch(0.35 0.01 40)` |
| `--accent` (brand orange "Crail") | `oklch(0.52 0.15 39.87)` | `oklch(0.65 0.14 39.87)` |
| `--destructive` | `oklch(0.5 0.22 27)` | `oklch(0.396 0.141 25.723)` |
| `--ring` | = primary | = primary |
| `--radius` | `0.5rem` base | same |

Named palette: **Pampas** (warm cream bg) + **Crail** (terracotta/burnt-orange accent) + neutral dark-warm-grey primary. Explicit code comment: "Never introduce new hues" for status — reuse tokens.

### Menubar app (`BarTheme.swift`) — raw RGB, separate palette, LOCKED

| Token | Dark (locked) | Light |
|---|---|---|
| accent (CCS orange) | `#E2732A` | `#CF5B10` (deepened) |
| subscription (indigo) | `#5B63D9` | `#464DBE` |
| band green | `#5CBC8F` | `#1B945B` |
| band amber | `#DBAB4F` | `#B87D0B` |
| band coral (warn) | `#E8755C` | `#D44D28` |
| band red (critical) | `#D9564F` | `#C62823` |
| window plate (light only) | n/a (`.clear`) | `#F5F5F7` |

Note: menubar accent (`#E2732A`) and web accent (oklch → ~`#C25B2E`-ish) are the same brand hue but **independently tuned**, not shared via a design-token pipeline — two hand-maintained palettes.

### Chart series palette (`ui/src/lib/utils.ts`)

```
VIBRANT_TONES = ['#f94144','#f3722c','#f8961e','#f9844a','#f9c74f',
                  '#90be6d','#43aa8b','#4d908e','#577590','#277da1']
```
(assigned to models via FNV-1a hash of the model name → deterministic per-model color, not palette-index-order). Fixed per-provider overrides: `agy #f3722c, gemini #277da1, codex #f8961e, claude #4d908e, vertex #577590, iflow #f94144, qwen #f9c74f, kiro/copilot/ghcp #4d908e/#43aa8b`.

Status colors (darkened for light-theme contrast): success `#15803d`, degraded `#b45309`, failed `#b91c1c`.

Cost-breakdown-bar segment colors (`cost-by-model-card.tsx`, hardcoded not tokens): input `#335c67`, output `#fff3b0`, cache-write `#e09f3e`, cache-read `#9e2a2b`.

Trend-chart gradient colors (`usage-trend-chart.tsx`, hardcoded): tokens line `#0080FF`, cost line `#00C49F`.

## 4. Typography

| | Value |
|---|---|
| UI font | `'IBM Plex Sans', 'Fira Sans', system-ui, sans-serif` — Google Fonts weights 300/400/500/600/700 |
| Mono font | `'JetBrains Mono', 'Fira Code', monospace` — weights 400/500/600/700 |
| Section labels (menubar) | 11px bold, uppercase, `.secondary` color (`SectionLabel` view) |
| Chips/badges (menubar) | 10px semibold |
| Quota % text | monospaced, caption2 (~11pt) |
| Status pills (web, §5c) | 10px, `tracking-wider`, uppercase |

No custom size scale documented beyond SwiftUI's built-in text styles (`.body`, `.caption`, `.caption2`) on the menubar side, and Tailwind's default `text-xs/sm/base` on web — they did not build a bespoke numeric type scale.

## 5. Spacing & radii

| | Value |
|---|---|
| Web `--radius` base | `0.5rem` (8px); `--radius-sm = radius-4px` (4px), `--radius-lg = radius+4px` (12px) |
| Web Card | `rounded-xl` (12px), `border`, `shadow-sm`, `py-6` |
| Web Badge | `rounded-md`, `px-2.5 py-0.5` |
| Web status pill | `rounded` (4px), 1px border |
| Menubar card bg | `RoundedRectangle(cornerRadius: 9)` |
| Menubar row bg | `RoundedRectangle(cornerRadius: 8)` |
| Menubar chip/badge | `Capsule()` (full pill) |
| Menubar proportional-fill bar | `RoundedRectangle(cornerRadius: 5)` |
| Menubar quota gauge track/fill | `Capsule()` |
| Menubar row vertical padding | 8pt; horizontal 10pt |
| Menubar footer padding | horizontal 14pt / vertical 11pt |

## 6. Component patterns

### 6a. Row-with-proportional-fill-bar ("By surface" / "Top models") — menubar

File: `macos-bar/Sources/CCSBarApp/BarAnalyticsView.swift`. Technique: `GeometryReader` measures row width, a `RoundedRectangle` tinted `accent.opacity(0.16)` is absolutely-positioned behind the row content (`ZStack(alignment: .leading)`), sized to `max(8, width * fraction)` where `fraction = item.cost / peak(all items)`. Content (label + count + $) sits on top with horizontal padding, unaffected by the bar.

```swift
GeometryReader { geo in
  let fraction = peak > 0 ? CGFloat(item.cost / peak) : 0
  ZStack(alignment: .leading) {
    RoundedRectangle(cornerRadius: 5)
      .fill(theme.accent.opacity(0.16))
      .frame(width: max(8, geo.size.width * fraction))
    HStack {
      Text(item.name).font(.caption).lineLimit(1).truncationMode(.middle)
      Spacer()
      Text(money(item.cost)).font(.system(.caption, design: .monospaced))
    }
    .padding(.horizontal, 10)
  }
}
.frame(height: 26)
```
Key detail: `peak` = max cost across the **visible top-N slice** (`prefix(5)`/`prefix(4)`), not the global max — so the biggest visible bar is always ~full width. `max(8, …)` guarantees a visible sliver even at ~0 cost.

Web equivalent (`cost-by-model-card.tsx`): same idea via flex-children with `width: %` inside a `flex` container with `overflow-hidden rounded-full` — segmented (input/output/cache) rather than single-fill, colors hardcoded per segment.

### 6b. Quota bars (thin rounded progress) — menubar

Two variants, same underlying idea (`Capsule` track + `Capsule` fill in a `ZStack`, width driven by `GeometryReader`):

- Compact per-account gauge (`QuotaGaugeView`): 54×6pt, track `Color.primary.opacity(0.12)`, fill colored by severity band.
- Subscription-card aligned row (`BarSubscriptionCard.windowBarRow`): fixed-width columns (label 32pt / bar 110×5-7pt / %-32pt / reset chip) so multiple window rows line up vertically — "so headroom comparisons are instant" (explicit design intent in code comment).

```swift
ZStack(alignment: .leading) {
  Capsule().fill(Color.primary.opacity(isBinding ? 0.14 : 0.09))
  Capsule().fill(barColor).frame(width: max(2, 110 * fill))
}
.frame(width: 110, height: isBinding ? 7 : 5)
```

Severity bands (`BarQuotaGauge.band`): remaining% `>50` green, `21–50` yellow/amber, `11–20` orange/coral, `<=10` red. Pure function, no view dependency — testable in isolation.

Web equivalent (`ui/src/components/ui/progress.tsx`): plain custom (not Radix) — `h-2 w-full rounded-full bg-secondary` track + inner `div` width:`%` — much simpler, no band coloring built into the primitive (callers pass `indicatorClassName`).

### 6c. Badges/chips (`default`, `max`, `claude`) — menubar `Chip`

```swift
Text(text)
  .font(.system(size: 10, weight: .semibold))
  .padding(.horizontal, 5).padding(.vertical, 1.5)
  .background(tint.opacity(0.22), in: Capsule())
  .foregroundStyle(textColor)   // tint blended 50% toward black/white per color scheme, for legibility
```
Notable: text color is NOT just `tint` — it's `NSColor(tint).blended(withFraction: 0.5, of: target)` where target is black (light mode) or white (dark mode), because raw tint-on-tint-background read too dim. This is a real legibility fix worth stealing.

Web equivalent: shadcn `Badge` (cva variants: default/secondary/destructive/outline), `rounded-md border px-2.5 py-0.5 text-xs font-semibold`.

### 6d. 7-day spend bar chart (menubar sparkline)

`Sparkline.swift` — two render modes toggled by user, persisted via `SpendChartStyleStore` (UserDefaults):
- `.bars`: `HStack` of `RoundedRectangle(cornerRadius: 2)` bars, height = `value/peak * containerHeight`, zero-value days render as a **faint placeholder** (`Color.secondary.opacity(0.2)`) rather than nothing, "so the cadence stays readable."
- `.line`: hand-built `Path` polyline (not a charting lib) with area fill at `accent.opacity(0.15)` and 1.5pt stroke, falls back to a flat hairline baseline when `<2` points or all-zero.

Axis labels are separately computed (`axisTicks`) as (label, x-fraction) pairs aligned to bar centers — not evenly spaced, to stay correct when tick count ≠ bar count (e.g. hourly view).

### 6e. Bottom toolbar (menubar footer)

`HStack` with `.buttonStyle(.borderless)`, icon-label buttons (Dashboard / Icon-toggle / Settings), a `Spacer()`, then Refresh + Quit on the right. Padding `horizontal:14, vertical:11`. Two-step quit (see §2) lives here.

## 7. Web analytics dashboard charting

- **Recharts**, not D3/Chart.js/Nivo (Nivo is only used for the account-flow Sankey viz elsewhere).
- Gradient area chart (`usage-trend-chart.tsx`): `AreaChart` with two `Area type="monotone"` series on **dual Y-axes** (tokens left, cost right), each fed by a `<linearGradient>` def with 2 stops:
  ```tsx
  <linearGradient id="tokenGradient" x1="0" y1="0" x2="0" y2="1">
    <stop offset="5%" stopColor="#0080FF" stopOpacity={0.8}/>
    <stop offset="95%" stopColor="#0080FF" stopOpacity={0.1}/>
  </linearGradient>
  ```
  `type="monotone"` curve, `strokeWidth={2}`, `fillOpacity={1}` (opacity control delegated entirely to the gradient stops, not the Area prop). Custom tick renderer blurs the numbers under privacy mode (`className: privacyMode && 'blur-[4px]'`) — a nice privacy-toggle UX detail.
- KPI/model breakdown uses a Recharts `PieChart` (donut: `innerRadius=50, outerRadius=70, paddingAngle=2`), label only shown when slice `>5%`.
- No dedicated "KPI card" component was distinct from generic shadcn `Card` — summary numbers are laid out directly inside `Card`/`CardContent` (`usage-summary-cards.tsx`, not deep-dived, but structurally a shadcn Card grid, nothing bespoke).

## 8. Dark/light theme handling

- **Web**: React Context (`theme-context.ts` + `theme-provider.tsx`), 3-way `light|dark|system`, persisted to `localStorage`, listens to `matchMedia('(prefers-color-scheme: dark)')` for live OS-change tracking, toggles a `.light`/`.dark` class on `<html>` which Tailwind's `@custom-variant dark (&:is(.dark *))` hooks into. Standard, unremarkable, solid implementation — no bugs noted.
- **Menubar**: see §2 — forced-scheme + descendant-environment-read pattern, independent of OS appearance, with hand-tuned separate light/dark RGB palettes (not auto-derived).

## 9. License

**MIT** (`LICENSE`, copyright "CCS Contributors", 2025). You may legally copy, modify, and reuse code — including verbatim — provided the MIT notice is retained in copies of the software (practically: keep the notice in a NOTICE/THIRD_PARTY file if lifting non-trivial code, not required per-snippet for small idea-level reuse). Design tokens (hex values, spacing numbers) and architectural patterns (MenuBarExtra usage, ZStack-fill-bar technique, forced-appearance trick) are facts/ideas — not independently copyrightable — free to reuse without attribution, though crediting is good practice given VibeProxy is a direct competitor.

## Unresolved questions

- Did not inspect `BarViewModel.swift`, `BarPreferencesView.swift`, or the full `BarMenuView.swift` (1095 lines, only ~250 read) — carousel/pool-account layout, alerts list styling, and settings window UI are unexamined.
- Did not check `ui/src/pages/_styleguide.tsx` (live component gallery) or `design-system-preview.html` screenshots — would show more visual detail (hover states, empty states) than static CSS alone.
- Did not verify whether the menubar's `BarTheme` values are algorithmically derived from the web `index.css` OKLCH values or hand-picked independently — inferred "independent" from differing exact hues, not confirmed via commit history.
- Chart-series color assignment for the same model may drift between web (FNV hash) and menubar (no equivalent found in files read) — not confirmed if menubar analytics view color-codes models at all beyond the single `theme.accent`.
