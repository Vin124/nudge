import { phrase } from "../alerts-shared/helpers";
import type { AlertFire } from "../alerts-shared/types";
import { pixelFox, smoke, type Crop } from "./fox";

export type Stage = "peek-top" | "peek-bottom" | "peek-left" | "peek-right" | "pop";

/** D21: peek out from behind the notch on its docked edge, or pop in at screen center. */
export function stageFor(fire: Pick<AlertFire, "mascotMode" | "edge">): Stage {
  return fire.mascotMode === "pop" ? "pop" : `peek-${fire.edge}`;
}

const CROP: Record<Stage, Crop> = {
  "peek-top": "peek",
  "peek-bottom": "full",
  "peek-left": "side",
  "peek-right": "side",
  pop: "full",
};

/**
 * Render the popup into `root`. Project names go in via textContent only; the
 * fox is built from static geometry. `toSrc` converts a file path to a
 * loadable URL (Tauri convertFileSrc).
 */
export function renderAvatar(root: HTMLElement, fire: AlertFire, toSrc: (p: string) => string): string {
  const text = phrase(fire.project, fire.kind, fire.escalation);
  const stage = stageFor(fire);
  root.replaceChildren();
  root.dataset.kind = fire.kind;
  root.dataset.stage = stage;

  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = text;

  const actor = document.createElement("div");
  actor.className = "actor";
  const hold = document.createElement("div");
  hold.className = "hold";
  if (fire.avatarSrc) {
    const img = document.createElement("img");
    img.src = toSrc(fire.avatarSrc);
    img.alt = "";
    img.draggable = false;
    hold.appendChild(img);
  } else {
    hold.appendChild(pixelFox(fire.kind, CROP[stage], stage === "pop" ? 5 : 4));
  }
  actor.appendChild(hold);

  root.append(actor, bubble);
  if (stage === "pop") root.append(smoke());
  return text;
}
