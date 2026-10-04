import { describe, expect, it } from "vitest";
import { formatCountdown, formatElapsed, formatPercent, usageColor } from "./format";

describe("formatElapsed", () => {
  it("formats across ranges", () => {
    expect(formatElapsed(0)).toBe("now");
    expect(formatElapsed(999)).toBe("now");
    expect(formatElapsed(59_000)).toBe("59s");
    expect(formatElapsed(61_000)).toBe("1m");
    expect(formatElapsed(2 * 60_000)).toBe("2m");
    expect(formatElapsed((2 * 3600 + 5 * 60) * 1000)).toBe("2h 5m");
    expect(formatElapsed(3600_000)).toBe("1h");
    expect(formatElapsed(-5000)).toBe("now");
  });
});

describe("formatCountdown", () => {
  const now = 1_000_000_000_000;
  it("reports resetting when resetsAt is in the past", () => {
    expect(formatCountdown(now / 1000 - 10, now)).toBe("resetting");
    expect(formatCountdown(now / 1000, now)).toBe("resetting");
  });
  it("formats the remaining time", () => {
    expect(formatCountdown(now / 1000 + 30, now)).toBe("<1m");
    expect(formatCountdown(now / 1000 + 12 * 60, now)).toBe("12m");
    expect(formatCountdown(now / 1000 + (3600 + 12 * 60), now)).toBe("1h 12m");
  });
});

describe("formatPercent / usageColor", () => {
  it("rounds and clamps", () => {
    expect(formatPercent(41.4)).toBe(41);
    expect(formatPercent(41.5)).toBe(42);
    expect(formatPercent(-3)).toBe(0);
    expect(formatPercent(140)).toBe(100);
    expect(formatPercent(NaN)).toBe(0);
  });
  it("ramps green < 60 <= amber < 85 <= red", () => {
    const g = usageColor(59.9);
    const a = usageColor(60);
    const r = usageColor(85);
    expect(new Set([g, a, r]).size).toBe(3);
    expect(usageColor(84.9)).toBe(a);
    expect(usageColor(100)).toBe(r);
  });
});
