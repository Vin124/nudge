// D21: the pixel fox mascot, ported from the "Notch Fox Mascot v2" design (2a).
// Sprites are generated from shapes into character maps, then drawn as SVG
// rect runs. Static geometry only: nothing user-controlled reaches this file.

import type { AlertKind } from "../shared/contracts";

const SVG_NS = "http://www.w3.org/2000/svg";
const PAL: Record<string, string> = {
  K: "#2a1810", O: "#f28a2e", D: "#c75b16", W: "#fff4e4", P: "#f7a59c", Y: "#ffcf3f", G: "#2f9e5f",
};

type Pt = [number, number];
function inTri(x: number, y: number, A: Pt, B: Pt, C: Pt): boolean {
  const f = (p: Pt, q: Pt, r: Pt) => (p[0] - r[0]) * (q[1] - r[1]) - (q[0] - r[0]) * (p[1] - r[1]);
  const p: Pt = [x, y];
  const d1 = f(p, A, B), d2 = f(p, B, C), d3 = f(p, C, A);
  return !((d1 < 0 || d2 < 0 || d3 < 0) && (d1 > 0 || d2 > 0 || d3 > 0));
}
const inEl = (x: number, y: number, cx: number, cy: number, rx: number, ry: number) =>
  ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1;

/** Rasterize `fill` into a W×H map, outline it in K, and color the inside with `paint`. */
function gen(W: number, H: number, fill: (x: number, y: number) => boolean, paint: (x: number, y: number) => string): string[] {
  const g: boolean[][] = [];
  for (let y = 0; y < H; y++) {
    const r: boolean[] = [];
    for (let x = 0; x < W; x++) r.push(fill(x + 0.5, y + 0.5));
    g.push(r);
  }
  return g.map((r, y) =>
    r.map((c, x) => {
      if (!c) return ".";
      const edge = ([[1, 0], [-1, 0], [0, 1], [0, -1]] as const).some(([dx, dy]) => !g[y + dy]?.[x + dx]);
      return edge ? "K" : paint(x + 0.5, y + 0.5);
    }).join(""),
  );
}

const HEAD = gen(24, 19,
  (x, y) => inEl(x, y, 12, 10.8, 11.9, 7.8) || inTri(x, y, [2.4, 0.2], [1.2, 9], [10, 4.6]) || inTri(x, y, [21.6, 0.2], [22.8, 9], [14, 4.6]),
  (x, y) => {
    if (inTri(x, y, [3.2, 2], [2.6, 7.6], [8.2, 5]) || inTri(x, y, [20.8, 2], [21.4, 7.6], [15.8, 5])) return "D";
    if (y > 10.6 && (inEl(x, y, 6.2, 14.8, 4.2, 3.8) || inEl(x, y, 17.8, 14.8, 4.2, 3.8) || inEl(x, y, 12, 15.6, 5, 2.8) || y > 16.2)) return "W";
    return "O";
  });
const BODY = gen(26, 26,
  (x, y) => inEl(x, y, 12, 20.8, 7.4, 4.6) || inEl(x, y, 8.5, 24.6, 2.7, 1.4) || inEl(x, y, 15.5, 24.6, 2.7, 1.4),
  (x, y) => (y > 24 ? "D" : inEl(x, y, 12, 21.6, 3.8, 3) ? "W" : "O"));
const TAIL = ["....KKK..", "...KWWWK.", "..KOWWWK.", ".KOOOWK..", "KOOOOOK..", "KOOOOK...", ".KOOK....", "..KK....."];
const PAW = [".KK.", "KWWK", "KOOK", "KOOK", ".KK."];
const PAWR = PAW.map((r) => r.split("").reverse().join(""));
const REST = ["KOOK", "KWWK", ".KK."];
const SIDEPAW = [".KKK", "KWWW", "KWWW", ".KKK"];
const ARM = ["KKKK", "KWWK", "KWWK", "KOOK", "KOOK", "KOOK"];
const FACE = [
  { map: ["PP"], x: 3, y: 13 }, { map: ["PP"], x: 19, y: 13 },
  { map: ["KK"], x: 11, y: 13 }, { map: ["K..K", ".KK."], x: 10, y: 14 },
];
const EYES = {
  tall: { m: [".KK.", "KWKK", "KKKK", ".KK."], l: [5, 9], r: [15, 9] },
  happy: { m: [".KK.", "K..K"], l: [5, 10], r: [15, 10] },
  blink: { m: ["KKKK"], l: [5, 11], r: [15, 11] },
} as const;
const QG = [".DDD.", "D...D", "....D", "..DD.", "..D..", ".....", "..D.."];
const CG = ["......G", ".....GG", "G...GG.", "GG.GG..", ".GGG...", "..G...."];
export const SPK = [".Y.", "YYY", ".Y."];

/** Speech bubble around glyph `g`, with a tail at the bottom left. */
function bubble(g: string[]): string[] {
  const gw = g[0].length, gh = g.length, w = gw + 4, hh = gh + 4, rows: string[] = [];
  for (let y = 0; y < hh; y++) {
    let r = "";
    for (let x = 0; x < w; x++) {
      const corner = (x === 0 || x === w - 1) && (y === 0 || y === hh - 1);
      const border = x === 0 || y === 0 || x === w - 1 || y === hh - 1;
      if (corner) r += ".";
      else if (border) r += "K";
      else {
        const c = (g[y - 2] || "")[x - 2];
        r += c && c !== "." ? c : "W";
      }
    }
    rows.push(r);
  }
  const last = rows[hh - 1].split("");
  last[2] = "W";
  rows[hh - 1] = last.join("");
  rows.push(".KWK".padEnd(w, "."));
  rows.push(".KK".padEnd(w, "."));
  return rows;
}
const QB = bubble(QG), CB = bubble(CG);

