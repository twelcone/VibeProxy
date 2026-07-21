<script lang="ts">
  /**
   * A list row whose *background* is the bar — the fill runs behind the label and value rather than
   * sitting on its own track below them. Roughly half the height of `BarRow` for the same
   * information, which is what makes it viable in the 420px popover.
   *
   * The fill is decorative: the value is always printed, so the row reads without it.
   */
  let {
    label,
    value,
    max,
    color = "var(--series-1)",
    valueText,
    secondaryText = "",
    title = "",
  }: {
    label: string;
    value: number;
    max: number;
    color?: string;
    valueText: string;
    secondaryText?: string;
    title?: string;
  } = $props();

  const widthPct = $derived(max > 0 ? Math.min(100, Math.max((value / max) * 100, 0)) : 0);
</script>

<div class="row" {title}>
  <div class="fill" style:width={`${widthPct}%`} style:background={color} aria-hidden="true"></div>
  <span class="label">{label}</span>
  {#if secondaryText}<span class="secondary">{secondaryText}</span>{/if}
  <span class="value">{valueText}</span>
</div>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 7px;
    background: var(--panel-2);
    overflow: hidden;
    font-size: 0.8rem;
  }
  .row + :global(.row) {
    margin-top: 4px;
  }
  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    opacity: 0.28;
    transition: width 0.3s ease-out;
  }
  .label,
  .value,
  .secondary {
    position: relative; /* above the fill */
  }
  .label {
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .secondary {
    margin-left: auto;
    color: var(--ink-soft);
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
  }
  .value {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .secondary + .value {
    margin-left: 0;
  }
  .label ~ .value:only-of-type {
    margin-left: auto;
  }
</style>
