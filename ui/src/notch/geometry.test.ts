import { describe, expect, it } from "vitest";
import {
  collapsedLength,
  ENVELOPE,
  MAX_LEN,
  MIN_LEN,
  PANEL_W,
  panelHeight,
  placeWindow,
  resolveMonitor,
  shapeRect,
  snapToEdge,
  THICK,
  type Monitor,
} from "./geometry";

const mon = (over: Partial<Monitor> = {}): Monitor => ({
  name: "A", x: 0, y: 0, width: 1920, height: 1080, scale: 1, ...over,
});
const EDGES = ["top", "bottom", "left", "right"] as const;

describe("collapsedLength", () => {
  it("never shrinks below MIN_LEN and grows with sessions", () => {
    expect(collapsedLength(0)).toBe(MIN_LEN);
    expect(collapsedLength(2)).toBe(MIN_LEN);
    expect(collapsedLength(6)).toBeGreaterThan(collapsedLength(5));
  });
  it("folds sessions past 6 into one chip, capped at MAX_LEN", () => {
    expect(collapsedLength(7)).toBe(MAX_LEN);
    expect(collapsedLength(40)).toBe(MAX_LEN);
    expect(MAX_LEN).toBeLessThanOrEqual(ENVELOPE);
  });
});

describe("panelHeight", () => {
  it("uses one row for the empty state and caps at 5 rows", () => {
    expect(panelHeight(0)).toBe(panelHeight(1));
    expect(panelHeight(3)).toBeGreaterThan(panelHeight(2));
    expect(panelHeight(5)).toBe(panelHeight(50));
    expect(panelHeight(50)).toBeLessThanOrEqual(ENVELOPE);
  });
});

describe("placeWindow", () => {
  it("puts the envelope flush against each edge", () => {
    const m = mon();
    expect(placeWindow("top", 0.5, m).origin).toEqual({ x: 960 - 200, y: 0 });
    expect(placeWindow("bottom", 0.5, m).origin).toEqual({ x: 760, y: 1080 - ENVELOPE });
    expect(placeWindow("left", 0.5, m).origin).toEqual({ x: 0, y: 540 - 200 });
    expect(placeWindow("right", 0.5, m).origin).toEqual({ x: 1920 - ENVELOPE, y: 340 });
  });
  it("centers the notch in the envelope away from corners", () => {
    for (const edge of EDGES) expect(placeWindow(edge, 0.5, mon()).along).toBe(200);
  });
  it("clamps near corners: envelope on-screen, notch fully visible", () => {
    const m = mon();
    const p = placeWindow("top", 0, m);
    expect(p.origin.x).toBe(0);
    expect(p.along).toBe(MAX_LEN / 2);
    const q = placeWindow("left", 1, m);
    expect(q.origin.y).toBe(1080 - ENVELOPE);
    expect(q.along).toBe(ENVELOPE - MAX_LEN / 2);
  });
  it("works in physical px at scale 1.5 with an offset monitor", () => {
    const m = mon({ x: 1920, y: -200, width: 2880, height: 1620, scale: 1.5 });
    const p = placeWindow("bottom", 0.5, m);
    expect(p.origin).toEqual({ x: 1920 + 1440 - 300, y: -200 + 1620 - 600 });
    expect(p.along).toBe(200);
  });
});

