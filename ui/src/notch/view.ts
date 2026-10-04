// DOM rendering for the notch from a Snapshot. No Tauri imports.
// All data goes in via textContent/attributes, never innerHTML.

import type { Edge, Session, SessionState, Snapshot, UsageWindow } from "../shared/contracts";
import { formatCountdown, formatElapsed, formatPercent, usageColor } from "./format";

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

const SVG_NS = "http://www.w3.org/2000/svg";
const R = 10;
const CIRC = 2 * Math.PI * R;

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
  if (e.className !== cls) e.className = cls;
}
function setAttr(e: Element, name: string, v: string): void {
  if (e.getAttribute(name) !== v) e.setAttribute(name, v);
}

interface Wheel {
  el: HTMLElement;
  arc: SVGCircleElement;
  label: HTMLElement;
  key: string;
}

function createWheel(name: string): Wheel {
  const root = el("div", "wheel empty");
  root.dataset.wheel = name;
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 28 28");
  const mk = (cls: string): SVGCircleElement => {
    const c = document.createElementNS(SVG_NS, "circle");
    c.setAttribute("class", cls);
    c.setAttribute("cx", "14");
    c.setAttribute("cy", "14");
    c.setAttribute("r", String(R));
    c.setAttribute("fill", "none");
    c.setAttribute("stroke-width", "3");
    return c;
  };
  const track = mk("track");
  const arc = mk("arc");
  arc.setAttribute("stroke-linecap", "round");
  arc.setAttribute("stroke-dasharray", `${CIRC} ${CIRC}`);
  arc.setAttribute("stroke-dashoffset", String(CIRC));
  arc.setAttribute("transform", "rotate(-90 14 14)");
  svg.append(track, arc);
  const label = el("span", "wheel-label");
  root.append(svg, label);
  return { el: root, arc, label, key: "" };
}

function updateWheel(w: Wheel, win: UsageWindow | null, available: boolean | null): void {
  let key: string;
  if (available === false) key = "na";
  else if (available === null || win === null) key = "empty";
  else key = `p${formatPercent(win.usedPercentage)}`;
  if (key === w.key) return;
  w.key = key;
  if (key === "na") {
    setClass(w.el, "wheel na");
    setAttr(w.el, "title", NA_TOOLTIP);
    setText(w.label, "n/a");
    w.arc.setAttribute("stroke-dashoffset", String(CIRC));
  } else if (key === "empty" || win === null) {
    setClass(w.el, "wheel empty");
    w.el.removeAttribute("title");
    setText(w.label, "");
    w.arc.setAttribute("stroke-dashoffset", String(CIRC));
  } else {
    const p = formatPercent(win.usedPercentage);
    setClass(w.el, "wheel");
    w.el.removeAttribute("title");
    setText(w.label, "");
    w.arc.setAttribute("stroke", usageColor(p));
    w.arc.setAttribute("stroke-dashoffset", String(CIRC * (1 - p / 100)));
  }
}

interface UsageItem {
  el: HTMLElement;
  wheel: Wheel;
  pct: HTMLElement;
  reset: HTMLElement;
}

function createUsageItem(name: string, title: string): UsageItem {
  const wheel = createWheel(name);
  const root = el("div", "usage-item");
  root.dataset.usage = name;
  const text = el("div", "usage-text");
  const head = el("div", "usage-head");
  const pct = el("span", "usage-pct");
  head.append(el("span", "usage-name", title), pct);
  const reset = el("div", "usage-reset");
  text.append(head, reset);
  root.append(wheel.el, text);
  return { el: root, wheel, pct, reset };
}

function updateUsageItem(u: UsageItem, win: UsageWindow | null, available: boolean | null, nowMs: number): void {
  updateWheel(u.wheel, win, available);
  if (available === false) {
    setText(u.pct, "n/a");
    setText(u.reset, "");
    setAttr(u.el, "title", NA_TOOLTIP);
    return;
  }
  u.el.removeAttribute("title");
  if (available === null || win === null) {
    setText(u.pct, "—");
    setText(u.reset, "");
    return;
  }
  setText(u.pct, `${formatPercent(win.usedPercentage)}%`);
  if (win.resetsAt == null) setText(u.reset, "");
  else {
    const c = formatCountdown(win.resetsAt, nowMs);
    setText(u.reset, c === "resetting" ? c : `resets in ${c}`);
  }
}

