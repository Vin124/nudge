//! Static checks of the Claude Code plugin in `plugin/` (D4), plus the Codex
//! manifest, hooks and PowerShell scripts that share the folder (D25, D31).

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

// ---- Codex plugin (D25, D27, D31, D32) --------------------------------------

const CODEX_EVENTS: [&str; 8] =
    ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "PermissionRequest", "Stop", "Interrupt", "SessionEnd"];

fn repo_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn codex_handler(event: &str) -> Value {
    let h = read_json("hooks/codex-hooks.json");
    h["hooks"][event][0]["hooks"][0].clone()
}

#[test]
fn codex_manifest_names_codex_hooks() {
    let m = read_json("plugin.json");
    assert_eq!(m["name"], "nudge");
    assert_eq!(m["version"], "0.1.0");
    assert_eq!(m["extensions"]["com.openai"]["hooks"], "./hooks/codex-hooks.json");
    assert!(plugin_dir().join("hooks/codex-hooks.json").is_file());
}

/// D25: Codex 0.154 ignores the root manifest and reads `.codex-plugin/plugin.json`;
/// without its `hooks` entry Codex falls back to `hooks/hooks.json` (Claude's,
/// verified E2E 2026-10-12). Both manifests must name the Codex hooks file.
#[test]
fn legacy_codex_manifest_names_codex_hooks_and_matches_root() {
    let legacy = read_json(".codex-plugin/plugin.json");
    let root = read_json("plugin.json");
    assert_eq!(legacy["hooks"], "./hooks/codex-hooks.json");
    for key in ["name", "version", "description"] {
        assert_eq!(legacy[key], root[key], "{key}");
    }
}

#[test]
fn codex_hooks_register_every_event() {
    let h = read_json("hooks/codex-hooks.json");
    let events = h["hooks"].as_object().expect("top-level \"hooks\" wrapper");
    assert_eq!(events.len(), CODEX_EVENTS.len(), "{:?}", events.keys().collect::<Vec<_>>());
    for ev in CODEX_EVENTS {
        let groups = events[ev].as_array().unwrap_or_else(|| panic!("{ev} missing"));
        assert_eq!(groups.len(), 1, "{ev}");
        assert_eq!(groups[0]["matcher"], "*", "{ev}");
        let hooks = groups[0]["hooks"].as_array().unwrap();
        assert_eq!(hooks.len(), 1, "{ev}");
        let hook = &hooks[0];
        assert_eq!(hook["type"], "command", "{ev}");
        // Codex caps SessionEnd/Interrupt hooks at 3 s and warns at startup otherwise (E2E 2026-10-12).
        let want = if matches!(ev, "SessionEnd" | "Interrupt") { 3 } else { 5 };
        assert_eq!(hook["timeout"], want, "{ev}");
        let cmd = hook["command"].as_str().unwrap_or_else(|| panic!("{ev}: command"));
        assert!(cmd.contains("hook.sh") && cmd.contains(ev) && cmd.contains("codex"), "{ev}: {cmd}");
        let win = hook["commandWindows"].as_str().unwrap_or_else(|| panic!("{ev}: commandWindows"));
        if ev == "SessionStart" {
            assert!(win.contains("hook.ps1"), "{ev}: {win}");
        } else {
            assert!(win.contains("nudge-hook.exe") && win.contains("--agent codex"), "{ev}: {win}");
            assert!(!win.contains("hook.ps1"), "{ev}: no second PowerShell start per event: {win}");
        }
    }
}

#[test]
fn claude_hooks_json_has_no_codex() {
    let raw = std::fs::read_to_string(plugin_dir().join("hooks/hooks.json")).unwrap();
    assert!(!raw.to_ascii_lowercase().contains("codex"));
}

