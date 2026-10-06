// Static SVG glyphs for the notch, built with DOM calls (no markup strings).
// Original artwork: a sparkle in Claude's orange and a small session critter.

const SVG_NS = "http://www.w3.org/2000/svg";

export function svgEl<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number>,
): SVGElementTagNameMap[K] {
  const e = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e;
}

/** Eight rounded rays, alternating long and short. */
export function sparkle(cls = "sparkle"): SVGSVGElement {
  const svg = svgEl("svg", { viewBox: "0 0 24 24", class: cls, "aria-hidden": "true" });
  const g = svgEl("g", { stroke: "currentColor", "stroke-width": 2.6, "stroke-linecap": "round" });
  for (let i = 0; i < 8; i++) {
    const a = (i * Math.PI) / 4 - Math.PI / 2;
    const r = i % 2 === 0 ? 10 : 6.8;
    const f = (v: number) => (12 + v).toFixed(2);
    g.append(svgEl("line", { x1: 12, y1: 12, x2: f(Math.cos(a) * r), y2: f(Math.sin(a) * r) }));
  }
  svg.append(g);
  return svg;
}

/** Rounded critter with two tall eyes; the body takes `currentColor`. */
export function critter(cls = "face"): SVGSVGElement {
  const svg = svgEl("svg", { viewBox: "0 0 24 24", class: cls, "aria-hidden": "true" });
  svg.append(
    svgEl("rect", { x: 3, y: 4.5, width: 18, height: 15, rx: 6, fill: "currentColor" }),
    svgEl("ellipse", { cx: 12, cy: 7.4, rx: 5.5, ry: 1.6, fill: "#ffffff", opacity: 0.18 }),
    svgEl("rect", { x: 8.2, y: 9.8, width: 2.4, height: 4.6, rx: 1.2, fill: "#141414" }),
    svgEl("rect", { x: 13.4, y: 9.8, width: 2.4, height: 4.6, rx: 1.2, fill: "#141414" }),
  );
  return svg;
}

/** Simple outline cog for the settings button. */
export function gear(cls = "gear-icon"): SVGSVGElement {
  const svg = svgEl("svg", { viewBox: "0 0 24 24", class: cls, "aria-hidden": "true" });
  const g = svgEl("g", { fill: "none", stroke: "currentColor", "stroke-width": 1.8, "stroke-linecap": "round" });
  for (let i = 0; i < 8; i++) {
    const a = (i * Math.PI) / 4;
    const f = (r: number, t: (x: number) => number) => (12 + t(a) * r).toFixed(2);
    g.append(svgEl("line", { x1: f(7.2, Math.cos), y1: f(7.2, Math.sin), x2: f(9.6, Math.cos), y2: f(9.6, Math.sin) }));
  }
  g.append(svgEl("circle", { cx: 12, cy: 12, r: 6.2 }), svgEl("circle", { cx: 12, cy: 12, r: 2.4 }));
  svg.append(g);
  return svg;
}
