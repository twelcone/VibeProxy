import { describe, expect, it } from "vitest";
import { fullTokens, modelName, pct, perMtok, projectName, tokens, usd } from "./format";

describe("tokens", () => {
  it("scales to K/M/B with uppercase units", () => {
    expect(tokens(999)).toBe("999");
    expect(tokens(1_500)).toBe("1.5K");
    expect(tokens(1_200_000)).toBe("1.2M");
    expect(tokens(2_800_000_000)).toBe("2.8B");
  });

  it("drops the decimal at and above 100 so axis labels keep a stable width", () => {
    expect(tokens(99_400_000)).toBe("99.4M");
    expect(tokens(250_000_000)).toBe("250M");
  });

  /**
   * Regression: `Intl` compact notation is locale-dependent. Under en-AU it produced "2.8bn" and
   * "116.2m", and a lowercase "m" reads as milli as readily as million.
   */
  it("uses fixed uppercase suffixes regardless of locale", () => {
    for (const v of [1_500, 1_200_000, 2_800_000_000]) {
      const out = tokens(v);
      expect(out).toMatch(/[KMB]$/);
      expect(out).not.toMatch(/bn|[km]$/);
    }
  });

  it("handles zero and negatives without a unit surprise", () => {
    expect(tokens(0)).toBe("0");
    expect(tokens(-1_500)).toBe("-1.5K");
  });

  it("fullTokens never abbreviates", () => {
    expect(fullTokens(2_800_000_000).replace(/[^\d]/g, "")).toBe("2800000000");
  });
});

describe("usd", () => {
  it("renders two decimals for ordinary amounts", () => {
    expect(usd(6173.21)).toContain("6,173.21");
  });

  /** Regression: en-AU renders "US$6,149.44"; the country prefix is noise in a single-currency UI. */
  it("uses a bare currency symbol, not a country-qualified one", () => {
    expect(usd(1234.5)).not.toContain("US$");
    expect(usd(1234.5)).toContain("$");
  });

  it("keeps more precision below a dollar so sub-cent values are not all $0.00", () => {
    expect(usd(0.1555)).toBe("$0.1555");
    expect(usd(0)).toBe("$0.00");
  });

  /** An unpriced model must read as unknown, never as free. */
  it("renders null and undefined as an em dash", () => {
    expect(usd(null)).toBe("—");
    expect(usd(undefined)).toBe("—");
  });
});

describe("pct", () => {
  it("rounds to whole percent in the ordinary case", () => {
    expect(pct(42.4)).toBe("42%");
    expect(pct(42.6)).toBe("43%");
  });

  /**
   * Regression: a 99.93% cache share rendered as "100%", asserting something untrue. The same
   * applies at the bottom of the scale.
   */
  it("never rounds to a value it has not reached", () => {
    expect(pct(99.93)).toBe("99.9%");
    expect(pct(99.6)).toBe("99.6%");
    expect(pct(0.02)).toBe("0.0%");
  });

  it("still shows exact endpoints as whole numbers", () => {
    expect(pct(100)).toBe("100%");
    expect(pct(0)).toBe("0%");
  });

  it("renders unknown and non-finite values as an em dash", () => {
    expect(pct(null)).toBe("—");
    expect(pct(Number.NaN)).toBe("—");
    expect(pct(Number.POSITIVE_INFINITY)).toBe("—");
  });
});

describe("perMtok", () => {
  it("suffixes the rate", () => {
    expect(perMtok(3.2)).toBe("$3.20/Mtok");
  });
  it("guards against a divide-by-zero producing Infinity upstream", () => {
    expect(perMtok(Number.POSITIVE_INFINITY)).toBe("—");
    expect(perMtok(null)).toBe("—");
  });
});

describe("projectName", () => {
  it("reduces a slugified cwd to its leaf", () => {
    expect(projectName("-Users-twel-Projects-VibeProxy")).toBe("VibeProxy");
  });
  it("passes through a name with no separators", () => {
    expect(projectName("VibeProxy")).toBe("VibeProxy");
  });
  it("does not throw on an empty slug", () => {
    expect(projectName("")).toBe("");
    expect(projectName("---")).toBe("---");
  });
});

describe("modelName", () => {
  it("strips the vendor prefix but keeps what distinguishes", () => {
    expect(modelName("claude-opus-4-8")).toBe("opus-4-8");
    expect(modelName("claude-haiku-4-5-20251001")).toBe("haiku-4-5-20251001");
  });
  it("leaves an unprefixed id alone", () => {
    expect(modelName("gpt-4")).toBe("gpt-4");
    expect(modelName("<synthetic>")).toBe("<synthetic>");
  });
});
