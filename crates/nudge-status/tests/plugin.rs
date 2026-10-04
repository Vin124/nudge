//! Static checks of the Claude Code plugin in `plugin/` (D4).

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn plugin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugin")
}

fn read_json(rel: &str) -> Value {
    let bytes = std::fs::read(plugin_dir().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn plugin_manifest_parses() {
    let m = read_json(".claude-plugin/plugin.json");
    assert_eq!(m["name"], "nudge");
    assert_eq!(m["version"], "0.1.0");
    assert_eq!(m["license"], "MIT");
    assert!(m.get("author").is_none());
}

#[test]
fn hooks_json_registers_every_event() {
    let h = read_json("hooks/hooks.json");
    let events = h["hooks"].as_object().expect("top-level \"hooks\" wrapper");
    for ev in ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Notification", "Stop", "SessionEnd"] {
        let groups = events[ev].as_array().unwrap_or_else(|| panic!("{ev} missing"));
        let hook = &groups[0]["hooks"][0];
        let cmd = hook["command"].as_str().unwrap();
        assert!(cmd.contains("\"${CLAUDE_PLUGIN_ROOT}/scripts/hook.sh\""), "{ev}: {cmd}");
        assert_eq!(hook["type"], "command", "{ev}");
        assert_eq!(hook["timeout"], 5, "{ev}");
    }
    assert_eq!(events.len(), 7);
}

fn bash_n(script: &str) {
    let path = plugin_dir().join("scripts").join(script);
    let out = Command::new(git_bash()).arg("-n").arg(&path).output().expect("bash on PATH");
    assert!(out.status.success(), "{script}: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn hook_sh_syntax() {
    bash_n("hook.sh");
}

#[test]
fn install_sh_syntax() {
    bash_n("install.sh");
}

/// `bash` from PATH, skipping `System32\bash.exe` (the WSL launcher), which
/// Windows' `CreateProcess` search would otherwise find before Git Bash.
fn git_bash() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let exe = if cfg!(windows) { "bash.exe" } else { "bash" };
    std::env::split_paths(&path)
        .map(|d| d.join(exe))
        .find(|p| p.is_file() && !p.to_string_lossy().to_ascii_lowercase().contains("system32"))
        .unwrap_or_else(|| PathBuf::from("bash"))
}
