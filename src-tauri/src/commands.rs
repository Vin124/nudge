//! Tauri commands: the full UI → core API. TS mirror: ui/src/shared/contracts.ts.

use crate::alerts::{AlertEngine, AlertKind};
use crate::config::Config;
use crate::hub::{Core, Snapshot};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_snapshot(core: State<'_, Arc<Core>>) -> Snapshot {
    core.snapshot()
}

#[tauri::command]
pub fn ack_session(core: State<'_, Arc<Core>>, id: String) {
    core.ack(&id);
}

/// D7: focus the session's terminal; clicking always acknowledges (D8).
#[tauri::command]
pub fn focus_session(core: State<'_, Arc<Core>>, id: String) -> Result<(), String> {
    core.ack(&id);
    let ancestors = core.session_ancestors(&id).ok_or("unknown session")?;
    crate::focus::focus_session(&ancestors)
}

#[tauri::command]
pub fn set_config(core: State<'_, Arc<Core>>, config: Config) -> Result<(), String> {
    core.set_config(config).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn preview_alert(core: State<'_, Arc<Core>>, alerts: State<'_, Arc<AlertEngine>>, kind: AlertKind) {
    alerts.preview(kind, &core.config());
}

/// D6: names of custom avatar packs under `~/.nudge/avatars/`.
#[tauri::command]
pub fn list_avatar_packs() -> Vec<String> {
    let dir = nudge_proto::nudge_dir().join("avatars");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}
