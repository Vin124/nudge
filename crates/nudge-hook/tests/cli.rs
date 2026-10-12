//! Black-box tests of the built `nudge-hook` binary (D15, AC #10).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

fn empty_nudge_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nudge-hook-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_hook(name: &str, stdin: &[u8]) -> (Output, Duration) {
    let home = empty_nudge_home(name);
    let out = run_hook_in(&home, &[], stdin);
    let _ = std::fs::remove_dir_all(&home);
    out
}

fn run_hook_in(home: &Path, args: &[&str], stdin: &[u8]) -> (Output, Duration) {
    // Warm-up: the first launch of a freshly built exe on Windows pays for an
    // AV scan, which is not the steady-state cost AC #10 is about.
    let _ = Command::new(env!("CARGO_BIN_EXE_nudge-hook"))
        .env("NUDGE_HOME", home)
        .stdin(Stdio::null())
        .output();
    let t = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_nudge-hook"))
        .args(args)
        .env("NUDGE_HOME", home)
        .env_remove("NUDGE_DEBUG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A write error (child exited early) is acceptable; the exit code is what matters.
    let _ = child.stdin.take().unwrap().write_all(stdin);
    let out = child.wait_with_output().unwrap();
    (out, t.elapsed())
}

fn assert_silent_success(out: &Output) {
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty(), "stdout must be empty, got {:?}", out.stdout);
    assert!(out.stderr.is_empty(), "stderr must be empty without NUDGE_DEBUG");
}

#[test]
fn app_down_session_start_exits_zero_fast_and_silent() {
    let (out, elapsed) = run_hook(
        "down",
        br#"{"session_id":"s","cwd":"/p","hook_event_name":"SessionStart","source":"startup"}"#,
    );
    assert_silent_success(&out);
    assert!(elapsed < Duration::from_millis(300), "took {elapsed:?}");
}

#[test]
fn app_down_tool_event_exits_zero_fast_and_silent() {
    let (out, elapsed) = run_hook("tool", br#"{"session_id":"s","hook_event_name":"PreToolUse","tool_input":{}}"#);
    assert_silent_success(&out);
    assert!(elapsed < Duration::from_millis(300), "took {elapsed:?}");
}

#[test]
fn invalid_json_exits_zero_silent() {
    let (out, _) = run_hook("badjson", b"\xff\xfe not json at all");
    assert_silent_success(&out);
}

#[test]
fn empty_stdin_exits_zero_silent() {
    let (out, _) = run_hook("empty", b"");
    assert_silent_success(&out);
}

#[test]
fn app_down_codex_stop_exits_zero_fast_and_silent() {
    let home = empty_nudge_home("codex-down");
    let transcript = home.join("t.jsonl");
    std::fs::write(&transcript, TOKEN_COUNT).unwrap();
    let stdin = stop_payload(&transcript);
    let (out, elapsed) = run_hook_in(&home, &["--agent", "codex"], stdin.as_bytes());
    let _ = std::fs::remove_dir_all(&home);
    assert_silent_success(&out);
    assert!(elapsed < Duration::from_millis(300), "took {elapsed:?}");
}

/// Real `token_count` line from Codex CLI 0.154.0 (D28).
const TOKEN_COUNT: &str = r#"{"timestamp":"2026-10-12T06:07:52.702Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":17929,"cached_input_tokens":11520,"cache_write_input_tokens":0,"output_tokens":153,"reasoning_output_tokens":11,"total_tokens":18082},"last_token_usage":{"input_tokens":17929,"cached_input_tokens":11520,"cache_write_input_tokens":0,"output_tokens":153,"reasoning_output_tokens":11,"total_tokens":18082},"model_context_window":258400},"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":0.0,"window_minutes":300,"resets_at":1790155491},"secondary":{"used_percent":0.0,"window_minutes":10080,"resets_at":1790742291},"credits":{"has_credits":false,"unlimited":false,"balance":"0"}}}}"#;

fn stop_payload(transcript: &Path) -> String {
    serde_json::json!({
        "session_id": "sess-codex", "cwd": "/p", "hook_event_name": "Stop",
        "transcript_path": transcript.to_str().unwrap()
    })
    .to_string()
}

/// Fake app: replies 204 to every request and records `(path, body)`.
struct FakeApp {
    port: u16,
    stop: Arc<AtomicBool>,
    seen: Arc<Mutex<Vec<(String, String)>>>,
    thread: Option<JoinHandle<()>>,
}

impl FakeApp {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (stop2, seen2) = (stop.clone(), seen.clone());
        let thread = std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Some(req) = handle(stream) {
                            seen2.lock().unwrap().push(req);
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        FakeApp { port, stop, seen, thread: Some(thread) }
    }

    fn write_runtime(&self, home: &Path) {
        let rt = format!(r#"{{"port":{},"token":"t","pid":1}}"#, self.port);
        std::fs::write(home.join("runtime.json"), rt).unwrap();
    }

    /// Stop accepting and return what was received. The hook waits for each
    /// response, so once it has exited every request is already recorded.
    fn finish(mut self) -> Vec<(String, String)> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        std::mem::take(&mut *self.seen.lock().unwrap())
    }
}

fn handle(mut stream: TcpStream) -> Option<(String, String)> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let path = head.split_whitespace().nth(1)?.to_string();
    let len: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .and_then(|v| v.trim().parse().ok())?;
    while buf.len() < head_end + len {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[head_end..head_end + len]).to_string();
    let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
    Some((path, body))
}

