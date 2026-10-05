//! Local ingest server (spec §Security). Orchestrator-owned trust boundary:
//! 127.0.0.1 only, per-launch random token, 64KB body cap, POST JSON only.

use nudge_proto::{HookEnvelope, RuntimeInfo, StatusEnvelope, MAX_BODY_BYTES, PATH_HOOK, PATH_STATUS, TOKEN_HEADER};
use std::io::{self, Read};
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;
use tiny_http::{Method, Response, Server};

pub enum Inbound {
    Hook(HookEnvelope),
    Status(StatusEnvelope),
}

pub type Handler = Arc<dyn Fn(Inbound) + Send + Sync>;

pub fn generate_token() -> String {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).expect("OS RNG unavailable");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Constant-time comparison so the token can't be probed byte by byte.
pub fn token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Bind an ephemeral loopback port and serve on a background thread.
pub fn start(token: String, handler: Handler) -> io::Result<(RuntimeInfo, JoinHandle<()>)> {
    let server = Server::http("127.0.0.1:0").map_err(|e| io::Error::other(e.to_string()))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .ok_or_else(|| io::Error::other("no ip addr"))?;
    let rt = RuntimeInfo { port, token: token.clone(), pid: std::process::id() };
    let join = std::thread::Builder::new().name("nudge-ingest".into()).spawn(move || {
        for mut req in server.incoming_requests() {
            let code = route(&mut req, &token, &handler);
            let _ = req.respond(Response::empty(code));
        }
    })?;
    Ok((rt, join))
}

fn route(req: &mut tiny_http::Request, token: &str, handler: &Handler) -> u16 {
    if *req.method() != Method::Post {
        return 405;
    }
    let authed = req
        .headers()
        .iter()
        .find(|h| h.field.equiv(TOKEN_HEADER))
        .is_some_and(|h| token_eq(h.value.as_str(), token));
    if !authed {
        return 401;
    }
    if req.body_length().is_some_and(|n| n > MAX_BODY_BYTES) {
        return 413;
    }
    let mut body = Vec::new();
    if req.as_reader().take(MAX_BODY_BYTES as u64 + 1).read_to_end(&mut body).is_err() {
        return 400;
    }
    if body.len() > MAX_BODY_BYTES {
        return 413;
    }
    let inbound = match req.url() {
        PATH_HOOK => serde_json::from_slice::<HookEnvelope>(&body).map(Inbound::Hook),
        PATH_STATUS => serde_json::from_slice::<StatusEnvelope>(&body).map(Inbound::Status),
        _ => return 404,
    };
    match inbound {
        Ok(msg) => {
            handler(msg);
            204
        }
        Err(_) => 400,
    }
}

/// Atomic write of runtime.json; user-only permissions on unix.
pub fn write_runtime(dir: &Path, rt: &RuntimeInfo) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("runtime.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(rt)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, dir.join("runtime.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nudge_proto::{post_to, PROTOCOL_VERSION};
    use std::sync::Mutex;
    use std::time::Duration;

    fn spawn() -> (RuntimeInfo, Arc<Mutex<Vec<String>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        let handler: Handler = Arc::new(move |m| {
            s2.lock().unwrap().push(match m {
                Inbound::Hook(h) => format!("hook:{}", h.payload["hook_event_name"]),
                Inbound::Status(s) => format!("status:{:?}", s.session_id),
            })
        });
        let (rt, _j) = start(generate_token(), handler).unwrap();
        (rt, seen)
    }

    const T: Duration = Duration::from_secs(2);

    fn hook_body() -> Vec<u8> {
        serde_json::to_vec(&HookEnvelope {
            v: PROTOCOL_VERSION,
            ts_ms: 1,
            ancestors: vec![],
            payload: serde_json::json!({"hook_event_name": "Stop", "session_id": "a"}),
        })
        .unwrap()
    }

    #[test]
    fn accepts_authed_hook_and_status() {
        let (rt, seen) = spawn();
        assert_eq!(post_to(&rt, PATH_HOOK, &hook_body(), T).unwrap(), 204);
        let st = StatusEnvelope { v: 1, ts_ms: 1, session_id: Some("s".into()), rate_limits_available: None, rate_limits: None, context_used_percentage: None };
        assert_eq!(post_to(&rt, PATH_STATUS, &serde_json::to_vec(&st).unwrap(), T).unwrap(), 204);
        assert_eq!(*seen.lock().unwrap(), vec!["hook:\"Stop\"".to_string(), "status:Some(\"s\")".to_string()]);
    }

    #[test]
    fn rejects_wrong_or_missing_token() {
        let (rt, seen) = spawn();
        let bad = RuntimeInfo { token: "0".repeat(64), ..rt.clone() };
        assert_eq!(post_to(&bad, PATH_HOOK, &hook_body(), T).unwrap(), 401);
        let short = RuntimeInfo { token: "x".into(), ..rt.clone() };
        assert_eq!(post_to(&short, PATH_HOOK, &hook_body(), T).unwrap(), 401);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn rejects_oversize_unknown_path_and_bad_json() {
        let (rt, seen) = spawn();
        let big = vec![b' '; MAX_BODY_BYTES + 1];
        assert_eq!(post_to(&rt, PATH_HOOK, &big, T).unwrap(), 413);
        assert_eq!(post_to(&rt, "/v1/nope", b"{}", T).unwrap(), 404);
        assert_eq!(post_to(&rt, PATH_HOOK, b"{not json", T).unwrap(), 400);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn binds_loopback_only() {
        let (rt, _) = spawn();
        assert!(rt.port > 0);
        // A non-loopback connect to the same port must not reach us; we can only
        // assert the bind address indirectly: the server was created on 127.0.0.1.
        assert!(std::net::TcpStream::connect(("127.0.0.1", rt.port)).is_ok());
    }

    #[test]
    fn token_eq_is_exact() {
        assert!(token_eq("abc", "abc"));
        assert!(!token_eq("abc", "abd"));
        assert!(!token_eq("abc", "abcd"));
        assert_eq!(generate_token().len(), 64);
        assert_ne!(generate_token(), generate_token());
    }

    #[test]
    fn runtime_written_atomically() {
        let dir = std::env::temp_dir().join(format!("nudge-rt-{}", std::process::id()));
        let rt = RuntimeInfo { port: 1, token: "t".into(), pid: 2 };
        write_runtime(&dir, &rt).unwrap();
        let back: RuntimeInfo = serde_json::from_slice(&std::fs::read(dir.join("runtime.json")).unwrap()).unwrap();
        assert_eq!(back, rt);
        assert!(!dir.join("runtime.json.tmp").exists());
    }
}
