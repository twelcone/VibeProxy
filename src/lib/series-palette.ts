/**
 * Categorical palette for chart series (per-model, per-account, per-project).
 *
 * Separate from `--accent` (brand/active only) and from the good/warn/crit quota scale — reusing
 * either would make a chart color read as state. Series are always paired with a direct text label,
 * so color is reinforcement, never the only carrier of meaning.
 */
const SERIES_COUNT = 6;

/** CSS var for the nth series, cycling once the palette runs out. */
export function seriesColor(index: number): string {
  return `var(--series-${(index % SERIES_COUNT) + 1})`;
}