#[test]
fn codex_stop_posts_hook_then_status_d28() {
    let home = empty_nudge_home("codex-up");
    let app = FakeApp::start();
    app.write_runtime(&home);
    // ~5 MB transcript (with a prompt that must not leak) ending in a token_count.
    let transcript = home.join("rollout.jsonl");
    let filler = format!(
        "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":[{{\"type\":\"input_text\",\"text\":\"SECRET-PROMPT-123 {}\"}}]}}}}\n",
        "x".repeat(1000)
    );
    std::fs::write(&transcript, format!("{}{TOKEN_COUNT}\n{}", filler.repeat(5000), filler.repeat(3))).unwrap();
    let (out, elapsed) = run_hook_in(&home, &["--agent", "codex"], stop_payload(&transcript).as_bytes());
    let seen = app.finish();
    let _ = std::fs::remove_dir_all(&home);
    eprintln!("codex Stop with 5 MB transcript, app up: {elapsed:?}");
    assert_silent_success(&out);
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_eq!(seen[0].0, "/v1/hook");
    let hook: serde_json::Value = serde_json::from_str(&seen[0].1).unwrap();
    assert_eq!(hook["agent"], "codex");
    assert_eq!(seen[1].0, "/v1/status");
    assert!(!seen[1].1.contains("SECRET-PROMPT-123"));
    let status: serde_json::Value = serde_json::from_str(&seen[1].1).unwrap();
    assert_eq!(status["agent"], "codex");
    assert_eq!(status["session_id"], "sess-codex");
    assert_eq!(status["limit_windows"][0]["window_minutes"], 300);
    assert_eq!(status["limit_windows"][1]["window_minutes"], 10080);
    assert!(status["context_used_percentage"].as_f64().unwrap() > 6.9);
}

#[test]
fn claude_stop_posts_hook_only() {
    let home = empty_nudge_home("claude-up");
    let app = FakeApp::start();
    app.write_runtime(&home);
    let transcript = home.join("t.jsonl");
    std::fs::write(&transcript, TOKEN_COUNT).unwrap();
    let (out, _) = run_hook_in(&home, &[], stop_payload(&transcript).as_bytes());
    let seen = app.finish();
    let _ = std::fs::remove_dir_all(&home);
    assert_silent_success(&out);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].0, "/v1/hook");
    let hook: serde_json::Value = serde_json::from_str(&seen[0].1).unwrap();
    assert_eq!(hook["agent"], "claude");
}
