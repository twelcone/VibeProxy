import { describe, expect, it } from "vitest";
import { areaPath, linePath, linearScale, niceTicks, seriesDash, shortDate, weekStart } from "./svg";

describe("linearScale", () => {
  it("maps domain endpoints onto range endpoints", () => {
    const s = linearScale([0, 100], [0, 200]);
    expect(s(0)).toBe(0);
    expect(s(50)).toBe(100);
    expect(s(100)).toBe(200);
  });

  it("inverts when the range is inverted, as an SVG y-axis is", () => {
    const y = linearScale([0, 100], [260, 0]);
    expect(y(0)).toBe(260);
    expect(y(100)).toBe(0);
  });

  /** A single-bucket chart has a zero-width domain; dividing by it would yield NaN paths. */
  it("pins a degenerate domain to the range midpoint instead of producing NaN", () => {
    const s = linearScale([5, 5], [0, 100]);
    expect(s(5)).toBe(50);
    expect(Number.isNaN(s(999))).toBe(false);
  });

  it("extrapolates outside the domain rather than clamping", () => {
    const s = linearScale([0, 10], [0, 100]);
    expect(s(20)).toBe(200);
  });
});

describe("niceTicks", () => {
  it("produces round 1/2/5 steps covering the max", () => {
    const t = niceTicks(1000);
    expect(t[0]).toBe(0);
    expect(t[t.length - 1]).toBeGreaterThanOrEqual(1000);
    expect(t.every((v) => Number.isFinite(v))).toBe(true);
  });

  it("keeps ticks ascending and unique", () => {
    for (const max of [7, 340_000_000, 1, 99]) {
      const t = niceTicks(max);
      expect(new Set(t).size).toBe(t.length);
      for (let i = 1; i < t.length; i++) expect(t[i]).toBeGreaterThan(t[i - 1]);
    }
  });

  /** An empty range must not produce an infinite loop or an empty axis. */
  it("degrades safely on zero, negative and non-finite maxima", () => {
    expect(niceTicks(0)).toEqual([0]);
    expect(niceTicks(-5)).toEqual([0]);
    expect(niceTicks(Number.NaN)).toEqual([0]);
    expect(niceTicks(Number.POSITIVE_INFINITY)).toEqual([0]);
  });
});

describe("linePath / areaPath", () => {
  it("emits a move then line commands", () => {
    expect(linePath([[0, 0], [10, 5]])).toBe("M0,0L10,5");
  });

  it("returns empty for no points rather than a malformed path", () => {
    expect(linePath([])).toBe("");
    expect(areaPath([], 100)).toBe("");
  });

  /** A single day of data still has to render something visible. */
  it("emits a zero-length segment for one point so the line cap draws a dot", () => {
    expect(linePath([[4, 7]])).toBe("M4,7L4,7");
  });

  it("closes the area back along the baseline", () => {
    const d = areaPath([[0, 10], [10, 20]], 100);
    expect(d.startsWith("M0,10")).toBe(true);
    expect(d.endsWith("Z")).toBe(true);
    expect(d).toContain("L10,100");
    expect(d).toContain("L0,100");
  });
});

describe("seriesDash", () => {
  it("gives the first series a solid line", () => {
    expect(seriesDash(0)).toBe("");
  });
  it("cycles rather than running out, so colour is never the only differentiator", () => {
    expect(seriesDash(6)).toBe(seriesDash(0));
    expect(seriesDash(7)).toBe(seriesDash(1));
  });
  it("gives adjacent series different patterns", () => {
    expect(seriesDash(1)).not.toBe(seriesDash(2));
  });
});

describe("shortDate", () => {
  /** Regression: a bare `new Date("2026-07-14")` parses as UTC and can render as the 13th. */
  it("parses a bare date as local, not UTC", () => {
    expect(shortDate("2026-07-14")).toMatch(/14/);
    expect(shortDate("2026-01-01")).toMatch(/1/);
  });
  it("passes malformed input through instead of rendering Invalid Date", () => {
    expect(shortDate("nonsense")).toBe("nonsense");
    expect(shortDate("")).toBe("");
  });
});

describe("weekStart", () => {
  it("snaps every day of a week to the same Monday", () => {
    // 2026-07-13 is a Monday.
    const week = ["2026-07-13", "2026-07-15", "2026-07-19"].map(weekStart);
    expect(new Set(week).size).toBe(1);
    expect(week[0]).toBe("2026-07-13");
  });

  /** Sunday is the end of its week, not the start of the next one. */
  it("treats Sunday as belonging to the preceding Monday", () => {
    expect(weekStart("2026-07-19")).toBe("2026-07-13");
    expect(weekStart("2026-07-20")).toBe("2026-07-20");
  });

  it("crosses a month boundary correctly", () => {
    // 2026-08-02 is a Sunday; its week starts in July.
    expect(weekStart("2026-08-02")).toBe("2026-07-27");
  });

  it("returns a zero-padded ISO date", () => {
    expect(weekStart("2026-01-05")).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});
