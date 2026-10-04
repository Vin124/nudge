//! Nudge app wiring. Orchestrator-owned: lanes plug in through `alerts`,
//! `focus`, `usage_poll` and `tray`, never by editing this file.

pub mod alerts;
mod commands;
pub mod config;
pub mod hub;
pub mod focus;
pub mod server;
pub mod session;
mod tray;
pub mod usage;
mod usage_poll;

use crate::alerts::AlertEngine;
use crate::config::Config;
use crate::hub::{Core, Sink, Snapshot};
use crate::session::Transition;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_SNAPSHOT: &str = "nudge://snapshot";
/// AC #3: dead sessions disappear within 10s.
const SWEEP_EVERY: Duration = Duration::from_secs(5);

struct TauriSink {
    app: AppHandle,
    alerts: OnceLock<Arc<AlertEngine>>,
}

impl Sink for TauriSink {
    fn snapshot(&self, snap: &Snapshot) {
        let _ = self.app.emit(EVENT_SNAPSHOT, snap);
    }
    fn transitions(&self, ts: &[Transition], cfg: &Config) {
        if let Some(a) = self.alerts.get() {
            a.on_transitions(ts, cfg);
        }
    }
}

fn spawn_sweeper(core: Arc<Core>) {
    std::thread::Builder::new()
        .name("nudge-sweep".into())
        .spawn(move || {
            use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
            loop {
                std::thread::sleep(SWEEP_EVERY);
                let pids: Vec<Pid> = core.claude_pids().into_iter().map(Pid::from_u32).collect();
                if pids.is_empty() {
                    continue;
                }
                // Fresh System each pass: a reused one can keep entries for dead pids.
                let mut sys = System::new();
                sys.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), ProcessRefreshKind::new());
                core.sweep(|pid| sys.process(Pid::from_u32(pid)).is_some());
            }
        })
        .expect("spawn sweeper");
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let sink = Arc::new(TauriSink { app: handle.clone(), alerts: OnceLock::new() });
            let dir = nudge_proto::nudge_dir();
            let core = Arc::new(Core::new(dir.join("config.json"), sink.clone()));

            let alerts = Arc::new(AlertEngine::init(&handle)?);
            let _ = sink.alerts.set(alerts.clone());
            app.manage(core.clone());
            app.manage(alerts);

            let c = core.clone();
            let (rt, _join) = server::start(server::generate_token(), Arc::new(move |m| c.handle(m)))?;
            server::write_runtime(&dir, &rt)?;

            spawn_sweeper(core.clone());
            usage_poll::spawn(core.clone());
            tray::init(&handle, core)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::ack_session,
            commands::focus_session,
            commands::set_config,
            commands::preview_alert,
            commands::list_avatar_packs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nudge");
}