interface Layer { map: readonly string[]; x: number; y: number; cls?: string }

function runs(map: readonly string[], ox: number, oy: number): SVGRectElement[] {
  const out: SVGRectElement[] = [];
  map.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      const c = row[x];
      if (!PAL[c]) { x++; continue; }
      let e = x;
      while (e < row.length && row[e] === c) e++;
      const r = document.createElementNS(SVG_NS, "rect");
      r.setAttribute("x", String(ox + x));
      r.setAttribute("y", String(oy + y));
      r.setAttribute("width", String(e - x));
      r.setAttribute("height", "1");
      r.setAttribute("fill", PAL[c]);
      out.push(r);
      x = e;
    }
  });
  return out;
}

export function sprite(layers: Layer[], W: number, H: number, px: number): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("width", String(W * px));
  svg.setAttribute("height", String(H * px));
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  svg.setAttribute("shape-rendering", "crispEdges");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", "sprite");
  for (const L of layers) {
    const g = document.createElementNS(SVG_NS, "g");
    if (L.cls) g.setAttribute("class", L.cls);
    g.append(...runs(L.map, L.x, L.y));
    svg.append(g);
  }
  return svg;
}

type Eyes = (typeof EYES)[keyof typeof EYES];
const face = (E: Eyes, ox: number, oy: number, cls?: string): Layer[] => [
  ...FACE.map((f) => ({ map: f.map, x: f.x + ox, y: f.y + oy })),
  { map: E.m, x: E.l[0] + ox, y: E.l[1] + oy, cls },
  { map: E.m, x: E.r[0] + ox, y: E.r[1] + oy, cls },
];

/** Asking: tall eyes with a CSS-driven blink. Done: happy ^ ^ eyes. */
function eyes(ask: boolean, ox: number, oy: number): Layer[] {
  if (!ask) return face(EYES.happy, ox, oy);
  return [...face(EYES.tall, ox, oy, "eyes-open"), ...face(EYES.blink, ox, oy, "eyes-blink").slice(FACE.length)];
}

export type Crop = "full" | "side" | "peek";

/** The fox for an alert kind: "blocked" asks for input, "done" celebrates. */
export function pixelFox(kind: AlertKind, crop: Crop, px: number): HTMLElement {
  const ask = kind === "blocked";
  const root = document.createElement("div");
  root.className = `fox fox-${crop} ${ask ? "ask" : "done"}`;
  if (crop === "side") {
    root.append(sprite([{ map: HEAD, x: 0, y: 0 }, ...eyes(ask, 0, 0), { map: SIDEPAW, x: 11, y: 16 }], 24, 20, px));
    return root;
  }
  if (crop === "peek") {
    root.append(sprite([{ map: ARM, x: 0, y: 0 }, { map: ARM, x: 20, y: 0 }, { map: HEAD, x: 0, y: 4 }, ...eyes(ask, 0, 4)], 24, 23, px));
    return root;
  }
  const L: Layer[] = [{ map: TAIL, x: 17, y: 15, cls: "tail" }, { map: BODY, x: 0, y: 0 }];
  if (ask) L.push({ map: REST, x: 14, y: 19 }, { map: PAW, x: 2, y: 16, cls: "paw-wave" });
  else L.push({ map: PAW, x: 2, y: 16, cls: "paw-cheer-l" }, { map: PAWR, x: 18, y: 16, cls: "paw-cheer-r" });
  L.push({ map: HEAD, x: 0, y: 0 }, ...eyes(ask, 0, 0));

  root.style.width = root.style.height = `${26 * px}px`;
  const body = document.createElement("div");
  body.className = "fox-body";
  body.append(sprite(L, 26, 26, px));
  const B = ask ? QB : CB;
  const bpx = Math.max(3, px - 1);
  const bub = document.createElement("div");
  bub.className = "fox-bubble";
  bub.style.right = `${-px * 3}px`;
  bub.style.top = `${-B.length * bpx * 0.72}px`;
  bub.append(sprite([{ map: B, x: 0, y: 0 }], B[0].length, B.length, bpx));
  root.append(body, bub);
  if (!ask) {
    ([[-8, "30%", "0s"], [96, "58%", ".35s"], [8, "-4%", ".7s"]] as const).forEach(([l, t, d]) => {
      const s = document.createElement("div");
      s.className = "fox-spark";
      s.style.left = `${l}%`;
      s.style.top = t;
      s.style.animationDelay = d;
      s.append(sprite([{ map: SPK, x: 0, y: 0 }], 3, 3, px));
      root.append(s);
    });
  }
  return root;
}

/** Pixel smoke ring for pop mode: [angle°, distance, size] per puff. */
const PUFFS = [[0, 58, 44], [40, 50, 32], [80, 60, 40], [120, 52, 28], [160, 62, 44], [200, 50, 36], [240, 58, 48], [280, 52, 32], [320, 60, 40]] as const;

export function smoke(): HTMLElement {
  const root = document.createElement("div");
  root.className = "smoke";
  const puff = (s: number, dx: number, dy: number, delayMs: number) => {
    const p = document.createElement("div");
    p.className = "puff";
    p.style.width = p.style.height = `${s}px`;
    p.style.left = p.style.top = `${-s / 2}px`;
    p.style.setProperty("--dx", `${dx}px`);
    p.style.setProperty("--dy", `${dy}px`);
    p.style.animationDelay = `${delayMs}ms`;
    return p;
  };
  root.append(puff(80, 0, 0, 0));
  PUFFS.forEach(([a, d, s], i) => {
    const r = (a * Math.PI) / 180;
    root.append(puff(s, Math.round(Math.cos(r) * d), Math.round(Math.sin(r) * d * 0.8), i * 18));
  });
  return root;
}
