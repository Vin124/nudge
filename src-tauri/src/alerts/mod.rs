//! Alert engine (lane L3). FOUNDATION STUB: signatures are the contract;
//! L3 replaces the bodies (glow/avatar windows, sound+TTS, D8 escalation).

use crate::config::Config;
use crate::session::Transition;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertKind {
    Done,
    Blocked,
}

pub struct AlertEngine {
    #[allow(dead_code)]
    app: AppHandle,
}

impl AlertEngine {
    /// Create alert windows (one glow per monitor, one avatar) and timers.
    pub fn init(app: &AppHandle) -> tauri::Result<Self> {
        Ok(AlertEngine { app: app.clone() })
    }

    /// Called by Core with every batch of transitions (after state lock is released).
    pub fn on_transitions(&self, _ts: &[Transition], _cfg: &Config) {}

    /// Fire a one-off alert from Settings ("Test" button). Ignores DND.
    pub fn preview(&self, _kind: AlertKind, _cfg: &Config) {}
}
