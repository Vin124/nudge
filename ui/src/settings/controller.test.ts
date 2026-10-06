import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Config } from "../shared/contracts";

const setConfig = vi.fn((_c: Config) => Promise.resolve());
vi.mock("../shared/contracts", () => ({
  api: { setConfig, previewAlert: vi.fn(() => Promise.resolve()), listAvatarPacks: vi.fn(() => Promise.resolve([])) },
}));

const cfg = (): Config => ({
  version: 1,
  notch: { edge: "top", offset: 0.5, monitor: null, showWeekly: true },
  alerts: {
    done: { glow: true, sound: "chime", tts: false, avatar: true, color: "#3ddc84" },
    blocked: { glow: true, sound: "alarm", tts: true, avatar: true, color: "#ff8a00" },
    escalateMinutes: [2, 5], glowPulses: 3, volume: 0.7, ttsVoice: null, avatarPack: null, mascotMode: "peek",
  },
  usage: { liveWhenIdle: false },
  dnd: false,
});

describe("mountSettings (D24)", () => {
  beforeEach(() => {
    setConfig.mockClear();
    document.body.innerHTML = "<div id=host></div>";
  });

  it("flush saves a pending edit immediately, so leaving Settings never drops it", async () => {
    const { mountSettings } = await import("./controller");
    const c = mountSettings(document.getElementById("host")!);
    c.show(cfg());
    (document.querySelector('.seg[data-level="quiet"]') as HTMLButtonElement).click();
    expect(setConfig).not.toHaveBeenCalled(); // still inside the debounce window
    await c.flush();
    expect(setConfig).toHaveBeenCalledTimes(1);
    expect(setConfig.mock.calls[0][0].alerts.escalateMinutes).toEqual([]);
    await c.flush();
    expect(setConfig).toHaveBeenCalledTimes(1); // nothing pending, nothing sent
  });

  it("skips repainting when the config is unchanged", async () => {
    const { mountSettings } = await import("./controller");
    const host = document.getElementById("host")!;
    const c = mountSettings(host);
    c.show(cfg());
    const dnd = [...host.querySelectorAll("label.toggle")].find((l) => l.textContent === "Do Not Disturb")!
      .querySelector("input") as HTMLInputElement;
    dnd.checked = true; // a local edit not yet echoed back by a snapshot
    c.show(cfg());
    expect(dnd.checked).toBe(true);
  });
});
