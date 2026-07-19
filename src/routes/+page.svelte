<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";

  type Profile = {
    id: string; label: string; email: string | null; configDir: string;
    subscriptionType: string | null; orgId: string | null; priority: number;
  };
  type Usage = {
    profileId: string; fiveHourPct: number | null; fiveHourResetsAt: string | null;
    weeklyPct: number | null; weeklyResetsAt: string | null; status: string;
  };
  type AddState = { configDir: string; label: string; message: string; error: boolean };

  let profiles = $state<Profile[]>([]);
  let activeId = $state<string | null>(null);
  let usage = $state<Record<string, Usage>>({});
  let add = $state<AddState | null>(null);
  let newLabel = $state("");
  let importPath = $state("");
  let importLabel = $state("");
  let banner = $state("");

  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let pollAttempts = 0;
  const MAX_POLL_ATTEMPTS = 150; // ~5 min at 2s
  let unlisten: (() => void) | null = null;

  async function refresh() {
    profiles = await invoke<Profile[]>("list_profiles");
    activeId = await invoke<string | null>("get_active_profile_id");
    const u = await invoke<Usage[]>("get_usage");
    usage = Object.fromEntries(u.map((x) => [x.profileId, x]));
  }

  onMount(async () => {
    await refresh();
    unlisten = await listen<Usage[]>("usage-updated", (e) => {
      const next = { ...usage };
      for (const u of e.payload) next[u.profileId] = u;
      usage = next;
    });
  });
  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
    unlisten?.();
  });

  const sev = (v: number) => (v >= 90 ? "crit" : v >= 70 ? "warn" : "good");

  function resetsIn(iso: string | null): string {
    if (!iso) return "";
    const ms = new Date(iso).getTime() - Date.now();
    if (ms <= 0) return "resetting…";
    const m = Math.round(ms / 60000);
    return m >= 60 ? `resets in ${Math.floor(m / 60)}h ${m % 60}m` : `resets in ${m}m`;
  }

  async function switchTo(id: string) {
    await invoke("set_active_profile", { id });
    await refresh();
  }
  async function del(p: Profile) {
    if (!confirm(`Remove "${p.label}" from VibeProxy? (Your Claude login is left untouched.)`)) return;
    await invoke("delete_profile", { id: p.id });
    await refresh();
  }

  async function startAdd() {
    const label = newLabel.trim() || "New account";
    try {
      const pending = await invoke<{ configDir: string }>("begin_add_profile");
      add = { configDir: pending.configDir, label, message: "Complete the login in the Terminal window that opened…", error: false };
      newLabel = "";
      pollAttempts = 0;
      pollTimer = setInterval(pollLogin, 2000);
    } catch (e) {
      banner = `Couldn't start login: ${e}`;
    }
  }
  async function pollLogin() {
    if (!add) return;
    if (++pollAttempts > MAX_POLL_ATTEMPTS) {
      stopPoll();
      await invoke("cancel_add_profile", { configDir: add.configDir });
      add = { ...add, message: "Login timed out. Close this and try again.", error: true };
      return;
    }
    try {
      const status = await invoke<{ loggedIn: boolean; email: string | null }>("check_login_status", { configDir: add.configDir });
      if (status.loggedIn) {
        stopPoll();
        try {
          await invoke("adopt_profile", { label: add.label, configDir: add.configDir });
          add = null;
          await refresh();
        } catch (e) {
          add = { ...add, message: `${e}`, error: true };
        }
      }
    } catch { /* still waiting for login — keep polling */ }
  }
  function stopPoll() { if (pollTimer) { clearInterval(pollTimer); pollTimer = null; } }
  async function cancelAdd() {
    stopPoll();
    if (add) await invoke("cancel_add_profile", { configDir: add.configDir });
    add = null;
  }

  async function importDir() {
    const label = importLabel.trim() || "Imported";
    try {
      await invoke("adopt_profile", { label, configDir: importPath.trim() });
      importPath = ""; importLabel = "";
      await refresh();
    } catch (e) {
      banner = `Import failed: ${e}`;
    }
  }
