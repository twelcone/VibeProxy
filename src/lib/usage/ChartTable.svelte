<script lang="ts">
  /**
   * Accessible data-table fallback for a chart. Every chart offers this — an SVG shape is not
   * readable by a screen reader, and precise values are hard to pull off any line.
   */
  let {
    caption,
    columnLabel,
    rows,
    columns,
    format,
  }: {
    caption: string;
    columnLabel: string;
    rows: string[];
    columns: { key: string; label: string }[];
    format: (row: string, col: string) => string;
  } = $props();
</script>

<div class="wrap">
  <table>
    <caption>{caption}</caption>
    <thead>
      <tr>
        <th scope="col">{columnLabel}</th>
        {#each columns as c (c.key)}<th scope="col" class="num">{c.label}</th>{/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as r (r)}
        <tr>
          <th scope="row">{r}</th>
          {#each columns as c (c.key)}<td>{format(r, c.key)}</td>{/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .wrap {
    max-height: 260px;
    overflow: auto;
    border: 1px solid var(--hair);
    border-radius: 8px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.75rem;
  }
  caption {
    text-align: left;
    padding: 7px 10px;
    font-size: 0.68rem;
    color: var(--ink-faint);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
  }
  thead th {
    position: sticky;
    top: 0;
    background: var(--panel-2);
    text-align: left;
    font-size: 0.63rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-faint);
    padding: 6px 10px;
    border-bottom: 1px solid var(--hair);
    white-space: nowrap;
  }
  thead th.num {
    text-align: right;
  }
  tbody th {
    text-align: left;
    font-weight: 550;
    white-space: nowrap;
  }
  tbody td {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--ink-soft);
  }
  tbody th,
  tbody td {
    padding: 5px 10px;
    border-bottom: 1px solid var(--hair);
  }
  tbody tr:last-child th,
  tbody tr:last-child td {
    border-bottom: 0;
  }
</style>
