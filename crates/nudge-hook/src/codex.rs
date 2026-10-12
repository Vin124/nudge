//! Codex usage from the session transcript (D28). Codex has no statusline, so
//! on `Stop` / `PostToolUse` the hook reads the tail of `transcript_path`,
//! takes the last `token_count` event and turns it into a `StatusEnvelope`.
//!
//! Privacy (D28): only numbers leave this module. The transcript holds the
//! user's prompts and tool output; no string from it is ever copied out.

use nudge_proto::{now_ms, Agent, LimitWindow, StatusEnvelope, PROTOCOL_VERSION};
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Never read more than this from the end of a transcript (D28).
pub const TAIL_BYTES: u64 = 256 * 1024;
/// Events after which Codex has written a fresh `token_count` (D28).
const USAGE_EVENTS: &[&str] = &["Stop", "PostToolUse"];

/// The numbers taken from one `token_count` event.
#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub limit_windows: Option<Vec<LimitWindow>>,
    pub context_used_percentage: Option<f64>,
}

impl Usage {
    fn is_empty(&self) -> bool {
        self.context_used_percentage.is_none() && self.limit_windows.as_ref().map_or(true, Vec::is_empty)
    }
}

/// Last `token_count` usage in the last `TAIL_BYTES` of a regular `.jsonl`
/// file; `None` on any I/O or parse failure, or when nothing usable is found
/// (fail soft, AC #10).
pub fn read_usage(path: &Path) -> Option<Usage> {
    let is_jsonl = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("jsonl"));
    if !is_jsonl {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    let start = meta.len().saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut buf).ok()?;
    let tail = if start > 0 {
        // Started mid-file: the first line is partial, drop it.
        let nl = buf.iter().position(|&b| b == b'\n')?;
        &buf[nl + 1..]
    } else {
        &buf[..]
    };
    // Last to first; unparsable lines (e.g. a half-written final line) are skipped.
    let payload = tail.split(|&b| b == b'\n').rev().find_map(token_count_payload)?;
    let usage = usage_from(&payload);
    (!usage.is_empty()).then_some(usage)
}

/// `payload` of a `{"type":"event_msg","payload":{"type":"token_count",..}}` line.
fn token_count_payload(line: &[u8]) -> Option<Value> {
    let mut v: Value = serde_json::from_slice(line).ok()?;
    if v.get("type")?.as_str()? != "event_msg" {
        return None;
    }
    let payload = v.get_mut("payload")?.take();
    (payload.get("type")?.as_str()? == "token_count").then_some(payload)
}

fn usage_from(payload: &Value) -> Usage {
    let limit_windows = payload.get("rate_limits").filter(|r| r.is_object()).map(|r| {
        ["primary", "secondary"].iter().filter_map(|k| r.get(*k).and_then(limit_window)).collect()
    });
    Usage { limit_windows, context_used_percentage: context_pct(payload.get("info")) }
}

/// D28: entries without a finite `used_percent` or a u32 `window_minutes` are dropped.
fn limit_window(w: &Value) -> Option<LimitWindow> {
    let used_percentage = w.get("used_percent")?.as_f64().filter(|p| p.is_finite())?;
    let window_minutes = u32::try_from(w.get("window_minutes")?.as_u64()?).ok()?;
    let resets_at = w.get("resets_at").and_then(Value::as_i64);
    Some(LimitWindow { used_percentage, window_minutes, resets_at })
}

/// `last_token_usage.total_tokens / model_context_window × 100`, clamped 0..=100.
fn context_pct(info: Option<&Value>) -> Option<f64> {
    let info = info?;
    let used = info.get("last_token_usage")?.get("total_tokens")?.as_f64()?;
    let window = info.get("model_context_window")?.as_f64()?;
    if !(used > 0.0 && window > 0.0 && used.is_finite() && window.is_finite()) {
        return None;
    }
    Some((used / window * 100.0).clamp(0.0, 100.0))
}

