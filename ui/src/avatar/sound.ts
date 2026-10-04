// D16: alert sounds are synthesized, no audio files. Each builder wires nodes
// into `out` (a master gain) and returns the sound length in seconds.
import type { SoundId } from "../shared/contracts";

/** The slice of the WebAudio API we use (lets tests pass a mock). */
export interface ParamLike {
  value: number;
  setValueAtTime(v: number, t: number): unknown;
  linearRampToValueAtTime(v: number, t: number): unknown;
  exponentialRampToValueAtTime(v: number, t: number): unknown;
}
export interface NodeLike {
  connect(dest: unknown): unknown;
}
export interface OscLike extends NodeLike {
  type: string;
  frequency: ParamLike;
  start(t?: number): void;
  stop(t?: number): void;
}
export interface GainLike extends NodeLike {
  gain: ParamLike;
}
export interface FilterLike extends NodeLike {
  type: string;
  frequency: ParamLike;
}
export interface CtxLike {
  currentTime: number;
  destination: unknown;
  createOscillator(): OscLike;
  createGain(): GainLike;
  createBiquadFilter(): FilterLike;
}

const FLOOR = 0.0001;

/** Gain envelope: quick attack, exponential decay to silence. */
function envelope(ctx: CtxLike, to: unknown, t: number, peak: number, len: number): GainLike {
  const g = ctx.createGain();
  g.gain.setValueAtTime(FLOOR, t);
  g.gain.linearRampToValueAtTime(peak, t + 0.012);
  g.gain.exponentialRampToValueAtTime(FLOOR, t + len);
  g.connect(to);
  return g;
}

function tone(ctx: CtxLike, to: unknown, type: string, freq: number, t: number, peak: number, len: number): OscLike {
  const osc = ctx.createOscillator();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, t);
  osc.connect(envelope(ctx, to, t, peak, len));
  osc.start(t);
  osc.stop(t + len + 0.05);
  return osc;
}

type Builder = (ctx: CtxLike, out: unknown, t: number) => number;

const builders: Record<SoundId, Builder> = {
  // two-note bell: sine with soft decay
  chime(ctx, out, t) {
    tone(ctx, out, "sine", 659.25, t, 0.6, 0.9);
    tone(ctx, out, "sine", 987.77, t + 0.18, 0.5, 1.1);
    return 1.3;
  },
  // single bright sine
  ding(ctx, out, t) {
    tone(ctx, out, "sine", 1318.5, t, 0.7, 0.6);
    return 0.65;
  },
  // three short square beeps, low-passed so they are not harsh
  alarm(ctx, out, t) {
    const lp = ctx.createBiquadFilter();
    lp.type = "lowpass";
    lp.frequency.setValueAtTime(1400, t);
    lp.connect(out);
    for (let i = 0; i < 3; i++) tone(ctx, lp, "square", 440, t + i * 0.2, 0.25, 0.12);
    return 0.65;
  },
  // FM-ish bell: a modulator wobbles the carrier frequency
  bell(ctx, out, t) {
    const carrier = ctx.createOscillator();
    carrier.type = "sine";
    carrier.frequency.setValueAtTime(520, t);
    carrier.connect(envelope(ctx, out, t, 0.6, 1.6));
    const mod = ctx.createOscillator();
    mod.type = "sine";
    mod.frequency.setValueAtTime(520 * 3.5, t);
    const depth = ctx.createGain();
    depth.gain.setValueAtTime(260, t);
    depth.gain.exponentialRampToValueAtTime(FLOOR, t + 1.4);
    mod.connect(depth);
    depth.connect(carrier.frequency);
    carrier.start(t);
    mod.start(t);
    carrier.stop(t + 1.7);
    mod.stop(t + 1.7);
    return 1.7;
  },
};

/** Schedule sound `id` now. Returns its length in seconds; a null id is silence (0). */
export function playSound(ctx: CtxLike, id: SoundId | null, volume: number): number {
  if (!id || !(id in builders)) return 0;
  const master = ctx.createGain();
  master.gain.setValueAtTime(Math.min(1, Math.max(0, Number.isFinite(volume) ? volume : 0)), ctx.currentTime);
  master.connect(ctx.destination);
  return builders[id](ctx, master, ctx.currentTime + 0.02);
}
