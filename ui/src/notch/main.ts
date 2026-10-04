// Tauri glue for the notch window: window placement, hover/click expand,
// pointer-driven drag with edge snapping. Pure logic lives in geometry/view.

import {
  availableMonitors,
  getCurrentWindow,
  primaryMonitor,
  PhysicalPosition,
  PhysicalSize,
  type Monitor as TauriMonitor,
} from "@tauri-apps/api/window";
import { api, onSnapshot, type Edge, type Snapshot } from "../shared/contracts";
import {
  collapsedSize,
  expandedSize,
  placeCollapsed,
  placeExpanded,
  resolveMonitor,
  snapToEdge,
  type Monitor,
  type Point,
  type Size,
} from "./geometry";
import { createView } from "./view";
import "./notch.css";

const HOVER_OPEN_MS = 300;
const HOVER_CLOSE_MS = 400;
const DRAG_THRESHOLD_PX = 4;
const SNAP_ANIM_MS = 120;
const SNAP_STEPS = 8;

const win = getCurrentWindow();
const root = document.getElementById("app")!;
const view = createView(root, { onFocus: (id) => api.focusSession(id) });

let monitors: Monitor[] = [];
let primary: Monitor | null = null;
let snapshot: Snapshot | null = null;
let edge: Edge = "top";
let offset = 0.5;
let monitorName: string | null = null;
let expanded = false;
let lastRows = -1;
let placed = false;
let persisting = 0;
let winPos: Point = { x: 0, y: 0 };
let winSize: Size = { width: 0, height: 0 };
let currentMonitor: Monitor | null = null;

let openTimer: ReturnType<typeof setTimeout> | null = null;
let closeTimer: ReturnType<typeof setTimeout> | null = null;
let ticker: ReturnType<typeof setInterval> | null = null;

// Serialized window operations (setPosition/setSize are async IPC).
let queue: Promise<void> = Promise.resolve();
function enqueue(fn: () => Promise<void>): Promise<void> {
  queue = queue.then(fn).catch((e) => console.error("notch window op failed", e));
  return queue;
}

function toMonitor(m: TauriMonitor): Monitor {
  const wa = m.workArea ?? { position: m.position, size: m.size };
  return {
    name: m.name,
    x: wa.position.x,
    y: wa.position.y,
    width: wa.size.width,
    height: wa.size.height,
    scale: m.scaleFactor,
  };
}

async function refreshMonitors(): Promise<void> {
  const [all, prim] = await Promise.all([availableMonitors(), primaryMonitor()]);
  monitors = all.map(toMonitor);
  primary = prim ? toMonitor(prim) : null;
}

function applyGeometry(pos: Point, size: Size): Promise<void> {
  winPos = pos;
  winSize = size;
  return enqueue(async () => {
    await win.setPosition(new PhysicalPosition(pos.x, pos.y));
    await win.setSize(new PhysicalSize(size.width, size.height));
  });
}

function layout(): Promise<void> {
  if (monitors.length === 0) return Promise.resolve();
  const mon = resolveMonitor(monitors, monitorName, primary);
  currentMonitor = mon;
  const cSize = collapsedSize(edge, mon.scale);
  const cPos = placeCollapsed(edge, offset, mon);
  view.setEdge(edge);
  if (!expanded) return applyGeometry(cPos, cSize);
  lastRows = view.sessionCount();
  const eSize = expandedSize(lastRows, mon.scale);
  return applyGeometry(placeExpanded(edge, { ...cPos, ...cSize }, eSize, mon), eSize);
}

// ---- expand / collapse ----
function clearHoverTimers(): void {
  if (openTimer) clearTimeout(openTimer);
  if (closeTimer) clearTimeout(closeTimer);
  openTimer = closeTimer = null;
}

function setExpanded(next: boolean): void {
  if (expanded === next) return;
  expanded = next;
  view.setExpanded(next);
  if (next) {
    view.tick();
    ticker = setInterval(() => view.tick(), 1000);
  } else if (ticker) {
    clearInterval(ticker);
    ticker = null;
  }
  void layout();
}

// ---- drag state ----
let dragging = false;
let pressed: { id: number; sx: number; sy: number } | null = null;
let dragStart: Point = { x: 0, y: 0 };
let dragScale = 1;
let dragTarget: Point = { x: 0, y: 0 };
let moveInFlight = false;
// A click that ends a drag must not expand or hit a row.
let suppressClick = false;

view.pill.addEventListener("pointerenter", () => {
  if (dragging) return;
  if (closeTimer) clearTimeout(closeTimer);
  closeTimer = null;
  if (!expanded && !openTimer) {
    openTimer = setTimeout(() => {
      openTimer = null;
      setExpanded(true);
    }, HOVER_OPEN_MS);
  }
});
view.pill.addEventListener("pointerleave", () => {
  if (dragging) return;
  if (openTimer) clearTimeout(openTimer);
  openTimer = null;
  if (expanded && !closeTimer) {
    closeTimer = setTimeout(() => {
      closeTimer = null;
      setExpanded(false);
    }, HOVER_CLOSE_MS);
  }
});

