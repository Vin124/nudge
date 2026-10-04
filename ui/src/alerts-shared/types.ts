// Mirrors `AlertFire` in src-tauri/src/alerts/mod.rs (serde camelCase).
import type { AlertKind, SoundId } from "../shared/contracts";

export const ALERT_EVENT = "nudge://alert";
/** Pages emit this with their window label when done; Rust hides the window (no focus steal). */
export const ALERT_HIDE_EVENT = "nudge://alert-hide";

export interface AlertFire {
  sessionId: string;
  project: string;
  kind: AlertKind;
  /** 0 = first alert, k >= 1 = k-th D8 escalation */
  escalation: number;
  color: string;
  glow: boolean;
  pulses: number;
  sound: SoundId | null;
  tts: boolean;
  ttsVoice: string | null;
  volume: number;
  avatar: boolean;
  /** absolute path of a custom avatar image, or null for the built-in mascot */
  avatarSrc: string | null;
}