interface Entry {
  dot: HTMLElement;
  row: HTMLElement;
  rowDot: HTMLElement;
  name: HTMLElement;
  state: HTMLElement;
  since: HTMLElement;
  msg: HTMLElement;
  msgTimer: ReturnType<typeof setTimeout> | null;
}

export interface ViewOptions {
  /** Called when a row is clicked. A rejection shows the inline row message. */
  onFocus: (id: string) => Promise<void>;
}

export interface NotchView {
  pill: HTMLElement;
  render(snapshot: Snapshot, nowMs?: number): void;
  /** Refresh time-based text (elapsed, countdowns) from the last snapshot. */
  tick(nowMs?: number): void;
  setExpanded(expanded: boolean): void;
  isExpanded(): boolean;
  setEdge(edge: Edge): void;
  showRowMessage(id: string, text: string, ms?: number): void;
  sessionCount(): number;
}

export function createView(root: HTMLElement, opts: ViewOptions): NotchView {
  const pill = el("div", "pill");
  pill.dataset.edge = "top";
  pill.dataset.expanded = "false";

  const bar = el("div", "bar");
  const wheels = el("div", "wheels");
  const w5 = createWheel("five");
  const w7 = createWheel("seven");
  wheels.append(w5.el, w7.el);
  const dots = el("div", "dots");
  bar.append(wheels, dots);

  const panel = el("div", "panel");
  const usage = el("div", "usage");
  const u5 = createUsageItem("five", "5h");
  const u7 = createUsageItem("seven", "Weekly");
  usage.append(u5.el, u7.el);
  const list = el("div", "list");
  const empty = el("div", "empty-state", EMPTY_TEXT);
  panel.append(usage, list, empty);

  pill.append(bar, panel);
  root.append(pill);

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
    const dot = el("span", "dot");
    dot.dataset.id = id;
    const row = el("div", "row");
    row.dataset.id = id;
    row.setAttribute("role", "button");
    const rowDot = el("span", "dot");
    const name = el("span", "row-name");
    const state = el("span", "row-state");
    const since = el("span", "row-since");
    const msg = el("div", "row-msg");
    row.append(rowDot, name, state, since, msg);
    row.addEventListener("click", () => {
      Promise.resolve()
        .then(() => opts.onFocus(id))
        .catch(() => showRowMessage(id, FOCUS_FAIL_TEXT));
    });
    return { dot, row, rowDot, name, state, since, msg, msgTimer: null };
  }

  function updateEntry(e: Entry, s: Session, nowMs: number): void {
    const cls = `dot state-${s.state}${s.alertPending ? " alert-pending" : ""}`;
    setClass(e.dot, cls);
    setClass(e.rowDot, cls);
    setAttr(e.dot, "title", `${s.project} — ${STATE_LABEL[s.state]}`);
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

    const seen = new Set<string>();
    for (const s of sessions) {
      seen.add(s.id);
      let e = entries.get(s.id);
      if (!e) {
        e = createEntry(s.id);
        entries.set(s.id, e);
      }
      updateEntry(e, s, nowMs);
    }
    for (const [id, e] of entries) {
      if (seen.has(id)) continue;
      if (e.msgTimer) clearTimeout(e.msgTimer);
      e.dot.remove();
      e.row.remove();
      entries.delete(id);
    }
    const ordered = sessions.map((s) => entries.get(s.id)!);
    reorder(dots, ordered.map((e) => e.dot));
    reorder(list, ordered.map((e) => e.row));
    empty.hidden = sessions.length > 0;

    const showWeekly = config.notch.showWeekly;
    w7.el.hidden = !showWeekly;
    u7.el.hidden = !showWeekly;
    updateWheel(w5, u.fiveHour, u.available);
    updateWheel(w7, u.sevenDay, u.available);
    updateUsageItem(u5, u.fiveHour, u.available, nowMs);
    updateUsageItem(u7, u.sevenDay, u.available, nowMs);
  }

  return {
    pill,
    render,
    tick(nowMs: number = Date.now()) {
      if (last) render(last, nowMs);
    },
    setExpanded(expanded: boolean) {
      setAttr(pill, "data-expanded", String(expanded));
    },
    isExpanded: () => pill.dataset.expanded === "true",
    setEdge(edge: Edge) {
      setAttr(pill, "data-edge", edge);
    },
    showRowMessage,
    sessionCount: () => entries.size,
  };
}
