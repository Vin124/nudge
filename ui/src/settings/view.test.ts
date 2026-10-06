import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Config } from "../shared/contracts";
import type { Form } from "./model";
import { createView, CUSTOM_TEXT, LEVEL_TEXT, USAGE_DISCLOSURE, type SettingsView } from "./view";

const cfg = (over: Partial<Config["alerts"]> = {}): Config => ({
  version: 1,
  notch: { edge: "top", offset: 0.5, monitor: null, showWeekly: true },
  alerts: {
    done: { glow: true, sound: "chime", tts: false, avatar: true, color: "#3ddc84" },
    blocked: { glow: true, sound: "alarm", tts: true, avatar: true, color: "#ff8a00" },
    escalateMinutes: [2, 5],
    glowPulses: 3,
    volume: 0.7,
    ttsVoice: null,
    avatarPack: null,
    mascotMode: "peek",
    ...over,
  },
  usage: { liveWhenIdle: false },
  dnd: false,
});

let root: HTMLElement;
let view: SettingsView;
let onChange: ReturnType<typeof vi.fn<(f: Form) => void>>;
let onPreview: ReturnType<typeof vi.fn>;

beforeEach(() => {
  document.body.innerHTML = "<div id=app></div>";
  root = document.getElementById("app")!;
  onChange = vi.fn();
  onPreview = vi.fn();
  view = createView(root, { onChange, onPreview });
  view.render(cfg());
});

describe("settings view", () => {
  it("renders every section and the verbatim usage disclosure", () => {
    const titles = [...root.querySelectorAll("h2")].map((h) => h.textContent);
    expect(titles).toEqual([
      "Notifications", "Do Not Disturb", "Notch", "Usage",
      "When a session finishes", "When a session needs you", "Alerts", "Avatar",
    ]);
    expect(root.textContent).toContain(USAGE_DISCLOSURE);
    expect(root.textContent).toContain("Drag the notch to move it.");
  });

  it("every control has an accessible label", () => {
    for (const el of root.querySelectorAll("input, select")) {
      const labelled = (el as HTMLInputElement).labels?.length || el.getAttribute("aria-label");
      expect(labelled, el.outerHTML).toBeTruthy();
    }
  });

  it("Test buttons call previewAlert with the right kind", () => {
    const btns = [...root.querySelectorAll<HTMLButtonElement>("button")].filter((b) => b.textContent === "Test");
    expect(btns).toHaveLength(2);
    btns[0].click();
    btns[1].click();
    expect(onPreview.mock.calls.map((c) => c[0])).toEqual(["done", "blocked"]);
  });

  it("a snapshot arriving while a field is focused does not overwrite it", () => {
    const pulses = root.querySelector<HTMLInputElement>('input[type="number"]')!;
    pulses.focus();
    pulses.value = "12";
    pulses.dispatchEvent(new Event("input"));
    view.render(cfg({ glowPulses: 7, volume: 0.2 }));
    expect(pulses.value).toBe("12");
    expect(view.form()!.glowPulses).toBe(12);
    // other fields do refresh
    expect(root.querySelector<HTMLInputElement>('input[type="range"]')!.value).toBe("0.2");
    pulses.blur();
    view.render(cfg({ glowPulses: 7 }));
    expect(pulses.value).toBe("7");
  });

  it("renders a pack name containing markup as text", () => {
    const evil = '<img src=x onerror="window.__pwned=1">';
    view.setPacks([evil]);
    expect(root.querySelector("img")).toBeNull();
    const opts = [...root.querySelectorAll("option")].map((o) => o.textContent);
    expect(opts).toContain(evil);
  });

  it("chips: add validates, remove emits the new list", () => {
    const input = root.querySelector<HTMLInputElement>('input[type="number"][max="120"]')!;
    const add = [...root.querySelectorAll("button")].find((b) => b.textContent === "Add")!;
    input.value = "abc";
    add.click();
    expect(root.querySelector('[role="alert"]')!.textContent).not.toBe("");
    expect(onChange).not.toHaveBeenCalled();
    input.value = "10";
    add.click();
    expect(onChange.mock.lastCall![0].escalateMinutes).toEqual([2, 5, 10]);
    root.querySelector<HTMLButtonElement>('button[aria-label="Remove 2 minutes"]')!.click();
    expect(onChange.mock.lastCall![0].escalateMinutes).toEqual([5, 10]);
  });

  it("editing a toggle emits the full form", () => {
    const row = [...root.querySelectorAll("label.toggle")].find((l) => l.textContent === "Show weekly wheel")!;
    const cb = row.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
    cb.checked = false;
    cb.dispatchEvent(new Event("change"));
    expect(onChange.mock.lastCall![0].showWeekly).toBe(false);
  });
});

describe("notification levels (D23)", () => {
  const checked = () => root.querySelector('.seg[aria-checked="true"]') as HTMLElement | null;
  const seg = (lv: string) => root.querySelector(`.seg[data-level="${lv}"]`) as HTMLButtonElement;

  it("reads the default config as Normal", () => {
    expect(checked()!.dataset.level).toBe("normal");
    expect(root.querySelector(".level-hint")!.textContent).toBe(LEVEL_TEXT.normal[1]);
  });

  it("applying Quiet sets the per-channel controls and saves", () => {
    seg("quiet").click();
    const f = onChange.mock.calls.at(-1)![0];
    expect(f.done).toMatchObject({ glow: false, sound: "none", tts: false, avatar: false });
    expect(f.blocked.sound).toBe("bell");
    expect(f.escalateMinutes).toEqual([]);
    expect(f.done.color).toBe("#3ddc84");
    expect(checked()!.dataset.level).toBe("quiet");
  });

  it("a manual change in Advanced reads as Custom", () => {
    const glow = root.querySelector(".advanced input[type=checkbox]") as HTMLInputElement;
    glow.checked = !glow.checked;
    glow.dispatchEvent(new Event("change"));
    expect(checked()).toBeNull();
    expect(root.querySelector(".level-hint")!.textContent).toBe(CUSTOM_TEXT);
  });

  it("keeps the per-channel settings in a closed Advanced section", () => {
    const adv = root.querySelector("details.advanced") as HTMLDetailsElement;
    expect(adv.open).toBe(false);
    expect(adv.querySelector("summary")!.textContent).toBe("Advanced");
    expect(adv.textContent).toContain("When a session finishes");
  });
});
