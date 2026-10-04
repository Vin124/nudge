//! Wire contract between the sidecars (`nudge-hook`, `nudge-status`) and the
//! Nudge app. Everything here is shared; changes are orchestrator-owned.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PROTOCOL_VERSION: u32 = 1;
/// Hard cap on any request body the app accepts (spec §Security).
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Sidecars must never block Claude Code longer than this (spec AC #10).
pub const SIDECAR_TIMEOUT: Duration = Duration::from_millis(300);
pub const TOKEN_HEADER: &str = "X-Nudge-Token";
pub const PATH_HOOK: &str = "/v1/hook";
pub const PATH_STATUS: &str = "/v1/status";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
}

/// Sent by `nudge-hook` for every Claude Code hook invocation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HookEnvelope {
    pub v: u32,
    pub ts_ms: u64,
    /// Process ancestors of the hook process, nearest parent first.
    pub ancestors: Vec<ProcInfo>,
    /// The raw JSON Claude Code wrote to the hook's stdin. Contains at least
    /// `session_id`, `cwd`, `hook_event_name`; `Notification` adds
    /// `notification_type` (docs/SPIKE.md, D13).
    pub payload: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RateWindow {
    pub used_percentage: f64,
    /// Unix seconds.
    pub resets_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct RateLimits {
    pub five_hour: Option<RateWindow>,
    pub seven_day: Option<RateWindow>,
}

/// Sent by `nudge-status` on every statusline render.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatusEnvelope {
    pub v: u32,
    pub ts_ms: u64,
    pub session_id: Option<String>,
    pub rate_limits_available: Option<bool>,
    pub rate_limits: Option<RateLimits>,
}

impl StatusEnvelope {
    /// Build from the raw statusline stdin JSON. Unknown/missing fields → None.
    pub fn from_statusline(raw: &serde_json::Value) -> Self {
        let rate_limits = raw
            .get("rate_limits")
            .filter(|v| !v.is_null())
            .and_then(|v| serde_json::from_value::<RateLimits>(v.clone()).ok());
        StatusEnvelope {
            v: PROTOCOL_VERSION,
            ts_ms: now_ms(),
            session_id: raw.get("session_id").and_then(|v| v.as_str()).map(str::to_owned),
            rate_limits_available: raw.get("rate_limits_available").and_then(|v| v.as_bool()),
            rate_limits,
        }
    }
}

/// Written by the app at startup to `<nudge_dir>/runtime.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RuntimeInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `~/.nudge`, overridable with `NUDGE_HOME` (used by tests).
pub fn nudge_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("NUDGE_HOME") {
        return PathBuf::from(p);
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".nudge")
}

pub fn runtime_path() -> PathBuf {
    nudge_dir().join("runtime.json")
}

pub fn read_runtime() -> io::Result<RuntimeInfo> {
    let bytes = std::fs::read(runtime_path())?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Minimal HTTP/1.1 POST over std only (keeps sidecars tiny and fast).
/// Returns the response status code.
pub fn post_to(rt: &RuntimeInfo, path: &str, body: &[u8], timeout: Duration) -> io::Result<u16> {
    let addr = SocketAddr::from(([127, 0, 0, 1], rt.port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n{TOKEN_HEADER}: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        rt.token,
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    let mut buf = [0u8; 64];
    let n = stream.read(&mut buf)?;
    parse_status_line(&buf[..n])
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad HTTP status line"))
}

/// Read runtime.json and POST. Any failure is an `Err`; callers in sidecars
/// must swallow it and exit 0 (spec AC #10).
pub fn post(path: &str, body: &[u8]) -> io::Result<u16> {
    let rt = read_runtime()?;
    post_to(&rt, path, body, SIDECAR_TIMEOUT)
}

fn parse_status_line(buf: &[u8]) -> Option<u16> {
    let s = std::str::from_utf8(buf).ok()?;
    let mut parts = s.split_whitespace();
    let proto = parts.next()?;
    if !proto.starts_with("HTTP/") {
        return None;
    }
    parts.next()?.parse().ok()
}

/// Ancestors of the current process, nearest parent first, at most `max`.
pub fn ancestors(max: usize) -> Vec<ProcInfo> {
    use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
    let sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new()),
    );
    let mut out = Vec::new();
    let mut cur = sys.process(Pid::from_u32(std::process::id())).and_then(|p| p.parent());
    while let Some(pid) = cur {
        if out.len() >= max {
            break;
        }
        let Some(p) = sys.process(pid) else { break };
        out.push(ProcInfo { pid: pid.as_u32(), name: p.name().to_string_lossy().into_owned() });
        cur = p.parent();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_from_full_statusline() {
        let raw = json!({
            "session_id": "abc",
            "rate_limits_available": true,
            "rate_limits": {
                "five_hour": {"used_percentage": 42.5, "resets_at": 1759550000},
                "seven_day": {"used_percentage": 18.0, "resets_at": 1760000000}
            }
        });
        let s = StatusEnvelope::from_statusline(&raw);
        assert_eq!(s.session_id.as_deref(), Some("abc"));
        assert_eq!(s.rate_limits_available, Some(true));
        let rl = s.rate_limits.unwrap();
        assert_eq!(rl.five_hour.unwrap().used_percentage, 42.5);
        assert_eq!(rl.seven_day.unwrap().resets_at, Some(1760000000));
    }

    #[test]
    fn status_from_api_key_statusline_has_no_limits() {
        let raw = json!({"session_id": "x", "rate_limits_available": false, "rate_limits": null});
        let s = StatusEnvelope::from_statusline(&raw);
        assert_eq!(s.rate_limits, None);
        assert_eq!(s.rate_limits_available, Some(false));
    }

    #[test]
    fn status_tolerates_partial_and_unknown_fields() {
        let raw = json!({"rate_limits": {"five_hour": {"used_percentage": 3}, "spend_limit": {"x": 1}}});
        let s = StatusEnvelope::from_statusline(&raw);
        let rl = s.rate_limits.unwrap();
        assert_eq!(rl.five_hour.unwrap().resets_at, None);
        assert!(rl.seven_day.is_none());
        assert!(s.session_id.is_none());
    }

    #[test]
    fn parses_status_line() {
        assert_eq!(parse_status_line(b"HTTP/1.1 204 No Content\r\n"), Some(204));
        assert_eq!(parse_status_line(b"garbage"), None);
        assert_eq!(parse_status_line(b""), None);
    }

    #[test]
    fn ancestors_is_bounded_and_nonempty() {
        let a = ancestors(3);
        assert!(!a.is_empty() && a.len() <= 3);
    }

    #[test]
    fn post_without_runtime_errors_fast() {
        let dir = std::env::temp_dir().join(format!("nudge-proto-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("NUDGE_HOME", &dir);
        let t = std::time::Instant::now();
        assert!(post(PATH_HOOK, b"{}").is_err());
        assert!(t.elapsed() < SIDECAR_TIMEOUT);
    }
}
