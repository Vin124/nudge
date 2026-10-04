import { describe, expect, it } from "vitest";
import type { AlertFire } from "../alerts-shared/types";
import { renderAvatar } from "./view";

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

  it("shows the built-in mascot (happy for done, urgent for blocked) when avatarSrc is null", () => {
    const root = document.createElement("div");
    renderAvatar(root, fire({ kind: "blocked" }), toSrc);
    expect(root.querySelector("svg.mascot.blocked")).not.toBeNull();
    expect(root.querySelector("img")).toBeNull();
    renderAvatar(root, fire({ kind: "done" }), toSrc);
    expect(root.querySelector("svg.mascot.done")).not.toBeNull();
  });

  it("shows the custom image through the converter when avatarSrc is set", () => {
    const root = document.createElement("div");
    renderAvatar(root, fire({ avatarSrc: "C:/x/done.gif" }), toSrc);
    expect(root.querySelector("img")!.getAttribute("src")).toBe("asset://C:/x/done.gif");
    expect(root.querySelector("svg")).toBeNull();
  });

  it("uses the escalation phrase", () => {
    const root = document.createElement("div");
    expect(renderAvatar(root, fire({ escalation: 1 }), toSrc)).toBe("api is still waiting");
  });
});
