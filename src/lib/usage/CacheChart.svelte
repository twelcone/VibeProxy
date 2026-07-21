<script lang="ts">
  import { linearScale, shortDate } from "$lib/chart/svg";
  import ChartTable from "./ChartTable.svelte";
  import { tokens as fmtTokens, pct } from "$lib/format";

  export type CacheDay = {
    date: string;
    input: number;
    cacheWrite: number;
    cacheRead: number;
  };

  let { days }: { days: CacheDay[] } = $props();

  const W = 800;
  const H = 220;
  const PAD = { top: 12, right: 16, bottom: 26, left: 46 };

  let showTable = $state(false);
  let hover = $state<number | null>(null);

  const total = (d: CacheDay) => d.input + d.cacheWrite + d.cacheRead;
  /** Share of cacheable input actually served from cache — cache writes aren't hits. */
  const hitPct = (d: CacheDay) => {
    const denom = d.input + d.cacheRead;
    return denom > 0 ? (d.cacheRead / denom) * 100 : 0;
  };
  const shareOf = (key: "input" | "cacheWrite" | "cacheRead") => {
    const t = days.reduce((n, d) => n + total(d), 0);
    return t > 0 ? (days.reduce((n, d) => n + d[key], 0) / t) * 100 : null;
  };

  /** Whole-range hit rate — lives here beside the detail rather than in a KPI card, where it read
   *  as a headline number despite barely moving (heavy caching pins it near 100%). */
  const overallHit = $derived.by(() => {
    const input = days.reduce((n, d) => n + d.input, 0);
    const read = days.reduce((n, d) => n + d.cacheRead, 0);
    return input + read > 0 ? (read / (input + read)) * 100 : null;
  });

  const summary = $derived.by(() => {
    if (days.length === 0) return "Token composition. No data.";
    const read = days.reduce((n, d) => n + d.cacheRead, 0);
    const written = days.reduce((n, d) => n + d.cacheWrite, 0);
    const fresh = days.reduce((n, d) => n + d.input, 0);
    return `Token composition across ${days.length} buckets, as a share of each bucket's input. Overall ${pct(shareOf("cacheRead"))} read from cache (${fmtTokens(read)}), ${pct(shareOf("cacheWrite"))} written to cache (${fmtTokens(written)}), ${pct(shareOf("input"))} fresh input (${fmtTokens(fresh)}). Cache hit rate ${pct(overallHit)}. Use "View as table" for exact values.`;
  });

  /**
   * Composition, not magnitude: every bar is normalized to 100%.
   *
   * Absolute stacking failed on real data — cache reads run ~96% of tokens, so the write and fresh
   * slices collapsed into invisible slivers and the chart read as a plain total-tokens bar chart.
   * Daily totals are already the trend chart's job; what this chart is *for* is the split.
   */
  const x = $derived(linearScale([0, Math.max(days.length - 1, 1)], [PAD.left, W - PAD.right]));
  const y = $derived(linearScale([0, 100], [H - PAD.bottom, PAD.top]));

  const barWidth = $derived(
    Math.max(2, ((W - PAD.left - PAD.right) / Math.max(days.length, 1)) * 0.72),
  );
  const labelEvery = $derived(Math.max(1, Math.ceil(days.length / 6)));

  // Cache read is the "good" outcome, so it takes the good hue; write and fresh input are neutral
  // surfaces rather than warn/crit — a low hit rate isn't an error state.
  const BANDS = [
    { key: "cacheRead", label: "Cache read", color: "var(--good)" },
    { key: "cacheWrite", label: "Cache write", color: "var(--series-2)" },
    { key: "input", label: "Fresh input", color: "var(--series-6)" },
  ] as const;

  /** Stacked from the baseline up in BANDS order, as percentages of that bucket's total. */
  function segments(d: CacheDay) {
    const t = total(d);
    let acc = 0;
    return BANDS.map((b) => {
      const share = t > 0 ? (d[b.key] / t) * 100 : 0;
      const seg = { ...b, value: d[b.key], share, y0: acc, y1: acc + share };
      acc += share;
      return seg;
    });
  }
</script>

<div class="chart">
  <div class="head">
    <h3>Token composition</h3>
    {#if overallHit !== null}
      <span class="headline">{pct(shareOf("cacheRead"))}<small>from cache</small></span>
    {/if}
    <div class="legend">
      {#each BANDS as b (b.key)}
        <span class="item"><i style:background={b.color}></i>{b.label}</span>
      {/each}
    </div>
    <button class="toggle" onclick={() => (showTable = !showTable)} aria-pressed={showTable}>
      {showTable ? "View as chart" : "View as table"}
    </button>
  </div>

  {#if showTable}
    <ChartTable
      caption="Cache efficiency by day"
      columnLabel="Date"
      rows={days.map((d) => d.date)}
      columns={[...BANDS.map((b) => ({ key: b.key, label: b.label })), { key: "hit", label: "Hit %" }]}
      format={(date, col) => {
        const d = days.find((x) => x.date === date);
        if (!d) return "—";
        if (col === "hit") return pct(hitPct(d));
        return fmtTokens(d[col as "input" | "cacheWrite" | "cacheRead"]);
      }}
    />
  {:else if days.length === 0}
    <p class="empty">No data in this range.</p>
  {:else}
    <div class="plot">
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={summary}>
        {#each [0, 25, 50, 75, 100] as t (t)}
          <line class="grid" x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} />
          <text class="tick" x={PAD.left - 8} y={y(t)} text-anchor="end" dominant-baseline="middle">
            {t}%
          </text>
        {/each}

        {#each days as d, i (d.date)}
          {#each segments(d) as s (s.key)}
            <rect
              x={x(i) - barWidth / 2}
              y={y(s.y1)}
              width={barWidth}
              height={Math.max(0, y(s.y0) - y(s.y1))}
              fill={s.color}
            />
          {/each}
          {#if i % labelEvery === 0 || i === days.length - 1}
            <text class="tick" x={x(i)} y={H - PAD.bottom + 16} text-anchor="middle">{shortDate(d.date)}</text>
          {/if}
        {/each}

        {#if hover !== null}
          <line class="cursor" x1={x(hover)} x2={x(hover)} y1={PAD.top} y2={H - PAD.bottom} />
        {/if}
        {#each days as d, i (d.date)}
          <rect
            x={x(i) - barWidth / 2} y={PAD.top} width={barWidth} height={H - PAD.top - PAD.bottom}
            fill="transparent"
            onmouseenter={() => (hover = i)}
            onmouseleave={() => (hover = null)}
            role="presentation"
          />
        {/each}
      </svg>

      {#if hover !== null}
        {@const d = days[hover]}
        <div class="tooltip" style:left={`${(x(hover) / W) * 100}%`}>
          <strong>{shortDate(d.date)}</strong>
          {#each segments(d) as s (s.key)}
            <span class="row">
              <i style:background={s.color}></i>{s.label}
              <b>{pct(s.share)} · {fmtTokens(s.value)}</b>
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
  .headline {
    display: flex;
    align-items: baseline;
    gap: 4px;
    font-size: 0.95rem;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .headline small {
    font-size: 0.63rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-faint);
  }
  .legend {
    display: flex;
    gap: 11px;
    flex-wrap: wrap;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 0.72rem;
    color: var(--ink-soft);
  }
  .item i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
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
