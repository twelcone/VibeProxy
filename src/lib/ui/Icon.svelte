<script lang="ts" module>
  /**
   * Inline Lucide glyphs (MIT). Inlined rather than pulled as a dependency: we use a handful, and
   * `stroke: currentColor` means they theme themselves.
   *
   * Path data is Lucide's 24×24 outline set, stroke-width 2, round caps/joins.
   */
  export const ICONS = {
    hash: "M4 9h16M4 15h16M10 3 8 21M16 3l-2 18",
    dollar: "M12 2v20M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6",
    trending: "M16 7h6v6M22 7l-8.5 8.5-5-5L2 17",
    calendar: "M8 2v4M16 2v4M3 10h18M5 4h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z",
    database: "M3 5a9 3 0 0 0 18 0 9 3 0 0 0-18 0M3 5v14a9 3 0 0 0 18 0V5M3 12a9 3 0 0 0 18 0",
    swap: "m16 3 4 4-4 4M20 7H4m8 14-4-4 4-4M4 17h16",
    home: "M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    chart: "M3 3v16a2 2 0 0 0 2 2h16M18 17V9M13 17V5M8 17v-3",
    settings: "M14 17H5M19 7h-9M17 14a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM7 4a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
    refresh: "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8M21 3v5h-5M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16M8 16H3v5",
    layers: "M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83zM2 12l8.58 3.91a2 2 0 0 0 1.66 0L21 12M2 17l8.58 3.91a2 2 0 0 0 1.66 0L21 17",
  } as const;

  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  let {
    name,
    size = 16,
    stroke = 2,
  }: { name: IconName; size?: number; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
><path d={ICONS[name]} /></svg>

<style>
  svg {
    display: block;
    flex: none;
  }
</style>
