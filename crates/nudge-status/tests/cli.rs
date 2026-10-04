//! Black-box tests of the built `nudge-status` binary (D14, AC #9).
//! Every run sets `CLAUDE_CONFIG_DIR` and `NUDGE_HOME` to temp dirs, so the
//! real `~/.claude` and `~/.nudge` are never touched.

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("nudge-status-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("claude")).unwrap();
        std::fs::create_dir_all(root.join("nudge")).unwrap();
        Sandbox { root }
    }
    fn settings(&self) -> PathBuf {
        self.root.join("claude").join("settings.json")
    }
    fn backup(&self) -> PathBuf {
        self.root.join("nudge").join("statusline.backup.json")
    }
    fn run(&self, args: &[&str], stdin: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nudge-status"))
            .args(args)
            .env("CLAUDE_CONFIG_DIR", self.root.join("claude"))
            .env("NUDGE_HOME", self.root.join("nudge"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let _ = child.stdin.take().unwrap().write_all(stdin);
        child.wait_with_output().unwrap()
    }
    fn write_settings(&self, v: &Value) -> Vec<u8> {
        let text = format!("{}\n", serde_json::to_string_pretty(v).unwrap());
        std::fs::write(self.settings(), &text).unwrap();
        text.into_bytes()
    }
    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

const STATUS_JSON: &[u8] =
    br#"{"session_id":"s","rate_limits":{"five_hour":{"used_percentage":42},"seven_day":{"used_percentage":18}}}"#;

fn keys(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

// ---- statusline mode -------------------------------------------------------

#[test]
fn no_wrap_prints_minimal_line() {
    let sb = Sandbox::new("nowrap");
    let out = sb.run(&[], STATUS_JSON);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "5h 42% · wk 18%\n");
}

#[test]
fn no_wrap_prints_nothing_without_data() {
    let sb = Sandbox::new("nodata");
    let out = sb.run(&[], br#"{"session_id":"s","rate_limits":null}"#);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
    let out = sb.run(&[], b"not json");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}

#[test]
fn wrap_passes_stdout_byte_identical() {
    let sb = Sandbox::new("wrap");
    // ANSI escapes, a non-ASCII char, and a trailing newline.
    let cmd = r"printf '\033[1;32mhi\033[0m · %s\n' ok";
    std::fs::write(sb.backup(), json!({"statusLine": {"type": "command", "command": cmd}}).to_string()).unwrap();
    let out = sb.run(&["--wrap-from", sb.backup().to_str().unwrap()], STATUS_JSON);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(out.stdout, "\x1b[1;32mhi\x1b[0m · ok\n".as_bytes());
}

#[test]
fn wrap_feeds_same_stdin_and_passes_exit_code() {
    let sb = Sandbox::new("wrapstdin");
    std::fs::write(sb.backup(), json!({"statusLine": {"type": "command", "command": "cat; exit 3"}}).to_string())
        .unwrap();
    let out = sb.run(&["--wrap-from", sb.backup().to_str().unwrap()], STATUS_JSON);
    assert_eq!(out.stdout, STATUS_JSON);
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn wrap_with_missing_backup_falls_back_to_no_wrap() {
    let sb = Sandbox::new("wrapmissing");
    let out = sb.run(&["--wrap-from", sb.backup().to_str().unwrap()], STATUS_JSON);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "5h 42% · wk 18%\n");
}

// ---- setup -----------------------------------------------------------------

#[test]
fn setup_on_missing_file_creates_it_with_null_backup() {
    let sb = Sandbox::new("setupmissing");
    let out = sb.run(&["setup"], b"");
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let s = Sandbox::read_json(&sb.settings());
    assert_eq!(s["statusLine"]["type"], "command");
    let cmd = s["statusLine"]["command"].as_str().unwrap();
    assert!(cmd.contains("nudge-status") && cmd.contains("--wrap-from") && cmd.contains("statusline.backup.json"));
    assert_eq!(Sandbox::read_json(&sb.backup()), json!({"statusLine": null}));
}

#[test]
fn setup_preserves_other_keys_and_order() {
    let sb = Sandbox::new("setuporder");
    sb.write_settings(&json!({"zeta": 1, "model": "opus", "alpha": {"b": 2, "a": 1}, "hooks": {}}));
    assert_eq!(sb.run(&["setup"], b"").status.code(), Some(0));
    let s = Sandbox::read_json(&sb.settings());
    assert_eq!(keys(&s), ["zeta", "model", "alpha", "hooks", "statusLine"]);
    assert_eq!(keys(&s["alpha"]), ["b", "a"]);
    assert_eq!(s["model"], "opus");
}

#[test]
fn setup_backs_up_custom_statusline_and_copies_padding() {
    let sb = Sandbox::new("setupcustom");
    let original = json!({"type": "command", "command": "ccstatusline", "padding": 0});
    sb.write_settings(&json!({"statusLine": original, "theme": "dark"}));
    assert_eq!(sb.run(&["setup"], b"").status.code(), Some(0));
    assert_eq!(Sandbox::read_json(&sb.backup()), json!({"statusLine": original}));
    let s = Sandbox::read_json(&sb.settings());
    assert_eq!(keys(&s), ["statusLine", "theme"]);
    assert_eq!(s["statusLine"]["padding"], 0);
    // The installed command points at the backup that holds the original.
    let cmd = s["statusLine"]["command"].as_str().unwrap();
    let backup_in_cmd = cmd.rsplit("--wrap-from ").next().unwrap().trim_matches('"');
    assert_eq!(Sandbox::read_json(Path::new(backup_in_cmd)), json!({"statusLine": original}));
}

#[test]
fn installed_command_runs_through_bash_and_wraps_original() {
    let sb = Sandbox::new("setupe2e");
    sb.write_settings(&json!({"statusLine": {"type": "command", "command": "printf 'orig\\n'"}}));
    assert_eq!(sb.run(&["setup"], b"").status.code(), Some(0));
    let cmd = Sandbox::read_json(&sb.settings())["statusLine"]["command"].as_str().unwrap().to_string();
    let mut child = Command::new(git_bash())
        .arg("-c")
        .arg(&cmd)
        .env("CLAUDE_CONFIG_DIR", sb.root.join("claude"))
        .env("NUDGE_HOME", sb.root.join("nudge"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("bash on PATH");
    let _ = child.stdin.take().unwrap().write_all(STATUS_JSON);
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.stdout, b"orig\n", "cmd: {cmd}");
}

#[test]
fn setup_twice_is_a_no_op() {
    let sb = Sandbox::new("setuptwice");
    sb.write_settings(&json!({"statusLine": {"type": "command", "command": "echo hi"}}));
    assert_eq!(sb.run(&["setup"], b"").status.code(), Some(0));
    let after_first = std::fs::read(sb.settings()).unwrap();
    let backup_first = std::fs::read(sb.backup()).unwrap();
    let out = sb.run(&["setup"], b"");
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("already set up"));
    assert_eq!(std::fs::read(sb.settings()).unwrap(), after_first);
    assert_eq!(std::fs::read(sb.backup()).unwrap(), backup_first);
}

#[test]
fn setup_on_invalid_json_exits_1_and_writes_nothing() {
    let sb = Sandbox::new("setupinvalid");
    std::fs::write(sb.settings(), b"{ \"model\": oops,").unwrap();
    let out = sb.run(&["setup"], b"");
    assert_eq!(out.status.code(), Some(1));
    assert!(!out.stderr.is_empty());
    assert_eq!(std::fs::read(sb.settings()).unwrap(), b"{ \"model\": oops,");
    assert!(!sb.backup().exists());
}

// ---- uninstall -------------------------------------------------------------

#[test]
fn uninstall_restores_original_exactly() {
    let sb = Sandbox::new("uninstall");
    let before = sb.write_settings(&json!({
        "model": "opus",
        "statusLine": {"type": "command", "command": "ccstatusline", "padding": 2},
        "theme": "dark"
    }));
    assert_eq!(sb.run(&["setup"], b"").status.code(), Some(0));
    assert_ne!(std::fs::read(sb.settings()).unwrap(), before);
    assert_eq!(sb.run(&["uninstall"], b"").status.code(), Some(0));
    assert_eq!(std::fs::read(sb.settings()).unwrap(), before);
    assert!(!sb.backup().exists());
}

#[test]
fn uninstall_with_null_original_removes_key() {
    let sb = Sandbox::new("uninstallnull");
    let before = sb.write_settings(&json!({"model": "opus", "theme": "dark"}));
    assert_eq!(sb.run(&["setup"], b"").status.code(), Some(0));
    assert_eq!(sb.run(&["uninstall"], b"").status.code(), Some(0));
    assert_eq!(std::fs::read(sb.settings()).unwrap(), before);
    assert!(!sb.backup().exists());
}

#[test]
fn uninstall_leaves_foreign_statusline_alone() {
    let sb = Sandbox::new("uninstallforeign");
    sb.write_settings(&json!({"statusLine": {"type": "command", "command": "echo old"}}));
    assert_eq!(sb.run(&["setup"], b"").status.code(), Some(0));
    // The user replaced our statusline after setup.
    let mine = sb.write_settings(&json!({"statusLine": {"type": "command", "command": "echo new"}}));
    assert_eq!(sb.run(&["uninstall"], b"").status.code(), Some(0));
    assert_eq!(std::fs::read(sb.settings()).unwrap(), mine);
}

#[test]
fn uninstall_on_invalid_json_exits_1_and_writes_nothing() {
    let sb = Sandbox::new("uninstallinvalid");
    std::fs::write(sb.settings(), b"[1,").unwrap();
    std::fs::write(sb.backup(), br#"{"statusLine":null}"#).unwrap();
    assert_eq!(sb.run(&["uninstall"], b"").status.code(), Some(1));
    assert_eq!(std::fs::read(sb.settings()).unwrap(), b"[1,");
    assert!(sb.backup().exists());
}

/// See `plugin.rs`: avoid the WSL `bash.exe` in System32.
fn git_bash() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let exe = if cfg!(windows) { "bash.exe" } else { "bash" };
    std::env::split_paths(&path)
        .map(|d| d.join(exe))
        .find(|p| p.is_file() && !p.to_string_lossy().to_ascii_lowercase().contains("system32"))
        .unwrap_or_else(|| PathBuf::from("bash"))
}
