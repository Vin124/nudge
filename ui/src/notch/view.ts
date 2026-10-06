// DOM rendering for the notch from a Snapshot. No Tauri imports.
// All data goes in via textContent/attributes, never innerHTML.

import type { Edge, Session, SessionState, Snapshot, UsageWindow } from "../shared/contracts";
import { formatCountdown, formatElapsed, formatPercent, usageColor } from "./format";
import { MAX_AVATARS, type Page, type Rect } from "./geometry";
import { chevronLeft, critter, gear, sparkle, svgEl } from "./icons";

export const NA_TOOLTIP =
  "Usage limits aren't available for API-key sessions — enable live usage in Settings";
export const EMPTY_TEXT = "No live Claude Code sessions";
export const FOCUS_FAIL_TEXT = "Couldn't focus that window";
export const ROW_MESSAGE_MS = 2000;

const STATE_LABEL: Record<SessionState, string> = {
  idle: "Idle",
  running: "Running",
  blocked: "Needs you",
  done: "Done",
};

/** Session critter colors: warm, muted, distinct from the state ring colors. */
export const TINTS = ["#D97757", "#E6B85C", "#7FA7D9", "#8DC07F", "#B39DDB", "#E58FB0", "#5FBFB2", "#C9A27E"];

/** Stable tint for `id`: hash slot, then the next free one if a live session holds it. */
export function pickTint(id: string, taken: ReadonlySet<number>): number {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
  const start = h % TINTS.length;
  for (let k = 0; k < TINTS.length; k++) {
    const t = (start + k) % TINTS.length;
    if (!taken.has(t)) return t;
  }
  return start;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function setText(e: Element, text: string): void {
  if (e.textContent !== text) e.textContent = text;
}
function setClass(e: Element, cls: string): void {
  if (e.getAttribute("class") !== cls) e.setAttribute("class", cls);
}
function setAttr(e: Element, name: string, v: string): void {
  if (e.getAttribute(name) !== v) e.setAttribute(name, v);
}

interface Arc {
  arc: SVGCircleElement;
  circ: number;
}

/** Track + arc circle pair in a 0..size viewBox, arc starting at 12 o'clock. */
function ring(svg: SVGSVGElement, size: number, r: number, stroke: number, cls = ""): Arc {
  const c = size / 2;
  const circ = 2 * Math.PI * r;
  const base = { cx: c, cy: c, r, fill: "none", "stroke-width": stroke };
  const track = svgEl("circle", { ...base, class: `track ${cls}`.trim() });
  const arc = svgEl("circle", {
    ...base,
    class: `arc ${cls}`.trim(),
    "stroke-linecap": "round",
    "stroke-dasharray": `${circ} ${circ}`,
    "stroke-dashoffset": circ,
    transform: `rotate(-90 ${c} ${c})`,
  });
  svg.append(track, arc);
  return { arc, circ };
}

function setArc(a: Arc, pct: number | null): void {
  const off = pct === null ? a.circ : a.circ * (1 - pct / 100);
  setAttr(a.arc, "stroke-dashoffset", String(off));
  if (pct === null) a.arc.removeAttribute("stroke");
  else setAttr(a.arc, "stroke", usageColor(pct));
}

type UsageKey = "na" | "empty" | "ok";
function usageKey(win: UsageWindow | null, available: boolean | null): UsageKey {
  if (available === false) return "na";
  return available === null || win === null ? "empty" : "ok";
}

// ---- usage gauges ----

interface Gauge {
  el: HTMLElement;
  five: Arc;
  seven: Arc;
}

/** Collapsed: 5h outer ring, weekly inner ring, sparkle in the middle. */
function createGauge(): Gauge {
  const root = el("div", "gauge");
  root.dataset.wheel = "usage";
  const svg = svgEl("svg", { viewBox: "0 0 32 32", class: "rings" });
  const five = ring(svg, 32, 14, 3, "five");
  const seven = ring(svg, 32, 9.5, 2.5, "seven");
  root.append(svg, sparkle());
  return { el: root, five, seven };
}

function updateGauge(g: Gauge, s: Snapshot): void {
  const { usage: u, config } = s;
  const key = usageKey(u.fiveHour, u.available);
  setClass(g.el, `gauge ${key}${config.notch.showWeekly ? " weekly" : ""}`);
  if (key === "na") setAttr(g.el, "title", NA_TOOLTIP);
  else if (key === "ok") setAttr(g.el, "title", `5-hour limit ${formatPercent(u.fiveHour!.usedPercentage)}% used`);
  else g.el.removeAttribute("title");
  setArc(g.five, key === "ok" ? formatPercent(u.fiveHour!.usedPercentage) : null);
  const weekly = u.available && u.sevenDay ? formatPercent(u.sevenDay.usedPercentage) : null;
  setArc(g.seven, config.notch.showWeekly ? weekly : null);
}

interface UsageItem {
  el: HTMLElement;
  arc: Arc;
  pct: HTMLElement;
  reset: HTMLElement;
}

/** Expanded header: one ring + percent + reset countdown. */
function createUsageItem(name: string, title: string): UsageItem {
  const root = el("div", "usage-item");
  root.dataset.usage = name;
  const dial = el("div", "dial");
  const svg = svgEl("svg", { viewBox: "0 0 44 44", class: "rings" });
  const arc = ring(svg, 44, 19, 4);
  dial.append(svg, sparkle());
  const text = el("div", "usage-text");
  const pct = el("div", "usage-pct");
  const reset = el("div", "usage-reset");
  text.append(el("div", "usage-name", title), pct, reset);
  root.append(dial, text);
  return { el: root, arc, pct, reset };
}

function updateUsageItem(u: UsageItem, win: UsageWindow | null, available: boolean | null, nowMs: number): void {
  const key = usageKey(win, available);
  setClass(u.el, `usage-item ${key}`);
  if (key === "na") {
    setArc(u.arc, null);
    setText(u.pct, "n/a");
    setText(u.reset, "API key session");
    setAttr(u.el, "title", NA_TOOLTIP);
    return;
  }
  u.el.removeAttribute("title");
  if (key === "empty" || win === null) {
    setArc(u.arc, null);
    setText(u.pct, "—");
    setText(u.reset, "waiting for data");
    return;
  }
  const p = formatPercent(win.usedPercentage);
  setArc(u.arc, p);
  setText(u.pct, `${p}%`);
  if (win.resetsAt == null) setText(u.reset, "");
  else {
    const c = formatCountdown(win.resetsAt, nowMs);
    setText(u.reset, c === "resetting" ? c : `resets in ${c}`);
  }
}

// ---- session avatars ----

interface Avatar {
  el: HTMLElement;
  arc: Arc;
}

/** State ring around a tinted critter. */
function createAvatar(tint: number): Avatar {
  const root = el("span", "avatar");
  root.style.setProperty("--tint", TINTS[tint]);
  const svg = svgEl("svg", { viewBox: "0 0 32 32", class: "rings" });
  const arc = ring(svg, 32, 14.5, 2.5);
  root.append(svg, critter());
  return { el: root, arc };
}

function avatarClass(s: Session): string {
  return `avatar state-${s.state}${s.alertPending ? " alert-pending" : ""}${s.contextPercent != null ? " ctx" : ""}`;
}

/** D22: the ring fills with context-window use; without data it stays a full state ring. */
function updateRing(a: Avatar, s: Session): void {
  const p = s.contextPercent == null ? 0 : formatPercent(s.contextPercent);
  setAttr(a.arc.arc, "stroke-dashoffset", String(a.arc.circ * (1 - p / 100)));
}

function contextClass(p: number): string {
  return p >= 85 ? "row-ctx full" : p >= 60 ? "row-ctx warn" : "row-ctx";
}

interface Entry {
  tint: number;
  dot: Avatar;
  row: HTMLElement;
  rowAvatar: Avatar;
  name: HTMLElement;
  state: HTMLElement;
  since: HTMLElement;
  ctx: HTMLElement;
  msg: HTMLElement;
  msgTimer: ReturnType<typeof setTimeout> | null;
}

export interface ViewOptions {
  /** Called when a row is clicked. A rejection shows the inline row message. */
  onFocus: (id: string) => Promise<void>;
  /** D23: the small gear in the expanded panel. */
  onSettings: () => void;
  /** D24: the back chevron on the Settings page. */
  onBack: () => void;
}

export type Motion = "open" | "close" | "snap" | "none";

export interface NotchView {
  /** The visible shape; drag and hover attach here. */
  shape: HTMLElement;
  render(snapshot: Snapshot, nowMs?: number): void;
  /** Refresh time-based text (elapsed, countdowns) from the last snapshot. */
  tick(nowMs?: number): void;
  setExpanded(expanded: boolean): void;
  isExpanded(): boolean;
  setEdge(edge: Edge): void;
  /** Lifted off the edge while dragging. */
  setFloating(floating: boolean): void;
  /** Picks the transition curve for the next `setRect`. */
  setMotion(motion: Motion): void;
  setRect(r: Rect): void;
  /** D24: which page the expanded notch shows. */
  setPage(page: Page): void;
  /** D24: the Settings page body; the caller mounts the settings view here. */
  settingsHost: HTMLElement;
  showRowMessage(id: string, text: string, ms?: number): void;
  sessionCount(): number;
}

export function createView(root: HTMLElement, opts: ViewOptions): NotchView {
  const shape = el("div", "shape");
  shape.dataset.edge = "top";
  shape.dataset.expanded = "false";
  shape.dataset.floating = "false";
  shape.dataset.motion = "none";
  shape.dataset.page = "home";
  const earA = el("span", "ear ear-a");
  const earB = el("span", "ear ear-b");
  const body = el("div", "body");

  const bar = el("div", "bar");
  const gauge = createGauge();
  const dots = el("div", "dots");
  const more = el("span", "more");
  bar.append(gauge.el, dots, more);

  const panel = el("div", "panel");
  const usage = el("div", "usage");
  const u5 = createUsageItem("five", "5-hour");
  const u7 = createUsageItem("seven", "Weekly");
  usage.append(u5.el, u7.el);
  const list = el("div", "list");
  const empty = el("div", "empty-state", EMPTY_TEXT);
  const settings = el("button", "gear");
  settings.type = "button";
  settings.title = "Settings";
  settings.setAttribute("aria-label", "Settings");
  settings.append(gear());
  settings.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onSettings();
  });
  panel.append(usage, list, empty, settings);

  // ---- D24: Settings page ----
  const pane = el("div", "pane");
  pane.setAttribute("aria-label", "Settings");
  const paneHead = el("div", "pane-head");
  const back = el("button", "back");
  back.type = "button";
  back.setAttribute("aria-label", "Back");
  back.append(chevronLeft());
  back.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onBack();
  });
  paneHead.append(back, el("div", "pane-title", "Settings"));
  const settingsHost = el("div", "pane-scroll settings-pane");
  pane.append(paneHead, settingsHost);

  body.append(bar, panel, pane);
  shape.append(earA, earB, body);
  root.append(shape);

  const entries = new Map<string, Entry>();
  let last: Snapshot | null = null;

  function showRowMessage(id: string, text: string, ms = ROW_MESSAGE_MS): void {
    const e = entries.get(id);
    if (!e) return;
    if (e.msgTimer) clearTimeout(e.msgTimer);
    setText(e.msg, text);
    e.row.classList.add("has-msg");
    e.msgTimer = setTimeout(() => {
      e.msgTimer = null;
      e.row.classList.remove("has-msg");
      setText(e.msg, "");
    }, ms);
  }

  function createEntry(id: string): Entry {
    const tint = pickTint(id, new Set([...entries.values()].map((e) => e.tint)));
    const dot = createAvatar(tint);
    dot.el.dataset.id = id;
    const row = el("div", "row");
    row.dataset.id = id;
    row.setAttribute("role", "button");
    const rowAvatar = createAvatar(tint);
    const text = el("div", "row-text");
    const name = el("div", "row-name");
    const sub = el("div", "row-sub");
    const state = el("span", "row-state");
    const since = el("span", "row-since");
    const ctx = el("span", "row-ctx");
    sub.append(state, since, ctx);
    text.append(name, sub);
    const msg = el("div", "row-msg");
    row.append(rowAvatar.el, text, msg);
    row.addEventListener("click", () => {
      Promise.resolve()
        .then(() => opts.onFocus(id))
        .catch(() => showRowMessage(id, FOCUS_FAIL_TEXT));
    });
    return { tint, dot, row, rowAvatar, name, state, since, ctx, msg, msgTimer: null };
  }

  function updateEntry(e: Entry, s: Session, nowMs: number): void {
    const cls = avatarClass(s);
    setClass(e.dot.el, cls);
    setClass(e.rowAvatar.el, cls);
    updateRing(e.dot, s);
    updateRing(e.rowAvatar, s);
    const ctxText = s.contextPercent == null ? "" : `${formatPercent(s.contextPercent)}% context`;
    setAttr(e.dot.el, "title", `${s.project} — ${STATE_LABEL[s.state]}${ctxText ? ` · ${ctxText}` : ""}`);
    setText(e.ctx, ctxText);
    setClass(e.ctx, contextClass(s.contextPercent == null ? 0 : formatPercent(s.contextPercent)));
    if (e.ctx.hidden !== !ctxText) e.ctx.hidden = !ctxText;
    setText(e.name, s.project);
    setAttr(e.name, "title", s.cwd);
    setClass(e.state, `row-state state-${s.state}`);
    setText(e.state, STATE_LABEL[s.state]);
    setText(e.since, formatElapsed(nowMs - s.sinceMs));
  }

  function reorder(container: HTMLElement, nodes: HTMLElement[]): void {
    let ref: ChildNode | null = container.firstChild;
    for (const n of nodes) {
      if (n === ref) ref = ref.nextSibling;
      else container.insertBefore(n, ref);
    }
  }

  function render(snapshot: Snapshot, nowMs: number = Date.now()): void {
    last = snapshot;
    const { sessions, usage: u, config } = snapshot;

    const seen = new Set(sessions.map((s) => s.id));
    for (const [id, e] of entries) {
      if (seen.has(id)) continue;
      if (e.msgTimer) clearTimeout(e.msgTimer);
      e.dot.el.remove();
      e.row.remove();
      entries.delete(id);
    }
    for (const s of sessions) {
      let e = entries.get(s.id);
      if (!e) {
        e = createEntry(s.id);
        entries.set(s.id, e);
      }
      updateEntry(e, s, nowMs);
    }
    const ordered = sessions.map((s) => entries.get(s.id)!);
    ordered.forEach((e, i) => {
      e.dot.el.hidden = i >= MAX_AVATARS;
    });
    reorder(dots, ordered.map((e) => e.dot.el));
    reorder(list, ordered.map((e) => e.row));
    const extra = sessions.length - MAX_AVATARS;
    more.hidden = extra <= 0;
    setText(more, extra > 0 ? `+${extra}` : "");
    empty.hidden = sessions.length > 0;

    updateGauge(gauge, snapshot);
    u7.el.hidden = !config.notch.showWeekly;
    updateUsageItem(u5, u.fiveHour, u.available, nowMs);
    updateUsageItem(u7, u.sevenDay, u.available, nowMs);
  }

  return {
    shape,
    render,
    tick(nowMs: number = Date.now()) {
      if (last) render(last, nowMs);
    },
    setExpanded(expanded: boolean) {
      setAttr(shape, "data-expanded", String(expanded));
    },
    isExpanded: () => shape.dataset.expanded === "true",
    setEdge(edge: Edge) {
      setAttr(shape, "data-edge", edge);
    },
    setFloating(floating: boolean) {
      setAttr(shape, "data-floating", String(floating));
    },
    setMotion(motion: Motion) {
      setAttr(shape, "data-motion", motion);
    },
    setRect(r: Rect) {
      const s = shape.style;
      s.left = `${r.x}px`;
      s.top = `${r.y}px`;
      s.width = `${r.width}px`;
      s.height = `${r.height}px`;
    },
    setPage(page: Page) {
      setAttr(shape, "data-page", page);
      if (page === "settings") settingsHost.scrollTop = 0;
    },
    settingsHost,
    showRowMessage,
    sessionCount: () => entries.size,
  };
}
