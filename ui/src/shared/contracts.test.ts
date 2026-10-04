// Regression: pages load before Rust setup() manages the core, so the first
// get_snapshot rejects. onSnapshot must retry, or Settings shows a blank form.
import { describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

describe("onSnapshot", () => {
  it("retries the initial fetch until the core is ready", async () => {
    vi.useFakeTimers();
    const snap = { sessions: [], usage: {}, config: { dnd: false } };
    invoke
      .mockRejectedValueOnce("state not managed")
      .mockRejectedValueOnce("state not managed")
      .mockResolvedValue(snap);
    const { onSnapshot } = await import("./contracts");
    const cb = vi.fn();
    const done = onSnapshot(cb);
    await vi.advanceTimersByTimeAsync(1000);
    await done;
    expect(invoke).toHaveBeenCalledTimes(3);
    expect(cb).toHaveBeenCalledWith(snap);
    vi.useRealTimers();
  });
});
