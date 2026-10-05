//! Alert engine (lane L3): glow + avatar windows, sound/TTS payloads and the
//! D8 escalation timers. The scheduler (`schedule.rs`) is pure; this file wires
//! it to Tauri windows and the 250 ms ticker.

mod schedule;
mod windows;

use crate::config::{Config, Edge, MascotMode, StateAlert};
use crate::hub::Core;
use crate::session::Transition;
use schedule::{Fire, Scheduler};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Listener, Manager};

pub const EVENT_ALERT: &str = "nudge://alert";
/// Alert pages emit this (payload = their window label) when they are finished.
pub const EVENT_HIDE: &str = "nudge://alert-hide";
const TICK: Duration = Duration::from_millis(250);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertKind {
    Done,
    Blocked,
}

impl AlertKind {
    fn file_stem(self) -> &'static str {
        match self {
            AlertKind::Done => "done",
            AlertKind::Blocked => "blocked",
        }
    }
}

/// Payload of the `nudge://alert` event. TS mirror: `ui/src/alerts-shared/types.ts`.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlertFire {
    pub session_id: String,
    pub project: String,
    pub kind: AlertKind,
    pub escalation: u32,
    pub color: String,
    pub glow: bool,
    pub pulses: u32,
    pub sound: Option<String>,
    pub tts: bool,
    pub tts_voice: Option<String>,
    pub volume: f64,
    pub avatar: bool,
    pub avatar_src: Option<String>,
    /// Where the notch is docked; the mascot peeks out from that side.
    pub edge: Edge,
    pub mascot_mode: MascotMode,
}

struct Inner {
    app: AppHandle,
    sched: Mutex<Scheduler>,
}

pub struct AlertEngine {
    inner: Arc<Inner>,
}

