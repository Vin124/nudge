import type { AlertKind } from "../shared/contracts";

/** Spoken / bubble phrase (SPEC §Alerts). */
export function phrase(project: string, kind: AlertKind, escalation: number): string {
  if (escalation >= 1) return `${project} is still waiting`;
  return kind === "done" ? `${project} is done` : `${project} needs you`;
}

const HEX = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

/** Colors come from config; only plain hex ever reaches a style. */
export function safeColor(c: string, fallback: string): string {
  return HEX.test(c) ? c : fallback;
}
