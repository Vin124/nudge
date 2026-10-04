//! User config at `~/.nudge/config.json` (spec §Config). Missing or partial
//! files fall back to defaults field by field; unknown fields are ignored.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NotchConfig {
    pub edge: Edge,
    /// 0..1 position along the edge.
    pub offset: f64,
    /// Monitor name at save time; falls back to primary if it's gone (AC #7).
    pub monitor: Option<String>,
    pub show_weekly: bool,
}

impl Default for NotchConfig {
    fn default() -> Self {
        NotchConfig { edge: Edge::Right, offset: 0.4, monitor: None, show_weekly: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct StateAlert {
    pub glow: bool,
    /// Bundled sound id ("chime" | "ding" | "alarm" | "bell") or null for none.
    pub sound: Option<String>,
    pub tts: bool,
    pub avatar: bool,
    pub color: String,
}

impl StateAlert {
    fn done() -> Self {
        StateAlert { glow: true, sound: Some("chime".into()), tts: false, avatar: true, color: "#3ddc84".into() }
    }
    fn blocked() -> Self {
        StateAlert { glow: true, sound: Some("alarm".into()), tts: true, avatar: true, color: "#ff8a00".into() }
    }
}

impl Default for StateAlert {
    fn default() -> Self {
        Self::done()
    }
}

fn default_blocked() -> StateAlert {
    StateAlert::blocked()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AlertsConfig {
    pub done: StateAlert,
    #[serde(default = "default_blocked")]
    pub blocked: StateAlert,
    /// D8: minutes after the first alert to re-fire; empty = fire once.
    pub escalate_minutes: Vec<u32>,
    pub glow_pulses: u32,
    pub volume: f64,
    pub tts_voice: Option<String>,
    /// Name of a folder under `~/.nudge/avatars`, or null for the built-in mascot (D6).
    pub avatar_pack: Option<String>,
}

impl Default for AlertsConfig {
    fn default() -> Self {
        AlertsConfig {
            done: StateAlert::done(),
            blocked: StateAlert::blocked(),
            escalate_minutes: vec![2, 5],
            glow_pulses: 3,
            volume: 0.7,
            tts_voice: None,
            avatar_pack: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UsageConfig {
    /// D3 opt-in: read the local OAuth token and poll the usage endpoint.
    pub live_when_idle: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub version: u32,
    pub notch: NotchConfig,
    pub alerts: AlertsConfig,
    pub usage: UsageConfig,
    pub dnd: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            notch: NotchConfig::default(),
            alerts: AlertsConfig::default(),
            usage: UsageConfig::default(),
            dnd: false,
        }
    }
}

impl Config {
    /// Clamp values a hand-edited file could break.
    pub fn sanitized(mut self) -> Self {
        self.notch.offset = self.notch.offset.clamp(0.0, 1.0);
        self.alerts.volume = self.alerts.volume.clamp(0.0, 1.0);
        self.alerts.glow_pulses = self.alerts.glow_pulses.clamp(1, 20);
        self.alerts.escalate_minutes.retain(|m| (1..=120).contains(m));
        self.alerts.escalate_minutes.sort_unstable();
        self.alerts.escalate_minutes.dedup();
        self.alerts.escalate_minutes.truncate(5);
        self
    }

    pub fn load(path: &Path) -> Config {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let c = Config::load(Path::new("/definitely/not/here.json"));
        assert_eq!(c, Config::default());
        assert_eq!(c.alerts.blocked.color, "#ff8a00");
        assert_eq!(c.alerts.escalate_minutes, vec![2, 5]);
    }

    #[test]
    fn partial_file_fills_defaults_per_field() {
        let c: Config = serde_json::from_str(r#"{"dnd":true,"alerts":{"volume":0.2}}"#).unwrap();
        assert!(c.dnd);
        assert_eq!(c.alerts.volume, 0.2);
        assert_eq!(c.alerts.blocked, StateAlert::blocked());
        assert_eq!(c.notch.edge, Edge::Right);
    }

    #[test]
    fn sanitize_clamps_hostile_values() {
        let mut c = Config::default();
        c.notch.offset = 9.0;
        c.alerts.volume = -1.0;
        c.alerts.glow_pulses = 0;
        c.alerts.escalate_minutes = vec![0, 5, 5, 2, 999, 1, 3, 4, 6];
        let c = c.sanitized();
        assert_eq!(c.notch.offset, 1.0);
        assert_eq!(c.alerts.volume, 0.0);
        assert_eq!(c.alerts.glow_pulses, 1);
        assert_eq!(c.alerts.escalate_minutes, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn corrupt_file_gives_defaults_and_roundtrip_works() {
        let dir = std::env::temp_dir().join(format!("nudge-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.json");
        std::fs::write(&p, b"{oops").unwrap();
        assert_eq!(Config::load(&p), Config::default());
        let mut c = Config::default();
        c.notch.edge = Edge::Top;
        c.save(&p).unwrap();
        assert_eq!(Config::load(&p), c);
    }

    #[test]
    fn serializes_camel_case() {
        let v = serde_json::to_value(Config::default()).unwrap();
        assert!(v["notch"]["showWeekly"].as_bool().unwrap());
        assert!(v["alerts"]["escalateMinutes"].is_array());
        assert!(v["usage"]["liveWhenIdle"].is_boolean());
    }
}
