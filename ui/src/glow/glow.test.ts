import { beforeEach, describe, expect, it } from "vitest";
import type { AlertFire } from "../alerts-shared/types";
import { applyGlow, PULSE_MS, STEADY_MS } from "./glow";

const fire = (over: Partial<AlertFire> = {}): AlertFire => ({
  sessionId: "s", project: "p", kind: "done", escalation: 0, color: "#3ddc84", glow: true,
  pulses: 3, sound: null, tts: false, ttsVoice: null, volume: 0.5, avatar: false, avatarSrc: null, edge: "top", mascotMode: "peek", ...over,
});

let el: HTMLElement;
beforeEach(() => { el = document.createElement("div"); });

describe("applyGlow", () => {
  it("drives animation-iteration-count from the pulse count", () => {
    applyGlow(el, fire({ pulses: 4 }), false);
    expect(el.style.animationIterationCount).toBe("4");
    expect(el.dataset.mode).toBe("pulse");
    expect(el.style.animationDuration).toBe(`${PULSE_MS}ms`);
    expect(el.style.getPropertyValue("--glow-color")).toBe("#3ddc84");
  });
  it("keeps pulses at least 1 and keeps the rate under 2 per second", () => {
    applyGlow(el, fire({ pulses: 0 }), false);
    expect(el.style.animationIterationCount).toBe("1");
    expect(1000 / PULSE_MS).toBeLessThanOrEqual(2);
  });
  it("uses one steady 1.5s glow under reduced motion", () => {
    applyGlow(el, fire({ pulses: 5 }), true);
    expect(el.dataset.mode).toBe("steady");
    expect(el.style.animationIterationCount).toBe("1");
    expect(el.style.animationDuration).toBe(`${STEADY_MS}ms`);
  });
  it("ignores a non-hex color", () => {
    applyGlow(el, fire({ color: "red;x:y" }), false);
    expect(el.style.getPropertyValue("--glow-color")).toBe("#3ddc84");
  });
});
