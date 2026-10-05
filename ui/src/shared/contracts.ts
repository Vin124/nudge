// UI ↔ core contract. Mirrors src-tauri/src/{session,usage,config,hub,commands}.rs.
// Orchestrator-owned: lanes import, never edit.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type SessionState = "idle" | "running" | "blocked" | "done";

export interface Session {
  id: string;
  project: string;
  cwd: string;
  state: SessionState;
  /** ms since epoch when `state` was entered */
  sinceMs: number;
  /** D8: true from Done/Blocked until acknowledged */
  alertPending: boolean;
  /** D22: context window used, 0..100; null until the session's statusline reports it */
  contextPercent: number | null;
}

export interface UsageWindow {
  usedPercentage: number;
  /** unix seconds */
  resetsAt: number | null;
}

export interface UsageSnapshot {
  fiveHour: UsageWindow | null;
  sevenDay: UsageWindow | null;
  /** null = never heard; false = API key / 3P provider → show "n/a" (AC #8) */
  available: boolean | null;
  source: "statusline" | "poll" | null;
  updatedMs: number;
}

export type Edge = "top" | "bottom" | "left" | "right";
/** D21: fox peeks out from the notch, or pops in at screen center. */
export type MascotMode = "peek" | "pop";
export type SoundId = "chime" | "ding" | "alarm" | "bell";

export interface StateAlert {
  glow: boolean;
  sound: SoundId | null;
  tts: boolean;
  avatar: boolean;
  color: string;
}

export interface Config {
  version: number;
  notch: { edge: Edge; offset: number; monitor: string | null; showWeekly: boolean };
  alerts: {
    done: StateAlert;
    blocked: StateAlert;
    escalateMinutes: number[];
    glowPulses: number;
    volume: number;
    ttsVoice: string | null;
    avatarPack: string | null;
    mascotMode: MascotMode;
  };
  usage: { liveWhenIdle: boolean };
  dnd: boolean;
}

export interface Snapshot {
  sessions: Session[];
  usage: UsageSnapshot;
  config: Config;
}

export type AlertKind = "done" | "blocked";

export const EVENT_SNAPSHOT = "nudge://snapshot";

export const api = {
  getSnapshot: () => invoke<Snapshot>("get_snapshot"),
  ackSession: (id: string) => invoke<void>("ack_session", { id }),
  /** D7: focus terminal + acknowledge. Rejects with a message if focus fails. */
  focusSession: (id: string) => invoke<void>("focus_session", { id }),
  setConfig: (config: Config) => invoke<void>("set_config", { config }),
  previewAlert: (kind: AlertKind) => invoke<void>("preview_alert", { kind }),
  /** D6: folder names under ~/.nudge/avatars */
  listAvatarPacks: () => invoke<string[]>("list_avatar_packs"),
};

/** D20: notch window mechanics (src-tauri/src/notch.rs). Rects are logical, window-relative. */
export const EVENT_NOTCH_HOVER = "nudge://notch-hover";
export const EVENT_NOTCH_DROP = "nudge://notch-drop";

export interface NotchRect { x: number; y: number; width: number; height: number }

export const notchApi = {
  /** The cursor is live only inside this rect; elsewhere the window is click-through. */
  setHit: (rect: NotchRect | null) => invoke<void>("notch_set_hit", { rect }),
  /** Rust moves the window with the cursor until release, then emits EVENT_NOTCH_DROP. */
  dragStart: () => invoke<void>("notch_drag_start"),
  dragEnd: () => invoke<void>("notch_drag_end"),
  /** Slide the window (physical px) with ease-out cubic. */
  animateTo: (x: number, y: number, ms: number) => invoke<void>("notch_animate_to", { x, y, ms }),
};

/** Subscribe to snapshots; also delivers the current one immediately. */
export async function onSnapshot(cb: (s: Snapshot) => void): Promise<UnlistenFn> {
  let gotEvent = false;
  const unlisten = await listen<Snapshot>(EVENT_SNAPSHOT, (e) => {
    gotEvent = true;
    cb(e.payload);
  });
  // Windows from tauri.conf.json load before the Rust setup() registers the
  // core state, so the first get_snapshot can fail. Retry until it answers.
  for (let delay = 100; ; delay = Math.min(delay * 2, 1000)) {
    try {
      const initial = await api.getSnapshot();
      // An event that raced ahead of the initial fetch is newer; don't clobber it.
      if (!gotEvent) cb(initial);
      return unlisten;
    } catch {
      if (gotEvent) return unlisten;
      await new Promise((r) => setTimeout(r, delay));
    }
  }
}
