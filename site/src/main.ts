// Landing page: a looping, hoverable demo of the notch plus small feature art.
// The fox and critter sprites come from the app's own code (ui/src), so the
// page shows exactly what the app draws.

import { pixelFox } from "../../ui/src/avatar/fox";
import { critter, sparkle } from "../../ui/src/notch/icons";
import "./style.css";

type State = "running" | "done" | "blocked" | "idle";
interface Sess { id: string; name: string; tint: string; state: State; ctx: number; since: number }

const SVG = "http://www.w3.org/2000/svg";
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
const $ = <T extends Element>(sel: string) => document.querySelector(sel) as T;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls = "", text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}
function svg(tag: string, attrs: Record<string, string | number>): SVGElement {
  const e = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e as SVGElement;
}

/** A ring gauge (track + arc from 12 o'clock) in a size×size box. */
function ring(size: number, r: number, stroke: number): { root: SVGElement; set(p: number): void; arc: SVGElement } {
  const c = size / 2, circ = 2 * Math.PI * r;
  const root = svg("svg", { viewBox: `0 0 ${size} ${size}`, class: "ring" });
  const base = { cx: c, cy: c, r, fill: "none", "stroke-width": stroke };
  root.append(svg("circle", { ...base, class: "track" }));
  const arc = svg("circle", { ...base, class: "arc", "stroke-linecap": "round", "stroke-dasharray": `${circ} ${circ}`, transform: `rotate(-90 ${c} ${c})` });
  root.append(arc);
  return { root, arc, set: (p) => arc.setAttribute("stroke-dashoffset", String(circ * (1 - Math.max(0, Math.min(100, p)) / 100))) };
}

function avatar(s: Sess, size = 32): { root: HTMLElement; update(s: Sess): void } {
  const root = el("span", "av");
  root.style.setProperty("--tint", s.tint);
  root.style.width = root.style.height = `${size}px`;
  const g = ring(32, 14.5, 2.5);
  root.append(g.root, critter("face"));
  const update = (x: Sess) => {
    root.dataset.state = x.state;
    g.set(x.ctx);
  };
  update(s);
  return { root, update };
}

// ---------------- demo ----------------

const START: Sess[] = [
  { id: "a", name: "api-refactor", tint: "#D97757", state: "running", ctx: 34, since: 0 },
  { id: "b", name: "web-app", tint: "#7FA7D9", state: "running", ctx: 58, since: 0 },
  { id: "c", name: "docs-site", tint: "#8DC07F", state: "idle", ctx: 12, since: 0 },
];
let sessions: Sess[] = START.map((s) => ({ ...s }));

const notch = $<HTMLElement>("#dnotch");
const foxSlot = $<HTMLElement>("#fox-slot");
const caption = $<HTMLElement>("#caption");
const term = $<HTMLElement>("#term");

// collapsed bar
const bar = el("div", "dn-bar");
const gauge = el("div", "dn-gauge");
const g5 = ring(32, 14, 3), g7 = ring(32, 9.5, 2.5);
g7.root.classList.add("inner");
gauge.append(g5.root, g7.root, sparkle("spark"));
const dots = el("div", "dn-dots");
bar.append(gauge, dots);
// expanded panel
const panel = el("div", "dn-panel");
const usage = el("div", "dn-usage");
function usageItem(label: string, pct: number, reset: string): HTMLElement {
  const it = el("div", "dn-uitem");
  const dial = el("div", "dn-dial");
  const r = ring(44, 19, 4);
  r.set(pct);
  dial.append(r.root, sparkle("spark"));
  const t = el("div", "dn-utext");
  t.append(el("div", "dn-ulabel", label), el("div", "dn-upct", `${pct}%`), el("div", "dn-ureset", reset));
  it.append(dial, t);
  return it;
}
usage.append(usageItem("5-hour", 42, "resets in 1h 12m"), usageItem("Weekly", 18, "resets in 4d 6h"));
const list = el("div", "dn-list");
panel.append(usage, list);
const ears = [el("span", "dn-ear l"), el("span", "dn-ear r")];
const body = el("div", "dn-body");
body.append(bar, panel);
notch.append(...ears, body);
g5.set(42);
g7.set(18);

