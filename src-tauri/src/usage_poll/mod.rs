//! D3 opt-in live usage poller (lane L4).
//!
//! Every 60 s: if `config.usage.live_when_idle` is on and the statusline isn't
//! already supplying fresh numbers, read the local OAuth token and GET the
//! usage endpoint. With the option off, no credential is ever read.

mod http;
mod parse;
mod token;

use crate::hub::Core;
use crate::usage::UsageSource;
use http::FetchError;
use std::sync::Arc;
use std::time::Duration;
use token::Token;

const TICK: Duration = Duration::from_secs(60);
const MIN_MS: u64 = 60_000;
/// Statusline data younger than this means there's no need to poll.
const STATUSLINE_FRESH_MS: u64 = 5 * MIN_MS;
const AUTH_BACKOFF_MS: u64 = 30 * MIN_MS;
const ERROR_BACKOFF_MIN: [u64; 4] = [1, 2, 5, 15];

#[derive(Debug, PartialEq, Eq)]
pub enum Tick {
    Disabled,
    StatuslineFresh,
    BackingOff,
    NoToken,
    TokenExpired,
    Applied,
    AuthRejected,
    Failed,
}

pub struct Poller<R, F> {
    read_token: R,
    fetch: F,
    next_at_ms: u64,
    failures: usize,
}

impl<R, F> Poller<R, F>
where
    R: FnMut() -> Option<Token>,
    F: FnMut(&Token) -> Result<serde_json::Value, FetchError>,
{
    pub fn new(read_token: R, fetch: F) -> Self {
        Poller { read_token, fetch, next_at_ms: 0, failures: 0 }
    }

    pub fn tick(&mut self, core: &Core, now_ms: u64) -> Tick {
        if !core.config().usage.live_when_idle {
            return Tick::Disabled;
        }
        let usage = core.snapshot().usage;
        if usage.source == Some(UsageSource::Statusline) && now_ms.saturating_sub(usage.updated_ms) < STATUSLINE_FRESH_MS {
            return Tick::StatuslineFresh;
        }
        if now_ms < self.next_at_ms {
            return Tick::BackingOff;
        }
        let Some(tok) = (self.read_token)() else { return Tick::NoToken };
        // Claude Code owns the refresh; an expired token is skipped, never refreshed here.
        if tok.expires_at_ms.is_some_and(|e| e <= now_ms) {
            return Tick::TokenExpired;
        }
        let result = (self.fetch)(&tok);
        drop(tok);
        match result.map(|body| parse::parse_usage(&body)) {
            Ok(Some((five, seven))) => {
                self.failures = 0;
                self.next_at_ms = 0;
                core.apply_poll(five, seven, now_ms);
                Tick::Applied
            }
            Err(FetchError::Auth) => {
                self.next_at_ms = now_ms + AUTH_BACKOFF_MS;
                Tick::AuthRejected
            }
            Ok(None) | Err(FetchError::Other(_)) => {
                let step = ERROR_BACKOFF_MIN[self.failures.min(ERROR_BACKOFF_MIN.len() - 1)];
                self.failures += 1;
                self.next_at_ms = now_ms + step * MIN_MS;
                Tick::Failed
            }
        }
    }
}

