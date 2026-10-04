//! Read Claude Code's OAuth access token (D3 opt-in only, spec §Security).
//! The token lives only in memory, inside `Token`, whose `Debug` is redacted.
//! Nothing in this module logs, prints, or persists it.

use std::path::PathBuf;

pub struct Token {
    secret: String,
    /// `claudeAiOauth.expiresAt`, unix milliseconds.
    pub expires_at_ms: Option<u64>,
}

impl Token {
    pub fn secret(&self) -> &str {
        &self.secret
    }
    #[cfg(test)]
    pub fn for_test(expires_at_ms: Option<u64>) -> Self {
        Token { secret: "test-token".into(), expires_at_ms }
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Token").field("secret", &"<redacted>").field("expires_at_ms", &self.expires_at_ms).finish()
    }
}

/// Parse the credentials JSON (`{"claudeAiOauth": {"accessToken", "expiresAt", ...}}`).
pub fn parse_credentials(bytes: &[u8]) -> Option<Token> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let o = v.get("claudeAiOauth")?;
    let secret = o.get("accessToken")?.as_str().filter(|s| !s.is_empty())?.to_owned();
    let expires_at_ms = o.get("expiresAt").and_then(|e| e.as_u64().or_else(|| e.as_f64().map(|f| f as u64)));
    Some(Token { secret, expires_at_ms })
}

/// The real token source. `None` on any failure (no file, not logged in, bad JSON).
pub fn read_token() -> Option<Token> {
    parse_credentials(&read_credentials_bytes()?)
}

#[cfg(not(target_os = "macos"))]
fn read_credentials_bytes() -> Option<Vec<u8>> {
    std::fs::read(claude_config_dir()?.join(".credentials.json")).ok()
}

#[cfg(target_os = "macos")]
fn read_credentials_bytes() -> Option<Vec<u8>> {
    use std::process::{Command, Stdio};
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// `CLAUDE_CONFIG_DIR`, else `~/.claude`.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn claude_config_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).filter(|p| !p.is_empty())?;
    Some(PathBuf::from(home).join(".claude"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_credentials_shape() {
        let raw = br#"{"mcpOAuth":{"x":{"accessToken":"other"}},
            "claudeAiOauth":{"accessToken":"abc","refreshToken":"r","expiresAt":1791136800000,
            "refreshTokenExpiresAt":1,"scopes":["user:inference"],"subscriptionType":"max","rateLimitTier":"t"}}"#;
        let t = parse_credentials(raw).unwrap();
        assert_eq!(t.secret(), "abc");
        assert_eq!(t.expires_at_ms, Some(1_791_136_800_000));
    }

    #[test]
    fn rejects_missing_or_empty_token() {
        assert!(parse_credentials(br#"{"claudeAiOauth":{"accessToken":""}}"#).is_none());
        assert!(parse_credentials(br#"{"mcpOAuth":{}}"#).is_none());
        assert!(parse_credentials(b"not json").is_none());
    }

    #[test]
    fn debug_is_redacted() {
        let t = parse_credentials(br#"{"claudeAiOauth":{"accessToken":"s3cret"}}"#).unwrap();
        assert!(!format!("{t:?}").contains("s3cret"));
    }
}
