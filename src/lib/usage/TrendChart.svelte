<script lang="ts">
  import { areaPath, linePath, linearScale, niceTicks, seriesDash, shortDate, type Point } from "$lib/chart/svg";
  import { seriesColor } from "$lib/series-palette";
  import ChartTable from "./ChartTable.svelte";

  export type Series = { key: string; label: string; values: Map<string, number> };

  let {
    dates,
    series,
    format,
    title,
  }: {
    dates: string[];
    series: Series[];
    format: (n: number) => string;
    title: string;
  } = $props();

  // Fixed viewBox scaled by CSS — strokes scale with it, which is fine at these sizes and avoids
  // measuring the container on every resize.
  const W = 800;
  const H = 260;
  const PAD = { top: 12, right: 16, bottom: 26, left: 56 };

  let showTable = $state(false);
  let hover = $state<number | null>(null);
  /** Series muted via the legend. Keyed by series key so it survives re-ordering. */
  let hidden = $state<string[]>([]);

  const shown = $derived(series.filter((s) => !hidden.includes(s.key)));

  function toggle(key: string) {
    hidden = hidden.includes(key) ? hidden.filter((k) => k !== key) : [...hidden, key];
  }

  /**
   * Text equivalent of the chart's shape — screen readers get the headline (range, peak) rather than
   * an unreadable path. Exact values live in the table fallback.
   */
  const summary = $derived.by(() => {
    if (dates.length === 0) return `${title}. No data.`;
    let peakDate = dates[0];
    let peak = 0;
    for (const d of dates) {
      const v = shown.reduce((n, s) => n + (s.values.get(d) ?? 0), 0);
      if (v > peak) {
        peak = v;
        peakDate = d;
      }
    }
    return `${title}. ${shown.length} series across ${dates.length} buckets, ${shortDate(dates[0])} to ${shortDate(dates[dates.length - 1])}. Peak ${format(peak)} on ${shortDate(peakDate)}. Use "View as table" for exact values.`;
  });

  const max = $derived(
    Math.max(1, ...shown.flatMap((s) => dates.map((d) => s.values.get(d) ?? 0))),
  );
  const ticks = $derived(niceTicks(max));
  const yMax = $derived(Math.max(max, ticks[ticks.length - 1] ?? max));

  const x = $derived(linearScale([0, Math.max(dates.length - 1, 1)], [PAD.left, W - PAD.right]));
  const y = $derived(linearScale([0, yMax], [H - PAD.bottom, PAD.top]));

  const points = (s: Series): Point[] => dates.map((d, i) => [x(i), y(s.values.get(d) ?? 0)]);

  /** Color/dash follow the series' original position, so hiding one never recolors the others. */
  const styleIndex = (key: string) => series.findIndex((s) => s.key === key);

  // Roughly 6 x-labels regardless of range length, always including the last day.
  const labelEvery = $derived(Math.max(1, Math.ceil(dates.length / 6)));

  const bandWidth = $derived((W - PAD.left - PAD.right) / Math.max(dates.length, 1));
</script>

