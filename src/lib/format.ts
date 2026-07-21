/** Number/currency formatting for the Usage view. Compact in the UI, exact on hover. */

const exact = new Intl.NumberFormat();

/**
 * `1234567` → `1.2M`. Hand-rolled rather than `Intl` compact notation, which is locale-dependent:
 * en-AU renders "2.8bn" / "116.2m", and a lowercase "m" reads as milli as easily as million. Unit
 * suffixes stay uppercase and identical everywhere; only the digits are localized.
 */
export function tokens(n: number): string {
  const abs = Math.abs(n);
  const unit = abs >= 1e9 ? ["B", 1e9] : abs >= 1e6 ? ["M", 1e6] : abs >= 1e3 ? ["K", 1e3] : null;
  if (!unit) return exact.format(Math.round(n));
  const [suffix, divisor] = unit as [string, number];
  const scaled = n / divisor;
  // One decimal below 100 (1.2M), none above (250M) — keeps axis labels a consistent width.
  const digits = Math.abs(scaled) < 100 ? 1 : 0;
  const text = scaled.toLocaleString(undefined, {
    minimumFractionDigits: 0,
    maximumFractionDigits: digits,
  });
  return `${text}${suffix}`;
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
    // Without this, en-AU renders "US$6,149.44"; the country prefix is noise in a single-currency UI.
    currencyDisplay: "narrowSymbol",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(v);
}

/** Dollars per million tokens, e.g. `$3.20/Mtok`. */
export function perMtok(v: number | null): string {
  if (v == null || !isFinite(v)) return "—";
  return `${usd(v)}/Mtok`;
}

/**
 * Rounds to a whole percent, except near the ends of the scale where that would assert something
 * false — 99.93% must not render as "100%", and 0.02% must not render as "0%".
 */
export function pct(v: number | null): string {
  if (v == null || !isFinite(v)) return "—";
  const rounded = Math.round(v);
  const lies = (rounded === 100 && v < 100) || (rounded === 0 && v > 0);
  return `${lies ? v.toFixed(1) : rounded}%`;
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
