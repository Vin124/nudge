//! `nudge-status setup|uninstall` (D14, AC #9, spec §Rollback).
//!
//! A plugin can't be assumed to set `statusLine`, so `setup` edits
//! `settings.json` directly: it saves the original `statusLine` to
//! `<nudge_dir>/statusline.backup.json` and points `statusLine` at
//! `nudge-status --wrap-from <backup>`. `uninstall` reverses it.
//! Every other key, and the key order, is preserved (`preserve_order`).

use nudge_proto::nudge_dir;
use serde_json::{json, Map, Value};
use std::io;
use std::path::{Path, PathBuf};

const KEY: &str = "statusLine";
/// Marker that identifies a statusLine we installed.
const MARKER: &str = "nudge-status";

/// `$CLAUDE_CONFIG_DIR/settings.json`, else `~/.claude/settings.json`.
fn settings_path() -> PathBuf {
    let dir = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(d) => PathBuf::from(d),
        None => dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".claude"),
    };
    dir.join("settings.json")
}

fn backup_path() -> PathBuf {
    nudge_dir().join("statusline.backup.json")
}

/// Missing file → empty object. Unreadable, invalid or non-object → `Err`
/// (callers must then write nothing).
fn load(path: &Path) -> Result<Map<String, Value>, String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(e) => return Err(e.to_string()),
    };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err("not a JSON object".into()),
        Err(e) => Err(format!("invalid JSON: {e}")),
    }
}

fn is_ours(settings: &Map<String, Value>) -> bool {
    settings
        .get(KEY)
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(MARKER))
}

/// 2-space pretty JSON with a trailing newline, written via tmp + rename.
fn write_atomic(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    text.push('\n');
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".nudge-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Double-quote a path for `bash -c` / `sh -c`. Windows backslashes become
/// forward slashes (Windows and Git Bash both accept them; bash would
/// otherwise treat some backslashes as escapes).
fn sh_quote(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = if cfg!(windows) { s.replace('\\', "/") } else { s.into_owned() };
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        if matches!(ch, '"' | '\\' | '$' | '`') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

fn fail(action: &str, path: &Path, err: impl std::fmt::Display) -> i32 {
    eprintln!("nudge-status {action}: {}: {err}", path.display());
    1
}

pub fn setup() -> i32 {
    let path = settings_path();
    let mut settings = match load(&path) {
        Ok(m) => m,
        Err(e) => return fail("setup", &path, e),
    };
    if is_ours(&settings) {
        println!("already set up");
        return 0;
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return fail("setup", Path::new("current_exe"), e),
    };
    let original = settings.get(KEY).cloned().unwrap_or(Value::Null);
    let backup = backup_path();
    if let Err(e) = write_atomic(&backup, &json!({ KEY: original })) {
        return fail("setup", &backup, e);
    }
    let mut line = Map::new();
    line.insert("type".into(), "command".into());
    line.insert("command".into(), format!("{} --wrap-from {}", sh_quote(&exe), sh_quote(&backup)).into());
    if let Some(padding) = original.get("padding") {
        line.insert("padding".into(), padding.clone());
    }
    settings.insert(KEY.into(), Value::Object(line));
    if let Err(e) = write_atomic(&path, &Value::Object(settings)) {
        return fail("setup", &path, e);
    }
    println!("Nudge statusline installed in {} (original saved to {})", path.display(), backup.display());
    0
}

pub fn uninstall() -> i32 {
    let path = settings_path();
    let mut settings = match load(&path) {
        Ok(m) => m,
        Err(e) => return fail("uninstall", &path, e),
    };
    let backup = backup_path();
    let saved: Option<Value> = std::fs::read(&backup)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get(KEY).cloned());

    if is_ours(&settings) {
        match saved {
            Some(v) if !v.is_null() => {
                settings.insert(KEY.into(), v); // keeps the key's position
            }
            // Null original, or no usable backup: a dangling wrapper would
            // break once ~/.nudge is gone, so drop the key.
            _ => {
                settings.shift_remove(KEY);
            }
        }
        if let Err(e) = write_atomic(&path, &Value::Object(settings)) {
            return fail("uninstall", &path, e);
        }
        println!("Nudge statusline removed; original restored in {}", path.display());
    } else {
        println!("statusLine is not managed by Nudge; left unchanged");
    }
    match std::fs::remove_file(&backup) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return fail("uninstall", &backup, e),
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_for_posix_shells() {
        assert_eq!(sh_quote(Path::new("/a b/c")), "\"/a b/c\"");
        assert_eq!(sh_quote(Path::new("/x$y`z\"")), "\"/x\\$y\\`z\\\"\"");
        if cfg!(windows) {
            assert_eq!(sh_quote(Path::new(r"C:\Users\A B\n.exe")), "\"C:/Users/A B/n.exe\"");
        }
    }

    #[test]
    fn ours_detection() {
        let m = |v: Value| v.as_object().unwrap().clone();
        assert!(is_ours(&m(json!({"statusLine": {"command": "\"/x/nudge-status\" --wrap-from y"}}))));
        assert!(!is_ours(&m(json!({"statusLine": {"command": "ccstatusline"}}))));
        assert!(!is_ours(&m(json!({}))));
    }
}
