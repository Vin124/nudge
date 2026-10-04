//! The one outbound request (D3 opt-in). The bearer token is sent only to
//! `https://api.anthropic.com`; redirects are disabled so it can't be forwarded.
//! Error values never contain the token or response bodies.

use super::token::Token;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const MAX_BODY: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum FetchError {
    /// 401/403: token rejected.
    Auth,
    /// Anything else, as a short reason safe to log.
    Other(String),
}

pub fn fetch_usage(token: &Token) -> Result<serde_json::Value, FetchError> {
    let tls = native_tls::TlsConnector::new().map_err(|_| FetchError::Other("tls init failed".into()))?;
    let agent = ureq::AgentBuilder::new()
        .tls_connector(Arc::new(tls))
        .timeout(Duration::from_secs(10))
        .redirects(0)
        .build();
    let resp = agent
        .get(USAGE_URL)
        .set("Authorization", &format!("Bearer {}", token.secret()))
        .set("anthropic-beta", "oauth-2025-04-20")
        .set("User-Agent", concat!("nudge/", env!("CARGO_PKG_VERSION")))
        .call();
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(401 | 403, _)) => return Err(FetchError::Auth),
        Err(ureq::Error::Status(code, _)) => return Err(FetchError::Other(format!("http {code}"))),
        Err(ureq::Error::Transport(t)) => return Err(FetchError::Other(format!("transport: {}", t.kind()))),
    };
    let mut body = Vec::new();
    resp.into_reader()
        .take(MAX_BODY)
        .read_to_end(&mut body)
        .map_err(|e| FetchError::Other(format!("read: {}", e.kind())))?;
    serde_json::from_slice(&body).map_err(|_| FetchError::Other("body is not JSON".into()))
}
