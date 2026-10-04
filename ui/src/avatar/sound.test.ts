import { describe, expect, it } from "vitest";
import { playSound, type CtxLike, type ParamLike } from "./sound";

function mockCtx() {
  const counts = { osc: 0, gain: 0, filter: 0, started: 0 };
  const param = (): ParamLike => ({
    value: 0,
    setValueAtTime() {},
    linearRampToValueAtTime() {},
    exponentialRampToValueAtTime() {},
  });
  const node = () => ({ connect: (d: unknown) => d });
  const ctx: CtxLike = {
    currentTime: 0,
    destination: {},
    createOscillator: () => {
      counts.osc++;
      return {
        ...node(),
        type: "",
        frequency: param(),
        start() {
          counts.started++;
        },
        stop() {},
      };
    },
    createGain: () => {
      counts.gain++;
      return { ...node(), gain: param() };
    },
    createBiquadFilter: () => {
      counts.filter++;
      return { ...node(), type: "", frequency: param() };
    },
  };
  return { ctx, counts };
}

describe("playSound", () => {
  it.each(["chime", "ding", "alarm", "bell"] as const)("%s builds a node graph and returns a length", (id) => {
    const { ctx, counts } = mockCtx();
    const len = playSound(ctx, id, 0.7);
    expect(len).toBeGreaterThan(0);
    expect(counts.osc).toBeGreaterThan(0);
    expect(counts.started).toBe(counts.osc);
    expect(counts.gain).toBeGreaterThan(0);
  });

  it("alarm is low-passed with three beeps; chime has two notes; bell is FM; ding is one", () => {
    let m = mockCtx();
    playSound(m.ctx, "alarm", 1);
    expect(m.counts.filter).toBe(1);
    expect(m.counts.osc).toBe(3);
    m = mockCtx();
    playSound(m.ctx, "chime", 1);
    expect(m.counts.osc).toBe(2);
    m = mockCtx();
    playSound(m.ctx, "bell", 1);
    expect(m.counts.osc).toBe(2);
    m = mockCtx();
    playSound(m.ctx, "ding", 1);
    expect(m.counts.osc).toBe(1);
  });

  it("null sound creates no nodes", () => {
    const { ctx, counts } = mockCtx();
    expect(playSound(ctx, null, 0.7)).toBe(0);
    expect(counts).toEqual({ osc: 0, gain: 0, filter: 0, started: 0 });
  });
});
