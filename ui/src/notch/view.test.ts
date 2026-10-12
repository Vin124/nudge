import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CodexUsage, Config, Session, Snapshot, UsageSnapshot } from "../shared/contracts";
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
  id, agent: "claude", project: `proj-${id}`, cwd: `/x/${id}`, state: "idle", sinceMs: 0, alertPending: false, contextPercent: null, ...over,
});

const snap = (sessions: Session[], u = usage(), showWeekly = true, codexUsage: CodexUsage | null = null): Snapshot => ({
  sessions, usage: u, codexUsage, config: config(showWeekly),
});

const NOW = 1_000_000 * 1000;
let root: HTMLElement;
let view: NotchView;
let onFocus: ReturnType<typeof vi.fn>;
let onSettings: ReturnType<typeof vi.fn>;
let onBack: ReturnType<typeof vi.fn>;

beforeEach(() => {
  document.body.innerHTML = "<div id=app></div>";
  root = document.getElementById("app")!;
  onFocus = vi.fn(() => Promise.resolve());
  onSettings = vi.fn();
  onBack = vi.fn();
  view = createView(root, { onFocus, onSettings, onBack });
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

describe("context ring", () => {
  const circ = 2 * Math.PI * 14.5;
  const dot = () => root.querySelector('.dots .avatar[data-id="a"]') as HTMLElement;
  it("fills the session ring with context use and shows it in the row", () => {
    view.render(snap([sess("a", { state: "running", contextPercent: 42 })]), NOW);
    expect(dot().classList.contains("ctx")).toBe(true);
    expect(Number(dot().querySelector(".arc")!.getAttribute("stroke-dashoffset"))).toBeCloseTo(circ * 0.58, 3);
    const ctx = root.querySelector(".row-ctx") as HTMLElement;
    expect(ctx.textContent).toBe("42% context");
    expect(ctx.hidden).toBe(false);
    expect(dot().getAttribute("title")).toBe("proj-a — Running · 42% context");
  });
  it("warns as the context fills up", () => {
    view.render(snap([sess("a", { contextPercent: 90 })]), NOW);
    expect(root.querySelector(".row-ctx")!.classList.contains("full")).toBe(true);
    view.render(snap([sess("a", { contextPercent: 70 })]), NOW);
    expect(root.querySelector(".row-ctx")!.classList.contains("warn")).toBe(true);
  });
  it("falls back to a plain state ring without context data", () => {
    view.render(snap([sess("a", { contextPercent: 30 })]), NOW);
    view.render(snap([sess("a", { contextPercent: null })]), NOW);
    expect(dot().classList.contains("ctx")).toBe(false);
    expect((root.querySelector(".row-ctx") as HTMLElement).hidden).toBe(true);
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

describe("settings gear", () => {
  it("opens settings without bubbling to the notch", () => {
    const bubbled = vi.fn();
    view.shape.addEventListener("click", bubbled);
    (root.querySelector(".panel .gear") as HTMLButtonElement).click();
    expect(onSettings).toHaveBeenCalledTimes(1);
    expect(bubbled).not.toHaveBeenCalled();
  });
});

describe("settings page (D24)", () => {
  it("switches pages and the back chevron calls onBack without bubbling", () => {
    expect(view.shape.dataset.page).toBe("home");
    view.setPage("settings");
    expect(view.shape.dataset.page).toBe("settings");
    expect(view.settingsHost.closest(".pane")).not.toBeNull();
    const bubbled = vi.fn();
    view.shape.addEventListener("click", bubbled);
    (root.querySelector(".pane .back") as HTMLButtonElement).click();
    expect(onBack).toHaveBeenCalledTimes(1);
    expect(bubbled).not.toHaveBeenCalled();
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

describe("Codex (D29, D30)", () => {
  const silent = () => usage({ available: null, fiveHour: null, sevenDay: null });
  const cx = (...w: [number, number][]): CodexUsage => ({
    windows: w.map(([windowMinutes, usedPercentage]) => ({ windowMinutes, usedPercentage, resetsAt: null })),
    updatedMs: 0,
  });
  const items = () => [...root.querySelectorAll(".usage-codex .usage-item")];
  const titles = () => items().map((i) => i.querySelector(".usage-name")!.textContent);
  const gauge = () => root.querySelector('.bar [data-wheel="usage"]') as HTMLElement;

  it("Claude-only snapshot has no tag, no group labels and a hidden Codex group", () => {
    view.render(snap([sess("a")]), NOW);
    expect(root.querySelector(".row-tag")).toBeNull();
    expect(root.querySelector(".row")!.getAttribute("data-agent")).toBe("claude");
    expect((root.querySelector(".usage-codex") as HTMLElement).hidden).toBe(true);
    expect(items()).toHaveLength(0);
    const label = root.querySelector(".usage:not(.usage-codex) .usage-label") as HTMLElement;
    expect(label.hidden).toBe(true);
    expect(view.usageExtra()).toBe(0);
  });
  it("tags Codex rows and dots, not Claude rows", () => {
    view.render(snap([sess("a", { agent: "codex" }), sess("b")]), NOW);
    const row = (id: string) => root.querySelector(`.row[data-id="${id}"]`) as HTMLElement;
    expect(row("a").dataset.agent).toBe("codex");
    expect(row("a").querySelector(".row-tag")!.textContent).toBe("Codex");
    expect(row("a").querySelector(".row-name")!.textContent).toBe("proj-aCodex");
    expect((root.querySelector('.dots .avatar[data-id="a"]') as HTMLElement).dataset.agent).toBe("codex");
    expect(row("b").dataset.agent).toBe("claude");
    expect(row("b").querySelector(".row-tag")).toBeNull();
    expect(row("b").querySelector(".row-name")!.textContent).toBe("proj-b");
  });
  it("relabels Codex items when the window lengths change, then removes the group", () => {
    view.render(snap([], usage(), true, cx([300, 20], [10080, 55])), NOW);
    expect(titles()).toEqual(["5-hour", "Weekly"]);
    expect(items().map((i) => i.querySelector(".usage-pct")!.textContent)).toEqual(["20%", "55%"]);
    view.render(snap([], usage(), true, cx([43200, 7])), NOW);
    expect(titles()).toEqual(["30-day"]);
    expect(items()[0].querySelector(".usage-pct")!.textContent).toBe("7%");
    view.render(snap([], usage(), true, null), NOW);
    expect(items()).toHaveLength(0);
    expect((root.querySelector(".usage-codex") as HTMLElement).hidden).toBe(true);
    expect(view.usageExtra()).toBe(0);
  });
  it("labels both groups and grows the header when Claude is heard", () => {
    view.render(snap([], usage(), true, cx([300, 20])), NOW);
    const claudeLabel = root.querySelector(".usage:not(.usage-codex) .usage-label") as HTMLElement;
    expect(claudeLabel.hidden).toBe(false);
    expect(claudeLabel.textContent).toBe("Claude");
    expect(root.querySelector(".usage-codex .usage-label")!.textContent).toBe("Codex");
    expect(view.usageExtra()).toBeGreaterThan(0);
    // gauge stays Claude's
    expect(gauge().getAttribute("title")).toBe("5-hour limit 42% used");
  });
  it("hides Claude's group and gauges Codex when Claude was never heard", () => {
    view.render(snap([], silent(), true, cx([300, 20], [10080, 55])), NOW);
    expect((root.querySelector(".usage:not(.usage-codex)") as HTMLElement).hidden).toBe(true);
    expect((root.querySelector(".usage-codex") as HTMLElement).hidden).toBe(false);
    expect(view.usageExtra()).toBe(0);
    expect(gauge().getAttribute("title")).toBe("Codex 5-hour limit 20% used");
    expect(gauge().classList.contains("ok")).toBe(true);
    const off = (n: string) => Number(gauge().querySelector(`.arc.${n}`)!.getAttribute("stroke-dashoffset"));
    expect(off("five")).toBeCloseTo(2 * Math.PI * 14 * 0.8, 3);
    expect(off("seven")).toBeCloseTo(2 * Math.PI * 9.5 * 0.45, 3);
    view.render(snap([], silent(), false, cx([300, 20], [10080, 55])), NOW);
    expect(gauge().querySelector(".arc.seven")!.hasAttribute("stroke")).toBe(false);
  });
});
