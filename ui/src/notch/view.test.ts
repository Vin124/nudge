import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Config, Session, Snapshot, UsageSnapshot } from "../shared/contracts";
import { createView, EMPTY_TEXT, FOCUS_FAIL_TEXT, NA_TOOLTIP, pickTint, TINTS, type NotchView } from "./view";

const config = (showWeekly = true): Config => ({
  version: 1,
  notch: { edge: "top", offset: 0.5, monitor: null, showWeekly },
  alerts: {} as Config["alerts"],
  usage: { liveWhenIdle: false },
  dnd: false,
});

const usage = (over: Partial<UsageSnapshot> = {}): UsageSnapshot => ({
  fiveHour: { usedPercentage: 42, resetsAt: 1_000_000 + 3600 + 12 * 60 },
  sevenDay: { usedPercentage: 90, resetsAt: null },
  available: true,
  source: "statusline",
  updatedMs: 0,
  ...over,
});

const sess = (id: string, over: Partial<Session> = {}): Session => ({
  id, project: `proj-${id}`, cwd: `/x/${id}`, state: "idle", sinceMs: 0, alertPending: false, ...over,
});

const snap = (sessions: Session[], u = usage(), showWeekly = true): Snapshot => ({
  sessions, usage: u, config: config(showWeekly),
});

const NOW = 1_000_000 * 1000;
let root: HTMLElement;
let view: NotchView;
let onFocus: ReturnType<typeof vi.fn>;

beforeEach(() => {
  document.body.innerHTML = "<div id=app></div>";
  root = document.getElementById("app")!;
  onFocus = vi.fn(() => Promise.resolve());
  view = createView(root, { onFocus });
});
afterEach(() => vi.useRealTimers());

describe("session avatars", () => {
  it("renders one dot per session", () => {
    view.render(snap([sess("a"), sess("b"), sess("c")]), NOW);
    expect(root.querySelectorAll(".dots .avatar")).toHaveLength(3);
    expect(root.querySelectorAll(".list .row")).toHaveLength(3);
  });
  it("maps states to classes", () => {
    view.render(snap([
      sess("a", { state: "idle" }), sess("b", { state: "running" }),
      sess("c", { state: "done" }), sess("d", { state: "blocked" }),
    ]), NOW);
    const dots = [...root.querySelectorAll(".dots .avatar")];
    expect(dots.map((d) => [...d.classList].find((c) => c.startsWith("state-")))).toEqual([
      "state-idle", "state-running", "state-done", "state-blocked",
    ]);
  });
  it("marks alertPending", () => {
    view.render(snap([sess("a", { alertPending: true }), sess("b")]), NOW);
    const dots = root.querySelectorAll(".dots .avatar");
    expect(dots[0].classList.contains("alert-pending")).toBe(true);
    expect(dots[1].classList.contains("alert-pending")).toBe(false);
  });
});

describe("usage", () => {
  const gauge = () => root.querySelector('.bar [data-wheel="usage"]') as HTMLElement;
  it("shows the weekly inner ring only when showWeekly is on", () => {
    view.render(snap([], usage(), false), NOW);
    expect(gauge().classList.contains("weekly")).toBe(false);
    expect((root.querySelector('[data-usage="seven"]') as HTMLElement).hidden).toBe(true);
    view.render(snap([], usage(), true), NOW);
    expect(gauge().classList.contains("weekly")).toBe(true);
    expect((root.querySelector('[data-usage="seven"]') as HTMLElement).hidden).toBe(false);
  });
  it("shows percent and reset countdown", () => {
    view.render(snap([]), NOW);
    expect(root.querySelector('[data-usage="five"] .usage-pct')!.textContent).toBe("42%");
    expect(root.querySelector('[data-usage="five"] .usage-reset')!.textContent).toBe("resets in 1h 12m");
  });
  it("fills the 5h ring in proportion to usage", () => {
    view.render(snap([]), NOW);
    const arc = gauge().querySelector(".arc.five")!;
    const circ = 2 * Math.PI * 14;
    expect(Number(arc.getAttribute("stroke-dashoffset"))).toBeCloseTo(circ * 0.58, 3);
  });
  it("shows n/a with tooltip when available is false", () => {
    view.render(snap([], usage({ available: false })), NOW);
    expect(gauge().classList.contains("na")).toBe(true);
    expect(gauge().getAttribute("title")).toBe(NA_TOOLTIP);
    expect(root.querySelector('[data-usage="five"] .usage-pct')!.textContent).toBe("n/a");
  });
  it("shows empty rings when available is null", () => {
    view.render(snap([], usage({ available: null, fiveHour: null, sevenDay: null })), NOW);
    expect(gauge().classList.contains("empty")).toBe(true);
    expect(gauge().hasAttribute("title")).toBe(false);
  });
  it("ramps the ring color with usage", () => {
    view.render(snap([], usage({ fiveHour: { usedPercentage: 10, resetsAt: null }, sevenDay: { usedPercentage: 90, resetsAt: null } })), NOW);
    const arc = (n: string) => gauge().querySelector(`.arc.${n}`)!.getAttribute("stroke");
    expect(arc("five")).not.toBe(arc("seven"));
  });
});