/// D28: the serialized `StatusEnvelope` for a Codex hook payload, or `None`
/// when the event carries no usage, the payload lacks `session_id` /
/// `transcript_path`, or the transcript yields nothing usable.
pub fn status_body(raw: &Value) -> Option<Vec<u8>> {
    let event = raw.get("hook_event_name")?.as_str()?;
    if !USAGE_EVENTS.contains(&event) {
        return None;
    }
    let session_id = raw.get("session_id")?.as_str()?;
    let transcript = raw.get("transcript_path")?.as_str()?;
    let usage = read_usage(Path::new(transcript))?;
    let env = StatusEnvelope {
        v: PROTOCOL_VERSION,
        ts_ms: now_ms(),
        agent: Agent::Codex,
        session_id: Some(session_id.to_owned()),
        rate_limits_available: None,
        rate_limits: None,
        context_used_percentage: usage.context_used_percentage,
        limit_windows: usage.limit_windows,
    };
    serde_json::to_vec(&env).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    /// Real line from Codex CLI 0.154.0 (2026-10-12).
    const REAL: &str = r#"{"timestamp":"2026-10-12T06:07:52.702Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":17929,"cached_input_tokens":11520,"cache_write_input_tokens":0,"output_tokens":153,"reasoning_output_tokens":11,"total_tokens":18082},"last_token_usage":{"input_tokens":17929,"cached_input_tokens":11520,"cache_write_input_tokens":0,"output_tokens":153,"reasoning_output_tokens":11,"total_tokens":18082},"model_context_window":258400},"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":0.0,"window_minutes":300,"resets_at":1790155491},"secondary":{"used_percent":0.0,"window_minutes":10080,"resets_at":1790742291},"credits":{"has_credits":false,"unlimited":false,"balance":"0"}}}}"#;
    const USER_MSG: &str = r#"{"timestamp":"2026-10-12T06:07:50.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"SECRET-PROMPT-123"}]}}"#;
    const TASK_COMPLETE: &str = r#"{"type":"event_msg","payload":{"type":"task_complete","last_agent_message":"done"}}"#;

    fn token_count(used: f64, window: u32) -> String {
        json!({"type":"event_msg","payload":{"type":"token_count","info":null,
            "rate_limits":{"primary":{"used_percent":used,"window_minutes":window},"secondary":null}}})
        .to_string()
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nudge-codex-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn write(name: &str, content: &[u8]) -> PathBuf {
        let p = tmp(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn real_token_count_line() {
        let p = write("real.jsonl", format!("{USER_MSG}\n{REAL}\n{TASK_COMPLETE}\n").as_bytes());
        let u = read_usage(&p).unwrap();
        assert_eq!(
            u.limit_windows.unwrap(),
            vec![
                LimitWindow { used_percentage: 0.0, window_minutes: 300, resets_at: Some(1790155491) },
                LimitWindow { used_percentage: 0.0, window_minutes: 10080, resets_at: Some(1790742291) },
            ]
        );
        let want = 18082.0 / 258400.0 * 100.0;
        assert!((u.context_used_percentage.unwrap() - want).abs() < 1e-9);
    }

    #[test]
    fn thirty_day_shape_with_null_secondary_has_one_window() {
        let line = r#"{"type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex","primary":{"used_percent":0,"window_minutes":43200,"resets_at":1794377116},"secondary":null}}}"#;
        let u = read_usage(&write("30d.jsonl", line.as_bytes())).unwrap();
        assert_eq!(
            u.limit_windows.unwrap(),
            vec![LimitWindow { used_percentage: 0.0, window_minutes: 43200, resets_at: Some(1794377116) }]
        );
    }

    #[test]
    fn picks_last_token_count_and_ignores_half_written_line() {
        let content = format!("{}\n{USER_MSG}\n{}\n{TASK_COMPLETE}\n{{\"type\":\"event_msg\",\"payl", token_count(10.0, 300), token_count(55.5, 300));
        let u = read_usage(&write("last.jsonl", content.as_bytes())).unwrap();
        assert_eq!(u.limit_windows.unwrap()[0].used_percentage, 55.5);
    }

    #[test]
    fn crlf_lines_parse() {
        let u = read_usage(&write("crlf.jsonl", format!("{REAL}\r\n{USER_MSG}\r\n").as_bytes())).unwrap();
        assert_eq!(u.limit_windows.unwrap().len(), 2);
    }

    /// A line of filler ~1 KiB, so files can be sized precisely.
    fn filler(n_lines: usize) -> String {
        let line = format!("{{\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"text\":\"{}\"}}}}\n", "x".repeat(1000));
        line.repeat(n_lines)
    }

    #[test]
    fn large_file_token_count_in_tail_is_found() {
        let content = format!("{}{}\n{}", filler(600), token_count(42.0, 300), filler(100));
        assert!(content.len() as u64 > 2 * TAIL_BYTES);
        let u = read_usage(&write("big-hit.jsonl", content.as_bytes())).unwrap();
        assert_eq!(u.limit_windows.unwrap()[0].used_percentage, 42.0);
    }

    #[test]
    fn large_file_token_count_before_tail_is_not_read() {
        let content = format!("{}{}\n{}", filler(10), token_count(42.0, 300), filler(300));
        assert!(content.len() as u64 > TAIL_BYTES);
        let p = write("big-miss.jsonl", content.as_bytes());
        assert_eq!(read_usage(&p), None);
        // Same line within a small file is found: the bound is the only difference.
        let small = format!("{}{}\n{}", filler(10), token_count(42.0, 300), filler(10));
        assert!(read_usage(&write("small-hit.jsonl", small.as_bytes())).is_some());
    }

    #[test]
    fn null_rate_limits_keeps_context_and_null_info_drops_it() {
        let no_limits = REAL.replace(
            r#""rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":0.0,"window_minutes":300,"resets_at":1790155491},"secondary":{"used_percent":0.0,"window_minutes":10080,"resets_at":1790742291},"credits":{"has_credits":false,"unlimited":false,"balance":"0"}}"#,
            r#""rate_limits":null"#,
        );
        assert_ne!(no_limits, REAL);
        let u = read_usage(&write("nolimits.jsonl", no_limits.as_bytes())).unwrap();
        assert_eq!(u.limit_windows, None);
        assert!(u.context_used_percentage.is_some());

        let u = read_usage(&write("noinfo.jsonl", token_count(5.0, 300).as_bytes())).unwrap();
        assert_eq!(u.context_used_percentage, None);
        assert_eq!(u.limit_windows.unwrap().len(), 1);
    }

    #[test]
    fn nothing_usable_is_none() {
        let both_null = r#"{"type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":null}}"#;
        assert_eq!(read_usage(&write("bothnull.jsonl", both_null.as_bytes())), None);
        assert_eq!(read_usage(&write("nocount.jsonl", format!("{USER_MSG}\n{TASK_COMPLETE}\n").as_bytes())), None);
        assert_eq!(read_usage(&write("empty.jsonl", b"")), None);
    }

    #[test]
    fn bad_paths_are_none() {
        assert_eq!(read_usage(&write("real.txt", REAL.as_bytes())), None, "non-.jsonl");
        let dir = tmp("dir.jsonl");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(read_usage(&dir), None, "directory");
        assert_eq!(read_usage(&tmp("missing.jsonl")), None, "missing");
        assert!(read_usage(&write("UPPER.JSONL", REAL.as_bytes())).is_some(), "extension is case-insensitive");
    }

    #[test]
    fn status_body_carries_only_numbers() {
        let p = write("privacy.jsonl", format!("{USER_MSG}\n{REAL}\n").as_bytes());
        let raw = json!({"session_id": "sess-1", "hook_event_name": "Stop", "transcript_path": p.to_str().unwrap()});
        let body = status_body(&raw).unwrap();
        let text = String::from_utf8(body.clone()).unwrap();
        assert!(!text.contains("SECRET-PROMPT-123"));
        assert!(!text.contains("limit_id") && !text.contains("balance") && !text.contains("2026-10-12"));
        let env: StatusEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(env.agent, Agent::Codex);
        assert_eq!(env.session_id.as_deref(), Some("sess-1"));
        assert_eq!(env.rate_limits, None);
        assert_eq!(env.rate_limits_available, None);
        assert_eq!(env.limit_windows.unwrap().len(), 2);
    }

    #[test]
    fn status_body_only_for_usage_events_with_ids() {
        let p = write("events.jsonl", REAL.as_bytes());
        let path = p.to_str().unwrap();
        for ev in ["SessionStart", "UserPromptSubmit", "PreToolUse", "PermissionRequest"] {
            assert!(status_body(&json!({"session_id": "s", "hook_event_name": ev, "transcript_path": path})).is_none(), "{ev}");
        }
        assert!(status_body(&json!({"session_id": "s", "hook_event_name": "PostToolUse", "transcript_path": path})).is_some());
        assert!(status_body(&json!({"hook_event_name": "Stop", "transcript_path": path})).is_none());
        assert!(status_body(&json!({"session_id": "s", "hook_event_name": "Stop"})).is_none());
        assert!(status_body(&json!({"session_id": 5, "hook_event_name": "Stop", "transcript_path": path})).is_none());
    }
}
