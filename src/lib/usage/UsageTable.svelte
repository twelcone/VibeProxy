<script lang="ts">
  import { fullTokens, tokens, usd } from "$lib/format";

  type Row = {
    key: string;
    label: string;
    input: number;
    output: number;
    cacheWrite: number;
    cacheRead: number;
    total: number;
    value: number | null;
  };
  type SortKey = "label" | "input" | "output" | "cacheWrite" | "cacheRead" | "total" | "value";

  let { rows, firstColumn }: { rows: Row[]; firstColumn: string } = $props();

  let sortKey = $state<SortKey>("total");
  let ascending = $state(false);

  const columns: Array<{ key: SortKey; label: string; numeric: boolean }> = $derived([
    { key: "label", label: firstColumn, numeric: false },
    { key: "input", label: "Input", numeric: true },
    { key: "output", label: "Output", numeric: true },
    { key: "cacheWrite", label: "Cache write", numeric: true },
    { key: "cacheRead", label: "Cache read", numeric: true },
    { key: "total", label: "Total", numeric: true },
    { key: "value", label: "API value", numeric: true },
  ]);

  const sorted = $derived(
    [...rows].sort((a, b) => {
      const dir = ascending ? 1 : -1;
      if (sortKey === "label") return a.label.localeCompare(b.label) * dir;
      // Unpriced models sort as if zero rather than jumping to the top on a null compare.
      const av = a[sortKey] ?? 0;
      const bv = b[sortKey] ?? 0;
      return (av - bv) * dir;
    }),
  );

  function sortBy(key: SortKey) {
    if (sortKey === key) ascending = !ascending;
    else {
      sortKey = key;
      ascending = key === "label"; // names read best A→Z, numbers biggest-first
    }
  }

  const ariaSort = (key: SortKey) =>
    sortKey === key ? (ascending ? "ascending" : "descending") : "none";
</script>

<div class="wrap">
  <table>
    <thead>
      <tr>
        {#each columns as c (c.key)}
          <th class:num={c.numeric} aria-sort={ariaSort(c.key)}>
            <button onclick={() => sortBy(c.key)}>
              {c.label}<span class="arrow" aria-hidden="true"
                >{sortKey === c.key ? (ascending ? "▲" : "▼") : ""}</span
              >
            </button>
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each sorted as r (r.key)}
        <tr>
          <th scope="row" title={r.label}>{r.label}</th>
          <td title={fullTokens(r.input)}>{tokens(r.input)}</td>
          <td title={fullTokens(r.output)}>{tokens(r.output)}</td>
          <td title={fullTokens(r.cacheWrite)}>{tokens(r.cacheWrite)}</td>
          <td title={fullTokens(r.cacheRead)}>{tokens(r.cacheRead)}</td>
          <td class="strong" title={fullTokens(r.total)}>{tokens(r.total)}</td>
          <td>{usd(r.value)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .wrap {
    border: 1px solid var(--hair);
    border-radius: 10px;
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8rem;
  }
  thead th {
    text-align: left;
    border-bottom: 1px solid var(--hair);
    background: var(--panel-2);
    white-space: nowrap;
  }
  thead th button {
    font: inherit;
    font-size: 0.63rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
    color: var(--ink-faint);
    background: none;
    border: 0;
    padding: 8px 10px;
    width: 100%;
    text-align: inherit;
    cursor: pointer;
  }
  thead th button:hover {
    color: var(--ink);
  }
  .arrow {
    margin-left: 4px;
    font-size: 0.7em;
  }
  th.num button {
    text-align: right;
  }
  tbody th {
    text-align: left;
    font-weight: 550;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  tbody td {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--ink-soft);
  }
  tbody td.strong {
    color: var(--ink);
    font-weight: 600;
  }
  tbody th,
  tbody td {
    padding: 7px 10px;
    border-bottom: 1px solid var(--hair);
  }
  tbody tr:last-child th,
  tbody tr:last-child td {
    border-bottom: 0;
  }
  tbody tr:hover th,
  tbody tr:hover td {
    background: var(--panel-2);
  }
</style>
