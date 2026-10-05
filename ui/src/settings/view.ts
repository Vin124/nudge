// DOM for the settings window. Built once; `render` refreshes control values from a
// config but never overwrites the control the user is currently editing.
// All data-derived text (pack names, voice names) goes through textContent / .value.

import type { AlertKind, Config } from "../shared/contracts";
import {
  addMinute,
  clampPulses,
  configToForm,
  NO_SOUND,
  removeMinute,
  SOUNDS,
  type Form,
  type StateForm,
} from "./model";

export const USAGE_DISCLOSURE =
  "Reads your Claude Code login token from this computer to ask Anthropic for your usage. " +
  "The token never leaves your machine except to api.anthropic.com, and is never stored by Nudge.";
export const DRAG_NOTE = "Drag the notch to move it.";
export const AVATAR_HINT = "Custom packs: ~/.nudge/avatars/<name>/done.png|gif|svg, blocked.*";
export const DEFAULT_VOICE_TEXT = "System default";
export const BUILTIN_PACK_TEXT = "Built-in mascot";

export interface ViewDeps {
  /** Called with the full form after every user edit. */
  onChange: (form: Form) => void;
  onPreview: (kind: AlertKind) => void;
}

export interface SettingsView {
  render(config: Config): void;
  setPacks(names: string[]): void;
  setVoices(names: string[]): void;
  form(): Form | null;
}

interface Binding {
  el: HTMLInputElement | HTMLSelectElement;
  /** control -> form (mutates) */
  read(f: Form): void;
  /** form -> control */
  write(f: Form): void;
}

type El<K extends keyof HTMLElementTagNameMap> = HTMLElementTagNameMap[K];

function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<El<K>> & { class?: string } = {},
  ...kids: (Node | string)[]
): El<K> {
  const { class: cls, ...rest } = props;
  const el = document.createElement(tag);
  Object.assign(el, rest);
  if (cls) el.className = cls;
  for (const k of kids) el.append(k);
  return el;
}

let uid = 0;
const nextId = (p: string) => `${p}-${++uid}`;

function ensureOption(sel: HTMLSelectElement, value: string, text: string): void {
  if (![...sel.options].some((o) => o.value === value)) sel.append(h("option", { value, textContent: text }));
}

