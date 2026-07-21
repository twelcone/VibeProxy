<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import KpiCard from "$lib/usage/KpiCard.svelte";
  import BarRow from "$lib/usage/BarRow.svelte";
  import UsageTable from "$lib/usage/UsageTable.svelte";
  import TrendChart, { type Series } from "$lib/usage/TrendChart.svelte";
  import CacheChart from "$lib/usage/CacheChart.svelte";
  import Filters, { type Granularity, type RangePreset } from "$lib/usage/Filters.svelte";
  import ExportMenu from "$lib/usage/ExportMenu.svelte";
  import Icon from "$lib/ui/Icon.svelte";
  import { weekStart } from "$lib/chart/svg";
  import { seriesColor } from "$lib/series-palette";
  import { fullTokens, modelName, pct, perMtok, projectName, tokens, usd } from "$lib/format";

  type Tokens = { input: number; output: number; cacheWrite: number; cacheRead: number };
  type AccountRow = { account: string; tokens: Tokens; messages: number; value: number | null };
  type ModelRow = { model: string; tokens: Tokens; messages: number; value: number | null };
  type DayRow = { date: string; tokens: Tokens; value: number | null };
  type ProjectRow = { project: string; tokens: Tokens; value: number | null };
  type ModelDayRow = { date: string; model: string; tokens: Tokens; value: number | null };
  type AccountDayRow = { date: string; account: string; tokens: Tokens; value: number | null };
  type Analytics = {
    totals: Tokens;
    messageCount: number;
    perAccount: AccountRow[];
    perModel: ModelRow[];
    perDay: DayRow[];
    perProject: ProjectRow[];
    perModelPerDay: ModelDayRow[];
    perAccountPerDay: AccountDayRow[];
    totalValue: number;
    pricedAsOf: string;
    unpricedModels: string[];
  };
  type Settings = {
    autoSwitchEnabled: boolean;
    thresholdPct: number;
    pollIntervalSecs: number;
    cooldownSecs: number;
    launchAtLogin: boolean;
    monthlyCostUsd: Record<string, number>;
  };

  const PROJECT_LIMIT = 8;
  /** Beyond this many trend series the chart becomes unreadable; the rest fold into "Other". */
  const SERIES_LIMIT = 6;

  let data = $state<Analytics | null>(null);
  let settings = $state<Settings | null>(null);
  let error = $state("");
  let loading = $state(true);
  let showAllProjects = $state(false);
  let tableMode = $state<"model" | "project">("model");

  // Filter state. Range is applied in Rust (re-query); model selection is applied client-side, since
  // the aggregate already carries the per-model split.
  let preset = $state<RangePreset>("30d");
  let customFrom = $state("");
  let customTo = $state("");
  let granularity = $state<Granularity>("day");
  let groupBy = $state<"model" | "account">("model");
  let selectedModels = $state<string[]>([]);
  /**
   * `share` normalizes each bucket to 100%. Without it the chart is effectively single-series on
   * real data: one model routinely accounts for ~95% of tokens, pinning every other line to the axis.
   */
  let metric = $state<"tokens" | "value" | "share">("tokens");

  const sum = (t: Tokens) => t.input + t.output + t.cacheWrite + t.cacheRead;
  const isoDay = (d: Date) => d.toLocaleDateString("sv"); // sv → YYYY-MM-DD in local time

  /** Presets resolve to a concrete local-date range; `all` sends null so Rust scans everything. */
  function currentRange(): { from?: string; to?: string } | null {
    if (preset === "all") return null;
    if (preset === "custom") {
      if (!customFrom && !customTo) return null;
      return { from: customFrom || undefined, to: customTo || undefined };
    }
    const days = preset === "7d" ? 7 : 30;
    const to = new Date();
    const from = new Date();
    from.setDate(from.getDate() - (days - 1));
    return { from: isoDay(from), to: isoDay(to) };
  }

  /** Announced to screen readers after a load, so a refresh isn't a silent change. */
  let status = $state("");

  async function load(opts: { hard?: boolean } = {}) {
    loading = true;
    error = "";
    try {
      // A hard refresh drops the Rust-side cache so edited/rotated logs are re-read.
      if (opts.hard) await invoke("refresh_usage_analytics");
      [data, settings] = await Promise.all([
        invoke<Analytics>("get_usage_analytics", { range: currentRange() }),
        invoke<Settings>("get_settings"),
      ]);
      status = `Updated. ${fullTokens(totalTokens)} tokens across ${data.perModel.length} models.`;
    } catch (e) {
      error = `${e}`;
      status = "Failed to load usage.";
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
    // Claude Code appends to the logs while this window sits open; refocusing is the natural
    // "is this still current?" moment. The Rust mtime cache makes an unchanged re-scan nearly free.
    const onFocus = () => load();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });

  const availableModels = $derived(data?.perModel.map((m) => m.model) ?? []);
  const modelFilterActive = $derived(
    selectedModels.length > 0 && selectedModels.length < availableModels.length,
  );
  const showsModel = (m: string) => !modelFilterActive || selectedModels.includes(m);

  /** `per_model_per_day` is the finest grain the backend emits, so every model-filtered figure
   *  derives from it — that keeps the KPIs, bars and charts from disagreeing. */
  const grain = $derived((data?.perModelPerDay ?? []).filter((r) => showsModel(r.model)));

  const bucketOf = (iso: string) => (granularity === "week" ? weekStart(iso) : iso);

  const filteredTotals = $derived.by(() => {
    const t: Tokens = { input: 0, output: 0, cacheWrite: 0, cacheRead: 0 };
    let value = 0;
    for (const r of grain) {
      t.input += r.tokens.input;
      t.output += r.tokens.output;
      t.cacheWrite += r.tokens.cacheWrite;
      t.cacheRead += r.tokens.cacheRead;
      value += r.value ?? 0;
    }
    return { tokens: t, value };
  });

  const totalTokens = $derived(sum(filteredTotals.tokens));
  const isEmpty = $derived(!!data && (data.perModelPerDay?.length ?? 0) === 0);

  /** Sorted bucket keys present in the filtered data. */
  const trendDates = $derived([...new Set(grain.map((r) => bucketOf(r.date)))].sort());

  const metricOf = (r: { tokens: Tokens; value: number | null }) =>
    metric === "value" ? (r.value ?? 0) : sum(r.tokens);

  const trendSeries = $derived.by<Series[]>(() => {
    const rows: { key: string; date: string; v: number }[] =
      groupBy === "model"
        ? grain.map((r) => ({ key: r.model, date: r.date, v: metricOf(r) }))
        : (data?.perAccountPerDay ?? []).map((r) => ({
            key: r.account,
            date: r.date,
            v: metricOf(r),
          }));

    const totalByKey = new Map<string, number>();
    for (const r of rows) totalByKey.set(r.key, (totalByKey.get(r.key) ?? 0) + r.v);
    const ranked = [...totalByKey.entries()].sort((a, b) => b[1] - a[1]).map(([k]) => k);
    const top = new Set(ranked.slice(0, SERIES_LIMIT));
    const hasOther = ranked.length > SERIES_LIMIT;

    const acc = new Map<string, Map<string, number>>();
    for (const r of rows) {
      const key = top.has(r.key) ? r.key : "__other";
      const bucket = bucketOf(r.date);
      if (!acc.has(key)) acc.set(key, new Map());
      const m = acc.get(key)!;
      m.set(bucket, (m.get(bucket) ?? 0) + r.v);
    }

    // Share mode: restate each bucket as percentages of that bucket's total.
    if (metric === "share") {
      const bucketTotals = new Map<string, number>();
      for (const m of acc.values()) {
        for (const [bucket, v] of m) bucketTotals.set(bucket, (bucketTotals.get(bucket) ?? 0) + v);
      }
      for (const m of acc.values()) {
        for (const [bucket, v] of m) {
          const t = bucketTotals.get(bucket) ?? 0;
          m.set(bucket, t > 0 ? (v / t) * 100 : 0);
        }
      }
    }

    const label = (k: string) =>
      k === "__other" ? `Other (${ranked.length - SERIES_LIMIT})` : groupBy === "model" ? modelName(k) : k;

    return [...(hasOther ? [...ranked.slice(0, SERIES_LIMIT), "__other"] : ranked)]
      .filter((k) => acc.has(k))
      .map((k) => ({ key: k, label: label(k), values: acc.get(k)! }));
  });

  const cacheDays = $derived.by(() => {
    const acc = new Map<string, { date: string; input: number; cacheWrite: number; cacheRead: number }>();
    for (const r of grain) {
      const date = bucketOf(r.date);
      const e = acc.get(date) ?? { date, input: 0, cacheWrite: 0, cacheRead: 0 };
      e.input += r.tokens.input;
      e.cacheWrite += r.tokens.cacheWrite;
      e.cacheRead += r.tokens.cacheRead;
      acc.set(date, e);
    }
    return [...acc.values()].sort((a, b) => a.date.localeCompare(b.date));
  });

  /** Burn rate — the number that actually matters for a quota-limited plan. Averaged over days that
   *  had activity, not calendar days, so a weekend off doesn't deflate it. */
  const activeDays = $derived(new Set(grain.map((r) => r.date)).size);
  const dailyAverage = $derived(activeDays > 0 ? totalTokens / activeDays : 0);

  const todayIso = new Date().toLocaleDateString("sv"); // sv locale → YYYY-MM-DD, local time
  const todayTokens = $derived(
    grain.filter((r) => r.date === todayIso).reduce((n, r) => n + sum(r.tokens), 0),
  );

  /**
   * How many months the loaded range spans. A subscription is billed monthly, so comparing a
   * multi-month token total against a single month's fee would flatter the numbers badly.
   */
  const spanMonths = $derived.by(() => {
    const days = [...new Set(grain.map((r) => r.date))].sort();
    if (days.length === 0) return 0;
    const ms = new Date(days[days.length - 1]).getTime() - new Date(days[0]).getTime();
    return Math.max(ms / 86_400_000 + 1, 1) / 30.44;
  });

  const shownModels = $derived((data?.perModel ?? []).filter((m) => showsModel(m.model)));
  const accountMax = $derived(Math.max(1, ...(data?.perAccount ?? []).map((a) => sum(a.tokens))));
  const modelMax = $derived(Math.max(1, ...shownModels.map((m) => sum(m.tokens))));
  const projects = $derived(
    [...(data?.perProject ?? [])].sort((a, b) => sum(b.tokens) - sum(a.tokens)),
  );
  const projectMax = $derived(Math.max(1, ...projects.map((p) => sum(p.tokens))));
  const shownProjects = $derived(showAllProjects ? projects : projects.slice(0, PROJECT_LIMIT));

  /** What the subscription actually cost per Mtok over the logged span, vs the API list price. */
  function effective(account: AccountRow) {
    const monthly = settings?.monthlyCostUsd?.[account.account];
    if (!monthly || spanMonths <= 0) return null;
    const spent = monthly * spanMonths;
    const mtok = sum(account.tokens) / 1_000_000;
    if (mtok <= 0) return null;
    return { spent, rate: spent / mtok, saved: (account.value ?? 0) - spent };
  }

  async function setMonthlyCost(accountLabel: string, raw: string) {
    if (!settings) return;
    const parsed = Number.parseFloat(raw);
    const next = { ...settings.monthlyCostUsd };
    if (Number.isFinite(parsed) && parsed > 0) next[accountLabel] = parsed;
    else delete next[accountLabel];
    try {
      settings = await invoke<Settings>("set_settings", {
        settings: { ...settings, monthlyCostUsd: next },
      });
    } catch (e) {
      error = `Couldn't save the monthly cost: ${e}`;
    }
  }

  const tableRows = $derived.by(() => {
    if (!data) return [];
    const src =
      tableMode === "model"
        ? shownModels.map((m) => ({ key: m.model, label: modelName(m.model), t: m.tokens, value: m.value }))
        : projects.map((p) => ({ key: p.project, label: projectName(p.project), t: p.tokens, value: p.value }));
    return src.map((r) => ({
      key: r.key,
      label: r.label,
      input: r.t.input,
      output: r.t.output,
      cacheWrite: r.t.cacheWrite,
      cacheRead: r.t.cacheRead,
      total: sum(r.t),
      value: r.value,
    }));
  });
