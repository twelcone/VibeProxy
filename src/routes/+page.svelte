<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  type Profile = {
    id: string;
    label: string;
    configDir: string;
    email: string | null;
    orgId: string | null;
    subscriptionType: string | null;
    priority: number;
  };

  type Settings = {
    autoSwitchEnabled: boolean;
    thresholdPct: number;
    pollIntervalSecs: number;
    cooldownSecs: number;
    launchAtLogin: boolean;
  };

  let profiles = $state<Profile[]>([]);
  let settings = $state<Settings | null>(null);
  let error = $state("");

  async function refresh() {
    try {
      profiles = await invoke<Profile[]>("list_profiles");
      settings = await invoke<Settings>("get_settings");
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    refresh();
  });
</script>

<main>
  <header>
    <h1>VibeProxy</h1>
    <p class="tagline">Claude Code account switcher</p>
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <section>
    <h2>Profiles</h2>
    {#if profiles.length === 0}
      <p class="empty">No profiles yet. Add one to get started (coming next phase).</p>
    {:else}
      <ul class="profiles">
        {#each profiles as p (p.id)}
          <li>
            <span class="label">{p.label}</span>
            <span class="meta">{p.email ?? p.configDir}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if settings}
    <section>
      <h2>Settings</h2>
      <dl>
        <dt>Auto-switch</dt><dd>{settings.autoSwitchEnabled ? "on" : "off"}</dd>
        <dt>Threshold</dt><dd>{settings.thresholdPct}%</dd>
        <dt>Poll interval</dt><dd>{settings.pollIntervalSecs}s</dd>
      </dl>
    </section>
  {/if}
</main>

<style>
  :root {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    color: #1a1a1a;
    background: #faf9f7;
  }
  main {
    padding: 1rem 1.25rem;
    max-width: 100%;
  }
  header h1 {
    margin: 0;
    font-size: 1.25rem;
  }
  .tagline {
    margin: 0.1rem 0 1rem;
    color: #6b6258;
    font-size: 0.85rem;
  }
  h2 {
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #6b6258;
    margin: 1rem 0 0.5rem;
  }
  .empty {
    color: #8a8078;
    font-size: 0.9rem;
  }
  ul.profiles {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  ul.profiles li {
    display: flex;
    flex-direction: column;
    padding: 0.5rem 0;
    border-bottom: 1px solid #ece7df;
  }
  .label {
    font-weight: 600;
  }
  .meta {
    font-size: 0.8rem;
    color: #8a8078;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.25rem 1rem;
    margin: 0;
    font-size: 0.9rem;
  }
  dt {
    color: #6b6258;
  }
  dd {
    margin: 0;
  }
  .error {
    color: #b8232c;
    font-size: 0.85rem;
  }

  @media (prefers-color-scheme: dark) {
    :root {
      color: #f0ece5;
      background: #1c1a17;
    }
    .tagline,
    h2,
    dt {
      color: #a89f94;
    }
    ul.profiles li {
      border-bottom-color: #2c2824;
    }
  }
</style>
