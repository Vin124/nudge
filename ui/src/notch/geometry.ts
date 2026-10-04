// Pure placement math for the notch window. All values are PHYSICAL pixels
// unless a name says "logical". No Tauri imports so it tests without mocks.

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

export const PILL_LONG = 220; // logical
export const PILL_SHORT = 44; // logical
export const EXPANDED_W = 320; // logical
export const HEADER_H = 44; // logical
export const ROW_H = 32; // logical
export const MAX_ROWS = 8;

export const isVertical = (edge: Edge): boolean => edge === "left" || edge === "right";

/** Collapsed pill size in physical px; width/height swap on side edges. */
export function collapsedSize(edge: Edge, scale: number): Size {
  const long = Math.round(PILL_LONG * scale);
  const short = Math.round(PILL_SHORT * scale);
  return isVertical(edge) ? { width: short, height: long } : { width: long, height: short };
}

/** Logical height of the expanded window for `rowCount` sessions (empty state uses one row). */
export function expandedHeightLogical(rowCount: number): number {
  return HEADER_H + Math.min(Math.max(rowCount, 1), MAX_ROWS) * ROW_H;
}

export function expandedSize(rowCount: number, scale: number): Size {
  return {
    width: Math.round(EXPANDED_W * scale),
    height: Math.round(expandedHeightLogical(rowCount) * scale),
  };
}

const clamp = (v: number, lo: number, hi: number): number => Math.min(Math.max(v, lo), Math.max(lo, hi));

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

/** Collapsed window top-left for an edge + offset (0..1 along the edge). */
export function placeCollapsed(edge: Edge, offset: number, m: Monitor): Point {
  const s = collapsedSize(edge, m.scale);
  const o = clamp(offset, 0, 1);
  switch (edge) {
    case "top": return { x: Math.round(m.x + o * (m.width - s.width)), y: m.y };
    case "bottom": return { x: Math.round(m.x + o * (m.width - s.width)), y: m.y + m.height - s.height };
    case "left": return { x: m.x, y: Math.round(m.y + o * (m.height - s.height)) };
    case "right": return { x: m.x + m.width - s.width, y: Math.round(m.y + o * (m.height - s.height)) };
  }
}

export interface Snap {
  monitor: Monitor;
  edge: Edge;
  /** 0..1 along the edge, clamped so the pill stays fully on-screen */
  offset: number;
  /** collapsed window size after snapping (orientation applied) */
  size: Size;
  /** collapsed window top-left after snapping */
  position: Point;
}

/**
 * Snap a dropped window (top-left `pos`, size `size`, physical px) to the
 * nearest edge of the monitor containing its center.
 */
export function snapToEdge(pos: Point, size: Size, monitors: Monitor[]): Snap {
  const center = { x: pos.x + size.width / 2, y: pos.y + size.height / 2 };
  const monitor = monitorAt(monitors, center);
  const edge = nearestEdge(monitor, center);
  const s = collapsedSize(edge, monitor.scale);
  // Offset from the pill center along the edge, so the pill keeps its drop point.
  let offset: number;
  if (isVertical(edge)) {
    const span = monitor.height - s.height;
    offset = span <= 0 ? 0 : (center.y - s.height / 2 - monitor.y) / span;
  } else {
    const span = monitor.width - s.width;
    offset = span <= 0 ? 0 : (center.x - s.width / 2 - monitor.x) / span;
  }
  offset = clamp(offset, 0, 1);
  return { monitor, edge, offset, size: s, position: placeCollapsed(edge, offset, monitor) };
}

/**
 * Top-left for the expanded window. It grows inward from the docked edge;
 * along the edge it stays centered on the collapsed pill, clamped on-screen.
 */
export function placeExpanded(edge: Edge, collapsed: Rect, exp: Size, m: Monitor): Point {
  switch (edge) {
    case "top":
    case "bottom": {
      const cx = collapsed.x + collapsed.width / 2;
      const x = clamp(Math.round(cx - exp.width / 2), m.x, m.x + m.width - exp.width);
      return { x, y: edge === "top" ? m.y : m.y + m.height - exp.height };
    }
    case "left":
    case "right": {
      const cy = collapsed.y + collapsed.height / 2;
      const y = clamp(Math.round(cy - exp.height / 2), m.y, m.y + m.height - exp.height);
      return { x: edge === "left" ? m.x : m.x + m.width - exp.width, y };
    }
  }
}