<div class="chart">
  <div class="head">
    <h3>{title}</h3>
    <div class="legend">
      {#each series as s, i (s.key)}
        <button
          class="item"
          class:off={hidden.includes(s.key)}
          aria-pressed={!hidden.includes(s.key)}
          title={hidden.includes(s.key) ? `Show ${s.label}` : `Hide ${s.label}`}
          onclick={() => toggle(s.key)}
        >
          <svg width="18" height="8" aria-hidden="true">
            <line
              x1="0" y1="4" x2="18" y2="4"
              stroke={seriesColor(i)} stroke-width="2" stroke-dasharray={seriesDash(i)}
            />
          </svg>
          {s.label}
        </button>
      {/each}
    </div>
    <button class="toggle" onclick={() => (showTable = !showTable)} aria-pressed={showTable}>
      {showTable ? "View as chart" : "View as table"}
    </button>
  </div>

  {#if showTable}
    <ChartTable
      caption={title}
      columnLabel="Date"
      rows={dates}
      columns={shown.map((s) => ({ key: s.key, label: s.label }))}
      format={(d, k) => format(shown.find((s) => s.key === k)?.values.get(d) ?? 0)}
    />
  {:else if dates.length === 0}
    <p class="empty">No data in this range.</p>
  {:else}
    <div class="plot">
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={summary}>
        {#each ticks as t (t)}
          <line class="grid" x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} />
          <text class="tick" x={PAD.left - 8} y={y(t)} text-anchor="end" dominant-baseline="middle">
            {format(t)}
          </text>
        {/each}

        {#each dates as d, i (d)}
          {#if i % labelEvery === 0 || i === dates.length - 1}
            <text class="tick" x={x(i)} y={H - PAD.bottom + 16} text-anchor="middle">{shortDate(d)}</text>
          {/if}
        {/each}

        {#each shown as s (s.key)}
          {#if shown.length === 1}
            <path class="area" d={areaPath(points(s), y(0))} fill={seriesColor(styleIndex(s.key))} />
          {/if}
          <path
            d={linePath(points(s))}
            fill="none"
            stroke={seriesColor(styleIndex(s.key))}
            stroke-width="2"
            stroke-dasharray={seriesDash(styleIndex(s.key))}
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        {/each}

        {#if hover !== null}
          <line class="cursor" x1={x(hover)} x2={x(hover)} y1={PAD.top} y2={H - PAD.bottom} />
          {#each shown as s (s.key)}
            <circle cx={x(hover)} cy={y(s.values.get(dates[hover]) ?? 0)} r="3.5" fill={seriesColor(styleIndex(s.key))} />
          {/each}
        {/if}

        <!-- Invisible hit bands: one per day, so hover snaps to a bucket instead of interpolating. -->
        {#each dates as d, i (d)}
          <rect
            x={x(i) - bandWidth / 2} y={PAD.top} width={bandWidth} height={H - PAD.top - PAD.bottom}
            fill="transparent"
            onmouseenter={() => (hover = i)}
            onmouseleave={() => (hover = null)}
            role="presentation"
          />
        {/each}
      </svg>

      {#if hover !== null}
        <div class="tooltip" style:left={`${(x(hover) / W) * 100}%`}>
          <strong>{shortDate(dates[hover])}</strong>
          {#each shown as s (s.key)}
            <span class="row">
              <i style:background={seriesColor(styleIndex(s.key))}></i>
              {s.label}
              <b>{format(s.values.get(dates[hover]) ?? 0)}</b>
            </span>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .chart {
    border: 1px solid var(--hair);
    border-radius: 10px;
    padding: 11px 13px 6px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 6px;
  }
  h3 {
    margin: 0;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-faint);
    font-weight: 600;
  }
  .legend {
    display: flex;
    gap: 11px;
    flex-wrap: wrap;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 4px;
    /* Model ids are hyphenated (opus-4-8), which CSS treats as break opportunities — without this
       the label wraps mid-id and orphans its swatch. */
    white-space: nowrap;
    flex: none;
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink-soft);
    background: none;
    border: 0;
    padding: 2px 4px;
    border-radius: 5px;
    cursor: pointer;
  }
  .item:hover {
    background: var(--panel-2);
    color: var(--ink);
  }
  /* Muted series stay legible and keep their swatch — the strikethrough carries the state, so it
     doesn't rely on the opacity change alone. */
  .item.off {
    color: var(--ink-faint);
    text-decoration: line-through;
  }
  .item.off svg {
    opacity: 0.4;
  }
  .toggle {
    margin-left: auto;
    font: inherit;
    font-size: 0.7rem;
    font-weight: 600;
    padding: 3px 8px;
    border: 1px solid var(--hair);
    border-radius: 6px;
    background: var(--panel);
    color: var(--ink-soft);
    cursor: pointer;
  }
  .toggle:hover {
    color: var(--ink);
    background: var(--panel-2);
  }

  .plot {
    position: relative;
  }
  svg {
    width: 100%;
    height: auto;
    display: block;
    overflow: visible;
  }
  .grid {
    stroke: var(--hair);
    stroke-width: 1;
  }
  .cursor {
    stroke: var(--ink-faint);
    stroke-width: 1;
    stroke-dasharray: 3 3;
  }
  .tick {
    fill: var(--ink-faint);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }
  .area {
    opacity: 0.18;
  }

  .tooltip {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    pointer-events: none;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-radius: 7px;
    padding: 6px 9px;
    font-size: 0.72rem;
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.14);
    white-space: nowrap;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tooltip .row {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--ink-soft);
  }
  .tooltip .row i {
    width: 7px;
    height: 7px;
    border-radius: 2px;
  }
  .tooltip .row b {
    margin-left: auto;
    padding-left: 10px;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }
  .empty {
    color: var(--ink-faint);
    font-size: 0.82rem;
    padding: 30px 0;
    text-align: center;
  }
</style>