document.addEventListener(
  "click",
  (e) => {
    if (suppressClick) {
      e.stopPropagation();
      e.preventDefault();
    }
  },
  true,
);
view.pill.addEventListener("click", () => {
  if (!expanded) {
    clearHoverTimers();
    setExpanded(true);
  }
});

function pumpMove(): void {
  if (moveInFlight) return;
  moveInFlight = true;
  const t = dragTarget;
  winPos = t;
  void enqueue(() => win.setPosition(new PhysicalPosition(t.x, t.y))).then(() => {
    moveInFlight = false;
    if (dragging && (dragTarget.x !== t.x || dragTarget.y !== t.y)) pumpMove();
  });
}

view.pill.addEventListener("pointerdown", (e) => {
  if (e.button !== 0) return;
  pressed = { id: e.pointerId, sx: e.screenX, sy: e.screenY };
});

view.pill.addEventListener("pointermove", (e) => {
  if (!pressed || e.pointerId !== pressed.id) return;
  const dx = e.screenX - pressed.sx;
  const dy = e.screenY - pressed.sy;
  if (!dragging) {
    if (Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) return;
    dragging = true;
    suppressClick = true;
    void beginDrag(e);
    return;
  }
  if (dragReady) {
    dragTarget = {
      x: Math.round(dragStart.x + dx * dragScale),
      y: Math.round(dragStart.y + dy * dragScale),
    };
    pumpMove();
  }
});

let dragReady = false;

async function beginDrag(e: PointerEvent): Promise<void> {
  dragReady = false;
  clearHoverTimers();
  view.pill.setPointerCapture(e.pointerId);
  await refreshMonitors();
  if (expanded) {
    expanded = false;
    view.setExpanded(false);
    if (ticker) clearInterval(ticker);
    ticker = null;
    await layout();
  }
  dragScale = (currentMonitor ?? monitors[0]).scale;
  dragStart = winPos;
  dragTarget = winPos;
  // Re-anchor on the press point so the pill does not jump after the awaits.
  if (pressed) pressed = { id: pressed.id, sx: e.screenX, sy: e.screenY };
  dragReady = true;
}

async function endDrag(): Promise<void> {
  pressed = null;
  if (!dragging) return;
  dragging = false;
  dragReady = false;
  setTimeout(() => {
    suppressClick = false;
  }, 0);
  if (monitors.length === 0) return;

  const snap = snapToEdge(dragTarget, winSize, monitors);
  edge = snap.edge;
  offset = snap.offset;
  monitorName = snap.monitor.name;
  currentMonitor = snap.monitor;
  view.setEdge(edge);

  const from = dragTarget;
  const to = snap.position;
  winSize = snap.size;
  winPos = to;
  await enqueue(() => win.setSize(new PhysicalSize(snap.size.width, snap.size.height)));
  for (let i = 1; i <= SNAP_STEPS; i++) {
    const t = i / SNAP_STEPS;
    const ease = 1 - (1 - t) * (1 - t);
    const p = {
      x: Math.round(from.x + (to.x - from.x) * ease),
      y: Math.round(from.y + (to.y - from.y) * ease),
    };
    await enqueue(() => win.setPosition(new PhysicalPosition(p.x, p.y)));
    await new Promise((r) => setTimeout(r, SNAP_ANIM_MS / SNAP_STEPS));
  }
  await persist(snap.monitor.name);
}

async function persist(name: string | null): Promise<void> {
  if (!snapshot) return;
  const cfg = snapshot.config;
  persisting++;
  try {
    await api.setConfig({ ...cfg, notch: { ...cfg.notch, edge, offset, monitor: name } });
  } catch (err) {
    console.error("failed to persist notch position", err);
  } finally {
    persisting--;
  }
}

view.pill.addEventListener("pointerup", () => {
  void endDrag();
});
view.pill.addEventListener("pointercancel", () => {
  void endDrag();
});

// ---- snapshots ----
function onSnap(s: Snapshot): void {
  snapshot = s;
  view.render(s);
  const n = s.config.notch;
  const changed = n.edge !== edge || n.offset !== offset || n.monitor !== monitorName;
  if ((changed || !placed) && !dragging && persisting === 0) {
    placed = true;
    edge = n.edge;
    offset = n.offset;
    monitorName = n.monitor;
    void layout();
  } else if (expanded && view.sessionCount() !== lastRows) {
    void layout();
  }
}

async function main(): Promise<void> {
  await refreshMonitors();
  await onSnapshot(onSnap);
}

main().catch((e) => console.error("notch init failed", e));
