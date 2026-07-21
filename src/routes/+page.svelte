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
  type Settings = {
    autoSwitchEnabled: boolean; thresholdPct: number; pollIntervalSecs: number;
    cooldownSecs: number; launchAtLogin: boolean;
  };

  let profiles = $state<Profile[]>([]);
  let activeId = $state<string | null>(null);
  let usage = $state<Record<string, Usage>>({});
  let add = $state<AddState | null>(null);
  let newLabel = $state("");
  let importPath = $state("");
  let importLabel = $state("");
  let banner = $state("");

  let notice = $state("");
  let settings = $state<Settings | null>(null);
  let activity = $state<string[]>([]);
  let copied = $state(false);
  const INTEGRATION_SNIPPET =
    `_vp="$(cat ~/.vibeproxy/active-path 2>/dev/null)"; [ -n "$_vp" ] && export CLAUDE_CONFIG_DIR="$_vp" || unset CLAUDE_CONFIG_DIR`;

  function logActivity(msg: string) {
    const t = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    activity = [`${t} · ${msg}`, ...activity].slice(0, 12);
  }
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let pollAttempts = 0;
  const MAX_POLL_ATTEMPTS = 150; // ~5 min at 2s
  let unlisteners: Array<() => void> = [];

  async function refresh() {
    profiles = await invoke<Profile[]>("list_profiles");
    activeId = await invoke<string | null>("get_active_profile_id");
    const u = await invoke<Usage[]>("get_usage");
    usage = Object.fromEntries(u.map((x) => [x.profileId, x]));
    settings = await invoke<Settings>("get_settings");
  }

  async function saveSettings() {
    if (!settings) return;
    try { settings = await invoke<Settings>("set_settings", { settings }); }
    catch (e) { banner = `${e}`; }
  }
  async function openUsage() {
    try { await invoke("open_usage_window"); } catch (e) { banner = `${e}`; }
  }
  async function copySnippet() {
    try { await navigator.clipboard.writeText(INTEGRATION_SNIPPET); copied = true; setTimeout(() => (copied = false), 1500); }
    catch { /* clipboard unavailable */ }
  }

  onMount(async () => {
    await refresh();
    unlisteners.push(
      await listen<Usage[]>("usage-updated", (e) => {
        const next = { ...usage };
        for (const u of e.payload) next[u.profileId] = u;
        usage = next;
      }),
    );
    unlisteners.push(
      await listen<{ from: string; to: string; pct: number }>("auto-switched", async (e) => {
        notice = `Auto-switched to ${e.payload.to} — ${e.payload.from} hit ${e.payload.pct}%.`;
        logActivity(`Auto-switched to ${e.payload.to} (${e.payload.from} hit ${e.payload.pct}%)`);
        await refresh();
      }),
    );
    unlisteners.push(
      await listen<{ active: string; pct: number }>("auto-switch-blocked", (e) => {
        notice = `All accounts near their limit (${e.payload.active} at ${e.payload.pct}%).`;
      }),
    );
  });
  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
    for (const u of unlisteners) u();
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
    const p = profiles.find((x) => x.id === id);
    if (p) logActivity(`Switched to ${p.label} (manual)`);
    await refresh();
  }
  async function relaunch() {
    try { await invoke("relaunch_claude"); } catch (e) { banner = `${e}`; }
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
    const a = add;
    if (!a) return;
    if (++pollAttempts > MAX_POLL_ATTEMPTS) {
      stopPoll();
      await invoke("cancel_add_profile", { configDir: a.configDir });
      add = { configDir: a.configDir, label: a.label, message: "Login timed out. Close this and try again.", error: true };
      return;
    }
    try {
      const status = await invoke<{ loggedIn: boolean; email: string | null }>("check_login_status", { configDir: a.configDir });
      if (status.loggedIn) {
        stopPoll();
        try {
          await invoke("adopt_profile", { label: a.label, configDir: a.configDir });
          logActivity(`Added ${a.label}`);
          add = null;
          await refresh();
        } catch (e) {
          add = { configDir: a.configDir, label: a.label, message: `${e}`, error: true };
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
  <header>
    <h1>VibeProxy</h1><span class="sub">Claude Code account switcher</span>
    <button class="btn small usage-link" onclick={openUsage}>Usage</button>
  </header>

  {#if banner}<div class="banner" role="alert">{banner} <button class="x" onclick={() => (banner = "")}>×</button></div>{/if}
  {#if notice}<div class="notice" role="status">{notice} <button class="x" onclick={() => (notice = "")}>×</button></div>{/if}

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
            <button class="btn" onclick={relaunch} title="Open a terminal on this account">Relaunch</button>
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

  {#if settings}
    <section>
      <h2>Automatic switching</h2>
      <div class="settings">
        <label class="set">
          <span><b>Auto-switch on quota</b><small>Move to the freshest account before this one runs out</small></span>
          <input type="checkbox" bind:checked={settings.autoSwitchEnabled} onchange={saveSettings} />
        </label>
        <label class="set">
          <span><b>Switch threshold</b><small>Trigger when 5-hour or weekly usage crosses this</small></span>
          <span class="ctl"><input type="range" min="50" max="100" bind:value={settings.thresholdPct} onchange={saveSettings} /><em>{settings.thresholdPct}%</em></span>
        </label>
        <label class="set">
          <span><b>Refresh usage every</b><small>How often to poll each account</small></span>
          <span class="ctl"><input class="num" type="number" min="60" step="30" bind:value={settings.pollIntervalSecs} onchange={saveSettings} /><em>s</em></span>
        </label>
        <label class="set">
          <span><b>Launch at login</b></span>
          <input type="checkbox" bind:checked={settings.launchAtLogin} onchange={saveSettings} />
        </label>
      </div>
    </section>
  {/if}

  <section>
    <h2>Claude Code integration</h2>
    <p class="hint2">Add this to your shell profile (e.g. <code>~/.zshrc</code>) so new terminals use the active account:</p>
    <div class="snippet"><code>{INTEGRATION_SNIPPET}</code><button class="btn small" onclick={copySnippet}>{copied ? "Copied ✓" : "Copy"}</button></div>
  </section>

  {#if activity.length}
    <section>
      <h2>Activity</h2>
      <ul class="activity">
        {#each activity as a}<li>{a}</li>{/each}
      </ul>
    </section>
  {/if}
</main>

<style>
  /* Color/type tokens live in src/lib/styles/tokens.css (shared with the Usage window). */
  main { padding: 14px 16px 22px; }
  header { display: flex; align-items: baseline; gap: 8px; }
  h1 { margin: 0; font-size: 1.15rem; }
  .sub { color: var(--ink-soft); font-size: .8rem; }
  h2 { font-size: .72rem; text-transform: uppercase; letter-spacing: .06em; color: var(--ink-faint); margin: 18px 0 8px; }
  .empty { color: var(--ink-faint); font-size: .9rem; }
  .banner { background: color-mix(in srgb, var(--crit) 12%, transparent); color: var(--crit); padding: 8px 10px; border-radius: 8px; font-size: .85rem; margin-top: 8px; display: flex; }
  .banner .x { margin-left: auto; background: none; border: 0; color: inherit; cursor: pointer; font-size: 1rem; }
  .notice { background: color-mix(in srgb, var(--accent) 12%, transparent); color: var(--accent); padding: 8px 10px; border-radius: 8px; font-size: .85rem; margin-top: 8px; display: flex; }
  .notice .x { margin-left: auto; background: none; border: 0; color: inherit; cursor: pointer; font-size: 1rem; }

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

  .settings { border: 1px solid var(--hair); border-radius: 10px; overflow: hidden; }
  .set { display: flex; align-items: center; gap: 12px; padding: 11px 12px; border-bottom: 1px solid var(--hair); cursor: pointer; }
  .set:last-child { border-bottom: 0; }
  .set > span:first-child { display: flex; flex-direction: column; }
  .set b { font-size: .9rem; font-weight: 600; }
  .set small { font-size: .75rem; color: var(--ink-soft); }
  .set .ctl { margin-left: auto; display: flex; align-items: center; gap: 8px; }
  .set input[type="checkbox"] { margin-left: auto; width: 18px; height: 18px; accent-color: var(--accent); cursor: pointer; }
  .set input[type="range"] { accent-color: var(--accent); width: 120px; }
  .set .num { width: 64px; font: inherit; padding: 4px 6px; border: 1px solid var(--hair); border-radius: 6px; background: var(--panel); color: var(--ink); }
  .set em { font-style: normal; font-variant-numeric: tabular-nums; color: var(--ink-soft); font-size: .8rem; min-width: 34px; text-align: right; }
  .hint2 { font-size: .8rem; color: var(--ink-soft); margin: 0 0 6px; }
  code { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: .85em; background: var(--panel-3); padding: 1px 4px; border-radius: 4px; }
  .snippet { display: flex; align-items: center; gap: 8px; background: var(--panel-3); border-radius: 8px; padding: 8px 10px; }
  .snippet code { font-size: .72rem; color: var(--ink-soft); overflow-x: auto; white-space: nowrap; flex: 1; background: none; padding: 0; }
  .btn.small { padding: 4px 9px; font-size: .72rem; }
  .activity { list-style: none; margin: 0; padding: 0; border: 1px solid var(--hair); border-radius: 10px; overflow: hidden; }
  .activity li { padding: 7px 12px; border-bottom: 1px solid var(--hair); font-size: .8rem; color: var(--ink-soft); font-variant-numeric: tabular-nums; }
  .activity li:last-child { border-bottom: 0; }

</style>
