//! `nudge-hook`: run by Claude Code for every hook event with the hook JSON on
//! stdin. Forwards a whitelisted subset to the Nudge app and exits 0.
//!
//! Contract (D15, AC #10):
//! - never writes to stdout (SessionStart/UserPromptSubmit stdout is injected
//!   into Claude's context);
//! - always exits 0 — bad JSON, app down, timeout and panics included;
//! - stderr only when `NUDGE_DEBUG=1`.

use nudge_proto::{ancestors, now_ms, post, HookEnvelope, MAX_BODY_BYTES, PATH_HOOK, PROTOCOL_VERSION};
use serde_json::{Map, Value};
use std::io::Read;
use std::time::Instant;

/// Stop reading stdin after this many bytes (tool payloads can be huge).
const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024;
/// Keys forwarded verbatim (D15). Everything else — notably `tool_input` and
/// `tool_response` — is dropped.
const FORWARD_KEYS: &[&str] =
    &["session_id", "cwd", "hook_event_name", "notification_type", "transcript_path", "source", "reason"];
const MAX_MESSAGE_CHARS: usize = 500;
/// Defensive cap for the other forwarded strings (paths, ids).
const MAX_FIELD_CHARS: usize = 2048;
/// Only these events carry a fresh ancestor chain; the app keeps the last
/// non-empty list (session.rs `refresh_identity`). See `ancestors_for`.
const ANCESTOR_EVENTS: &[&str] = &["SessionStart", "UserPromptSubmit"];

fn debug() -> bool {
    std::env::var_os("NUDGE_DEBUG").is_some_and(|v| v == "1")
}

macro_rules! dbg_err {
    ($($t:tt)*) => { if debug() { eprintln!("nudge-hook: {}", format!($($t)*)); } };
}

fn truncate(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// D15: keep only the whitelisted string fields, plus `message` cut to 500 chars.
fn filter_payload(raw: &Value) -> Value {
    let mut out = Map::new();
    let Some(obj) = raw.as_object() else { return Value::Object(out) };
    for key in FORWARD_KEYS {
        if let Some(s) = obj.get(*key).and_then(Value::as_str) {
            out.insert((*key).to_string(), Value::String(truncate(s, MAX_FIELD_CHARS)));
        }
    }
    if let Some(s) = obj.get("message").and_then(Value::as_str) {
        out.insert("message".into(), Value::String(truncate(s, MAX_MESSAGE_CHARS)));
    }
    Value::Object(out)
}

/// Process-tree walk is the expensive part of a run on Windows; only the
/// events that open a turn need it (the app keeps the last non-empty list).
fn ancestors_for(event: &str) -> Vec<nudge_proto::ProcInfo> {
    if ANCESTOR_EVENTS.contains(&event) {
        ancestors(8)
    } else {
        Vec::new()
    }
}

/// Serialized envelope, or `None` if the input is unusable or too big.
fn build_body(stdin: &[u8]) -> Option<Vec<u8>> {
    let raw: Value = serde_json::from_slice(stdin)
        .map_err(|e| dbg_err!("invalid JSON on stdin: {e}"))
        .ok()?;
    let payload = filter_payload(&raw);
    let event = payload.get("hook_event_name").and_then(Value::as_str).unwrap_or("");
    let t = Instant::now();
    let ancestors = ancestors_for(event);
    dbg_err!("ancestors: {} entries in {:?}", ancestors.len(), t.elapsed());
    let env = HookEnvelope { v: PROTOCOL_VERSION, ts_ms: now_ms(), ancestors, payload };
    let body = serde_json::to_vec(&env).ok()?;
    if body.len() >= MAX_BODY_BYTES {
        dbg_err!("envelope too large ({} bytes), dropped", body.len());
        return None;
    }
    Some(body)
}

fn run() {
    let start = Instant::now();
    let mut buf = Vec::new();
    if let Err(e) = std::io::stdin().lock().take(MAX_STDIN_BYTES).read_to_end(&mut buf) {
        dbg_err!("stdin read failed: {e}");
        return;
    }
    if let Some(body) = build_body(&buf) {
        match post(PATH_HOOK, &body) {
            Ok(code) => dbg_err!("posted, HTTP {code}"),
            Err(e) => dbg_err!("post failed (app down?): {e}"),
        }
    }
    dbg_err!("total {:?}", start.elapsed());
}

fn main() {
    // Silence the default panic message unless debugging; never let a panic
    // change the exit code (AC #10).
    std::panic::set_hook(Box::new(|info| dbg_err!("panic: {info}")));
    let _ = std::panic::catch_unwind(run);
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn whitelist_strips_tool_payloads_and_keeps_seven_keys() {
        let raw = json!({
            "session_id": "s1", "cwd": "C:/p", "hook_event_name": "PostToolUse",
            "notification_type": "permission_prompt", "transcript_path": "/t.jsonl",
            "source": "startup", "reason": "clear",
            "tool_input": {"command": "ls"}, "tool_response": {"stdout": "x"},
            "permission_mode": "default", "prompt": "secret prompt"
        });
        let out = filter_payload(&raw);
        let keys: Vec<&str> = out.as_object().unwrap().keys().map(String::as_str).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        let mut want = FORWARD_KEYS.to_vec();
        want.sort_unstable();
        assert_eq!(sorted, want);
        assert_eq!(out["session_id"], "s1");
        assert_eq!(out["notification_type"], "permission_prompt");
    }

    #[test]
    fn message_truncated_to_500_chars() {
        let long = "é".repeat(2000);
        let out = filter_payload(&json!({"message": long}));
        assert_eq!(out["message"].as_str().unwrap().chars().count(), 500);
        let short = filter_payload(&json!({"message": "hi"}));
        assert_eq!(short["message"], "hi");
    }

    #[test]
    fn non_object_and_non_string_values_are_dropped() {
        assert_eq!(filter_payload(&json!([1, 2])), json!({}));
        assert_eq!(filter_payload(&json!({"session_id": 5, "cwd": null})), json!({}));
    }

    #[test]
    fn one_megabyte_tool_response_yields_small_envelope() {
        let raw = json!({
            "session_id": "s1", "cwd": "/p", "hook_event_name": "PostToolUse",
            "tool_response": {"stdout": "x".repeat(1024 * 1024)}
        });
        let input = serde_json::to_vec(&raw).unwrap();
        assert!(input.len() > 1024 * 1024);
        let body = build_body(&input).expect("body");
        assert!(body.len() < 64 * 1024, "body was {} bytes", body.len());
        let env: HookEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(env.payload["hook_event_name"], "PostToolUse");
        assert!(env.payload.get("tool_response").is_none());
        assert!(env.ancestors.is_empty(), "tool events skip the process walk");
    }

    #[test]
    fn invalid_json_is_dropped_without_panic() {
        assert!(build_body(b"{not json").is_none());
        assert!(build_body(b"").is_none());
    }

    #[test]
    fn session_start_carries_ancestors() {
        let body = build_body(br#"{"session_id":"s","hook_event_name":"SessionStart"}"#).unwrap();
        let env: HookEnvelope = serde_json::from_slice(&body).unwrap();
        assert!(!env.ancestors.is_empty() && env.ancestors.len() <= 8);
    }
}
