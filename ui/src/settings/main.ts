// Tauri glue for the settings window: snapshot in, debounced whole-config saves out.

import { api, onSnapshot, type AlertKind, type Config } from "../shared/contracts";
import { debounce, formToConfig, SAVE_DEBOUNCE_MS, type Form } from "./model";
import { createView } from "./view";
import "./settings.css";

let latest: Config | null = null;
let pending: Form | null = null;

const flush = () => {
  const f = pending;
  pending = null;
  if (!f || !latest) return Promise.resolve();
  return api.setConfig(formToConfig(f, latest)).catch((e) => console.error("save failed", e));
};
const save = debounce(() => void flush(), SAVE_DEBOUNCE_MS);

const view = createView(document.getElementById("app")!, {
  onChange: (f) => {
    pending = f;
    save();
  },
  // Send any pending edit first so the preview uses what's on screen.
  onPreview: (kind: AlertKind) => {
    save.cancel();
    void flush().then(() => api.previewAlert(kind)).catch((e) => console.error("preview failed", e));
  },
});

const loadPacks = () => api.listAvatarPacks().then(view.setPacks).catch((e) => console.error("packs", e));
const loadVoices = () => {
  if (typeof speechSynthesis === "undefined") return;
  const names = () => [...new Set(speechSynthesis.getVoices().map((v) => v.name))].sort();
  view.setVoices(names());
  speechSynthesis.addEventListener("voiceschanged", () => view.setVoices(names()));
};

void onSnapshot((s) => {
  latest = s.config;
  view.render(s.config);
});
void loadPacks();
loadVoices();
// The window is hidden, not destroyed; pick up newly added packs when it's shown again.
window.addEventListener("focus", () => void loadPacks());
