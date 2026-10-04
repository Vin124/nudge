import "./glow.css";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ALERT_EVENT, ALERT_HIDE_EVENT, type AlertFire } from "../alerts-shared/types";
import { applyGlow } from "./glow";

const app = document.getElementById("app")!;
const edge = document.createElement("div");
edge.id = "edge";
app.appendChild(edge);

const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");

edge.addEventListener("animationend", () => {
  edge.removeAttribute("data-mode");
  void emit(ALERT_HIDE_EVENT, getCurrentWindow().label);
});

// Window-scoped: Rust emit_to()s each alert window; a global listen() would
// receive every window's copy (3x restarts, 3x sound).
void getCurrentWebviewWindow().listen<AlertFire>(ALERT_EVENT, (e) => {
  if (!e.payload.glow) return;
  applyGlow(edge, e.payload, reduced.matches);
});
