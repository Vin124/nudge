//! Black-box tests of the built `nudge-hook` binary (D15, AC #10).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn empty_nudge_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nudge-hook-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_hook(name: &str, stdin: &[u8]) -> (Output, Duration) {
    let home = empty_nudge_home(name);
    // Warm-up: the first launch of a freshly built exe on Windows pays for an
    // AV scan, which is not the steady-state cost AC #10 is about.
    let _ = Command::new(env!("CARGO_BIN_EXE_nudge-hook"))
        .env("NUDGE_HOME", &home)
        .stdin(Stdio::null())
        .output();
    let t = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_nudge-hook"))
        .env("NUDGE_HOME", &home)
        .env_remove("NUDGE_DEBUG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A write error (child exited early) is acceptable; the exit code is what matters.
    let _ = child.stdin.take().unwrap().write_all(stdin);
    let out = child.wait_with_output().unwrap();
    let elapsed = t.elapsed();
    let _ = std::fs::remove_dir_all(&home);
    (out, elapsed)
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
