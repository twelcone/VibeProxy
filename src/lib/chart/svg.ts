/**
 * Minimal SVG chart primitives — scales, paths, ticks.
 *
 * Hand-rolled rather than pulling a charting library: the charts here are daily buckets with ≤7
 * series and no zoom/brush, which is well inside what a few dozen lines of path math handles. Revisit
 * if interactions ever get heavier.
 */

export type Point = [number, number];

/** Maps a value from `domain` onto `range`. Degenerate domains pin to the range midpoint. */
export function linearScale(
  domain: [number, number],
  range: [number, number],
): (v: number) => number {
  const [d0, d1] = domain;
  const [r0, r1] = range;
  const span = d1 - d0;
  if (span === 0) return () => (r0 + r1) / 2;
  return (v) => r0 + ((v - d0) / span) * (r1 - r0);
}

/**
 * Human-friendly tick values covering [0, max] — steps of 1/2/5×10ⁿ, so labels read as round
 * numbers instead of 3.7M / 7.4M.
 */
export function niceTicks(max: number, count = 4): number[] {
  if (!isFinite(max) || max <= 0) return [0];
  const rough = max / count;
  const mag = 10 ** Math.floor(Math.log10(rough));
  const norm = rough / mag;
  const step = (norm >= 5 ? 5 : norm >= 2 ? 2 : 1) * mag;
  const out: number[] = [];
  for (let v = 0; v <= max + step * 0.001; v += step) out.push(Math.round(v * 1e6) / 1e6);
  return out;
}

export function linePath(points: Point[]): string {
  if (points.length === 0) return "";
  if (points.length === 1) {
    // A lone point has no line; emit a zero-length segment so stroke-linecap renders a dot.
    const [x, y] = points[0];
    return `M${x},${y}L${x},${y}`;
  }
  return points.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x},${y}`).join("");
}

/** Closed path for the fill under a line. */
export function areaPath(points: Point[], baselineY: number): string {
  if (points.length === 0) return "";
  const first = points[0];
  const last = points[points.length - 1];
  return `${linePath(points)}L${last[0]},${baselineY}L${first[0]},${baselineY}Z`;
}

/**
 * Dash patterns paired with the series palette so lines stay distinguishable in greyscale or with
 * colorblindness — color is never the only signal.
 */
const DASHES = ["", "5 3", "2 3", "8 3 2 3", "1 4", "10 4"];
export function seriesDash(index: number): string {
  return DASHES[index % DASHES.length];
}

/** `2026-07-14` → `Jul 14`. Parsed as local time (a bare date string would be read as UTC). */
export function shortDate(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  if (!y || !m || !d) return iso;
  return new Date(y, m - 1, d).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/** ISO date of the Monday starting that date's week — the bucket key for weekly granularity. */
export function weekStart(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  const dt = new Date(y, m - 1, d);
  const dow = (dt.getDay() + 6) % 7; // Monday = 0
  dt.setDate(dt.getDate() - dow);
  return `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, "0")}-${String(dt.getDate()).padStart(2, "0")}`;
}
