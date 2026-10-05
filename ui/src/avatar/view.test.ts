import { describe, expect, it } from "vitest";
import type { AlertFire } from "../alerts-shared/types";
import { renderAvatar, stageFor } from "./view";

const fire = (over: Partial<AlertFire> = {}): AlertFire => ({
  sessionId: "s",
  project: "api",
  kind: "done",
  escalation: 0,
  color: "#3ddc84",
  glow: true,
  pulses: 3,
  sound: null,
  tts: false,
  ttsVoice: null,
  volume: 0.5,
  avatar: true,
  avatarSrc: null,
  edge: "top",
  mascotMode: "peek",
  ...over,
});
const toSrc = (p: string) => `asset://${p}`;

describe("renderAvatar", () => {
  it("renders project names as text, not HTML", () => {
    const root = document.createElement("div");
    renderAvatar(root, fire({ project: '<img src=x onerror="boom()">' }), toSrc);
    expect(root.querySelector(".bubble img")).toBeNull();
    expect(root.querySelector(".bubble")!.textContent).toBe('<img src=x onerror="boom()"> is done');
  });

  it("shows the pixel fox: asking for blocked, celebrating for done", () => {
    const root = document.createElement("div");
    renderAvatar(root, fire({ kind: "blocked", mascotMode: "pop" }), toSrc);
    expect(root.querySelector(".fox.ask .paw-wave")).not.toBeNull();
    expect(root.querySelector(".fox .eyes-blink")).not.toBeNull();
    expect(root.querySelector("img")).toBeNull();
    renderAvatar(root, fire({ kind: "done", mascotMode: "pop" }), toSrc);
    expect(root.querySelector(".fox.done .paw-cheer-r")).not.toBeNull();
    expect(root.querySelectorAll(".fox-spark")).toHaveLength(3);
  });

  it("peeks from the notch edge, or pops at center with smoke", () => {
    expect(stageFor({ mascotMode: "peek", edge: "left" })).toBe("peek-left");
    expect(stageFor({ mascotMode: "pop", edge: "left" })).toBe("pop");
    const root = document.createElement("div");
    renderAvatar(root, fire({ edge: "top" }), toSrc);
    expect(root.dataset.stage).toBe("peek-top");
    expect(root.querySelector(".fox.fox-peek")).not.toBeNull();
    expect(root.querySelector(".smoke")).toBeNull();
    renderAvatar(root, fire({ edge: "right" }), toSrc);
    expect(root.querySelector(".fox.fox-side")).not.toBeNull();
    renderAvatar(root, fire({ edge: "bottom" }), toSrc);
    expect(root.querySelector(".fox.fox-full")).not.toBeNull();
    renderAvatar(root, fire({ mascotMode: "pop" }), toSrc);
    expect(root.querySelectorAll(".smoke .puff")).toHaveLength(10);
  });

  it("shows the custom image through the converter when avatarSrc is set", () => {
    const root = document.createElement("div");
    renderAvatar(root, fire({ avatarSrc: "C:/x/done.gif" }), toSrc);
    expect(root.querySelector("img")!.getAttribute("src")).toBe("asset://C:/x/done.gif");
    expect(root.querySelector(".fox")).toBeNull();
  });

  it("uses the escalation phrase", () => {
    const root = document.createElement("div");
    expect(renderAvatar(root, fire({ escalation: 1 }), toSrc)).toBe("api is still waiting");
  });
});
