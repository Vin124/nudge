// Pure placement math for the notch. No Tauri imports so it tests without mocks.
//
// D20: the notch window is a fixed square ENVELOPE (logical px) that never
// resizes. The visible shape is drawn and animated inside it in CSS, and the
// rest of the window is click-through (src-tauri/src/notch.rs). Screen values
// are PHYSICAL px; shape rects are LOGICAL px relative to the window.

import type { Edge } from "../shared/contracts";

export interface Monitor {
  name: string | null;
  /** usable area, physical px */
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
}
export interface Point { x: number; y: number }
export interface Size { width: number; height: number }
export interface Rect extends Point, Size {}

export const ENVELOPE = 400;
/** Collapsed thickness, away from the edge. */
export const THICK = 46;
export const PAD = 12;
export const RING = 32;
export const GAP = 8;
/** Space between the usage ring and the first session avatar. */
export const SEP = 14;
export const MAX_AVATARS = 6;
export const MIN_LEN = 200;
/** Collapsed length with every slot filled; clamps placement so it never depends on session count. */
export const MAX_LEN = PAD + RING + SEP + (MAX_AVATARS + 1) * RING + MAX_AVATARS * GAP + PAD;
export const PANEL_W = 380;
export const PANEL_PAD = 10;
export const HEADER_H = 84;
export const ROW_H = 52;
export const MAX_ROWS = 5;

export const isVertical = (edge: Edge): boolean => edge === "left" || edge === "right";

const clamp = (v: number, lo: number, hi: number): number => Math.min(Math.max(v, lo), Math.max(lo, hi));

/** Collapsed length along the edge for `n` sessions (avatars past MAX_AVATARS fold into a "+k" chip). */
export function collapsedLength(n: number): number {
  const shown = Math.min(n, MAX_AVATARS);
  const chip = n > MAX_AVATARS ? GAP + RING : 0;
  const avatars = shown > 0 ? SEP + shown * RING + (shown - 1) * GAP + chip : 0;
  return Math.max(MIN_LEN, PAD + RING + avatars + PAD);
}

/** Expanded panel height for `n` sessions (empty state uses one row). */
export function panelHeight(n: number): number {
  return PANEL_PAD + HEADER_H + clamp(n, 1, MAX_ROWS) * ROW_H + PANEL_PAD;
}

function contains(m: Monitor, p: Point): boolean {
  return p.x >= m.x && p.x < m.x + m.width && p.y >= m.y && p.y < m.y + m.height;
}

function distanceTo(m: Monitor, p: Point): number {
  const dx = Math.max(m.x - p.x, 0, p.x - (m.x + m.width));
  const dy = Math.max(m.y - p.y, 0, p.y - (m.y + m.height));
  return Math.hypot(dx, dy);
}

/** Monitor containing `p`, else the nearest one. */
export function monitorAt(monitors: Monitor[], p: Point): Monitor {
  const hit = monitors.find((m) => contains(m, p));
  if (hit) return hit;
  return monitors.reduce((best, m) => (distanceTo(m, p) < distanceTo(best, p) ? m : best));
}

/** Named monitor, else the primary, else the first. (AC #7) */
export function resolveMonitor(monitors: Monitor[], name: string | null, primary: Monitor | null): Monitor {
  const byName = name == null ? undefined : monitors.find((m) => m.name === name);
  return byName ?? primary ?? monitors[0];
}

/** Nearest edge of `m` to point `p`. Ties resolve top, bottom, left, right. */
export function nearestEdge(m: Monitor, p: Point): Edge {
  const d: [Edge, number][] = [
    ["top", Math.abs(p.y - m.y)],
    ["bottom", Math.abs(m.y + m.height - p.y)],
    ["left", Math.abs(p.x - m.x)],
    ["right", Math.abs(m.x + m.width - p.x)],
  ];
  return d.reduce((a, b) => (b[1] < a[1] ? b : a))[0];
}

export interface Placement {
  /** window top-left, physical px */
  origin: Point;
  /** notch center along the edge, logical px from the window's along-edge start */
  along: number;
}

/**
 * Window placement for a notch centered at `offset` (0..1) along `edge`.
 * The center is clamped so the longest collapsed notch fits; the envelope is
 * clamped on-screen. Neither depends on session count, so the window never
 * moves when sessions come and go.
 */
export function placeWindow(edge: Edge, offset: number, m: Monitor): Placement {
  const env = ENVELOPE * m.scale;
  const half = (MAX_LEN / 2) * m.scale;
  const [start, len] = isVertical(edge) ? [m.y, m.height] : [m.x, m.width];
  const center = clamp(start + clamp(offset, 0, 1) * len, start + half, start + len - half);
  const winStart = Math.round(clamp(center - env / 2, start, start + len - env));
  const along = (center - winStart) / m.scale;
  const crossNear = isVertical(edge) ? m.x : m.y;
  const crossFar = isVertical(edge) ? m.x + m.width - env : m.y + m.height - env;
  const cross = Math.round(edge === "top" || edge === "left" ? crossNear : crossFar);
  return { origin: isVertical(edge) ? { x: cross, y: winStart } : { x: winStart, y: cross }, along };
}

/** The visible shape inside the envelope, logical px. */
export function shapeRect(edge: Edge, along: number, expanded: boolean, sessions: number): Rect {
  const vertical = isVertical(edge);
  const [w, h] = expanded
    ? [PANEL_W, panelHeight(sessions)]
    : vertical ? [THICK, collapsedLength(sessions)] : [collapsedLength(sessions), THICK];
  const alongSize = vertical ? h : w;
  const a = clamp(along - alongSize / 2, 0, ENVELOPE - alongSize);
  switch (edge) {
    case "top": return { x: a, y: 0, width: w, height: h };
    case "bottom": return { x: a, y: ENVELOPE - h, width: w, height: h };
    case "left": return { x: 0, y: a, width: w, height: h };
    case "right": return { x: ENVELOPE - w, y: a, width: w, height: h };
  }
}

export interface Snap {
  monitor: Monitor;
  edge: Edge;
  /** notch center as a 0..1 fraction along the edge */
  offset: number;
}

/** Snap a dropped notch (its center, physical px) to the nearest edge of the monitor under it. */
export function snapToEdge(center: Point, monitors: Monitor[]): Snap {
  const monitor = monitorAt(monitors, center);
  const edge = nearestEdge(monitor, center);
  const offset = isVertical(edge)
    ? (center.y - monitor.y) / monitor.height
    : (center.x - monitor.x) / monitor.width;
  return { monitor, edge, offset: clamp(offset, 0, 1) };
}
