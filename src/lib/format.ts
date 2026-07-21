/** Number/currency formatting for the Usage view. Compact in the UI, exact on hover. */

const compact = new Intl.NumberFormat(undefined, {
  notation: "compact",
  maximumFractionDigits: 1,
});
const exact = new Intl.NumberFormat();

/** `1234567` → `1.2M`. Use with `fullTokens()` as the title/tooltip. */
export function tokens(n: number): string {
  return compact.format(n);
}

/** The unabbreviated count, for tooltips and the table. */
export function fullTokens(n: number): string {
  return exact.format(n);
}

/**
 * API-equivalent value in USD. Sub-cent amounts would all render as "$0.00", so small values keep
 * more precision. `null` (unpriced model) renders as an em dash, never "$0".
 */
export function usd(v: number | null | undefined): string {
  if (v == null) return "—";
  const digits = v !== 0 && Math.abs(v) < 1 ? 4 : 2;
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(v);
}

/** Dollars per million tokens, e.g. `$3.20/Mtok`. */
export function perMtok(v: number | null): string {
  if (v == null || !isFinite(v)) return "—";
  return `${usd(v)}/Mtok`;
}

export function pct(v: number | null): string {
  if (v == null || !isFinite(v)) return "—";
  return `${Math.round(v)}%`;
}

/**
 * Claude Code names project dirs after a slugified cwd (`-Users-twel-Projects-VibeProxy`).
 * Show the leaf, which is what the user actually recognizes.
 */
export function projectName(slug: string): string {
  const parts = slug.split("-").filter(Boolean);
  return parts.length ? parts[parts.length - 1] : slug;
}

/** `claude-opus-4-8[1m]` → `opus-4-8[1m]` — drop the vendor prefix, keep what distinguishes. */
export function modelName(id: string): string {
  return id.replace(/^claude-/, "");
}
