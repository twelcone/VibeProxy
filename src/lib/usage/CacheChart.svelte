<script lang="ts">
  import { linearScale, linePath, niceTicks, shortDate, type Point } from "$lib/chart/svg";
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
  const PAD = { top: 12, right: 44, bottom: 26, left: 56 };

  let showTable = $state(false);
  let hover = $state<number | null>(null);

  const total = (d: CacheDay) => d.input + d.cacheWrite + d.cacheRead;
  /** Share of cacheable input actually served from cache — cache writes aren't hits. */
  const hitPct = (d: CacheDay) => {
    const denom = d.input + d.cacheRead;
    return denom > 0 ? (d.cacheRead / denom) * 100 : 0;
  };

  /** Whole-range hit rate — lives here beside the detail rather than in a KPI card, where it read
   *  as a headline number despite barely moving (heavy caching pins it near 100%). */
  const overallHit = $derived.by(() => {
    const input = days.reduce((n, d) => n + d.input, 0);
    const read = days.reduce((n, d) => n + d.cacheRead, 0);
    return input + read > 0 ? (read / (input + read)) * 100 : null;
  });

  const summary = $derived.by(() => {
    if (days.length === 0) return "Cache efficiency. No data.";
    const read = days.reduce((n, d) => n + d.cacheRead, 0);
    const written = days.reduce((n, d) => n + d.cacheWrite, 0);
    const fresh = days.reduce((n, d) => n + d.input, 0);
    return `Cache efficiency across ${days.length} buckets. ${fmtTokens(read)} read from cache, ${fmtTokens(written)} written to cache, ${fmtTokens(fresh)} fresh input. Overall hit rate ${pct(overallHit)}. Use "View as table" for exact values.`;
  });

  const max = $derived(Math.max(1, ...days.map(total)));
  const ticks = $derived(niceTicks(max));
  const yMax = $derived(Math.max(max, ticks[ticks.length - 1] ?? max));

  const x = $derived(linearScale([0, Math.max(days.length - 1, 1)], [PAD.left, W - PAD.right]));
  const y = $derived(linearScale([0, yMax], [H - PAD.bottom, PAD.top]));
  const yPct = $derived(linearScale([0, 100], [H - PAD.bottom, PAD.top]));

  const barWidth = $derived(
    Math.max(2, ((W - PAD.left - PAD.right) / Math.max(days.length, 1)) * 0.62),
  );
  const labelEvery = $derived(Math.max(1, Math.ceil(days.length / 6)));
  const hitPoints = $derived<Point[]>(days.map((d, i) => [x(i), yPct(hitPct(d))]));

  // Cache read is the "good" outcome, so it takes the good hue; write and fresh input are neutral
  // surfaces rather than warn/crit — a low hit rate isn't an error state.
  const BANDS = [
    { key: "cacheRead", label: "Cache read", color: "var(--good)" },
    { key: "cacheWrite", label: "Cache write", color: "var(--series-2)" },
    { key: "input", label: "Fresh input", color: "var(--series-6)" },
  ] as const;

  /** Stacked from the baseline up, in BANDS order. */
  function segments(d: CacheDay) {
    let acc = 0;
    return BANDS.map((b) => {
      const v = d[b.key];
      const seg = { ...b, value: v, y0: acc, y1: acc + v };
      acc += v;
      return seg;
    });
  }
</script>

<div class="chart">
  <div class="head">
    <h3>Cache efficiency</h3>
    {#if overallHit !== null}
      <span class="headline">{pct(overallHit)}<small>hit rate</small></span>
    {/if}
    <div class="legend">
      {#each BANDS as b (b.key)}
        <span class="item"><i style:background={b.color}></i>{b.label}</span>
      {/each}
      <span class="item"><i class="line"></i>Hit %</span>
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
        {#each ticks as t (t)}
          <line class="grid" x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} />
          <text class="tick" x={PAD.left - 8} y={y(t)} text-anchor="end" dominant-baseline="middle">
            {fmtTokens(t)}
          </text>
        {/each}
        {#each [0, 50, 100] as p (p)}
          <text class="tick pct" x={W - PAD.right + 8} y={yPct(p)} dominant-baseline="middle">{p}%</text>
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

        <path class="hit" d={linePath(hitPoints)} />

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
          {#each BANDS as b (b.key)}
            <span class="row"><i style:background={b.color}></i>{b.label}<b>{fmtTokens(d[b.key])}</b></span>
          {/each}
          <span class="row"><i class="line"></i>Hit rate<b>{pct(hitPct(d))}</b></span>
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
  .item i.line {
    width: 14px;
    height: 0;
    border-radius: 0;
    border-top: 2px dashed var(--accent);
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
  .hit {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-dasharray: 4 3;
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
  .tooltip .row i.line {
    width: 12px;
    height: 0;
    border-top: 2px dashed var(--accent);
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
