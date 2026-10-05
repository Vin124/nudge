//! Session state machine (spec §Session state machine). Orchestrator-owned spine:
//! every alert decision downstream trusts the transitions emitted here.

use nudge_proto::{HookEnvelope, ProcInfo};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Idle,
    Running,
    Blocked,
    Done,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub project: String,
    pub cwd: String,
    pub state: SessionState,
    /// When `state` was entered (ms since epoch, from the hook timestamp).
    pub since_ms: u64,
    /// True from entering Done/Blocked until acknowledged (D8).
    pub alert_pending: bool,
    /// D22: share of the context window in use, 0..100, from this session's statusline.
    pub context_percent: Option<f64>,
    #[serde(skip)]
    pub claude_pid: Option<u32>,
    #[serde(skip)]
    pub ancestors: Vec<ProcInfo>,
    #[serde(skip)]
    last_event_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Transition {
    Changed { id: String, project: String, from: Option<SessionState>, to: SessionState },
    Acked { id: String },
    Removed { id: String },
}

impl Transition {
    pub fn id(&self) -> &str {
        match self {
            Transition::Changed { id, .. } | Transition::Acked { id } | Transition::Removed { id } => id,
        }
    }
    /// True when this transition should fire an alert (D2).
    pub fn is_alert(&self) -> bool {
        matches!(self, Transition::Changed { to: SessionState::Done | SessionState::Blocked, .. })
    }
}

/// D13: notification types that mean "Claude is waiting on the user".
pub const BLOCKING_NOTIFICATIONS: &[&str] = &[
    "permission_prompt",
    "elicitation_dialog",
    "elicitation_url_dialog",
    "agent_needs_input",
    "worker_permission_prompt",
];

#[derive(Default)]
pub struct SessionStore {
    sessions: BTreeMap<String, Session>,
}

fn project_of(cwd: &str) -> String {
    let trimmed = cwd.trim_end_matches(['/', '\\']);
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("session")
        .to_string()
}

/// First ancestor that looks like the Claude Code process. Native installs run
/// `claude(.exe)`; npm installs run under `node(.exe)`.
fn find_claude_pid(ancestors: &[ProcInfo]) -> Option<u32> {
    let lower = |p: &ProcInfo| p.name.to_ascii_lowercase();
    ancestors
        .iter()
        .find(|p| lower(p).starts_with("claude"))
        .or_else(|| ancestors.iter().find(|p| lower(p).starts_with("node")))
        .map(|p| p.pid)
}

impl SessionStore {
    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    pub fn snapshot(&self) -> Vec<Session> {
        let mut v: Vec<Session> = self.sessions.values().cloned().collect();
        v.sort_by(|a, b| a.project.cmp(&b.project).then(a.id.cmp(&b.id)));
        v
    }

    pub fn apply_hook(&mut self, env: &HookEnvelope) -> Vec<Transition> {
        let p = &env.payload;
        let Some(id) = p.get("session_id").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) else {
            return vec![];
        };
        let event = p.get("hook_event_name").and_then(|v| v.as_str()).unwrap_or("");
        let cwd = p.get("cwd").and_then(|v| v.as_str()).unwrap_or("");

        if event == "SessionEnd" {
            return match self.sessions.remove(id) {
                Some(_) => vec![Transition::Removed { id: id.to_string() }],
                None => vec![],
            };
        }

        let target = match event {
            "SessionStart" => None, // create-only; never changes an existing state (compact/resume fire it mid-session)
            "UserPromptSubmit" => Some(SessionState::Running),
            "PreToolUse" | "PostToolUse" => Some(SessionState::Running),
            "Stop" => Some(SessionState::Done),
            "Notification" => {
                let ty = p.get("notification_type").and_then(|v| v.as_str()).unwrap_or("");
                if BLOCKING_NOTIFICATIONS.contains(&ty) {
                    Some(SessionState::Blocked)
                } else {
                    return self.touch(id, cwd, env);
                }
            }
            _ => return self.touch(id, cwd, env),
        };