export function createView(root: HTMLElement, deps: ViewDeps): SettingsView {
  const bindings: Binding[] = [];
  let form: Form | null = null;
  let packNames: string[] = [];
  let voiceNames: string[] = [];

  const emit = () => {
    if (form) deps.onChange(structuredClone(form));
  };

  /** Register a control: on user input, read it into `form` then emit. */
  function bind<E extends HTMLInputElement | HTMLSelectElement>(
    el: E,
    read: (f: Form) => void,
    write: (f: Form) => void,
    event = "change",
  ): E {
    bindings.push({ el, read, write });
    el.addEventListener(event, () => {
      if (!form) return;
      read(form);
      emit();
    });
    return el;
  }

  function toggle(text: string, read: (f: Form) => boolean, set: (f: Form, v: boolean) => void): HTMLLabelElement {
    const input = h("input", { type: "checkbox" });
    bind(
      input,
      (f) => set(f, input.checked),
      (f) => (input.checked = read(f)),
    );
    return h("label", { class: "row toggle" }, h("span", { textContent: text }), input);
  }

  function selectRow(text: string, sel: HTMLSelectElement): HTMLElement {
    const id = nextId("sel");
    sel.id = id;
    return h("div", { class: "row" }, h("label", { htmlFor: id, textContent: text }), sel);
  }

  function stateSection(kind: AlertKind, title: string): HTMLElement {
    const get = (f: Form): StateForm => f[kind];

    const sound = h("select");
    for (const s of SOUNDS) sound.append(h("option", { value: s, textContent: s[0].toUpperCase() + s.slice(1) }));
    sound.append(h("option", { value: NO_SOUND, textContent: "None" }));
    bind(
      sound,
      (f) => (get(f).sound = sound.value),
      (f) => (sound.value = get(f).sound),
    );

    const colorId = nextId("color");
    const color = h("input", { type: "color", id: colorId });
    bind(
      color,
      (f) => (get(f).color = color.value),
      (f) => (color.value = get(f).color),
      "input",
    );

    const test = h("button", { type: "button", class: "btn", textContent: "Test" });
    test.setAttribute("aria-label", `Test ${title.toLowerCase()} alert`);
    test.addEventListener("click", () => deps.onPreview(kind));

    return h(
      "section",
      { class: "card" },
      h("h2", { textContent: title }),
      toggle("Glow", (f) => get(f).glow, (f, v) => (get(f).glow = v)),
      selectRow("Sound", sound),
      toggle("Voice", (f) => get(f).tts, (f, v) => (get(f).tts = v)),
      toggle("Avatar", (f) => get(f).avatar, (f, v) => (get(f).avatar = v)),
      h("div", { class: "row" }, h("label", { htmlFor: colorId, textContent: "Color" }), color),
      h("div", { class: "actions" }, test),
    );
  }

  // ---- Notch ----
  const notch = h(
    "section",
    { class: "card" },
    h("h2", { textContent: "Notch" }),
    toggle("Show weekly wheel", (f) => f.showWeekly, (f, v) => (f.showWeekly = v)),
    h("p", { class: "hint", textContent: DRAG_NOTE }),
  );

  // ---- Alerts ----
  const volumeId = nextId("vol");
  const volumeOut = h("output");
  volumeOut.setAttribute("for", volumeId);
  const volume = h("input", { type: "range", id: volumeId, min: "0", max: "1", step: "0.05" });
  bind(
    volume,
    (f) => {
      const v = Number(volume.value);
      if (Number.isFinite(v)) f.volume = Math.min(1, Math.max(0, v));
      volumeOut.textContent = `${Math.round(f.volume * 100)}%`;
    },
    (f) => {
      volume.value = String(f.volume);
      volumeOut.textContent = `${Math.round(f.volume * 100)}%`;
    },
    "input",
  );

  const pulsesId = nextId("pulses");
  const pulses = h("input", { type: "number", id: pulsesId, min: "1", max: "20", step: "1" });
  bind(
    pulses,
    (f) => {
      // Empty or half-typed input: keep the previous value rather than sending NaN.
      if (pulses.value.trim() === "" || !Number.isFinite(Number(pulses.value))) return;
      f.glowPulses = clampPulses(Number(pulses.value));
    },
    (f) => (pulses.value = String(f.glowPulses)),
    "input",
  );

  const voice = h("select");
  voice.append(h("option", { value: "", textContent: DEFAULT_VOICE_TEXT }));
  bind(
    voice,
    (f) => (f.ttsVoice = voice.value),
    (f) => {
      ensureOption(voice, f.ttsVoice, f.ttsVoice);
      voice.value = f.ttsVoice;
    },
  );

  const chips = h("ul", { class: "chips" });
  chips.setAttribute("aria-label", "Reminder times in minutes");
  const minuteId = nextId("minute");
  const minuteInput = h("input", { type: "number", id: minuteId, min: "1", max: "120", step: "1" });
  const minuteErr = h("p", { class: "error" });
  minuteErr.setAttribute("role", "alert");
  const addBtn = h("button", { type: "button", class: "btn", textContent: "Add" });

  function renderChips(): void {
    chips.replaceChildren();
    for (const m of form?.escalateMinutes ?? []) {
      const rm = h("button", { type: "button", class: "chip-x", textContent: "×" });
      rm.setAttribute("aria-label", `Remove ${m} minutes`);
      rm.addEventListener("click", () => {
        if (!form) return;
        form.escalateMinutes = removeMinute(form.escalateMinutes, m);
        minuteErr.textContent = "";
        renderChips();
        emit();
      });
      chips.append(h("li", { class: "chip" }, h("span", { textContent: `${m} min` }), rm));
    }
    if (!chips.children.length) chips.append(h("li", { class: "hint", textContent: "Fire once, no reminders." }));
  }

  function submitMinute(): void {
    if (!form) return;
    const r = addMinute(form.escalateMinutes, minuteInput.value);
    if (!r.ok) {
      minuteErr.textContent = r.error;
      return;
    }
    minuteErr.textContent = "";
    form.escalateMinutes = r.list;
    minuteInput.value = "";
    renderChips();
    emit();
  }
  addBtn.addEventListener("click", submitMinute);
  minuteInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      submitMinute();
    }
  });

  const alerts = h(
    "section",
    { class: "card" },
    h("h2", { textContent: "Alerts" }),
    h("div", { class: "row" }, h("label", { htmlFor: volumeId, textContent: "Volume" }), h("span", { class: "inline" }, volume, volumeOut)),
    h("div", { class: "row" }, h("label", { htmlFor: pulsesId, textContent: "Glow pulses (1–20)" }), pulses),
    selectRow("Voice", voice),
    h("div", { class: "stack" }, h("label", { htmlFor: minuteId, textContent: "Remind me again after (minutes, up to 5)" }), chips),
    h("div", { class: "row" }, minuteInput, addBtn),
    minuteErr,
  );

  // ---- Avatar ----
  const pack = h("select");
  pack.append(h("option", { value: "", textContent: BUILTIN_PACK_TEXT }));
  bind(
    pack,
    (f) => (f.avatarPack = pack.value),
    (f) => {
      ensureOption(pack, f.avatarPack, f.avatarPack);
      pack.value = f.avatarPack;
    },
  );
  const style = h("select");
  style.append(
    h("option", { value: "peek", textContent: "Peek from the notch" }),
    h("option", { value: "pop", textContent: "Pop in at screen center" }),
  );
  bind(
    style,
    (f) => (f.mascotMode = style.value === "pop" ? "pop" : "peek"),
    (f) => (style.value = f.mascotMode),
  );
  const avatar = h(
    "section",
    { class: "card" },
    h("h2", { textContent: "Avatar" }),
    selectRow("Mascot style", style),
    selectRow("Avatar pack", pack),
    h("p", { class: "hint", textContent: AVATAR_HINT }),
  );

  // ---- Usage / DND ----
  const usage = h(
    "section",
    { class: "card" },
    h("h2", { textContent: "Usage" }),
    toggle("Live usage when no session is running", (f) => f.liveWhenIdle, (f, v) => (f.liveWhenIdle = v)),
    h("p", { class: "hint", textContent: USAGE_DISCLOSURE }),
  );
  const dnd = h(
    "section",
    { class: "card" },
    h("h2", { textContent: "Do Not Disturb" }),
    toggle("Do Not Disturb", (f) => f.dnd, (f, v) => (f.dnd = v)),
  );

  root.replaceChildren(
    h("main", { class: "settings" },
      h("h1", { textContent: "Nudge settings" }),
      notch,
      stateSection("done", "When a session finishes"),
      stateSection("blocked", "When a session needs you"),
      alerts,
      avatar,
      usage,
      dnd,
    ),
  );

  function fillSelect(sel: HTMLSelectElement, first: [string, string], names: string[]): void {
    const keep = sel.value;
    sel.replaceChildren(h("option", { value: first[0], textContent: first[1] }));
    for (const n of names) sel.append(h("option", { value: n, textContent: n }));
    if (keep !== first[0]) ensureOption(sel, keep, keep);
    sel.value = keep;
  }

  return {
    render(config) {
      const next = configToForm(config);
      const active = document.activeElement;
      // The focused control wins: carry its in-progress value into the new form.
      for (const b of bindings) if (b.el === active) b.read(next);
      form = next;
      for (const b of bindings) if (b.el !== active) b.write(next);
      renderChips();
    },
    setPacks(names) {
      packNames = names;
      fillSelect(pack, ["", BUILTIN_PACK_TEXT], packNames);
      if (form) {
        ensureOption(pack, form.avatarPack, form.avatarPack);
        pack.value = form.avatarPack;
      }
    },
    setVoices(names) {
      voiceNames = names;
      fillSelect(voice, ["", DEFAULT_VOICE_TEXT], voiceNames);
      if (form) {
        ensureOption(voice, form.ttsVoice, form.ttsVoice);
        voice.value = form.ttsVoice;
      }
    },
    form: () => (form ? structuredClone(form) : null),
  };
}
