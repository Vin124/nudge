import { describe, expect, it } from "vitest";
import {
  collapsedSize,
  expandedSize,
  placeCollapsed,
  placeExpanded,
  resolveMonitor,
  snapToEdge,
  type Monitor,
} from "./geometry";

const mon = (over: Partial<Monitor> = {}): Monitor => ({
  name: "A", x: 0, y: 0, width: 1920, height: 1080, scale: 1, ...over,
});

describe("collapsedSize", () => {
  it("swaps width/height on side edges", () => {
    expect(collapsedSize("top", 1)).toEqual({ width: 220, height: 44 });
    expect(collapsedSize("bottom", 1)).toEqual({ width: 220, height: 44 });
    expect(collapsedSize("left", 1)).toEqual({ width: 44, height: 220 });
    expect(collapsedSize("right", 1)).toEqual({ width: 44, height: 220 });
  });
  it("scales to physical pixels at 1.5", () => {
    expect(collapsedSize("top", 1.5)).toEqual({ width: 330, height: 66 });
    expect(collapsedSize("left", 1.5)).toEqual({ width: 66, height: 330 });
  });
});

describe("snapToEdge", () => {
  const size = { width: 220, height: 44 };
  it("snaps to each of the 4 edges", () => {
    expect(snapToEdge({ x: 850, y: 30 }, size, [mon()]).edge).toBe("top");
    expect(snapToEdge({ x: 850, y: 1000 }, size, [mon()]).edge).toBe("bottom");
    expect(snapToEdge({ x: 20, y: 500 }, size, [mon()]).edge).toBe("left");
    expect(snapToEdge({ x: 1700, y: 500 }, size, [mon()]).edge).toBe("right");
  });
  it("places flush against the edge with orientation applied", () => {
    const r = snapToEdge({ x: 1700, y: 500 }, size, [mon()]);
    expect(r.size).toEqual({ width: 44, height: 220 });
    expect(r.position.x).toBe(1920 - 44);
    const b = snapToEdge({ x: 850, y: 1000 }, size, [mon()]);
    expect(b.position.y).toBe(1080 - 44);
  });
  it("corner case picks the nearest edge", () => {
    // center (130,70): 70 from top, 130 from left
    expect(snapToEdge({ x: 20, y: 48 }, size, [mon()]).edge).toBe("top");
    // center (80, 200): 80 from left, 200 from top
    expect(snapToEdge({ x: -30, y: 178 }, size, [mon()]).edge).toBe("left");
  });
  it("clamps offset at both ends", () => {
    expect(snapToEdge({ x: -500, y: 0 }, size, [mon()]).offset).toBe(0);
    expect(snapToEdge({ x: 5000, y: 0 }, size, [mon()]).offset).toBe(1);
    const side = snapToEdge({ x: 0, y: -400 }, { width: 44, height: 220 }, [mon()]);
    expect(side.edge).toBe("left");
    expect(side.offset).toBe(0);
    expect(side.position.y).toBe(0);
    const end = snapToEdge({ x: 0, y: 5000 }, { width: 44, height: 220 }, [mon()]);
    expect(end.offset).toBe(1);
    expect(end.position.y).toBe(1080 - 220);
  });
  it("keeps the drop point along the edge (centered offset)", () => {
    const r = snapToEdge({ x: 850, y: 0 }, size, [mon()]);
    expect(r.position.x).toBe(850);
    expect(r.offset).toBeCloseTo(850 / (1920 - 220), 5);
  });
  it("multi-monitor: center on second monitor snaps there", () => {
    const b = mon({ name: "B", x: 1920, y: 0, width: 2560, height: 1440 });
    const r = snapToEdge({ x: 2400, y: 20 }, size, [mon(), b]);
    expect(r.monitor.name).toBe("B");
    expect(r.edge).toBe("top");
    expect(r.position.x).toBeGreaterThanOrEqual(1920);
    expect(r.position.y).toBe(0);
    const left = snapToEdge({ x: 1930, y: 600 }, size, [mon(), b]);
    expect(left.monitor.name).toBe("B");
    expect(left.edge).toBe("left");
    expect(left.position.x).toBe(1920);
  });
  it("scale 1.5: sizes and positions are physical", () => {
    const m = mon({ width: 2880, height: 1620, scale: 1.5 });
    const r = snapToEdge({ x: 100, y: 1500 }, { width: 330, height: 66 }, [m]);
    expect(r.edge).toBe("bottom");
    expect(r.size).toEqual({ width: 330, height: 66 });
    expect(r.position.y).toBe(1620 - 66);
    const side = snapToEdge({ x: 2800, y: 700 }, { width: 330, height: 66 }, [m]);
    expect(side.edge).toBe("right");
    expect(side.size).toEqual({ width: 66, height: 330 });
    expect(side.position.x).toBe(2880 - 66);
  });
});

describe("placeCollapsed", () => {
  it("round-trips with snap at scale 1.0 and 1.5", () => {
    for (const scale of [1, 1.5]) {
      const m = mon({ width: 1920 * scale, height: 1080 * scale, scale });
      for (const edge of ["top", "bottom", "left", "right"] as const) {
        const pos = placeCollapsed(edge, 0.3, m);
        const s = snapToEdge(pos, collapsedSize(edge, scale), [m]);
        expect(s.edge).toBe(edge);
        expect(s.offset).toBeCloseTo(0.3, 2);
        expect(s.position).toEqual(pos);
      }
    }
  });
  it("clamps out-of-range offsets", () => {
    expect(placeCollapsed("top", -2, mon()).x).toBe(0);
    expect(placeCollapsed("top", 9, mon()).x).toBe(1920 - 220);
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

describe("placeExpanded", () => {
  const m = mon();
  it("expanded size is 320 wide and header + rows tall (max 8 rows)", () => {
    expect(expandedSize(0, 1)).toEqual({ width: 320, height: 44 + 32 });
    expect(expandedSize(3, 1)).toEqual({ width: 320, height: 44 + 96 });
    expect(expandedSize(20, 1).height).toBe(44 + 8 * 32);
    expect(expandedSize(3, 1.5)).toEqual({ width: 480, height: 210 });
  });
  it("top/bottom stay centered on the pill and clamp on-screen", () => {
    const e = expandedSize(2, 1);
    const c = { ...placeCollapsed("top", 0.5, m), ...collapsedSize("top", 1) };
    expect(placeExpanded("top", c, e, m)).toEqual({ x: c.x + 110 - 160, y: 0 });
    const edgeC = { ...placeCollapsed("top", 0, m), ...collapsedSize("top", 1) };
    expect(placeExpanded("top", edgeC, e, m).x).toBe(0);
    const bc = { ...placeCollapsed("bottom", 1, m), ...collapsedSize("bottom", 1) };
    const p = placeExpanded("bottom", bc, e, m);
    expect(p.x).toBe(1920 - 320);
    expect(p.y).toBe(1080 - e.height);
  });
  it("side edges grow inward from the edge", () => {
    const e = expandedSize(2, 1);
    const lc = { ...placeCollapsed("left", 0.5, m), ...collapsedSize("left", 1) };
    expect(placeExpanded("left", lc, e, m).x).toBe(0);
    const rc = { ...placeCollapsed("right", 0.5, m), ...collapsedSize("right", 1) };
    expect(placeExpanded("right", rc, e, m).x).toBe(1920 - 320);
    const bottomC = { ...placeCollapsed("right", 1, m), ...collapsedSize("right", 1) };
    expect(placeExpanded("right", bottomC, e, m).y + e.height).toBeLessThanOrEqual(1080);
  });
});
