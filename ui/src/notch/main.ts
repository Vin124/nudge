// Tauri glue for the notch window (D20). The window is a fixed envelope that
// only moves (never resizes); the shape morphs in CSS. Hover, drag and the
// snap slide are driven by src-tauri/src/notch.rs, not per-frame IPC.

import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { availableMonitors, primaryMonitor, PhysicalPosition, type Monitor as TauriMonitor } from "@tauri-apps/api/window";
import {
  api,
  EVENT_NOTCH_DROP,
  EVENT_NOTCH_HOVER,
  EVENT_NOTCH_OPEN_SETTINGS,
  notchApi,
  onSnapshot,
  type Edge,
  type Snapshot,
} from "../shared/contracts";
import { mountSettings, type SettingsController } from "../settings/controller";
import { placeWindow, resolveMonitor, shapeRect, snapToEdge, type Monitor, type Page, type Point } from "./geometry";
import { createView, type Motion } from "./view";
import "./notch.css";
import "../settings/settings.css";

const HOVER_OPEN_MS = 220;
const HOVER_CLOSE_MS = 320;
const DRAG_THRESHOLD_PX = 4;
/** Must match the `snap` duration in notch.css. */
const SNAP_MS = 280;

const win = getCurrentWebviewWindow();
const root = document.getElementById("app")!;
const view = createView(root, {
  onFocus: (id) => api.focusSession(id),
  onSettings: () => openSettings(),
  onBack: () => closeSettings(),
});
view.shape.classList.add("intro");

let monitors: Monitor[] = [];
let primary: Monitor | null = null;
let snapshot: Snapshot | null = null;
let edge: Edge = "top";
let offset = 0.5;
let monitorName: string | null = null;
let along = 0;
let origin: Point | null = null;
let expanded = false;
let dragging = false;
let placed = false;
let persisting = 0;
/** D24: the expanded notch shows the session list ("home") or Settings. */
let page: Page = "home";
let settings: SettingsController | null = null;
/** Last cursor state from Rust; Back needs it to collapse a notch the cursor already left. */
let lastInside = false;

let openTimer: ReturnType<typeof setTimeout> | null = null;
let closeTimer: ReturnType<typeof setTimeout> | null = null;
let ticker: ReturnType<typeof setInterval> | null = null;

const report = (what: string) => (e: unknown) => console.error(`notch: ${what} failed`, e);

function toMonitor(m: TauriMonitor): Monitor {
  const wa = m.workArea ?? { position: m.position, size: m.size };
  return { name: m.name, x: wa.position.x, y: wa.position.y, width: wa.size.width, height: wa.size.height, scale: m.scaleFactor };
}

async function refreshMonitors(): Promise<void> {
  const [all, prim] = await Promise.all([availableMonitors(), primaryMonitor()]);
  monitors = all.map(toMonitor);
  primary = prim ? toMonitor(prim) : null;
}

/** Push the current shape rect to the DOM (animated by `motion`) and to the hit test. */
function applyShape(motion: Motion): void {
  const r = shapeRect(edge, along, expanded, view.sessionCount(), page);
  view.setMotion(motion);
  view.setEdge(edge);
  view.setRect(r);
  if (!dragging) notchApi.setHit(r).catch(report("set hit rect"));
}

/** Place the envelope for edge/offset/monitor; slide there when `slideMs` > 0. */
function layout(motion: Motion, slideMs = 0): void {
  if (monitors.length === 0) return;
  const p = placeWindow(edge, offset, resolveMonitor(monitors, monitorName, primary));
  along = p.along;
  applyShape(motion);
  if (origin && origin.x === p.origin.x && origin.y === p.origin.y) return;
  origin = p.origin;
  const move = slideMs > 0
    ? notchApi.animateTo(p.origin.x, p.origin.y, slideMs)
    : win.setPosition(new PhysicalPosition(p.origin.x, p.origin.y));
  move.catch(report("move window"));
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
  if (!next && page === "settings") leaveSettings();
  view.setExpanded(next);
  if (next) {
    view.tick();
    ticker = setInterval(() => view.tick(), 1000);
  } else if (ticker) {
    clearInterval(ticker);
    ticker = null;
  }
  applyShape(next ? "open" : "close");
}

function onHover(inside: boolean): void {
  lastInside = inside;
  if (dragging) return;
  // D24: Settings stays open while the cursor is out (dropdowns and the color
  // picker open outside the notch). Back, the gear, or Esc close it.
  if (page === "settings" && !inside) return;
  if (inside) {
    if (closeTimer) clearTimeout(closeTimer);
    closeTimer = null;
    if (!expanded && !openTimer) {
      openTimer = setTimeout(() => {
        openTimer = null;
        setExpanded(true);
      }, HOVER_OPEN_MS);
    }
  } else {
    if (openTimer) clearTimeout(openTimer);
    openTimer = null;
    if (expanded && !closeTimer) {
      closeTimer = setTimeout(() => {
        closeTimer = null;
        setExpanded(false);
      }, HOVER_CLOSE_MS);
    }
  }
}

