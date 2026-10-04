// D6: original mascot, a small black "pebble" with two eyes and a tiny spark.
// Static markup only: nothing user-controlled is ever interpolated.
import type { AlertKind } from "../shared/contracts";

const BODY =
  '<path d="M60 14c30 0 48 20 48 50 0 28-18 46-48 46S12 92 12 64c0-30 18-50 48-50z" fill="#0b0b0d" stroke="#2a2a30" stroke-width="2"/>';
const SPARK = '<path class="spark" d="M96 20l3 8 8 3-8 3-3 8-3-8-8-3 8-3z" fill="#ffd54a"/>';

const happy =
  '<g fill="#fff"><circle cx="44" cy="62" r="7"/><circle cx="76" cy="62" r="7"/></g>' +
  '<g fill="#0b0b0d"><circle cx="46" cy="63" r="3"/><circle cx="78" cy="63" r="3"/></g>' +
  '<path d="M48 82q12 10 24 0" fill="none" stroke="#3ddc84" stroke-width="4" stroke-linecap="round"/>';

const urgent =
  '<g fill="#fff"><circle cx="44" cy="64" r="8"/><circle cx="76" cy="64" r="8"/></g>' +
  '<g fill="#0b0b0d"><circle cx="45" cy="65" r="3.5"/><circle cx="75" cy="65" r="3.5"/></g>' +
  '<path d="M33 46l20 6M87 46l-20 6" stroke="#ff8a00" stroke-width="5" stroke-linecap="round"/>' +
  '<ellipse cx="60" cy="88" rx="6" ry="5" fill="#ff8a00"/>';

export function mascotSvg(kind: AlertKind): string {
  const face = kind === "done" ? happy : urgent;
  return `<svg class="mascot ${kind}" viewBox="0 0 120 120" width="120" height="120" aria-hidden="true">${BODY}${face}${SPARK}</svg>`;
}
