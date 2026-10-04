//! App hub: owns session/usage/config state, turns inbound messages into
//! snapshots + transitions. Tauri-free so it is unit-testable; the Tauri side
//! implements `Sink`.

use crate::config::Config;
use crate::server::Inbound;
use crate::session::{Session, SessionStore, Transition};
use crate::usage::{UsageSnapshot, UsageStore, UsageWindow};
use nudge_proto::ProcInfo;
use serde::Serialize;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub sessions: Vec<Session>,
    pub usage: UsageSnapshot,
    pub config: Config,
}

pub trait Sink: Send + Sync {
    fn snapshot(&self, snap: &Snapshot);
    fn transitions(&self, ts: &[Transition], cfg: &Config);
}

struct State {
    sessions: SessionStore,
    usage: UsageStore,
    config: Config,
}

pub struct Core {
    state: Mutex<State>,
    sink: Arc<dyn Sink>,
    config_path: PathBuf,
}

impl Core {
    pub fn new(config_path: PathBuf, sink: Arc<dyn Sink>) -> Self {
        let config = Config::load(&config_path);
        Core {
            state: Mutex::new(State { sessions: SessionStore::default(), usage: UsageStore::default(), config }),
            sink,
            config_path,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // A panic while holding the lock must not take the whole app down.
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn snap_of(st: &State) -> Snapshot {
        Snapshot { sessions: st.sessions.snapshot(), usage: st.usage.snapshot().clone(), config: st.config.clone() }
    }

    pub fn snapshot(&self) -> Snapshot {
        Self::snap_of(&self.lock())
    }

    pub fn config(&self) -> Config {
        self.lock().config.clone()
    }

    /// Sink calls happen after the lock is released (no re-entrancy deadlocks).
    fn publish(&self, ts: Vec<Transition>, changed: bool) {
        if ts.is_empty() && !changed {
            return;
        }
        let (snap, cfg) = {
            let st = self.lock();
            (Self::snap_of(&st), st.config.clone())
        };
        if !ts.is_empty() {
            self.sink.transitions(&ts, &cfg);
        }
        self.sink.snapshot(&snap);
    }

    pub fn handle(&self, msg: Inbound) {
        let (ts, changed) = {
            let mut st = self.lock();
            match msg {
                Inbound::Hook(h) => (st.sessions.apply_hook(&h), false),
                Inbound::Status(s) => (vec![], st.usage.apply_status(&s)),
            }
        };
        self.publish(ts, changed);
    }

    pub fn ack(&self, id: &str) {
        let t = self.lock().sessions.ack(id);
        self.publish(t.into_iter().collect(), false);
    }

    pub fn sweep(&self, is_alive: impl Fn(u32) -> bool) {
        let ts = self.lock().sessions.sweep(is_alive);
        self.publish(ts, false);
    }

    pub fn claude_pids(&self) -> Vec<u32> {
        self.lock().sessions.snapshot().iter().filter_map(|s| s.claude_pid).collect()
    }

    pub fn session_ancestors(&self, id: &str) -> Option<Vec<ProcInfo>> {
        self.lock().sessions.get(id).map(|s| s.ancestors.clone())
    }

    pub fn apply_poll(&self, five: Option<UsageWindow>, seven: Option<UsageWindow>, ts_ms: u64) {
        let changed = self.lock().usage.apply_poll(five, seven, ts_ms);
        self.publish(vec![], changed);
    }

    pub fn set_config(&self, cfg: Config) -> io::Result<()> {
        let cfg = cfg.sanitized();
        cfg.save(&self.config_path)?;
        self.lock().config = cfg;
        self.publish(vec![], true);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nudge_proto::HookEnvelope;
    use serde_json::json;

    #[derive(Default)]
    struct Rec {
        snaps: Mutex<Vec<Snapshot>>,
        ts: Mutex<Vec<Transition>>,
    }
    impl Sink for Rec {
        fn snapshot(&self, s: &Snapshot) {
            self.snaps.lock().unwrap().push(s.clone());
        }
        fn transitions(&self, t: &[Transition], _: &Config) {
            self.ts.lock().unwrap().extend_from_slice(t);
        }
    }

    fn core() -> (Core, Arc<Rec>) {
        let rec = Arc::new(Rec::default());
        let p = std::env::temp_dir().join(format!("nudge-core-{}-{:?}.json", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_file(&p);
        (Core::new(p, rec.clone()), rec)
    }

    fn hook(id: &str, ev: &str, ts: u64) -> Inbound {
        Inbound::Hook(HookEnvelope {
            v: 1,
            ts_ms: ts,
            ancestors: vec![ProcInfo { pid: 42, name: "claude".into() }],
            payload: json!({"session_id": id, "hook_event_name": ev, "cwd": "/p/x"}),
        })
    }

    #[test]
    fn hook_publishes_transitions_then_snapshot() {
        let (c, rec) = core();
        c.handle(hook("a", "Stop", 1));
        assert_eq!(rec.ts.lock().unwrap().len(), 2);
        let snaps = rec.snaps.lock().unwrap();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].sessions[0].id, "a");
    }

    #[test]
    fn no_op_event_publishes_nothing() {
        let (c, rec) = core();
        c.handle(hook("a", "Stop", 1));
        c.handle(hook("a", "Stop", 2));
        assert_eq!(rec.snaps.lock().unwrap().len(), 1);
    }

    #[test]
    fn ack_and_sweep_publish() {
        let (c, rec) = core();
        c.handle(hook("a", "Stop", 1));
        c.ack("a");
        assert!(matches!(rec.ts.lock().unwrap().last(), Some(Transition::Acked { .. })));
        assert_eq!(c.claude_pids(), vec![42]);
        c.sweep(|_| false);
        assert!(matches!(rec.ts.lock().unwrap().last(), Some(Transition::Removed { .. })));
        assert!(c.snapshot().sessions.is_empty());
    }

    #[test]
    fn set_config_sanitizes_persists_and_publishes() {
        let (c, rec) = core();
        let mut cfg = Config::default();
        cfg.alerts.volume = 5.0;
        cfg.dnd = true;
        c.set_config(cfg).unwrap();
        assert_eq!(c.config().alerts.volume, 1.0);
        assert!(rec.snaps.lock().unwrap().last().unwrap().config.dnd);
    }
}
