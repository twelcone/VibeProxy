<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";

  let { range, onstatus }: { range: () => object | null; onstatus: (msg: string) => void } = $props();

  let busy = $state(false);

  /** Default filename carries the date so repeat exports don't silently overwrite each other. */
  const suggested = () => `vibeproxy-usage-${new Date().toLocaleDateString("sv")}.csv`;

  async function exportCsv() {
    if (busy) return;
    busy = true;
    try {
      const path = await save({
        defaultPath: suggested(),
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) return; // user cancelled — not an error, say nothing
      await invoke("export_usage_csv", { range: range(), path });
      onstatus(`Exported to ${path}`);
    } catch (e) {
      onstatus(`Export failed: ${e}`);
    } finally {
      busy = false;
    }
  }
</script>

<button
  class="btn"
  onclick={exportCsv}
  disabled={busy}
  title="Export the selected date range as CSV — all accounts, models and projects (the model filter is a view setting and is not applied)"
>
  {busy ? "Exporting…" : "Export CSV"}
</button>

<style>
  .btn {
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    padding: 6px 12px;
    border-radius: 7px;
    cursor: pointer;
    border: 1px solid var(--hair);
    background: var(--panel-2);
    color: var(--ink);
  }
  .btn:hover:not(:disabled) {
    background: var(--panel-3);
  }
  .btn:disabled {
    cursor: default;
    color: var(--ink-faint);
  }
</style>
