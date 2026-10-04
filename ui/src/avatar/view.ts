import { phrase } from "../alerts-shared/helpers";
import type { AlertFire } from "../alerts-shared/types";
import { mascotSvg } from "./mascot";

/**
 * Render the popup into `root`. Project names go in via textContent only; the
 * only innerHTML is the static mascot markup. `toSrc` converts a file path to
 * a loadable URL (Tauri convertFileSrc).
 */
export function renderAvatar(root: HTMLElement, fire: AlertFire, toSrc: (p: string) => string): string {
  const text = phrase(fire.project, fire.kind, fire.escalation);
  root.replaceChildren();
  root.dataset.kind = fire.kind;

  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = text;

  const figure = document.createElement("div");
  figure.className = "figure";
  if (fire.avatarSrc) {
    const img = document.createElement("img");
    img.src = toSrc(fire.avatarSrc);
    img.alt = "";
    img.draggable = false;
    figure.appendChild(img);
  } else {
    figure.innerHTML = mascotSvg(fire.kind);
  }

  root.append(bubble, figure);
  return text;
}
