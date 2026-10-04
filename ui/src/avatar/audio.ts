import type { AlertFire } from "../alerts-shared/types";
import { phrase } from "../alerts-shared/helpers";
import { playSound, type CtxLike } from "./sound";

let ctx: AudioContext | null = null;

/** Lazily create and resume the AudioContext. Warns loudly if the webview keeps it suspended. */
async function audio(): Promise<AudioContext | null> {
  try {
    ctx ??= new AudioContext();
    if (ctx.state !== "running") await ctx.resume();
    if (ctx.state !== "running") {
      console.warn(`nudge: AudioContext is ${ctx.state}; alert sound will be silent`);
      return null;
    }
    return ctx;
  } catch (e) {
    console.warn("nudge: AudioContext unavailable", e);
    return null;
  }
}

function speak(fire: AlertFire): void {
  if (!fire.tts || typeof speechSynthesis === "undefined") return;
  const u = new SpeechSynthesisUtterance(phrase(fire.project, fire.kind, fire.escalation));
  const voice = speechSynthesis.getVoices().find((v) => v.name === fire.ttsVoice);
  if (voice) u.voice = voice;
  u.volume = Math.min(1, Math.max(0, fire.volume));
  speechSynthesis.cancel();
  speechSynthesis.speak(u);
}

/** Play the alert sound, then speak the phrase once the sound has ended. */
export async function playAlertAudio(fire: AlertFire): Promise<void> {
  let len = 0;
  if (fire.sound) {
    const c = await audio();
    if (c) len = playSound(c as unknown as CtxLike, fire.sound, fire.volume);
  }
  if (fire.tts) setTimeout(() => speak(fire), len * 1000 + 100);
}