/// `~/.nudge/avatars/<pack>/<kind>.(gif|png|svg)` if the pack is set and the file exists.
fn avatar_src(avatars_dir: &std::path::Path, pack: Option<&str>, kind: AlertKind) -> Option<String> {
    let pack = pack?;
    // The pack name comes from config: it must be a single plain folder name.
    if pack.is_empty() || pack == "." || pack.contains(['/', '\\', ':']) || pack.contains("..") {
        return None;
    }
    let dir: PathBuf = avatars_dir.join(pack);
    ["gif", "png", "svg"]
        .iter()
        .map(|ext| dir.join(format!("{}.{ext}", kind.file_stem())))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

fn build_payload(fire: &Fire, cfg: &Config, avatars_dir: &std::path::Path) -> AlertFire {
    let st: &StateAlert = match fire.kind {
        AlertKind::Done => &cfg.alerts.done,
        AlertKind::Blocked => &cfg.alerts.blocked,
    };
    AlertFire {
        session_id: fire.session_id.clone(),
        project: fire.project.clone(),
        kind: fire.kind,
        escalation: fire.escalation,
        color: st.color.clone(),
        glow: st.glow,
        pulses: cfg.alerts.glow_pulses,
        sound: st.sound.clone(),
        tts: st.tts,
        tts_voice: cfg.alerts.tts_voice.clone(),
        volume: cfg.alerts.volume,
        avatar: st.avatar,
        avatar_src: avatar_src(avatars_dir, cfg.alerts.avatar_pack.as_deref(), fire.kind),
        edge: cfg.notch.edge,
        mascot_mode: cfg.alerts.mascot_mode,
    }
}

impl Inner {
    /// Show the enabled windows and emit the event. Never called with the
    /// scheduler lock held.
    fn emit(&self, fire: &Fire, cfg: &Config) {
        let payload = build_payload(fire, cfg, &nudge_proto::nudge_dir().join("avatars"));
        if payload.glow {
            // Re-enumerate on every fire so hot-plugged monitors are covered.
            for label in windows::sync_glow(&self.app) {
                if let Some(w) = self.app.get_webview_window(&label) {
                    windows::show_no_focus(&w); // never set_focus()/show(): alerts must not steal typing focus
                }
                let _ = self.app.emit_to(label, EVENT_ALERT, &payload);
            }
        }
        if let Some(avatar) = self.app.get_webview_window(windows::AVATAR_LABEL) {
            if payload.avatar {
                windows::place_avatar(&self.app, &avatar, payload.mascot_mode);
                windows::show_no_focus(&avatar);
            }
            // The avatar page owns audio even when it stays hidden, so exactly one page plays sound.
            let _ = self.app.emit_to(windows::AVATAR_LABEL, EVENT_ALERT, &payload);
        }
    }

    fn emit_all(&self, fires: &[Fire], cfg: &Config) {
        for f in fires {
            self.emit(f, cfg);
        }
    }
}

impl AlertEngine {
    /// Create alert windows (one glow per monitor, one avatar) and timers.
    pub fn init(app: &AppHandle) -> tauri::Result<Self> {
        windows::sync_glow(app);
        windows::build_avatar(app)?;
        let a = app.clone();
        app.listen(EVENT_HIDE, move |e| {
            let label: String = serde_json::from_str(e.payload()).unwrap_or_default();
            if label == windows::AVATAR_LABEL || label.starts_with("glow-") {
                if let Some(w) = a.get_webview_window(&label) {
                    windows::hide_window(&w);
                }
            }
        });
        let inner = Arc::new(Inner { app: app.clone(), sched: Mutex::new(Scheduler::default()) });

        let t = inner.clone();
        std::thread::Builder::new()
            .name("nudge-alert-tick".into())
            .spawn(move || loop {
                std::thread::sleep(TICK);
                // Fresh config each tick so DND toggles apply at fire time. Core is
                // managed just after init; until then there is nothing to tick.
                let Some(core) = t.app.try_state::<Arc<Core>>() else { continue };
                let cfg = core.config();
                let due = t.sched.lock().expect("alert scheduler").tick(Instant::now(), &cfg);
                t.emit_all(&due, &cfg);
            })?;
        Ok(AlertEngine { inner })
    }

    /// Called by Core with every batch of transitions (after state lock is released).
    pub fn on_transitions(&self, ts: &[Transition], cfg: &Config) {
        let fires = self.inner.sched.lock().expect("alert scheduler").apply(ts, cfg, Instant::now());
        self.inner.emit_all(&fires, cfg);
    }

    /// Fire a one-off alert from Settings ("Test" button). Ignores DND.
    pub fn preview(&self, kind: AlertKind, cfg: &Config) {
        self.inner.emit(&Scheduler::preview(kind), cfg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_serializes_camel_case_with_kind_settings() {
        let cfg = Config::default();
        let f = Fire { session_id: "s".into(), project: "p".into(), kind: AlertKind::Blocked, escalation: 1 };
        let p = build_payload(&f, &cfg, std::path::Path::new("nonexistent"));
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["sessionId"], "s");
        assert_eq!(v["kind"], "blocked");
        assert_eq!(v["color"], cfg.alerts.blocked.color);
        assert_eq!(v["sound"], "alarm");
        assert_eq!(v["pulses"], 3);
        assert!(v["avatarSrc"].is_null());
    }

    #[test]
    fn avatar_src_prefers_gif_then_png_then_svg_and_rejects_traversal() {
        let dir = std::env::temp_dir().join(format!("nudge-avatar-test-{}", std::process::id()));
        let pack = dir.join("p");
        std::fs::create_dir_all(&pack).unwrap();
        assert_eq!(avatar_src(&dir, Some("p"), AlertKind::Done), None);
        std::fs::write(pack.join("done.svg"), "x").unwrap();
        assert!(avatar_src(&dir, Some("p"), AlertKind::Done).unwrap().ends_with("done.svg"));
        std::fs::write(pack.join("done.png"), "x").unwrap();
        assert!(avatar_src(&dir, Some("p"), AlertKind::Done).unwrap().ends_with("done.png"));
        std::fs::write(pack.join("done.gif"), "x").unwrap();
        assert!(avatar_src(&dir, Some("p"), AlertKind::Done).unwrap().ends_with("done.gif"));
        assert_eq!(avatar_src(&dir, Some("p"), AlertKind::Blocked), None);
        assert_eq!(avatar_src(&dir, Some("../p"), AlertKind::Done), None);
        assert_eq!(avatar_src(&dir, None, AlertKind::Done), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
