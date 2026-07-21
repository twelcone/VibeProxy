<script lang="ts">
  import Icon, { type IconName } from "$lib/ui/Icon.svelte";

  let {
    label,
    value,
    sub = "",
    title = "",
    icon,
    tint = "var(--series-1)",
  }: {
    label: string;
    value: string;
    sub?: string;
    title?: string;
    icon?: IconName;
    /**
     * Decorative hue for the icon chip. Drawn from the series palette, never `--accent` — the
     * accent means active/primary, and a card that is merely present is neither.
     */
    tint?: string;
  } = $props();
</script>

<div class="kpi">
  <div class="top">
    <span class="label">{label}</span>
    {#if icon}
      <span class="chip" style:color={tint} style:background={`color-mix(in srgb, ${tint} 16%, transparent)`}>
        <Icon name={icon} size={14} />
      </span>
    {/if}
  </div>
  <strong class="value" {title}>{value}</strong>
  {#if sub}<span class="sub">{sub}</span>{/if}
</div>

<style>
  .kpi {
    /* Filled surface with a hairline, rather than outline-only: an unfilled box reads as a
       wireframe placeholder next to real content. */
    background: var(--panel-2);
    border: 1px solid var(--hair);
    border-radius: 12px;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 2px;
  }
  .label {
    font-size: 0.63rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
    color: var(--ink-faint);
  }
  .chip {
    margin-left: auto;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 7px;
  }
  .value {
    font-size: 1.55rem;
    font-weight: 650;
    line-height: 1.1;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
  }
  .sub {
    font-size: 0.72rem;
    color: var(--ink-soft);
    font-variant-numeric: tabular-nums;
  }
</style>