const LABEL: Record<State, string> = { running: "Running", done: "Done", blocked: "Needs you", idle: "Idle" };
const views = new Map<string, { dot: ReturnType<typeof avatar>; row: HTMLElement; ravatar: ReturnType<typeof avatar>; sub: HTMLElement }>();
for (const s of sessions) {
  const dot = avatar(s);
  dots.append(dot.root);
  const row = el("div", "dn-row");
  const ravatar = avatar(s, 36);
  const txt = el("div", "dn-rtext");
  const sub = el("div", "dn-rsub");
  txt.append(el("div", "dn-rname", s.name), sub);
  row.append(ravatar.root, txt);
  list.append(row);
  views.set(s.id, { dot, row, ravatar, sub });
}

function paint(): void {
  for (const s of sessions) {
    const v = views.get(s.id)!;
    v.dot.update(s);
    v.ravatar.update(s);
    v.sub.dataset.state = s.state;
    v.sub.textContent = `${LABEL[s.state]} · ${Math.round(s.ctx)}% context`;
  }
}

const TERM: Record<State, string[]> = {
  running: ["> refactor the auth middleware to use sessions", "  Read 6 files · Editing src/auth/session.ts…"],
  done: ["> refactor the auth middleware to use sessions", "  Read 6 files · Edited 3 files", "✓ Done in 2m 14s — 4 files changed"],
  blocked: ["> refactor the auth middleware to use sessions", "  Read 6 files", "● Allow edit to src/auth/session.ts?  1 Yes  2 No"],
  idle: ["> "],
};
function paintTerm(state: State): void {
  term.replaceChildren(
    el("div", "t-dim", "~/api-refactor $ claude"),
    ...TERM[state].map((line) => el("div", line.startsWith("✓") ? "t-ok" : line.startsWith("●") ? "t-ask" : "", line)),
  );
}

let expanded = false;
function setExpanded(on: boolean): void {
  expanded = on;
  notch.dataset.expanded = String(on);
}

function setCaption(text: string): void {
  caption.classList.remove("in");
  void caption.offsetWidth;
  caption.textContent = text;
  caption.classList.add("in");
}

let foxTimer: number | undefined;
function showFox(kind: "done" | "blocked", project: string): void {
  window.clearTimeout(foxTimer);
  foxSlot.replaceChildren();
  const wrap = el("div", "fox-wrap");
  const hold = el("div", "fox-hold");
  hold.append(pixelFox(kind, "peek", 4));
  const pill = el("div", `fox-pill ${kind}`, kind === "done" ? `${project} is done` : `${project} needs you`);
  wrap.append(hold, pill);
  foxSlot.append(wrap);
  foxSlot.dataset.on = "true";
  foxTimer = window.setTimeout(hideFox, 3200);
}
function hideFox(): void {
  foxSlot.dataset.on = "false";
}

function set(id: string, state: State): void {
  const s = sessions.find((x) => x.id === id)!;
  s.state = state;
  paint();
  if (id === "a") paintTerm(state);
}

// Context creeps up while sessions run.
let last = performance.now();
function tick(now: number): void {
  const dt = (now - last) / 1000;
  last = now;
  for (const s of sessions) if (s.state === "running") s.ctx = Math.min(92, s.ctx + dt * 1.6);
  paint();
  requestAnimationFrame(tick);
}

// ---- storyboard ----
type Step = [ms: number, run: () => void];
const STORY: Step[] = [
  [0, () => { sessions = START.map((s) => ({ ...s })); for (const s of sessions) views.get(s.id)!.dot.update(s); set("a", "running"); setExpanded(false); setCaption("Three sessions, one glance."); }],
  [3200, () => { set("a", "done"); showFox("done", "api-refactor"); setCaption("Done? The fox lets you know."); }],
  [7000, () => { set("b", "blocked"); showFox("blocked", "web-app"); setCaption("Needs your OK? You'll hear about it."); }],
  [10800, () => { hideFox(); setExpanded(true); setCaption("Hover to see everything. Click to jump back."); }],
  [14600, () => { setExpanded(false); set("a", "running"); set("b", "running"); setCaption("Then get back to your day."); }],
];
const LOOP_MS = 17000;
let timers: number[] = [];
let paused = false;

