//! Pure D8 escalation scheduler. No Tauri types: the clock is injected so the
//! whole thing is unit-testable.

use super::AlertKind;
use crate::config::Config;
use crate::session::{SessionState, Transition};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fire {
    pub session_id: String,
    pub project: String,
    pub kind: AlertKind,
    /// 0 = the first alert, k >= 1 = k-th escalation.
    pub escalation: u32,
}

struct Pending {
    project: String,
    kind: AlertKind,
    first: Instant,
    /// Escalation delays in minutes, captured when the alert was raised.
    minutes: Vec<u32>,
    /// Index into `minutes` of the next escalation to fire.
    next: usize,
}

impl Pending {
    fn due_at(&self) -> Option<Instant> {
        self.minutes.get(self.next).map(|m| self.first + Duration::from_secs(u64::from(*m) * 60))
    }
}

#[derive(Default)]
pub struct Scheduler {
    timers: HashMap<String, Pending>,
}

fn kind_of(t: &Transition) -> Option<AlertKind> {
    match t {
        Transition::Changed { to: SessionState::Done, .. } => Some(AlertKind::Done),
        Transition::Changed { to: SessionState::Blocked, .. } => Some(AlertKind::Blocked),
        _ => None,
    }
}

impl Scheduler {
    /// Apply a batch of transitions. Returns the fires to emit right now.
    /// Any transition for a session first cancels its pending escalations;
    /// an alert transition then starts a fresh schedule (AC #6, D8).
    pub fn apply(&mut self, ts: &[Transition], cfg: &Config, now: Instant) -> Vec<Fire> {
        let mut out = Vec::new();
        for t in ts {
            self.timers.remove(t.id());
            if !t.is_alert() {
                continue;
            }
            let (Some(kind), Transition::Changed { id, project, .. }) = (kind_of(t), t) else {
                continue;
            };
            let minutes = cfg.alerts.escalate_minutes.clone();
            if !minutes.is_empty() {
                self.timers.insert(id.clone(), Pending { project: project.clone(), kind, first: now, minutes, next: 0 });
            }
            // DND is checked at fire time; the schedule above stays armed.
            if !cfg.dnd {
                out.push(Fire { session_id: id.clone(), project: project.clone(), kind, escalation: 0 });
            }
        }
        out
    }

    /// Return the escalations due at `now`. If several are overdue for one
    /// session (e.g. after sleep) only the latest fires. DND drops the fire
    /// but the schedule is still consumed.
    pub fn tick(&mut self, now: Instant, cfg: &Config) -> Vec<Fire> {
        let mut out = Vec::new();
        self.timers.retain(|id, p| {
            let mut last = None;
            while p.due_at().is_some_and(|d| d <= now) {
                p.next += 1;
                last = Some(p.next as u32);
            }
            if let Some(escalation) = last {
                if !cfg.dnd {
                    out.push(Fire { session_id: id.clone(), project: p.project.clone(), kind: p.kind, escalation });
                }
            }
            p.due_at().is_some()
        });
        out.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        out
    }

    /// One-off Settings "Test": ignores DND, schedules nothing.
    pub fn preview(kind: AlertKind) -> Fire {
        Fire { session_id: "preview".into(), project: "my-project".into(), kind, escalation: 0 }
    }