</script>

<main>
  <header><h1>VibeProxy</h1><span class="sub">Claude Code account switcher</span></header>

  {#if banner}<div class="banner" role="alert">{banner} <button class="x" onclick={() => (banner = "")}>×</button></div>{/if}

  <section>
    <h2>Accounts</h2>
    {#if profiles.length === 0}
      <p class="empty">No accounts yet — add one below.</p>
    {/if}
    {#each profiles as p (p.id)}
      {@const u = usage[p.id]}
      <div class="card" class:active={p.id === activeId}>
        <div class="id">
          <div class="name">
            {p.label}
            {#if p.subscriptionType}<span class="tier">{p.subscriptionType}</span>{/if}
            {#if u?.status === "needsReauth"}<span class="chip crit">re-login</span>{/if}
            {#if u?.fiveHourPct != null && u.fiveHourPct >= 90}<span class="chip crit">near limit</span>{/if}
          </div>
          <div class="email">{p.email ?? p.configDir}</div>
        </div>
        <div class="actions">
          {#if p.id === activeId}
            <button class="btn ghost" disabled>✓ Active</button>
          {:else}
            <button class="btn" onclick={() => switchTo(p.id)}>Switch</button>
          {/if}
          <button class="btn icon" title="Remove" onclick={() => del(p)}>Remove</button>
        </div>
        <div class="usage">
          <div class="metric">
            <span class="k">5-hour</span>
            <span class="bar"><i class={u?.fiveHourPct != null ? `fill-${sev(u.fiveHourPct)}` : ""} style={`width:${u?.fiveHourPct ?? 0}%`}></i></span>
            <span class="v">{u?.fiveHourPct != null ? Math.round(u.fiveHourPct) + "%" : "—"}</span>
          </div>
          <div class="metric">
            <span class="k">weekly</span>
            <span class="bar"><i class={u?.weeklyPct != null ? `fill-${sev(u.weeklyPct)}` : ""} style={`width:${u?.weeklyPct ?? 0}%`}></i></span>
            <span class="v">{u?.weeklyPct != null ? Math.round(u.weeklyPct) + "%" : "—"}</span>
          </div>
          {#if u?.fiveHourResetsAt}<div class="reset">{resetsIn(u.fiveHourResetsAt)}</div>{/if}
        </div>
      </div>
    {/each}
  </section>

  <section>
    <h2>Add account</h2>
    {#if add}
      <div class="adding" class:err={add.error}>
        <p>{add.message}</p>
        <div class="row">
          <button class="btn" onclick={cancelAdd}>Cancel</button>
          {#if !add.error}<span class="spinner">Waiting for login…</span>{/if}
        </div>
      </div>
    {:else}
      <div class="row">
        <input placeholder="Label (e.g. Work)" bind:value={newLabel} />
        <button class="btn primary" onclick={startAdd}>Add via login</button>
      </div>
      <details class="import">
        <summary>Import an existing config dir</summary>
        <div class="row">
          <input placeholder="Label" bind:value={importLabel} />
          <input placeholder="~/vp-spike or /path/to/dir" bind:value={importPath} />
          <button class="btn" onclick={importDir}>Import</button>
        </div>
      </details>
    {/if}
  </section>
</main>

<style>
  :root {
    --panel: #fbfaf8; --panel-2: #f1eee9; --panel-3: #e9e5de; --ink: #26231f; --ink-soft: #6e675e;
    --ink-faint: #837a6e; --hair: #e3ded5; --accent: #c4623f; --good: #3e9b5f; --warn: #cf9422; --crit: #ce4530;
    --bar: #e6e1d8;
    color: var(--ink); background: var(--panel);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  }
  :global(body) { margin: 0; }
  main { padding: 14px 16px 22px; }
  header { display: flex; align-items: baseline; gap: 8px; }
  h1 { margin: 0; font-size: 1.15rem; }
  .sub { color: var(--ink-soft); font-size: .8rem; }
  h2 { font-size: .72rem; text-transform: uppercase; letter-spacing: .06em; color: var(--ink-faint); margin: 18px 0 8px; }
  .empty { color: var(--ink-faint); font-size: .9rem; }
  .banner { background: color-mix(in srgb, var(--crit) 12%, transparent); color: var(--crit); padding: 8px 10px; border-radius: 8px; font-size: .85rem; margin-top: 8px; display: flex; }
  .banner .x { margin-left: auto; background: none; border: 0; color: inherit; cursor: pointer; font-size: 1rem; }

  .card { border: 1px solid var(--hair); border-radius: 10px; padding: 11px 12px; margin-bottom: 9px;
    display: grid; grid-template-columns: 1fr auto; gap: 8px 10px; }
  .card.active { border-color: color-mix(in srgb, var(--accent) 55%, transparent); box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent); }
  .name { font-weight: 600; font-size: .95rem; display: flex; align-items: center; gap: 7px; flex-wrap: wrap; }
  .tier { font-size: .62rem; text-transform: uppercase; letter-spacing: .03em; background: var(--panel-3); color: var(--ink-soft); padding: 1px 5px; border-radius: 4px; font-weight: 700; }
  .chip { font-size: .62rem; text-transform: uppercase; font-weight: 700; padding: 1px 5px; border-radius: 4px; }
  .chip.crit { color: var(--crit); background: color-mix(in srgb, var(--crit) 14%, transparent); }
  .email { font-size: .78rem; color: var(--ink-soft); margin-top: 2px; word-break: break-all; }
  .actions { display: flex; gap: 6px; align-items: start; }
  .usage { grid-column: 1 / -1; display: grid; grid-template-columns: 1fr 1fr auto; gap: 4px 16px; align-items: center; }
  .metric { display: flex; align-items: center; gap: 7px; }
  .metric .k { font-size: .68rem; color: var(--ink-faint); width: 42px; }
  .bar { flex: 1; height: 5px; border-radius: 3px; background: var(--bar); overflow: hidden; }
  .bar > i { display: block; height: 100%; border-radius: 3px; transition: width .3s ease-out; }
  .fill-good { background: var(--good); } .fill-warn { background: var(--warn); } .fill-crit { background: var(--crit); }
  .metric .v { font-size: .72rem; font-variant-numeric: tabular-nums; color: var(--ink-soft); width: 34px; text-align: right; }
  .reset { grid-column: 1 / -1; font-size: .7rem; color: var(--ink-faint); }

  .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  input { flex: 1; min-width: 120px; font: inherit; font-size: .85rem; padding: 6px 9px; border: 1px solid var(--hair); border-radius: 7px; background: var(--panel); color: var(--ink); }
  .btn { font: inherit; font-size: .8rem; font-weight: 600; padding: 6px 12px; border-radius: 7px; cursor: pointer; border: 1px solid var(--hair); background: var(--panel-2); color: var(--ink); }
  .btn:hover { background: var(--panel-3); }
  .btn.primary { background: var(--accent); border-color: transparent; color: #fff; }
  .btn.ghost { color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); border-color: transparent; }
  .btn.icon { color: var(--ink-soft); }
  .btn:disabled { cursor: default; }
  .adding { border: 1px solid var(--hair); border-radius: 9px; padding: 11px 12px; font-size: .85rem; }
  .adding.err { border-color: var(--crit); color: var(--crit); }
  .adding p { margin: 0 0 8px; }
  .spinner { color: var(--ink-soft); font-size: .8rem; }
  .import { margin-top: 10px; }
  .import summary { font-size: .8rem; color: var(--ink-soft); cursor: pointer; }
  .import .row { margin-top: 8px; }

  @media (prefers-color-scheme: dark) {
    :root {
      --panel: #232120; --panel-2: #2c2a27; --panel-3: #35322e; --ink: #f1ede6; --ink-soft: #a79e92;
      --ink-faint: #928a7e; --hair: #35312c; --accent: #e0805c; --good: #58b776; --warn: #e3b457; --crit: #e0654e; --bar: #3a352f;
    }
  }
  :focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