describe("shapeRect", () => {
  it("collapsed hugs the edge, centered on `along`", () => {
    const len = collapsedLength(3);
    expect(shapeRect("top", 200, false, 3)).toEqual({ x: 200 - len / 2, y: 0, width: len, height: THICK });
    expect(shapeRect("bottom", 200, false, 3)).toEqual({ x: 200 - len / 2, y: ENVELOPE - THICK, width: len, height: THICK });
    expect(shapeRect("left", 200, false, 3)).toEqual({ x: 0, y: 200 - len / 2, width: THICK, height: len });
    expect(shapeRect("right", 200, false, 3)).toEqual({ x: ENVELOPE - THICK, y: 200 - len / 2, width: THICK, height: len });
  });
  it("expanded grows inward from the same edge and stays inside the envelope", () => {
    for (const edge of EDGES) {
      for (const along of [0, MAX_LEN / 2, 200, ENVELOPE]) {
        const r = shapeRect(edge, along, true, 9);
        expect(r.width).toBe(PANEL_W);
        expect(r.height).toBe(panelHeight(9));
        expect(r.x).toBeGreaterThanOrEqual(0);
        expect(r.y).toBeGreaterThanOrEqual(0);
        expect(r.x + r.width).toBeLessThanOrEqual(ENVELOPE);
        expect(r.y + r.height).toBeLessThanOrEqual(ENVELOPE);
      }
    }
    expect(shapeRect("top", 200, true, 1).y).toBe(0);
    expect(shapeRect("right", 200, true, 1).x).toBe(ENVELOPE - PANEL_W);
  });
});

describe("snapToEdge", () => {
  it("snaps to each of the 4 edges", () => {
    expect(snapToEdge({ x: 960, y: 30 }, [mon()]).edge).toBe("top");
    expect(snapToEdge({ x: 960, y: 1050 }, [mon()]).edge).toBe("bottom");
    expect(snapToEdge({ x: 20, y: 500 }, [mon()]).edge).toBe("left");
    expect(snapToEdge({ x: 1900, y: 500 }, [mon()]).edge).toBe("right");
  });
  it("corner case picks the nearest edge", () => {
    expect(snapToEdge({ x: 130, y: 70 }, [mon()]).edge).toBe("top");
    expect(snapToEdge({ x: 80, y: 200 }, [mon()]).edge).toBe("left");
  });
  it("offset is the center fraction along the edge, clamped", () => {
    expect(snapToEdge({ x: 480, y: 0 }, [mon()]).offset).toBeCloseTo(0.25, 5);
    expect(snapToEdge({ x: -500, y: 0 }, [mon()]).offset).toBe(0);
    expect(snapToEdge({ x: 5000, y: 0 }, [mon()]).offset).toBe(1);
  });
  it("multi-monitor: a center on the second monitor snaps there", () => {
    const b = mon({ name: "B", x: 1920, y: 0, width: 2560, height: 1440 });
    const r = snapToEdge({ x: 2400, y: 20 }, [mon(), b]);
    expect(r.monitor.name).toBe("B");
    expect(r.edge).toBe("top");
    expect(snapToEdge({ x: 1930, y: 600 }, [mon(), b]).edge).toBe("left");
  });
  it("round-trips with placeWindow at scale 1 and 1.5", () => {
    for (const scale of [1, 1.5]) {
      const m = mon({ width: 1920 * scale, height: 1080 * scale, scale });
      for (const edge of EDGES) {
        const p = placeWindow(edge, 0.3, m);
        const r = shapeRect(edge, p.along, false, 2);
        const center = { x: p.origin.x + (r.x + r.width / 2) * scale, y: p.origin.y + (r.y + r.height / 2) * scale };
        const s = snapToEdge(center, [m]);
        expect(s.edge).toBe(edge);
        expect(s.offset).toBeCloseTo(0.3, 2);
      }
    }
  });
});

describe("resolveMonitor", () => {
  const a = mon({ name: "A" });
  const b = mon({ name: "B", x: 1920 });
  it("finds a monitor by name", () => {
    expect(resolveMonitor([a, b], "B", a)).toBe(b);
  });
  it("falls back to primary when the name no longer exists", () => {
    expect(resolveMonitor([a, b], "GONE", b)).toBe(b);
    expect(resolveMonitor([a, b], null, b)).toBe(b);
  });
  it("falls back to the first monitor when there is no primary", () => {
    expect(resolveMonitor([a, b], "GONE", null)).toBe(a);
  });
});
