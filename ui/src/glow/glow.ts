import { safeColor } from "../alerts-shared/helpers";
import type { AlertFire } from "../alerts-shared/types";

export const PULSE_MS = 700; // fade in -> peak -> fade out; 1.43 pulses/s, under the 2/s cap
export const STEADY_MS = 1500;

/** Configure the edge element for one alert. CSS does the animating (opacity only). */
export function applyGlow(el: HTMLElement, fire: AlertFire, reducedMotion: boolean): void {
  el.style.setProperty("--glow-color", safeColor(fire.color, "#3ddc84"));
  const pulses = Math.max(1, Math.min(Math.floor(fire.pulses) || 1, 10));
  // Restart the animation even if the same one is already running.
  el.style.animation = "none";
  void el.offsetWidth;
  el.style.animation = "";
  if (reducedMotion) {
    el.dataset.mode = "steady";
    el.style.animationDuration = `${STEADY_MS}ms`;
    el.style.animationIterationCount = "1";
  } else {
    el.dataset.mode = "pulse";
    el.style.animationDuration = `${PULSE_MS}ms`;
    el.style.animationIterationCount = String(pulses);
  }
}
