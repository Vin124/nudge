//! `nudge-status`: Claude Code statusline sidecar (D3) and its installer (D14).
//!
//! Statusline mode forwards `rate_limits` + `session_id` to the app in the
//! background and either prints a minimal usage line or runs the user's
//! original statusline command and prints its stdout byte-for-byte (AC #9).

mod settings;

use nudge_proto::{post, StatusEnvelope, PATH_STATUS, SIDECAR_TIMEOUT};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

const USAGE: &str = "\
usage: nudge-status                         statusline mode: reads Claude Code statusline JSON on stdin
       nudge-status --wrap-from <backup>    statusline mode, wrapping the original command saved in <backup>
       nudge-status setup                   point settings.json statusLine at nudge-status (keeps a backup)
       nudge-status uninstall               restore the original statusLine from the backup";

/// Statusline payloads are a few KB; cap anyway.
const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("setup") => settings::setup(),
        Some("uninstall") => settings::uninstall(),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            0
        }
        Some("--wrap-from") => statusline(args.get(1).map(Path::new)),
        // Unknown args still render a statusline: a statusline must never error.
        _ => statusline(None),
    };
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

/// Returns the process exit code (the wrapped command's, when wrapping).
fn statusline(wrap_from: Option<&Path>) -> i32 {
    let start = Instant::now();
    let mut input = Vec::new();
    let _ = std::io::stdin().lock().take(MAX_STDIN_BYTES).read_to_end(&mut input);
    let raw: Value = serde_json::from_slice(&input).unwrap_or(Value::Null);

    // POST in the background so the render is never delayed by the app.
    let (done_tx, done_rx) = mpsc::channel::<()>();
    if raw.is_object() {
        let env = StatusEnvelope::from_statusline(&raw);
        std::thread::spawn(move || {
            if let Ok(body) = serde_json::to_vec(&env) {
                let _ = post(PATH_STATUS, &body);
            }
            let _ = done_tx.send(());
        });
    }

    let wrapped = wrap_from.and_then(original_command);
    let code = match wrapped.and_then(|cmd| run_wrapped(&cmd, &input)) {
        Some(code) => code,
        None => {
            if let Some(line) = minimal_line(&raw) {
                println!("{line}");
            }
            0
        }
    };
    let _ = std::io::stdout().flush();
    // Give the POST until SIDECAR_TIMEOUT (measured from start) to land.
    let _ = done_rx.recv_timeout(SIDECAR_TIMEOUT.saturating_sub(start.elapsed()));
    code
}

/// `5h 42% · wk 18%`, omitting missing windows; `None` when there is no data.
fn minimal_line(raw: &Value) -> Option<String> {
    let rl = StatusEnvelope::from_statusline(raw).rate_limits?;
    let parts: Vec<String> = [("5h", rl.five_hour), ("wk", rl.seven_day)]
        .into_iter()
        .filter_map(|(label, w)| w.map(|w| format!("{label} {}%", w.used_percentage.round() as i64)))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The user's original statusline command from the backup written by `setup`.
/// `None` (→ no-wrap mode) if the backup is missing, invalid, empty, or
/// points back at us (would recurse).
fn original_command(backup: &Path) -> Option<String> {
    let bytes = std::fs::read(backup).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    let cmd = v.get("statusLine")?.get("command")?.as_str()?;
    (!cmd.trim().is_empty() && !cmd.contains("nudge-status")).then(|| cmd.to_string())
}

/// Run `cmd` the way Claude Code runs a statusline, feeding it `input` and
/// copying its stdout unchanged. `None` if it could not be started.
fn run_wrapped(cmd: &str, input: &[u8]) -> Option<i32> {
    let mut child = shell_command(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let input = input.to_vec();
    // Separate thread: a command that doesn't read stdin must not deadlock us.
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let out = child.wait_with_output().ok()?;
    let _ = writer.join();
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(&out.stdout);
    let _ = stdout.flush();
    Some(out.status.code().unwrap_or(1))
}

#[cfg(not(windows))]
fn shell_command(cmd: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(cmd);
    c
}

/// Windows: Git Bash if available (as Claude Code does), else `cmd /C`.
#[cfg(windows)]
fn shell_command(cmd: &str) -> Command {
    match find_git_bash() {
        Some(bash) => {
            let mut c = Command::new(bash);
            c.arg("-c").arg(cmd);
            c
        }
        None => {
            use std::os::windows::process::CommandExt;
            let mut c = Command::new("cmd");
            c.arg("/C").raw_arg(cmd);
            c
        }
    }
}

/// `CLAUDE_CODE_GIT_BASH_PATH` first, then `bash.exe` on PATH — skipping
/// `System32\bash.exe`, which is the WSL launcher, not Git Bash.
#[cfg(windows)]
fn find_git_bash() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(std::path::PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("bash.exe"))
        .find(|p| p.is_file() && !p.to_string_lossy().to_ascii_lowercase().contains("system32"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn minimal_line_both_windows() {
        let raw = json!({"rate_limits": {
            "five_hour": {"used_percentage": 42.4, "resets_at": 1},
            "seven_day": {"used_percentage": 17.6, "resets_at": 2}
        }});
        assert_eq!(minimal_line(&raw).as_deref(), Some("5h 42% · wk 18%"));
    }

    #[test]
    fn minimal_line_only_five_hour() {
        let raw = json!({"rate_limits": {"five_hour": {"used_percentage": 3}}});
        assert_eq!(minimal_line(&raw).as_deref(), Some("5h 3%"));
    }

    #[test]
    fn minimal_line_none_without_data() {
        assert_eq!(minimal_line(&json!({"rate_limits": null, "rate_limits_available": false})), None);
        assert_eq!(minimal_line(&json!({"rate_limits": {}})), None);
        assert_eq!(minimal_line(&Value::Null), None);
    }

    #[test]
    fn original_command_rejects_missing_null_and_self() {
        let dir = std::env::temp_dir().join(format!("nudge-status-unit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("b.json");
        assert_eq!(original_command(&f), None);
        std::fs::write(&f, r#"{"statusLine": null}"#).unwrap();
        assert_eq!(original_command(&f), None);
        std::fs::write(&f, r#"{"statusLine": {"type":"command","command":"x/nudge-status --wrap-from y"}}"#).unwrap();
        assert_eq!(original_command(&f), None);
        std::fs::write(&f, r#"{"statusLine": {"type":"command","command":"echo hi"}}"#).unwrap();
        assert_eq!(original_command(&f).as_deref(), Some("echo hi"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