#[test]
fn codex_marketplace_resolves_to_plugin() {
    let bytes = std::fs::read(repo_dir().join(".agents/plugins/marketplace.json")).unwrap();
    let m: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(m["name"], "nudge");
    let p = &m["plugins"][0];
    assert_eq!(p["name"], "nudge");
    assert_eq!(p["source"]["source"], "local");
    let dir = repo_dir().join(p["source"]["path"].as_str().unwrap());
    assert!(dir.join("plugin.json").is_file(), "{}", dir.display());
}

// ---- PowerShell scripts (Windows only) --------------------------------------

/// A fresh, empty directory under the system temp dir.
fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    let d = std::env::temp_dir().join(format!(
        "nudge-plugin-test-{tag}-{}-{}-{nanos}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `powershell -NoProfile -NonInteractive` with a clean NUDGE_* environment.
fn powershell() -> Command {
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive"]);
    for k in ["NUDGE_HOME", "NUDGE_REPO", "NUDGE_BASE_URL", "NUDGE_TEST_REC", "PLUGIN_ROOT"] {
        c.env_remove(k);
    }
    c
}

/// Run `cmd` with `stdin` piped in; returns (exit code, stdout, stderr).
fn run_with_stdin(mut cmd: Command, stdin: &[u8]) -> (i32, Vec<u8>, String) {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn");
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1), out.stdout, String::from_utf8_lossy(&out.stderr).into_owned())
}

/// A tiny console exe compiled with PowerShell's Add-Type (the C# compiler that
/// ships with .NET Framework). It writes its args to `$NUDGE_TEST_REC\<name>.args`
/// and its stdin bytes to `<name>.stdin`, where <name> is its own file name.
/// Copied into a fake `~/.nudge/bin` as nudge-hook.exe / nudge-status.exe.
fn recorder_exe() -> PathBuf {
    use std::sync::OnceLock;
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        const SRC: &str = r#"
using System; using System.IO;
public static class Rec {
    public static int Main(string[] args) {
        string dir = Environment.GetEnvironmentVariable("NUDGE_TEST_REC");
        if (String.IsNullOrEmpty(dir)) return 0;
        string name = Path.GetFileName(System.Reflection.Assembly.GetEntryAssembly().Location);
        File.WriteAllText(Path.Combine(dir, name + ".args"), String.Join("\n", args));
        if (Console.IsInputRedirected) {
            using (Stream s = Console.OpenStandardInput())
            using (FileStream f = File.Create(Path.Combine(dir, name + ".stdin"))) { s.CopyTo(f); }
        }
        return 0;
    }
}"#;
        let exe = temp_dir("rec").join("rec.exe");
        let out = powershell()
            .env("REC_SRC", SRC)
            .env("REC_OUT", &exe)
            .args(["-Command", "Add-Type -TypeDefinition $env:REC_SRC -OutputAssembly $env:REC_OUT -OutputType ConsoleApplication"])
            .output()
            .unwrap();
        assert!(exe.is_file(), "Add-Type failed: {}", String::from_utf8_lossy(&out.stderr));
        exe
    })
    .clone()
}

/// The hook JSON Codex sends, with a non-ASCII cwd.
const UTF8_PAYLOAD: &str =
    r#"{"session_id":"s1","hook_event_name":"Stop","cwd":"C:\\Users\\José\\projét","turn_id":"t1","model":"gpt"}"#;

