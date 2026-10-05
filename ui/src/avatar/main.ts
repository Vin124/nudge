import "./avatar.css";
import { convertFileSrc } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ALERT_EVENT, ALERT_HIDE_EVENT, type AlertFire } from "../alerts-shared/types";
import { api } from "../shared/contracts";
import { playAlertAudio } from "./audio";
import { renderAvatar } from "./view";

const HIDE_AFTER_MS = 6000;
const FADE_MS = 450;
const app = document.getElementById("app")!;
let current: AlertFire | null = null;
let timer: number | undefined;

function hideSoon(delay: number): void {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    app.classList.add("leaving");
    window.setTimeout(() => {
      app.replaceChildren();
      void emit(ALERT_HIDE_EVENT, getCurrentWindow().label);
    }, FADE_MS);
  }, delay);
}

app.addEventListener("click", () => {
  const f = current;
  if (!f) return;
  window.clearTimeout(timer);
  // The session may be gone (or this is a preview); focus failures are expected.
  void api.focusSession(f.sessionId).catch(() => {});
  app.classList.add("leaving");
  void emit(ALERT_HIDE_EVENT, getCurrentWindow().label);
});

// Window-scoped: Rust emit_to()s each alert window; a global listen() would
// receive every window's copy (3x restarts, 3x sound).
void getCurrentWebviewWindow().listen<AlertFire>(ALERT_EVENT, (e) => {
  const f = e.payload;
  // Audio always plays here (this is the only page that does), even when the popup is off.
  void playAlertAudio(f);
  if (!f.avatar) return;
  current = f;
  app.classList.remove("leaving");
  renderAvatar(app, f, convertFileSrc);
  hideSoon(HIDE_AFTER_MS);
});
