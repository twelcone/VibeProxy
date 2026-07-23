<script lang="ts">
  import { modelName } from "$lib/format";

  export type RangePreset = "7d" | "30d" | "all" | "custom";
  export type Granularity = "day" | "week";

  let {
    preset = $bindable(),
    from = $bindable(),
    to = $bindable(),
    granularity = $bindable(),
    groupBy = $bindable(),
    selectedModels = $bindable(),
    availableModels,
    onchange,
  }: {
    preset: RangePreset;
    from: string;
    to: string;
    granularity: Granularity;
    groupBy: "model" | "account";
    selectedModels: string[];
    availableModels: string[];
    onchange: () => void;
  } = $props();

  const PRESETS: { key: RangePreset; label: string }[] = [
    { key: "7d", label: "7 days" },
    { key: "30d", label: "30 days" },
    { key: "all", label: "All" },
    { key: "custom", label: "Custom" },
  ];

  let modelsEl = $state<HTMLDetailsElement | null>(null);

  function pick(p: RangePreset) {
    preset = p;
    onchange();
  }

  function toggleModel(m: string) {
    selectedModels = selectedModels.includes(m)
      ? selectedModels.filter((x) => x !== m)
      : [...selectedModels, m];
  }
</script>

<div class="filters">
  <div class="group" role="group" aria-label="Date range">
    {#each PRESETS as p (p.key)}
      <button class:on={preset === p.key} aria-pressed={preset === p.key} onclick={() => pick(p.key)}>
        {p.label}
      </button>
    {/each}
  </div>

  {#if preset === "custom"}
    <label class="dates">
      <input type="date" bind:value={from} onchange={onchange} aria-label="From date" />
      <span>→</span>
      <input type="date" bind:value={to} onchange={onchange} aria-label="To date" />
    </label>
  {/if}

  <div class="group" role="group" aria-label="Granularity">
    <button class:on={granularity === "day"} aria-pressed={granularity === "day"}
      onclick={() => (granularity = "day")}>Daily</button>
    <button class:on={granularity === "week"} aria-pressed={granularity === "week"}
      onclick={() => (granularity = "week")}>Weekly</button>
  </div>

  <div class="group" role="group" aria-label="Trend series">
    <button class:on={groupBy === "model"} aria-pressed={groupBy === "model"}
      onclick={() => (groupBy = "model")}>By model</button>
    <button class:on={groupBy === "account"} aria-pressed={groupBy === "account"}
      onclick={() => (groupBy = "account")}>By account</button>
  </div>

  {#if availableModels.length > 1}
    <!-- Native <details> stays open until re-clicked; closing on focus leaving the subtree matches
         what a dropdown is expected to do, for both mouse and keyboard. -->
    <details
      class="models"
      bind:this={modelsEl}
      onfocusout={(e) => {
        if (!modelsEl?.contains(e.relatedTarget as Node)) modelsEl?.removeAttribute("open");
      }}
    >
      <summary>
        Models
        <span class="count">
          {selectedModels.length === 0 ? "all" : `${selectedModels.length}/${availableModels.length}`}
        </span>
      </summary>
      <div class="list">
        {#each availableModels as m (m)}
          <label>
            <input
              type="checkbox"
              checked={selectedModels.length === 0 || selectedModels.includes(m)}
              onchange={() => toggleModel(m)}
            />
            {modelName(m)}
          </label>
        {/each}
        {#if selectedModels.length > 0}
          <button class="clear" onclick={() => (selectedModels = [])}>Show all</button>
        {/if}
      </div>
    </details>
  {/if}
</div>

<style>
  .filters {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }
  .group {
    display: flex;
    border: 1px solid var(--hair);
    border-radius: 7px;
    overflow: hidden;
  }
  .group button {
    font: inherit;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 5px 10px;
    border: 0;
    border-right: 1px solid var(--hair);
    background: var(--panel);
    color: var(--ink-soft);
    cursor: pointer;
  }
  .group button:last-child {
    border-right: 0;
  }
  .group button:hover:not(.on) {
    background: var(--panel-2);
    color: var(--ink);
  }
  .group button.on {
    background: var(--accent);
    color: var(--accent-ink);
  }

  .dates {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.72rem;
    color: var(--ink-faint);
  }
  .dates input {
    font: inherit;
    font-size: 0.72rem;
    padding: 4px 7px;
    border: 1px solid var(--hair);
    border-radius: 6px;
    background: var(--panel);
    color: var(--ink);
  }

  .models {
    position: relative;
  }
  .models summary {
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--ink-soft);
    padding: 5px 10px;
    border: 1px solid var(--hair);
    border-radius: 7px;
    cursor: pointer;
    list-style: none;
    user-select: none;
  }
  .models summary::-webkit-details-marker {
    display: none;
  }
  .models summary:hover {
    background: var(--panel-2);
    color: var(--ink);
  }
  .count {
    color: var(--ink-faint);
    font-weight: 400;
  }
  .list {
    position: absolute;
    z-index: 5;
    top: calc(100% + 4px);
    left: 0;
    min-width: 190px;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-radius: 8px;
    padding: 7px;
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.16);
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .list label {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 0.75rem;
    padding: 3px 4px;
    border-radius: 5px;
    cursor: pointer;
  }
  .list label:hover {
    background: var(--panel-2);
  }
  .list input {
    accent-color: var(--accent);
    cursor: pointer;
  }
  .clear {
    font: inherit;
    font-size: 0.7rem;
    font-weight: 600;
    margin-top: 4px;
    padding: 4px;
    border: 0;
    border-top: 1px solid var(--hair);
    background: none;
    color: var(--accent);
    cursor: pointer;
    text-align: left;
  }
</style>
