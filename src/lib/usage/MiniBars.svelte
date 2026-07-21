<script lang="ts">
  /**
   * Compact daily bar chart for the popover — enough to read the shape of a week at a glance
   * without opening the Usage window. No axes: the label row and the hover title carry the detail.
   *
   * Uses a series hue rather than the accent. The accent marks the *active* account elsewhere in
   * this same popover, and reusing it for a neutral spend chart would blur that meaning.
   */
  let {
    days,
    format,
  }: {
    days: { date: string; value: number }[];
    format: (n: number) => string;
  } = $props();

  const max = $derived(Math.max(1, ...days.map((d) => d.value)));

  /** Single-letter weekday, e.g. `M` — the popover is too narrow for `Mon`. */
  function initial(iso: string): string {
    const [y, m, d] = iso.split("-").map(Number);
    if (!y || !m || !d) return "";
    return new Date(y, m - 1, d).toLocaleDateString(undefined, { weekday: "narrow" });
  }
</script>

<div class="mini">
  <div class="bars">
    {#each days as d (d.date)}
      <div class="col" title={`${d.date} · ${format(d.value)}`}>
        <div class="bar" style:height={`${Math.max((d.value / max) * 100, d.value > 0 ? 3 : 0)}%`}></div>
      </div>
    {/each}
  </div>
  <div class="labels">
    {#each days as d (d.date)}<span>{initial(d.date)}</span>{/each}
  </div>
</div>

<style>
  .mini {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .bars {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 54px;
  }
  .col {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
  }
  .bar {
    width: 100%;
    background: var(--series-1);
    border-radius: 3px 3px 0 0;
    min-height: 0;
    transition: height 0.3s ease-out;
  }
  .labels {
    display: flex;
    gap: 4px;
  }
  .labels span {
    flex: 1;
    text-align: center;
    font-size: 0.62rem;
    color: var(--ink-faint);
    font-variant-numeric: tabular-nums;
  }
</style>