describe("tints and overflow", () => {
  it("gives live sessions distinct tints that stay put across renders", () => {
    const ids = ["a", "b", "c", "d", "e", "f"];
    view.render(snap(ids.map((i) => sess(i))), NOW);
    const tint = (id: string) => (root.querySelector(`.dots .avatar[data-id="${id}"]`) as HTMLElement).style.getPropertyValue("--tint");
    const first = ids.map(tint);
    expect(new Set(first).size).toBe(ids.length);
    view.render(snap([...ids].reverse().map((i) => sess(i, { state: "running" }))), NOW);
    expect(ids.map(tint)).toEqual(first);
  });
  it("pickTint skips taken slots", () => {
    const t = pickTint("x", new Set());
    expect(pickTint("x", new Set([t]))).toBe((t + 1) % TINTS.length);
  });
  it("shows at most 6 avatars plus a +k chip", () => {
    view.render(snap(Array.from({ length: 9 }, (_, i) => sess(`s${i}`))), NOW);
    const shown = [...root.querySelectorAll(".dots .avatar")].filter((a) => !(a as HTMLElement).hidden);
    expect(shown).toHaveLength(6);
    const more = root.querySelector(".more") as HTMLElement;
    expect(more.hidden).toBe(false);
    expect(more.textContent).toBe("+3");
    expect(root.querySelectorAll(".list .row")).toHaveLength(9);
    view.render(snap([sess("a")]), NOW);
    expect(more.hidden).toBe(true);
  });
});

describe("safety and keyed updates", () => {
  it("renders a hostile project name as text", () => {
    view.render(snap([sess("a", { project: "<script>alert(1)</script>" })]), NOW);
    expect(root.querySelector("script")).toBeNull();
    expect(root.querySelector(".row-name")!.textContent).toBe("<script>alert(1)</script>");
  });
  it("preserves node identity and touches only the changed row", () => {
    const base = [sess("a"), sess("b"), sess("c")];
    view.render(snap(base), NOW);
    const before = [...root.querySelectorAll(".row")];
    const dotsBefore = [...root.querySelectorAll(".dots .avatar")];

    const muts: MutationRecord[] = [];
    const mo = new MutationObserver((r) => muts.push(...r));
    mo.observe(root, { subtree: true, childList: true, attributes: true, characterData: true });
    view.render(snap([base[0], sess("b", { state: "done", alertPending: true }), base[2]]), NOW);
    muts.push(...mo.takeRecords());
    mo.disconnect();

    const after = [...root.querySelectorAll(".row")];
    expect(after).toEqual(before);
    expect([...root.querySelectorAll(".dots .avatar")]).toEqual(dotsBefore);
    const touched = new Set(muts.map((m) => (m.target as Node).parentElement?.closest(".row, .dots .avatar") ?? (m.target as Element).closest?.(".row, .dots .avatar")));
    touched.delete(null);
    const ids = [...touched].map((n) => (n as HTMLElement).dataset.id);
    expect(new Set(ids)).toEqual(new Set(["b"]));
  });
  it("removes sessions that disappear and keeps order", () => {
    view.render(snap([sess("a"), sess("b"), sess("c")]), NOW);
    const c = root.querySelector('.row[data-id="c"]');
    view.render(snap([sess("c"), sess("a")]), NOW);
    const ids = [...root.querySelectorAll(".row")].map((r) => (r as HTMLElement).dataset.id);
    expect(ids).toEqual(["c", "a"]);
    expect(root.querySelector('.row[data-id="c"]')).toBe(c);
    expect(root.querySelectorAll(".dots .avatar")).toHaveLength(2);
  });
  it("shows the empty state only with no sessions", () => {
    view.render(snap([]), NOW);
    const e = root.querySelector(".empty-state") as HTMLElement;
    expect(e.textContent).toBe(EMPTY_TEXT);
    expect(e.hidden).toBe(false);
    view.render(snap([sess("a")]), NOW);
    expect(e.hidden).toBe(true);
  });
  it("updates elapsed text on tick", () => {
    view.render(snap([sess("a", { sinceMs: NOW - 61_000 })]), NOW);
    expect(root.querySelector(".row-since")!.textContent).toBe("1m");
    view.tick(NOW + 3600_000);
    expect(root.querySelector(".row-since")!.textContent).toBe("1h 1m");
  });
});

describe("row click", () => {
  it("calls onFocus with the session id", async () => {
    view.render(snap([sess("a")]), NOW);
    (root.querySelector(".row") as HTMLElement).click();
    await Promise.resolve();
    expect(onFocus).toHaveBeenCalledWith("a");
  });
  it("shows an inline message for 2 seconds when focus rejects", async () => {
    vi.useFakeTimers();
    onFocus.mockRejectedValue(new Error("nope"));
    view.render(snap([sess("a")]), NOW);
    const row = root.querySelector(".row") as HTMLElement;
    row.click();
    await vi.advanceTimersByTimeAsync(0);
    expect(row.classList.contains("has-msg")).toBe(true);
    expect(row.querySelector(".row-msg")!.textContent).toBe(FOCUS_FAIL_TEXT);
    await vi.advanceTimersByTimeAsync(2000);
    expect(row.classList.contains("has-msg")).toBe(false);
  });
});
