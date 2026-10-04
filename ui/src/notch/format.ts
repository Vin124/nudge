// Text/number formatting for the notch. No Tauri or DOM imports.

/** Time since a state began: "now", "59s", "1m", "2h 5m", "1d 3h". */
export function formatElapsed(ms: number): string {
  const s = Math.floor(Math.max(0, ms) / 1000);
  if (s < 1) return "now";
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return m % 60 ? `${h}h ${m % 60}m` : `${h}h`;
  const d = Math.floor(h / 24);
  return h % 24 ? `${d}d ${h % 24}h` : `${d}d`;
}

/** Time until `resetsAt` (unix seconds): "1h 12m", "<1m", or "resetting" if already past. */
export function formatCountdown(resetsAtSec: number, nowMs: number): string {
  const ms = resetsAtSec * 1000 - nowMs;
  if (ms <= 0) return "resetting";
  const m = Math.floor(ms / 60000);
  if (m < 1) return "<1m";
  const h = Math.floor(m / 60);
  if (h < 24) return h ? (m % 60 ? `${h}h ${m % 60}m` : `${h}h`) : `${m}m`;
  const d = Math.floor(h / 24);
  return h % 24 ? `${d}d ${h % 24}h` : `${d}d`;
}

/** Rounded percent clamped to 0..100. */
export function formatPercent(p: number): number {
  if (!Number.isFinite(p)) return 0;
  return Math.min(100, Math.max(0, Math.round(p)));
}

/** green < 60 <= amber < 85 <= red */
export function usageColor(p: number): string {
  if (p < 60) return "#3ddc84";
  if (p < 85) return "#ffb020";
  return "#ff4d4f";
}
