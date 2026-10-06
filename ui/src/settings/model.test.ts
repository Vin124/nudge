import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Config } from "../shared/contracts";
import { addMinute, configToForm, debounce, formToConfig, normalizeMinutes, parseMinute, applyLevel, detectLevel, LEVELS } from "./model";

const cfg = (): Config => ({
  version: 1,
  notch: { edge: "left", offset: 0.3, monitor: "DP-1", showWeekly: false },
  alerts: {
    done: { glow: true, sound: "ding", tts: false, avatar: true, color: "#112233" },
    blocked: { glow: false, sound: null, tts: true, avatar: false, color: "#ff8a00" },
    escalateMinutes: [2, 5],
    glowPulses: 4,
    volume: 0.5,
    ttsVoice: null,
    avatarPack: "cat",
    mascotMode: "pop",
  },
  usage: { liveWhenIdle: true },
  dnd: true,
});

describe("model mapping", () => {
  it("config -> form -> config is lossless", () => {
    const c = cfg();
    expect(formToConfig(configToForm(c), c)).toEqual(c);
  });
  it("null voice <-> System default, null sound <-> None, null pack <-> built-in", () => {
    const f = configToForm(cfg());
    expect(f.ttsVoice).toBe("");
    expect(f.blocked.sound).toBe("none");
    const c2 = formToConfig({ ...f, avatarPack: "", done: { ...f.done, sound: "none" }, ttsVoice: "Zira" }, cfg());
    expect(c2.alerts.avatarPack).toBeNull();
    expect(c2.alerts.done.sound).toBeNull();
    expect(c2.alerts.ttsVoice).toBe("Zira");
  });
  it("keeps notch edge/offset/monitor from the base config", () => {
    const base = cfg();
    const out = formToConfig({ ...configToForm(base), showWeekly: true }, { ...base, notch: { ...base.notch, offset: 0.9 } });
    expect(out.notch).toEqual({ edge: "left", offset: 0.9, monitor: "DP-1", showWeekly: true });
  });
});

describe("minutes", () => {
  it("parses only whole numbers in 1..120", () => {
    expect(parseMinute("5")).toBe(5);
    expect(parseMinute(" 120 ")).toBe(120);
    for (const bad of ["", "abc", "0", "121", "1.5", "-3", "1e2", "NaN"]) expect(parseMinute(bad)).toBeNull();
  });
  it("normalize dedupes, sorts, clamps range, caps at 5", () => {
    expect(normalizeMinutes([30, 2, 2, 0, 500, 5, 1, 10, 60, 90])).toEqual([1, 2, 5, 10, 30]);
  });
  it("addMinute rejects non-numbers, duplicates and a sixth entry", () => {
    expect(addMinute([2], "x").ok).toBe(false);
    expect(addMinute([2], "2").ok).toBe(false);
    expect(addMinute([1, 2, 3, 4, 5], "6").ok).toBe(false);
    expect(addMinute([5], "2")).toEqual({ ok: true, list: [2, 5] });
  });
});

describe("debounce", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());
  it("3 edits within 300 ms produce one call with the last value", () => {
    const fn = vi.fn();
    const d = debounce(fn, 300);
    d(1);
    vi.advanceTimersByTime(100);
    d(2);
    vi.advanceTimersByTime(100);
    d(3);
    vi.advanceTimersByTime(299);
    expect(fn).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith(3);
  });
  it("cancel drops the pending call", () => {
    const fn = vi.fn();
    const d = debounce(fn, 300);
    d(1);
    d.cancel();
    vi.advanceTimersByTime(1000);
    expect(fn).not.toHaveBeenCalled();
  });
});

describe("levels (D23)", () => {
  it("every preset round-trips through detectLevel", () => {
    const base = configToForm(cfg());
    for (const lv of LEVELS) expect(detectLevel(applyLevel(base, lv))).toBe(lv);
  });
  it("leaves personal settings alone", () => {
    const base = configToForm(cfg());
    const q = applyLevel(base, "quiet");
    expect(q.done.color).toBe(base.done.color);
    expect(q.volume).toBe(base.volume);
    expect(q.mascotMode).toBe(base.mascotMode);
    expect(q.avatarPack).toBe(base.avatarPack);
  });
  it("anything off-preset is custom", () => {
    const f = applyLevel(configToForm(cfg()), "loud");
    expect(detectLevel({ ...f, escalateMinutes: [3] })).toBe("custom");
  });
});