        let mut out = Vec::new();
        let created = !self.sessions.contains_key(id);
        if created {
            self.sessions.insert(id.to_string(), new_session(id, cwd, env));
            out.push(Transition::Changed {
                id: id.to_string(),
                project: project_of(cwd),
                from: None,
                to: SessionState::Idle,
            });
        }
        let s = self.sessions.get_mut(id).expect("present");
        // Out-of-order guard: hooks run as separate processes and can race.
        if !created && env.ts_ms < s.last_event_ms {
            return out;
        }
        s.last_event_ms = env.ts_ms;
        refresh_identity(s, cwd, env);

        let Some(to) = target else { return out };
        // Tool hooks only resume a blocked/idle session; they never undo Done
        // (a late PostToolUse must not swallow a finished alert).
        if matches!(event, "PreToolUse" | "PostToolUse")
            && !matches!(s.state, SessionState::Blocked | SessionState::Idle)
        {
            return out;
        }
        if s.state == to {
            return out;
        }
        let from = s.state;
        s.state = to;
        s.since_ms = env.ts_ms;
        s.alert_pending = matches!(to, SessionState::Done | SessionState::Blocked);
        out.push(Transition::Changed { id: id.to_string(), project: s.project.clone(), from: Some(from), to });
        out
    }

    /// Unknown events still register the session (sessions started before install).
    fn touch(&mut self, id: &str, cwd: &str, env: &HookEnvelope) -> Vec<Transition> {
        if let Some(s) = self.sessions.get_mut(id) {
            if env.ts_ms >= s.last_event_ms {
                s.last_event_ms = env.ts_ms;
                refresh_identity(s, cwd, env);
            }
            return vec![];
        }
        self.sessions.insert(id.to_string(), new_session(id, cwd, env));
        vec![Transition::Changed { id: id.to_string(), project: project_of(cwd), from: None, to: SessionState::Idle }]
    }

    /// D8 acknowledgement. Returns Acked only if an alert was pending.
    /// Record a session's context usage. Unknown sessions are ignored (hooks create sessions).
    /// Returns whether the snapshot changed.
    pub fn apply_context(&mut self, id: &str, percent: Option<f64>) -> bool {
        let Some(s) = self.sessions.get_mut(id) else { return false };
        let p = percent.map(|p| p.clamp(0.0, 100.0));
        // null (no API reply yet, or after /clear) clears it: a stale value would mislead.
        if s.context_percent == p {
            return false;
        }
        s.context_percent = p;
        true
    }

    pub fn ack(&mut self, id: &str) -> Option<Transition> {
        let s = self.sessions.get_mut(id)?;
        if !s.alert_pending {
            return None;
        }
        s.alert_pending = false;
        Some(Transition::Acked { id: id.to_string() })
    }

    /// Remove sessions whose Claude process is gone (AC #3: within 10s).
    pub fn sweep(&mut self, is_alive: impl Fn(u32) -> bool) -> Vec<Transition> {
        let dead: Vec<String> = self
            .sessions
            .values()
            .filter(|s| s.claude_pid.is_some_and(|pid| !is_alive(pid)))
            .map(|s| s.id.clone())
            .collect();
        dead.into_iter()
            .map(|id| {
                self.sessions.remove(&id);
                Transition::Removed { id }
            })
            .collect()
    }
}

fn new_session(id: &str, cwd: &str, env: &HookEnvelope) -> Session {
    Session {
        id: id.to_string(),
        project: project_of(cwd),
        cwd: cwd.to_string(),
        state: SessionState::Idle,
        since_ms: env.ts_ms,
        alert_pending: false,
        context_percent: None,
        claude_pid: find_claude_pid(&env.ancestors),
        ancestors: env.ancestors.clone(),
        last_event_ms: env.ts_ms,
    }
}

