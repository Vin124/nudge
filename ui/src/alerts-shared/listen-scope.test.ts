// Regression: Rust emit_to()s each alert window separately. A global listen()
// receives every window's copy (3x glow restarts, 3x sound). Each alert page
// must listen on its own webview window only.
import { beforeEach, describe, expect, it, vi } from "vitest";

const globalListen = vi.fn(() => Promise.resolve(() => {}));
const windowListen = vi.fn(() => Promise.resolve(() => {}));

vi.mock("@tauri-apps/api/event", () => ({ listen: globalListen, emit: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({ label: "test", listen: windowListen }),
}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: "test", hide: vi.fn() }) }));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => p, invoke: vi.fn() }));

describe.each(["../glow/main", "../avatar/main"])("%s", (mod) => {
  beforeEach(() => {
    vi.resetModules();
    globalListen.mockClear();
    windowListen.mockClear();
    document.body.innerHTML = '<div id="app"></div>';
    window.matchMedia ??= (() => ({ matches: false, addEventListener() {}, removeEventListener() {} })) as never;
  });

  it("subscribes to nudge://alert on its own window, never globally", async () => {
    await import(/* @vite-ignore */ mod);
    const globalAlert = globalListen.mock.calls.filter((c) => (c as unknown[])[0] === "nudge://alert");
    const windowAlert = windowListen.mock.calls.filter((c) => (c as unknown[])[0] === "nudge://alert");
    expect(globalAlert).toHaveLength(0);
    expect(windowAlert).toHaveLength(1);
  });
});
