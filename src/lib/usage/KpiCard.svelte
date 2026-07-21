<script lang="ts">
  let {
    label,
    value,
    sub = "",
    title = "",
  }: { label: string; value: string; sub?: string; title?: string } = $props();
</script>

<div class="kpi">
  <span class="label">{label}</span>
  <strong class="value" {title}>{value}</strong>
  {#if sub}<span class="sub">{sub}</span>{/if}
</div>

<style>
  .kpi {
    border: 1px solid var(--hair);
    border-radius: 10px;
    padding: 11px 13px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    background: var(--panel);
  }
  .label {
    font-size: 0.63rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
    color: var(--ink-faint);
  }
  .value {
    font-size: 1.5rem;
    font-weight: 650;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }
  .sub {
    font-size: 0.72rem;
    color: var(--ink-soft);
    font-variant-numeric: tabular-nums;
  }
</style>