fn refresh_identity(s: &mut Session, cwd: &str, env: &HookEnvelope) {
    if !cwd.is_empty() && s.cwd != cwd {
        s.cwd = cwd.to_string();
        s.project = project_of(cwd);
    }
    if !env.ancestors.is_empty() {
        s.ancestors = env.ancestors.clone();
        if let Some(pid) = find_claude_pid(&env.ancestors) {
            s.claude_pid = Some(pid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(id: &str, event: &str, ts: u64) -> HookEnvelope {
        ev_with(id, event, ts, json!({}))
    }

    fn ev_with(id: &str, event: &str, ts: u64, extra: serde_json::Value) -> HookEnvelope {
        let mut payload = json!({"session_id": id, "hook_event_name": event, "cwd": "/home/u/api-refactor"});
        if let (Some(p), Some(e)) = (payload.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                p.insert(k.clone(), v.clone());
            }
        }
        HookEnvelope {
            v: 1,
            ts_ms: ts,
            ancestors: vec![
                ProcInfo { pid: 10, name: "bash.exe".into() },
                ProcInfo { pid: 20, name: "claude.exe".into() },
                ProcInfo { pid: 30, name: "WindowsTerminal.exe".into() },
            ],
            payload,
        }
    }

    fn state(st: &SessionStore, id: &str) -> SessionState {
        st.get(id).unwrap().state
    }

    #[test]
    fn full_lifecycle_emits_expected_transitions() {
        let mut st = SessionStore::default();
        let t = st.apply_hook(&ev("a", "SessionStart", 1));
        assert_eq!(t, vec![Transition::Changed { id: "a".into(), project: "api-refactor".into(), from: None, to: SessionState::Idle }]);
        let t = st.apply_hook(&ev("a", "UserPromptSubmit", 2));
        assert_eq!(t.len(), 1);
        assert_eq!(state(&st, "a"), SessionState::Running);
        let t = st.apply_hook(&ev("a", "Stop", 3));
        assert!(t[0].is_alert());
        assert_eq!(state(&st, "a"), SessionState::Done);
        assert!(st.get("a").unwrap().alert_pending);
        let t = st.apply_hook(&ev("a", "SessionEnd", 4));
        assert_eq!(t, vec![Transition::Removed { id: "a".into() }]);
        assert!(st.get("a").is_none());
    }

    #[test]
    fn blocking_notification_types_block_and_alert() {
        for ty in BLOCKING_NOTIFICATIONS {
            let mut st = SessionStore::default();
            st.apply_hook(&ev("a", "UserPromptSubmit", 1));
            let t = st.apply_hook(&ev_with("a", "Notification", 2, json!({"notification_type": ty})));
            assert!(t[0].is_alert(), "{ty}");
            assert_eq!(state(&st, "a"), SessionState::Blocked);
        }
    }

    #[test]
    fn idle_prompt_and_unknown_notifications_do_not_change_state() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "Stop", 1));
        for ty in ["idle_prompt", "auth_success", "agent_completed", "push_notification", ""] {
            let t = st.apply_hook(&ev_with("a", "Notification", 2, json!({"notification_type": ty})));
            assert!(t.is_empty(), "{ty}");
        }
        assert_eq!(state(&st, "a"), SessionState::Done);
    }

    #[test]
    fn tool_use_resumes_blocked_session_without_alert() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "UserPromptSubmit", 1));
        st.apply_hook(&ev_with("a", "Notification", 2, json!({"notification_type": "permission_prompt"})));
        let t = st.apply_hook(&ev("a", "PostToolUse", 3));
        assert_eq!(state(&st, "a"), SessionState::Running);
        assert!(!t[0].is_alert());
        assert!(!st.get("a").unwrap().alert_pending);
    }

    #[test]
    fn late_tool_hook_does_not_undo_done() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "UserPromptSubmit", 1));
        st.apply_hook(&ev("a", "Stop", 5));
        let t = st.apply_hook(&ev("a", "PostToolUse", 6));
        assert!(t.is_empty());
        assert_eq!(state(&st, "a"), SessionState::Done);
    }

    #[test]
    fn out_of_order_event_is_ignored() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "UserPromptSubmit", 10));
        st.apply_hook(&ev("a", "Stop", 20));
        let t = st.apply_hook(&ev("a", "UserPromptSubmit", 15));
        assert!(t.is_empty());
        assert_eq!(state(&st, "a"), SessionState::Done);
    }

    #[test]
    fn duplicate_stop_alerts_once() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "UserPromptSubmit", 1));
        assert_eq!(st.apply_hook(&ev("a", "Stop", 2)).len(), 1);
        assert!(st.apply_hook(&ev("a", "Stop", 3)).is_empty());
    }

    #[test]
    fn session_start_mid_session_keeps_state() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "UserPromptSubmit", 1));
        assert!(st.apply_hook(&ev("a", "SessionStart", 2)).is_empty());
        assert_eq!(state(&st, "a"), SessionState::Running);
    }

    #[test]
    fn first_event_from_unknown_session_registers_it() {
        let mut st = SessionStore::default();
        let t = st.apply_hook(&ev("pre", "Stop", 1));
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], Transition::Changed { id: "pre".into(), project: "api-refactor".into(), from: None, to: SessionState::Idle });
        assert!(t[1].is_alert());
        let t = st.apply_hook(&ev("other", "SubagentStop", 1));
        assert_eq!(t.len(), 1);
        assert_eq!(state(&st, "other"), SessionState::Idle);
    }

    #[test]
    fn missing_session_id_is_ignored() {
        let mut st = SessionStore::default();
        let mut e = ev("a", "Stop", 1);
        e.payload.as_object_mut().unwrap().remove("session_id");
        assert!(st.apply_hook(&e).is_empty());
        let mut e = ev("", "Stop", 1);
        e.payload["session_id"] = json!("");
        assert!(st.apply_hook(&e).is_empty());
        assert!(st.snapshot().is_empty());
    }

    #[test]
    fn prompt_submit_clears_pending_alert() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "Stop", 1));
        st.apply_hook(&ev("a", "UserPromptSubmit", 2));
        assert!(!st.get("a").unwrap().alert_pending);
    }

    #[test]
    fn context_percent_tracks_statusline_and_clears_on_null() {
        let mut st = SessionStore::default();
        assert!(!st.apply_context("a", Some(10.0)), "unknown sessions are ignored");
        st.apply_hook(&ev("a", "SessionStart", 1));
        assert!(st.apply_context("a", Some(42.0)));
        assert!(!st.apply_context("a", Some(42.0)), "no change, no snapshot");
        assert_eq!(st.get("a").unwrap().context_percent, Some(42.0));
        assert!(st.apply_context("a", Some(140.0)));
        assert_eq!(st.get("a").unwrap().context_percent, Some(100.0));
        assert!(st.apply_context("a", None));
        assert_eq!(st.get("a").unwrap().context_percent, None);
    }

    #[test]
    fn ack_only_when_pending() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "UserPromptSubmit", 1));
        assert_eq!(st.ack("a"), None);
        st.apply_hook(&ev("a", "Stop", 2));
        assert_eq!(st.ack("a"), Some(Transition::Acked { id: "a".into() }));
        assert_eq!(st.ack("a"), None);
        assert_eq!(st.ack("missing"), None);
        assert_eq!(state(&st, "a"), SessionState::Done);
    }

    #[test]
    fn sweep_removes_only_dead_claude_processes() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("alive", "SessionStart", 1));
        let mut e = ev("dead", "SessionStart", 1);
        e.ancestors = vec![ProcInfo { pid: 99, name: "claude".into() }];
        st.apply_hook(&e);
        let mut e = ev("nopid", "SessionStart", 1);
        e.ancestors = vec![ProcInfo { pid: 5, name: "zsh".into() }];
        st.apply_hook(&e);
        let t = st.sweep(|pid| pid != 99);
        assert_eq!(t, vec![Transition::Removed { id: "dead".into() }]);
        assert!(st.get("alive").is_some() && st.get("nopid").is_some());
    }

    #[test]
    fn claude_pid_prefers_claude_then_node() {
        let a = |n: &str, p| ProcInfo { pid: p, name: n.into() };
        assert_eq!(find_claude_pid(&[a("sh", 1), a("node.exe", 2), a("Claude.exe", 3)]), Some(3));
        assert_eq!(find_claude_pid(&[a("sh", 1), a("node", 2)]), Some(2));
        assert_eq!(find_claude_pid(&[a("sh", 1)]), None);
    }

    #[test]
    fn project_names_from_paths() {
        assert_eq!(project_of("/home/u/api-refactor"), "api-refactor");
        assert_eq!(project_of("C:\\Users\\Vin\\proj\\"), "proj");
        assert_eq!(project_of(""), "session");
    }

    #[test]
    fn snapshot_serializes_camel_case_without_internal_fields() {
        let mut st = SessionStore::default();
        st.apply_hook(&ev("a", "Stop", 7));
        let v = serde_json::to_value(st.snapshot()).unwrap();
        let s = &v[0];
        assert_eq!(s["state"], "done");
        assert_eq!(s["sinceMs"], 7);
        assert_eq!(s["alertPending"], true);
        assert!(s.get("claudePid").is_none() && s.get("ancestors").is_none());
    }
}