/// Start the background poller. It does nothing (and reads no credentials)
/// while `config.usage.live_when_idle` is false.
pub fn spawn(core: Arc<Core>) {
    let res = std::thread::Builder::new().name("nudge-usage-poll".into()).spawn(move || {
        let mut p = Poller::new(token::read_token, http::fetch_usage);
        loop {
            let t = p.tick(&core, nudge_proto::now_ms());
            if matches!(t, Tick::AuthRejected | Tick::Failed) {
                // Outcome only: never the token, never a response body.
                eprintln!("nudge: live usage poll: {t:?}");
            }
            std::thread::sleep(TICK);
        }
    });
    if let Err(e) = res {
        eprintln!("nudge: usage poller thread failed to start: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::hub::{Sink, Snapshot};
    use crate::server::Inbound;
    use crate::session::Transition;
    use nudge_proto::{RateLimits, RateWindow, StatusEnvelope};
    use serde_json::json;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Null;
    impl Sink for Null {
        fn snapshot(&self, _: &Snapshot) {}
        fn transitions(&self, _: &[Transition], _: &Config) {}
    }

    static N: AtomicUsize = AtomicUsize::new(0);

    fn core(enabled: bool) -> Core {
        let p = std::env::temp_dir().join(format!("nudge-poll-{}-{}.json", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_file(&p);
        let c = Core::new(p, Arc::new(Null));
        let mut cfg = Config::default();
        cfg.usage.live_when_idle = enabled;
        c.set_config(cfg).unwrap();
        c
    }

    fn statusline(c: &Core, ts_ms: u64) {
        c.handle(Inbound::Status(StatusEnvelope {
            v: 1,
            ts_ms,
            session_id: None,
            rate_limits_available: Some(true),
            rate_limits: Some(RateLimits { five_hour: Some(RateWindow { used_percentage: 1.0, resets_at: None }), seven_day: None }),
            context_used_percentage: None,
        }));
    }

    fn ok_body() -> serde_json::Value {
        json!({"five_hour": {"utilization": 40.0, "resets_at": "2026-10-04T18:00:00Z"},
               "seven_day": {"utilization": 10.0, "resets_at": "2026-10-09T00:00:00Z"}})
    }

    const NOW: u64 = 1_791_000_000_000;

    #[test]
    fn disabled_reads_no_token_and_fetches_nothing() {
        let c = core(false);
        let (reads, fetches) = (Cell::new(0), Cell::new(0));
        let mut p = Poller::new(
            || {
                reads.set(reads.get() + 1);
                Some(Token::for_test(None))
            },
            |_: &Token| {
                fetches.set(fetches.get() + 1);
                Ok(ok_body())
            },
        );
        for i in 0..5 {
            assert_eq!(p.tick(&c, NOW + i * MIN_MS), Tick::Disabled);
        }
        assert_eq!((reads.get(), fetches.get()), (0, 0));
        assert_eq!(c.snapshot().usage.source, None);
    }

    #[test]
    fn fresh_statusline_skips_and_stale_fetches() {
        let c = core(true);
        statusline(&c, NOW - 2 * MIN_MS);
        let reads = Cell::new(0);
        let mut p = Poller::new(
            || {
                reads.set(reads.get() + 1);
                Some(Token::for_test(Some(NOW + 3_600_000)))
            },
            |_: &Token| Ok(ok_body()),
        );
        assert_eq!(p.tick(&c, NOW), Tick::StatuslineFresh);
        assert_eq!(reads.get(), 0);
        // Same statusline data, now 6 minutes old.
        assert_eq!(p.tick(&c, NOW + 4 * MIN_MS), Tick::Applied);
        let u = c.snapshot().usage;
        assert_eq!(u.source, Some(UsageSource::Poll));
        assert_eq!(u.five_hour.unwrap().used_percentage, 40.0);
        assert_eq!(u.seven_day.unwrap().resets_at, Some(1_791_504_000));
        // Our own poll data doesn't count as "fresh statusline": keep polling.
        assert_eq!(p.tick(&c, NOW + 5 * MIN_MS), Tick::Applied);
    }

    #[test]
    fn expired_token_skips_fetch() {
        let c = core(true);
        let fetches = Cell::new(0);
        let mut p = Poller::new(
            || Some(Token::for_test(Some(NOW - 1))),
            |_: &Token| {
                fetches.set(fetches.get() + 1);
                Ok(ok_body())
            },
        );
        assert_eq!(p.tick(&c, NOW), Tick::TokenExpired);
        assert_eq!(fetches.get(), 0);
        let mut p = Poller::new(|| None, |_: &Token| Ok(ok_body()));
        assert_eq!(p.tick(&c, NOW), Tick::NoToken);
    }

    #[test]
    fn auth_failure_backs_off_30_minutes() {
        let c = core(true);
        let fetches = Cell::new(0);
        let mut p = Poller::new(
            || Some(Token::for_test(None)),
            |_: &Token| {
                fetches.set(fetches.get() + 1);
                Err(FetchError::Auth)
            },
        );
        assert_eq!(p.tick(&c, NOW), Tick::AuthRejected);
        assert_eq!(p.tick(&c, NOW + MIN_MS), Tick::BackingOff);
        assert_eq!(p.tick(&c, NOW + 29 * MIN_MS), Tick::BackingOff);
        assert_eq!(fetches.get(), 1);
        assert_eq!(p.tick(&c, NOW + 30 * MIN_MS), Tick::AuthRejected);
        assert_eq!(fetches.get(), 2);
    }

    #[test]
    fn other_errors_back_off_1_2_5_15_then_reset_on_success() {
        let c = core(true);
        let fail = Cell::new(true);
        let mut p = Poller::new(
            || Some(Token::for_test(None)),
            |_: &Token| if fail.get() { Err(FetchError::Other("http 500".into())) } else { Ok(ok_body()) },
        );
        let mut t = NOW;
        for step in [1, 2, 5, 15, 15] {
            assert_eq!(p.tick(&c, t), Tick::Failed);
            assert_eq!(p.tick(&c, t + step * MIN_MS - 1), Tick::BackingOff);
            t += step * MIN_MS;
        }
        fail.set(false);
        assert_eq!(p.tick(&c, t), Tick::Applied);
        fail.set(true);
        assert_eq!(p.tick(&c, t + 1), Tick::Failed);
        assert_eq!(p.tick(&c, t + MIN_MS), Tick::BackingOff);
        assert_eq!(p.tick(&c, t + 1 + MIN_MS), Tick::Failed, "backoff restarted at 1 min");
    }

    #[test]
    fn unparseable_body_counts_as_error() {
        let c = core(true);
        let mut p = Poller::new(|| Some(Token::for_test(None)), |_: &Token| Ok(json!({"error": "x"})));
        assert_eq!(p.tick(&c, NOW), Tick::Failed);
        assert_eq!(c.snapshot().usage.source, None);
    }
}