    #[cfg(test)]
    fn pending(&self) -> usize {
        self.timers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(id: &str, to: SessionState) -> Transition {
        Transition::Changed { id: id.into(), project: format!("proj-{id}"), from: None, to }
    }
    fn mins(m: u64) -> Duration {
        Duration::from_secs(m * 60)
    }

    #[test]
    fn alert_fires_now_then_escalates_at_2m_and_5m_then_stops() {
        let (mut s, cfg, t0) = (Scheduler::default(), Config::default(), Instant::now());
        let f = s.apply(&[changed("a", SessionState::Done)], &cfg, t0);
        assert_eq!(f, vec![Fire { session_id: "a".into(), project: "proj-a".into(), kind: AlertKind::Done, escalation: 0 }]);
        assert!(s.tick(t0 + mins(1), &cfg).is_empty());
        let f = s.tick(t0 + mins(2), &cfg);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].escalation, 1);
        let f = s.tick(t0 + mins(5), &cfg);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].escalation, 2);
        assert!(s.tick(t0 + mins(60), &cfg).is_empty());
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn acked_running_and_removed_each_cancel_escalations() {
        for cancel in [
            Transition::Acked { id: "a".into() },
            changed("a", SessionState::Running),
            Transition::Removed { id: "a".into() },
        ] {
            let (mut s, cfg, t0) = (Scheduler::default(), Config::default(), Instant::now());
            s.apply(&[changed("a", SessionState::Blocked)], &cfg, t0);
            assert!(s.apply(std::slice::from_ref(&cancel), &cfg, t0).is_empty());
            assert!(s.tick(t0 + mins(10), &cfg).is_empty(), "{cancel:?} should cancel");
        }
    }

    #[test]
    fn second_alert_replaces_schedule() {
        let (mut s, cfg, t0) = (Scheduler::default(), Config::default(), Instant::now());
        s.apply(&[changed("a", SessionState::Done)], &cfg, t0);
        let f = s.apply(&[changed("a", SessionState::Blocked)], &cfg, t0 + mins(1));
        assert_eq!(f[0].kind, AlertKind::Blocked);
        assert_eq!(s.pending(), 1);
        // Old schedule would fire at t0+2m; the new one at t0+3m.
        assert!(s.tick(t0 + mins(2), &cfg).is_empty());
        let f = s.tick(t0 + mins(3), &cfg);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, AlertKind::Blocked);
    }

    #[test]
    fn sessions_are_independent() {
        let (mut s, cfg, t0) = (Scheduler::default(), Config::default(), Instant::now());
        s.apply(&[changed("a", SessionState::Done)], &cfg, t0);
        s.apply(&[changed("b", SessionState::Done)], &cfg, t0 + mins(1));
        s.apply(&[Transition::Acked { id: "a".into() }], &cfg, t0 + mins(1));
        let f = s.tick(t0 + mins(3), &cfg);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].session_id, "b");
    }

    #[test]
    fn empty_escalation_list_fires_once() {
        let (mut s, mut cfg, t0) = (Scheduler::default(), Config::default(), Instant::now());
        cfg.alerts.escalate_minutes = vec![];
        assert_eq!(s.apply(&[changed("a", SessionState::Done)], &cfg, t0).len(), 1);
        assert_eq!(s.pending(), 0);
        assert!(s.tick(t0 + mins(60), &cfg).is_empty());
    }

    #[test]
    fn dnd_drops_fire_at_fire_time_and_lifting_it_resumes() {
        let (mut s, mut cfg, t0) = (Scheduler::default(), Config::default(), Instant::now());
        cfg.dnd = true;
        assert!(s.apply(&[changed("a", SessionState::Done)], &cfg, t0).is_empty());
        assert!(s.tick(t0 + mins(2), &cfg).is_empty(), "dropped but consumed");
        cfg.dnd = false;
        let f = s.tick(t0 + mins(5), &cfg);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].escalation, 2);
    }

    #[test]
    fn done_and_blocked_map_to_kinds() {
        let (mut s, cfg, t0) = (Scheduler::default(), Config::default(), Instant::now());
        let f = s.apply(&[changed("a", SessionState::Done), changed("b", SessionState::Blocked)], &cfg, t0);
        assert_eq!(f[0].kind, AlertKind::Done);
        assert_eq!(f[1].kind, AlertKind::Blocked);
    }

    #[test]
    fn preview_bypasses_dnd_and_schedules_nothing() {
        let s = Scheduler::default();
        let f = Scheduler::preview(AlertKind::Blocked);
        assert_eq!(f.kind, AlertKind::Blocked);
        assert_eq!(f.escalation, 0);
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn non_alert_transitions_produce_no_fires() {
        let (mut s, cfg, t0) = (Scheduler::default(), Config::default(), Instant::now());
        let ts = [
            changed("a", SessionState::Idle),
            changed("a", SessionState::Running),
            Transition::Acked { id: "a".into() },
            Transition::Removed { id: "a".into() },
        ];
        assert!(s.apply(&ts, &cfg, t0).is_empty());
        assert_eq!(s.pending(), 0);
    }
}