function play(): void {
  timers.forEach(clearTimeout);
  timers = STORY.map(([ms, run]) => window.setTimeout(() => !paused && run(), ms));
  timers.push(window.setTimeout(play, LOOP_MS));
}

function interact(): void {
  // A visitor took over: stop the story until they've been idle a while.
  paused = true;
  timers.forEach(clearTimeout);
  timers = [window.setTimeout(() => { paused = false; play(); }, 9000)];
}

notch.addEventListener("mouseenter", () => { interact(); hideFox(); setExpanded(true); setCaption("Click a session to jump right back to it."); });
notch.addEventListener("mouseleave", () => setExpanded(false));
for (const b of document.querySelectorAll<HTMLButtonElement>(".chip-btn")) {
  b.addEventListener("click", () => {
    interact();
    const act = b.dataset.act;
    if (act === "done") { setExpanded(false); set("a", "done"); showFox("done", "api-refactor"); setCaption("Done? The fox lets you know."); }
    if (act === "ask") { setExpanded(false); set("b", "blocked"); showFox("blocked", "web-app"); setCaption("Needs your OK? You'll hear about it."); }
    if (act === "peek") { hideFox(); setExpanded(!expanded); setCaption(expanded ? "Every session, its state, and how full its context is." : "Three sessions, one glance."); }
  });
}

// Scale the fixed 960×600 stage to the available width.
const outer = $<HTMLElement>("#stage-outer");
const stage = $<HTMLElement>("#stage");
new ResizeObserver(() => {
  const k = Math.min(1, outer.clientWidth / 960);
  stage.style.transform = `scale(${k})`;
  outer.style.height = `${600 * k}px`;
}).observe(outer);

paint();
paintTerm("running");
if (reduced) {
  set("a", "done");
  setCaption("Done? The fox lets you know.");
} else {
  requestAnimationFrame(tick);
  play();
}

// ---------------- feature art ----------------

{
  const art = $<HTMLElement>("#art-rings");
  const row = el("div", "art-row");
  const demo: [State, number, string][] = [["running", 30, "#D97757"], ["done", 64, "#8DC07F"], ["blocked", 82, "#7FA7D9"]];
  for (const [state, ctx, tint] of demo) row.append(avatar({ id: state, name: "", tint, state, ctx, since: 0 }, 56).root);
  art.append(row);
}
{
  const art = $<HTMLElement>("#art-usage");
  const g = el("div", "art-gauge");
  const a = ring(32, 14, 3), b = ring(32, 9.5, 2.5);
  b.root.classList.add("inner");
  a.set(42);
  b.set(18);
  g.append(a.root, b.root, sparkle("spark"));
  const label = el("div", "art-usage-text");
  label.append(el("strong", "", "42%"), el("span", "", "of your 5-hour limit"));
  art.append(g, label);
}
$<HTMLElement>("#art-fox").append(pixelFox("done", "full", 4));
{
  const art = $<HTMLElement>("#art-levels");
  const seg = el("div", "art-seg");
  const names = ["Quiet", "Normal", "Loud"];
  let i = 1;
  const btns = names.map((n) => el("span", "", n));
  seg.append(...btns);
  const paintSeg = () => btns.forEach((b, j) => b.classList.toggle("on", j === i));
  paintSeg();
  if (!reduced) setInterval(() => { i = (i + 1) % 3; paintSeg(); }, 2200);
  art.append(seg);
}

// ---------------- copy buttons ----------------
for (const b of document.querySelectorAll<HTMLButtonElement>(".copy")) {
  b.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(b.dataset.copy ?? "");
      b.textContent = "Copied";
    } catch {
      b.textContent = "Press Ctrl+C";
    }
    setTimeout(() => (b.textContent = "Copy"), 1600);
  });
}
