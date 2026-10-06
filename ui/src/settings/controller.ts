// D24: Settings mounted inside the notch. Owns the debounced whole-config save
// that used to live in the separate Settings window, plus flush-on-close so
// leaving Settings never drops the last edit.

import { api, type AlertKind, type Config } from "../shared/contracts";
import { debounce, formToConfig, SAVE_DEBOUNCE_MS, type Form } from "./model";
import { createView } from "./view";

export interface SettingsController {
  /** Refresh controls from config (skipped when the config is unchanged). */
  show(config: Config): void;
  /** Save any pending edit now. */
  flush(): Promise<void>;
  /** Re-read avatar packs (call when the page opens). */
  refreshPacks(): void;
}

export function mountSettings(host: HTMLElement): SettingsController {
  let latest: Config | null = null;
  let shown = "";
  let pending: Form | null = null;

  const flush = (): Promise<void> => {
    save.cancel();
    const f = pending;
    pending = null;
    if (!f || !latest) return Promise.resolve();
    return api.setConfig(formToConfig(f, latest)).catch((e) => console.error("settings: save failed", e));
  };
  const save = debounce(() => void flush(), SAVE_DEBOUNCE_MS);

  const view = createView(host, {
    onChange: (f) => {
      pending = f;
      save();
    },
    // Send any pending edit first so the preview uses what's on screen.
    onPreview: (kind: AlertKind) => {
      void flush().then(() => api.previewAlert(kind)).catch((e) => console.error("settings: preview failed", e));
    },
  });

  if (typeof speechSynthesis !== "undefined") {
    const names = () => [...new Set(speechSynthesis.getVoices().map((v) => v.name))].sort();
    view.setVoices(names());
    speechSynthesis.addEventListener("voiceschanged", () => view.setVoices(names()));
  }

  return {
    show(config) {
      latest = config;
      // Snapshots arrive for every session event; only repaint on a config change.
      const key = JSON.stringify(config);
      if (key === shown) return;
      shown = key;
      view.render(config);
    },
    flush,
    refreshPacks() {
      api.listAvatarPacks().then(view.setPacks).catch((e) => console.error("settings: packs", e));
    },
  };
}
