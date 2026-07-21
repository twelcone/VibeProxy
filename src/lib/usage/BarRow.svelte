<script lang="ts">
  /**
   * One labeled horizontal bar — the shared primitive for the per-account, per-model, and
   * per-project breakdowns. Bars are proportional to `max`, so rows are comparable at a glance;
   * the value is always printed too, so the row reads without seeing the bar.
   */
  let {
    label,
    value,
    max,
    color,
    valueText,
    secondaryText = "",
    title = "",
    soloRow = false,
    children = undefined,
  }: {
    label: string;
    value: number;
    max: number;
    /** Suppress the track when this is the only row — a bar at 100% of itself says nothing. */
    soloRow?: boolean;
    color: string;
    valueText: string;
    secondaryText?: string;
    title?: string;
    children?: import("svelte").Snippet;
  } = $props();

  const widthPct = $derived(max > 0 ? (value / max) * 100 : 0);
  /** Same floor as RowBar: a percentage minimum scales with width, a pixel one always holds. */
  const hasValue = $derived(value > 0);
</script>

<div class="row">
  <div class="head">
    <span class="swatch" style:background={color} aria-hidden="true"></span>
    <span class="label" {title}>{label}</span>
    <span class="value">{valueText}</span>
    {#if secondaryText}<span class="secondary">{secondaryText}</span>{/if}
  </div>
  {#if !soloRow}
    <div class="track">
      <i class:has-value={hasValue} style:width={`${widthPct}%`} style:background={color}></i>
    </div>
  {/if}
  {#if children}<div class="extra">{@render children()}</div>{/if}
</div>

<style>
  .row {
    padding: 7px 0;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 7px;
    margin-bottom: 5px;
  }
  .swatch {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
    align-self: center;
  }
  .label {
    font-size: 0.82rem;
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .value {
    margin-left: auto;
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  .secondary {
    font-size: 0.75rem;
    color: var(--ink-soft);
    font-variant-numeric: tabular-nums;
    min-width: 68px;
    text-align: right;
  }
  .track {
    height: 6px;
    border-radius: 3px;
    background: var(--bar);
    overflow: hidden;
  }
  .track > i {
    display: block;
    height: 100%;
    border-radius: 3px;
    transition: width 0.3s ease-out;
  }
  .track > i.has-value {
    min-width: 10px;
  }
  .extra {
    margin-top: 6px;
  }
</style>