// ---- D24: Settings page ----
function leaveSettings(): void {
  page = "home";
  view.setPage("home");
  void settings?.flush();
}

function openSettings(): void {
  clearHoverTimers();
  if (!settings) settings = mountSettings(view.settingsHost);
  if (snapshot) settings.show(snapshot.config);
  settings.refreshPacks();
  page = "settings";
  view.setPage("settings");
  if (!expanded) setExpanded(true);
  else applyShape("open");
}

function closeSettings(): void {
  if (page !== "settings") return;
  leaveSettings();
  applyShape("open");
  if (!lastInside) onHover(false);
}

document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  if (page === "settings") closeSettings();
  else if (expanded) setExpanded(false);
});

// ---- drag ----
let pressed: { id: number; sx: number; sy: number } | null = null;
// A click that ends a drag must not expand or hit a row.
let suppressClick = false;

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
view.shape.addEventListener("click", () => {
  if (!expanded) {
    clearHoverTimers();
    setExpanded(true);
  }
});

view.shape.addEventListener("pointerdown", (e) => {
  if (e.button !== 0) return;
  // Controls (sliders, selects, inputs) and the Settings page never start a drag.
  if ((e.target as Element).closest(".pane, input, select, button, textarea")) return;
  pressed = { id: e.pointerId, sx: e.screenX, sy: e.screenY };
});

view.shape.addEventListener("pointermove", (e) => {
  if (!pressed || e.pointerId !== pressed.id || dragging) return;
  if (Math.hypot(e.screenX - pressed.sx, e.screenY - pressed.sy) < DRAG_THRESHOLD_PX) return;
  dragging = true;
  suppressClick = true;
  clearHoverTimers();
  view.shape.setPointerCapture(e.pointerId);
  if (expanded) setExpanded(false);
  view.setFloating(true);
  notchApi.dragStart().catch((err) => {
    report("start drag")(err);
    void onDrop(null);
  });
});

function release(): void {
  pressed = null;
  if (dragging) notchApi.dragEnd().catch(report("end drag"));
}
view.shape.addEventListener("pointerup", release);
view.shape.addEventListener("pointercancel", release);

/** Rust reports where the window ended up; snap the notch to the nearest edge. */
async function onDrop(pos: Point | null): Promise<void> {
  pressed = null;
  if (!dragging) return;
  setTimeout(() => {
    suppressClick = false;
  }, 0);
  try {
    if (pos) {
      const scale = await win.scaleFactor();
      const r = shapeRect(edge, along, false, view.sessionCount());
      const center = { x: pos.x + (r.x + r.width / 2) * scale, y: pos.y + (r.y + r.height / 2) * scale };
      await refreshMonitors();
      const snap = snapToEdge(center, monitors);
      edge = snap.edge;
      offset = snap.offset;
      monitorName = snap.monitor.name;
      origin = pos;
    }
  } finally {
    dragging = false;
    view.setFloating(false);
    layout("snap", SNAP_MS);
  }
  await persist();
}

async function persist(): Promise<void> {
  if (!snapshot) return;
  const cfg = snapshot.config;
  persisting++;
  try {
    await api.setConfig({ ...cfg, notch: { ...cfg.notch, edge, offset, monitor: monitorName } });
  } catch (err) {
    report("persist position")(err);
  } finally {
    persisting--;
  }
}

// ---- snapshots ----
function onSnap(s: Snapshot): void {
  const before = view.sessionCount();
  snapshot = s;
  view.render(s);
  settings?.show(s.config);
  const n = s.config.notch;
  const moved = n.edge !== edge || n.offset !== offset || n.monitor !== monitorName;
  if ((moved || !placed) && !dragging && persisting === 0) {
    edge = n.edge;
    offset = n.offset;
    monitorName = n.monitor;
    layout(placed ? "snap" : "none", placed ? SNAP_MS : 0);
    if (!placed) {
      placed = true;
      requestAnimationFrame(() => view.shape.classList.remove("intro"));
    }
  } else if (view.sessionCount() !== before && !dragging) {
    applyShape(expanded ? "open" : "close");
  }
}

async function main(): Promise<void> {
  await refreshMonitors();
  await win.listen<boolean>(EVENT_NOTCH_HOVER, (e) => onHover(e.payload));
  await win.listen<Point>(EVENT_NOTCH_DROP, (e) => void onDrop(e.payload));
  await win.listen(EVENT_NOTCH_OPEN_SETTINGS, () => openSettings());
  await onSnapshot(onSnap);
}

main().catch(report("init"));
