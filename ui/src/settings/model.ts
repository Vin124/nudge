// Pure settings logic: config <-> form mapping, minutes-list validation, debounce.
// No DOM, no Tauri.

import type { Config, MascotMode, SoundId, StateAlert } from "../shared/contracts";

export const SOUNDS: readonly SoundId[] = ["chime", "ding", "alarm", "bell"];
export const NO_SOUND = "none";
export const MIN_MINUTE = 1;
export const MAX_MINUTE = 120;
export const MAX_MINUTES_ENTRIES = 5;
export const MIN_PULSES = 1;
export const MAX_PULSES = 20;
export const SAVE_DEBOUNCE_MS = 300;

export interface StateForm {
  glow: boolean;
  /** SoundId, or "none" for null */
  sound: string;
  tts: boolean;
  avatar: boolean;
  color: string;
}

export interface Form {
  showWeekly: boolean;
  done: StateForm;
  blocked: StateForm;
  volume: number;
  glowPulses: number;
  escalateMinutes: number[];
  /** "" = System default (null) */
  ttsVoice: string;
  /** "" = Built-in mascot (null) */
  avatarPack: string;
  mascotMode: MascotMode;
  liveWhenIdle: boolean;
  dnd: boolean;
}

const stateToForm = (a: StateAlert): StateForm => ({
  glow: a.glow,
  sound: a.sound ?? NO_SOUND,
  tts: a.tts,
  avatar: a.avatar,
  color: a.color,
});

const formToState = (f: StateForm): StateAlert => ({
  glow: f.glow,
  sound: f.sound === NO_SOUND ? null : (f.sound as SoundId),
  tts: f.tts,
  avatar: f.avatar,
  color: f.color,
});

export function configToForm(c: Config): Form {
  return {
    showWeekly: c.notch.showWeekly,
    done: stateToForm(c.alerts.done),
    blocked: stateToForm(c.alerts.blocked),
    volume: c.alerts.volume,
    glowPulses: c.alerts.glowPulses,
    escalateMinutes: [...c.alerts.escalateMinutes],
    ttsVoice: c.alerts.ttsVoice ?? "",
    avatarPack: c.alerts.avatarPack ?? "",
    mascotMode: c.alerts.mascotMode ?? "peek",
    liveWhenIdle: c.usage.liveWhenIdle,
    dnd: c.dnd,
  };
}

/** `base` supplies everything the form doesn't edit (version, notch edge/offset/monitor). */
export function formToConfig(f: Form, base: Config): Config {
  return {
    ...base,
    notch: { ...base.notch, showWeekly: f.showWeekly },
    alerts: {
      ...base.alerts,
      done: formToState(f.done),
      blocked: formToState(f.blocked),
      escalateMinutes: [...f.escalateMinutes],
      glowPulses: f.glowPulses,
      volume: f.volume,
      ttsVoice: f.ttsVoice === "" ? null : f.ttsVoice,
      avatarPack: f.avatarPack === "" ? null : f.avatarPack,
      mascotMode: f.mascotMode,
    },
    usage: { ...base.usage, liveWhenIdle: f.liveWhenIdle },
    dnd: f.dnd,
  };
}

/** Parse a whole-number minute value in 1..120; null if it isn't one. */
export function parseMinute(raw: string | number): number | null {
  const s = typeof raw === "number" ? String(raw) : raw.trim();
  if (!/^\d+$/.test(s)) return null;
  const n = Number(s);
  if (!Number.isSafeInteger(n) || n < MIN_MINUTE || n > MAX_MINUTE) return null;
  return n;
}

/** Mirror of the server's sanitize: keep valid, dedupe, sort, cap at 5. */
export function normalizeMinutes(list: readonly number[]): number[] {
  const ok = list.filter((m) => Number.isInteger(m) && m >= MIN_MINUTE && m <= MAX_MINUTE);
  return [...new Set(ok)].sort((a, b) => a - b).slice(0, MAX_MINUTES_ENTRIES);
}

export type AddMinuteResult = { ok: true; list: number[] } | { ok: false; error: string };

export function addMinute(list: readonly number[], raw: string | number): AddMinuteResult {
  const n = parseMinute(raw);
  if (n === null) return { ok: false, error: `Enter a whole number from ${MIN_MINUTE} to ${MAX_MINUTE}.` };
  if (list.includes(n)) return { ok: false, error: `${n} is already in the list.` };
  if (list.length >= MAX_MINUTES_ENTRIES) {
    return { ok: false, error: `At most ${MAX_MINUTES_ENTRIES} reminders.` };
  }
  return { ok: true, list: normalizeMinutes([...list, n]) };
}

export function removeMinute(list: readonly number[], m: number): number[] {
  return list.filter((x) => x !== m);
}

export const clampPulses = (n: number): number => Math.min(MAX_PULSES, Math.max(MIN_PULSES, Math.round(n)));

export interface Debounced<A extends unknown[]> {
  (...args: A): void;
  cancel(): void;
}

/** Trailing debounce: only the last call's arguments are delivered. */
export function debounce<A extends unknown[]>(fn: (...args: A) => void, ms: number): Debounced<A> {
  let t: ReturnType<typeof setTimeout> | null = null;
  const d = ((...args: A) => {
    if (t !== null) clearTimeout(t);
    t = setTimeout(() => {
      t = null;
      fn(...args);
    }, ms);
  }) as Debounced<A>;
  d.cancel = () => {
    if (t !== null) clearTimeout(t);
    t = null;
  };
  return d;
}