#[test]
fn ps1_scripts_parse() {
    if !cfg!(windows) {
        return;
    }
    for script in ["hook.ps1", "install.ps1"] {
        let path = plugin_dir().join("scripts").join(script);
        let out = powershell()
            .env("PS1_PATH", &path)
            .args([
                "-Command",
                "$e = $null; $null = [System.Management.Automation.Language.Parser]::ParseFile($env:PS1_PATH, [ref]$null, [ref]$e); $e.Count; $e | ForEach-Object { [Console]::Error.WriteLine($_.ToString()) }",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(stdout.trim(), "0", "{script}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

#[test]
fn hook_ps1_without_install_is_silent() {
    if !cfg!(windows) {
        return;
    }
    let home = temp_dir("hookps1");
    let mut c = powershell();
    c.env("NUDGE_HOME", &home).args(["-ExecutionPolicy", "Bypass", "-File"]).arg(plugin_dir().join("scripts/hook.ps1")).args(["Stop", "codex"]);
    let (code, stdout, stderr) = run_with_stdin(c, UTF8_PAYLOAD.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty(), "{}", String::from_utf8_lossy(&stdout));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn inline_stop_command_without_install_is_silent() {
    if !cfg!(windows) {
        return;
    }
    let home = temp_dir("inline");
    let win = codex_handler("Stop")["commandWindows"].as_str().unwrap().to_owned();
    let mut c = powershell();
    c.env("NUDGE_HOME", &home).args(["-Command", &win]);
    let (code, stdout, stderr) = run_with_stdin(c, UTF8_PAYLOAD.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty(), "{}", String::from_utf8_lossy(&stdout));
    let _ = std::fs::remove_dir_all(&home);
}

/// A NUDGE_HOME whose bin holds the recorder as nudge-hook.exe; recordings go to `<home>\rec`.
fn home_with_fake_hook(tag: &str) -> (PathBuf, PathBuf) {
    let home = temp_dir(tag);
    let rec = home.join("rec");
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::create_dir_all(&rec).unwrap();
    std::fs::copy(recorder_exe(), home.join("bin/nudge-hook.exe")).unwrap();
    (home, rec)
}

fn assert_forwarded(rec: &std::path::Path, stdout: &[u8], code: i32, stderr: &str) {
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty(), "{}", String::from_utf8_lossy(stdout));
    let got = std::fs::read(rec.join("nudge-hook.exe.stdin")).unwrap_or_else(|e| panic!("hook not run: {e}; {stderr}"));
    assert_eq!(got, UTF8_PAYLOAD.as_bytes(), "byte-exact UTF-8: {}", String::from_utf8_lossy(&got));
    assert!(String::from_utf8_lossy(&got).contains("José\\\\projét"));
    let args = std::fs::read_to_string(rec.join("nudge-hook.exe.args")).unwrap();
    assert_eq!(args, "--agent\ncodex");
}

#[test]
fn session_start_command_forwards_utf8_stdin() {
    if !cfg!(windows) {
        return;
    }
    // The real SessionStart commandWindows string, through hook.ps1.
    let (home, rec) = home_with_fake_hook("ss-utf8");
    let win = codex_handler("SessionStart")["commandWindows"].as_str().unwrap().to_owned();
    let mut c = powershell();
    c.env("NUDGE_HOME", &home).env("NUDGE_TEST_REC", &rec).env("PLUGIN_ROOT", plugin_dir()).args(["-Command", &win]);
    let (code, stdout, stderr) = run_with_stdin(c, UTF8_PAYLOAD.as_bytes());
    assert_forwarded(&rec, &stdout, code, &stderr);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn inline_stop_command_forwards_utf8_stdin() {
    if !cfg!(windows) {
        return;
    }
    let (home, rec) = home_with_fake_hook("stop-utf8");
    let win = codex_handler("Stop")["commandWindows"].as_str().unwrap().to_owned();
    let mut c = powershell();
    c.env("NUDGE_HOME", &home).env("NUDGE_TEST_REC", &rec).args(["-Command", &win]);
    let (code, stdout, stderr) = run_with_stdin(c, UTF8_PAYLOAD.as_bytes());
    assert_forwarded(&rec, &stdout, code, &stderr);
    let _ = std::fs::remove_dir_all(&home);
}

/// A release directory (served as file:///) holding nudge-windows-x86_64.zip, with
/// the recorder as nudge-hook.exe and nudge-status.exe, and SHA256SUMS. `sha` =
/// None writes the zip's real hash.
fn fake_release(tag: &str, sha: Option<&str>) -> PathBuf {
    let rel = temp_dir(tag);
    let stage = rel.join("stage");
    std::fs::create_dir_all(&stage).unwrap();
    for name in ["nudge-hook.exe", "nudge-status.exe"] {
        std::fs::copy(recorder_exe(), stage.join(name)).unwrap();
    }
    let zip = rel.join("nudge-windows-x86_64.zip");
    let out = powershell()
        .env("STAGE", &stage)
        .env("ZIP", &zip)
        .args([
            "-Command",
            "$ProgressPreference='SilentlyContinue'; Compress-Archive -Path (Join-Path $env:STAGE '*') -DestinationPath $env:ZIP; (Get-FileHash -LiteralPath $env:ZIP -Algorithm SHA256).Hash",
        ])
        .output()
        .unwrap();
    assert!(zip.is_file(), "{}", String::from_utf8_lossy(&out.stderr));
    let real = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let sum = sha.map(str::to_owned).unwrap_or(real);
    std::fs::write(rel.join("SHA256SUMS"), format!("{sum} *nudge-windows-x86_64.zip\n")).unwrap();
    rel
}

fn file_url(dir: &std::path::Path) -> String {
    format!("file:///{}", dir.to_string_lossy().replace('\\', "/"))
}

fn run_install(home: &std::path::Path, rel: &std::path::Path, rec: &std::path::Path, agent: &str) -> (i32, Vec<u8>, String) {
    let mut c = powershell();
    c.env("NUDGE_HOME", home)
        .env("NUDGE_BASE_URL", file_url(rel))
        .env("NUDGE_TEST_REC", rec)
        .args(["-ExecutionPolicy", "Bypass", "-File"])
        .arg(plugin_dir().join("scripts/install.ps1"))
        .args(["-Agent", agent]);
    run_with_stdin(c, b"")
}

#[test]
fn install_ps1_refuses_checksum_mismatch() {
    if !cfg!(windows) {
        return;
    }
    let rel = fake_release("rel-bad", Some(&"0".repeat(64)));
    let home = temp_dir("inst-bad");
    let rec = temp_dir("inst-bad-rec");
    let (code, stdout, stderr) = run_install(&home, &rel, &rec, "claude");
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty(), "{}", String::from_utf8_lossy(&stdout));
    let log = std::fs::read_to_string(home.join("install.log")).unwrap();
    assert!(log.contains("ERROR: checksum mismatch"), "{log}");
    let bin = home.join("bin");
    assert!(!bin.exists() || std::fs::read_dir(&bin).unwrap().next().is_none(), "files landed in bin");
    assert!(!home.join("install.lock").exists(), "lock left behind");
    for d in [rel, home, rec] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn install_ps1_installs_and_runs_setup_only_for_claude() {
    if !cfg!(windows) {
        return;
    }
    let rel = fake_release("rel-ok", None);
    for (agent, setup_runs) in [("codex", false), ("claude", true)] {
        let home = temp_dir("inst-ok");
        let rec = temp_dir("inst-ok-rec");
        let (code, stdout, stderr) = run_install(&home, &rel, &rec, agent);
        assert_eq!(code, 0, "{agent}: {stderr}");
        assert!(stdout.is_empty(), "{agent}: {}", String::from_utf8_lossy(&stdout));
        let log = std::fs::read_to_string(home.join("install.log")).unwrap();
        assert!(log.contains("checksum ok") && log.contains("install complete"), "{agent}: {log}");
        assert!(home.join("bin/nudge-hook.exe").is_file(), "{agent}");
        assert!(home.join("bin/nudge-status.exe").is_file(), "{agent}");
        let setup = rec.join("nudge-status.exe.args");
        assert_eq!(setup.is_file(), setup_runs, "{agent}: {log}");
        if setup_runs {
            assert_eq!(std::fs::read_to_string(&setup).unwrap(), "setup");
        } else {
            assert!(log.contains("skipping nudge-status setup"), "{log}");
        }
        assert!(!home.join("install.lock").exists(), "{agent}: lock left behind");
        for d in [home, rec] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
    let _ = std::fs::remove_dir_all(rel);
}