</script>

<svelte:head><title>Usage Analytics — VibeProxy</title></svelte:head>

<main>
  <header>
    <div>
      <h1>Usage Analytics</h1>
      <p class="sub">Token usage across every account, read from Claude Code's local logs.</p>
    </div>
    <div class="actions">
      <ExportMenu range={currentRange} onstatus={(m) => (status = m)} />
      <button class="btn" onclick={() => load({ hard: true })} disabled={loading}>
        {loading ? "Loading…" : "Refresh"}
      </button>
    </div>
  </header>

  <p class="sr-only" role="status" aria-live="polite">{status}</p>

  {#if error}
    <div class="banner" role="alert">
      {error}
      <button class="btn small" onclick={() => load({ hard: true })}>Retry</button>
    </div>
  {/if}

  {#if loading && !data}
    <div class="kpis">
      {#each Array(4) as _, i (i)}<div class="skeleton"></div>{/each}
    </div>
    <div class="skeleton tall"></div>
  {:else if isEmpty}
    <div class="empty">
      <h2>No usage yet</h2>
      <p>
        Once you've used Claude Code on one of your accounts, its local logs will show up here. Nothing
        is uploaded — VibeProxy only reads the files already on this Mac.
      </p>
    </div>
  {:else if data}
    <Filters
      bind:preset bind:from={customFrom} bind:to={customTo}
      bind:granularity bind:groupBy bind:selectedModels
      {availableModels}
      onchange={load}
    />

    <section class="kpis">
      <KpiCard label="Total tokens" value={tokens(totalTokens)} title={fullTokens(totalTokens)}
        icon="hash" tint="var(--series-1)"
        sub={modelFilterActive ? "selected models" : `${fullTokens(data.messageCount)} messages`} />
      <KpiCard label="API-equivalent value" value={usd(filteredTotals.value)}
        icon="dollar" tint="var(--good)"
        sub="what these tokens would cost on the API" />
      <KpiCard label="Daily average" value={tokens(dailyAverage)} title={fullTokens(Math.round(dailyAverage))}
        icon="trending" tint="var(--series-2)"
        sub={`over ${activeDays} active ${activeDays === 1 ? "day" : "days"}`} />
      <KpiCard label="Tokens today" value={tokens(todayTokens)} title={fullTokens(todayTokens)}
        icon="calendar" tint="var(--series-3)"
        sub={todayIso} />
    </section>

    <section>
      <div class="section-head">
        <h2><Icon name="trending" size={13} />Over time</h2>
        <div class="toggle" role="group" aria-label="Trend metric">
          <button class:on={metric === "tokens"} onclick={() => (metric = "tokens")}
            aria-pressed={metric === "tokens"}>Tokens</button>
          <button class:on={metric === "value"} onclick={() => (metric = "value")}
            aria-pressed={metric === "value"}>API value</button>
          <button class:on={metric === "share"} onclick={() => (metric = "share")}
            aria-pressed={metric === "share"}>Share</button>
        </div>
      </div>
      <TrendChart
        dates={trendDates}
        series={trendSeries}
        format={metric === "value" ? usd : metric === "share" ? pct : tokens}
        title={`${metric === "value" ? "API-equivalent value" : metric === "share" ? "Share of tokens" : "Tokens"} per ${granularity === "week" ? "week" : "day"}, by ${groupBy}`}
      />
    </section>

    <section>
      <CacheChart days={cacheDays} />
    </section>

    <section>
      <h2><Icon name="layers" size={13} />By account</h2>
      {#if modelFilterActive}
        <p class="note">Account totals cover all models — the model filter applies to the KPIs, charts, and the model breakdown.</p>
      {/if}
      <div class="panel">
        {#each data.perAccount as a, i (a.account)}
          {@const eff = effective(a)}
          <BarRow
            label={a.account}
            value={sum(a.tokens)}
            max={accountMax}
            color={seriesColor(i)}
            valueText={tokens(sum(a.tokens))}
            secondaryText={usd(a.value)}
            title={fullTokens(sum(a.tokens))}
            soloRow={data.perAccount.length === 1}
          >
            <div class="sub-cost">
              <label>
                <span>Plan $/month</span>
                <input
                  type="number"
                  min="0"
                  step="1"
                  placeholder="—"
                  value={settings?.monthlyCostUsd?.[a.account] ?? ""}
                  onchange={(e) => setMonthlyCost(a.account, e.currentTarget.value)}
                />
              </label>
              {#if eff}
                <span class="eff">
                  Effective <b>{perMtok(eff.rate)}</b> over {spanMonths.toFixed(1)} months
                  {#if eff.saved > 0}
                    · <b class="save">{usd(eff.saved)}</b> less than API list price
                  {:else}
                    · <b>{usd(-eff.saved)}</b> more than the API would have cost
                  {/if}
                </span>
              {:else}
                <span class="eff hint">Enter your plan's monthly cost to see effective $/Mtok.</span>
              {/if}
            </div>
          </BarRow>
        {/each}
      </div>
    </section>

    <div class="cols">
      <section>
        <h2><Icon name="hash" size={13} />By model</h2>
        <div class="panel">
          {#each shownModels as m, i (m.model)}
            <BarRow
              label={modelName(m.model)}
              value={sum(m.tokens)}
              max={modelMax}
              color={seriesColor(i)}
              valueText={tokens(sum(m.tokens))}
              secondaryText={usd(m.value)}
              title={m.model}
              soloRow={shownModels.length === 1}
            />
          {/each}
        </div>
      </section>

      <section>
        <h2><Icon name="home" size={13} />By project</h2>
        {#if modelFilterActive}
          <p class="note">Not filtered by model.</p>
        {/if}
        <div class="panel">
          {#each shownProjects as p, i (p.project)}
            <BarRow
              label={projectName(p.project)}
              value={sum(p.tokens)}
              max={projectMax}
              color={seriesColor(i)}
              valueText={tokens(sum(p.tokens))}
              secondaryText={usd(p.value)}
              title={p.project}
              soloRow={shownProjects.length === 1}
            />
          {/each}
        </div>
        {#if projects.length > PROJECT_LIMIT}
          <button class="btn small link" onclick={() => (showAllProjects = !showAllProjects)}>
            {showAllProjects ? "Show top 8" : `Show all ${projects.length} projects`}
          </button>
        {/if}
      </section>
    </div>

    <section>
      <div class="section-head">
        <h2><Icon name="chart" size={13} />Detail</h2>
        <div class="toggle" role="group" aria-label="Table grouping">
          <button class:on={tableMode === "model"} onclick={() => (tableMode = "model")}
            aria-pressed={tableMode === "model"}>By model</button>
          <button class:on={tableMode === "project"} onclick={() => (tableMode = "project")}
            aria-pressed={tableMode === "project"}>By project</button>
        </div>
      </div>
      <UsageTable rows={tableRows} firstColumn={tableMode === "model" ? "Model" : "Project"} />
      <p class="disclaimer">
        Dollar figures are <b>estimates of equivalent API value</b> — what these tokens would have cost
        at pay-per-token API rates. They are not what you were charged; a Pro/Max plan is a flat fee.
        Priced as of {data.pricedAsOf}.
        {#if data.unpricedModels.length}
          {data.unpricedModels.length} model{data.unpricedModels.length === 1 ? "" : "s"} had no price
          ({data.unpricedModels.map(modelName).join(", ")}) — their tokens are counted, their value shown
          as “—”.
        {/if}
      </p>
    </section>
  {/if}
</main>

<style>
  main {
    padding: 18px 22px 32px;
    max-width: 1100px;
    margin: 0 auto;
  }
  header {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    margin-bottom: 18px;
  }
  h1 {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 650;
  }
  .sub {
    margin: 3px 0 0;
    font-size: 0.82rem;
    color: var(--ink-soft);
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }
  /* Visually hidden but announced — the live region must stay in the a11y tree. */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
    border: 0;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-faint);
    margin: 22px 0 8px;
    font-weight: 600;
  }
  .section-head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .section-head h2 {
    margin-bottom: 8px;
  }
  .section-head .toggle {
    margin-left: auto;
  }

  .kpis {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
    gap: 10px;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 0 22px;
  }
  .panel {
    background: var(--panel-2);
    border: 1px solid var(--hair);
    border-radius: 12px;
    padding: 3px 13px;
  }
  /* Divider between bar rows lives here: BarRow instances are siblings only from the parent's view. */
  .panel > :global(* + *) {
    border-top: 1px solid var(--hair);
  }

  .sub-cost {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 0.75rem;
    color: var(--ink-soft);
  }
  .sub-cost label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sub-cost input {
    width: 72px;
    font: inherit;
    font-variant-numeric: tabular-nums;
    padding: 3px 6px;
    border: 1px solid var(--hair);
    border-radius: 6px;
    background: var(--panel);
    color: var(--ink);
  }
  .eff b {
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }
  .eff b.save {
    color: var(--good);
  }
  .eff.hint {
    color: var(--ink-faint);
  }

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
  .btn.small {
    padding: 4px 9px;
    font-size: 0.72rem;
  }
  .btn.link {
    margin-top: 8px;
    background: none;
    border-color: transparent;
    color: var(--accent);
  }

  .toggle {
    display: flex;
    border: 1px solid var(--hair);
    border-radius: 7px;
    overflow: hidden;
  }
  .toggle button {
    font: inherit;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 5px 10px;
    border: 0;
    background: var(--panel);
    color: var(--ink-soft);
    cursor: pointer;
  }
  .toggle button.on {
    background: var(--accent);
    color: var(--accent-ink);
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 10px;
    background: color-mix(in srgb, var(--crit) 12%, transparent);
    color: var(--crit);
    padding: 9px 12px;
    border-radius: 8px;
    font-size: 0.82rem;
    margin-bottom: 12px;
  }
  .banner .btn {
    margin-left: auto;
    color: inherit;
  }

  .empty {
    border: 1px solid var(--hair);
    border-radius: 10px;
    padding: 34px 24px;
    text-align: center;
  }
  .empty h2 {
    margin: 0 0 6px;
    font-size: 0.95rem;
    text-transform: none;
    letter-spacing: 0;
    color: var(--ink);
  }
  .empty p {
    margin: 0 auto;
    max-width: 46ch;
    font-size: 0.83rem;
    color: var(--ink-soft);
    line-height: 1.5;
  }

  .skeleton {
    height: 78px;
    border-radius: 10px;
    background: var(--panel-2);
    animation: pulse 1.4s ease-in-out infinite;
  }
  .skeleton.tall {
    height: 240px;
    margin-top: 22px;
  }
  @keyframes pulse {
    50% {
      opacity: 0.55;
    }
  }

  .disclaimer {
    font-size: 0.73rem;
    color: var(--ink-faint);
    line-height: 1.55;
    margin: 10px 0 0;
  }
  .disclaimer b {
    color: var(--ink-soft);
  }
  .note {
    font-size: 0.72rem;
    color: var(--ink-faint);
    margin: -2px 0 7px;
  }
</style>
