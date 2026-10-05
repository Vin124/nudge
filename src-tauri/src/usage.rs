//! Latest 5h / weekly usage (D3). Fed by the statusline wrapper and, when the
//! user opts in, by the poller in `usage_poll.rs` (lane L4).

use nudge_proto::{RateWindow, StatusEnvelope};
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UsageSource {
    Statusline,
    Poll,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub used_percentage: f64,
    /// Unix seconds.
    pub resets_at: Option<i64>,
}

impl From<&RateWindow> for UsageWindow {
    fn from(w: &RateWindow) -> Self {
        UsageWindow { used_percentage: w.used_percentage.clamp(0.0, 100.0), resets_at: w.resets_at }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub five_hour: Option<UsageWindow>,
    pub seven_day: Option<UsageWindow>,
    /// None = never heard; Some(false) = API key / 3P provider (show "n/a", AC #8).
    pub available: Option<bool>,
    pub source: Option<UsageSource>,
    pub updated_ms: u64,
}

#[derive(Default)]
pub struct UsageStore {
    snap: UsageSnapshot,
}

impl UsageStore {
    pub fn snapshot(&self) -> &UsageSnapshot {
        &self.snap
    }

    /// Returns true if the snapshot changed.
    pub fn apply_status(&mut self, s: &StatusEnvelope) -> bool {
        if s.ts_ms < self.snap.updated_ms {
            return false;
        }
        let next = match &s.rate_limits {
            Some(rl) => UsageSnapshot {
                five_hour: rl.five_hour.as_ref().map(Into::into),
                seven_day: rl.seven_day.as_ref().map(Into::into),
                available: Some(true),
                source: Some(UsageSource::Statusline),
                updated_ms: s.ts_ms,
            },
            // No limits in this render: only record availability, keep any
            // numbers we already have (a poller may be supplying them).
            None => UsageSnapshot {
                available: s.rate_limits_available.or(self.snap.available),
                updated_ms: s.ts_ms,
                ..self.snap.clone()
            },
        };
        let changed = !same_ignoring_time(&next, &self.snap);
        self.snap = next;
        changed
    }

    /// Entry point for the opt-in poller (L4). Same newest-wins rule.
    pub fn apply_poll(&mut self, five_hour: Option<UsageWindow>, seven_day: Option<UsageWindow>, ts_ms: u64) -> bool {
        if ts_ms < self.snap.updated_ms {
            return false;
        }
        let next = UsageSnapshot { five_hour, seven_day, available: Some(true), source: Some(UsageSource::Poll), updated_ms: ts_ms };
        let changed = !same_ignoring_time(&next, &self.snap);
        self.snap = next;
        changed
    }
}

fn same_ignoring_time(a: &UsageSnapshot, b: &UsageSnapshot) -> bool {
    a.five_hour == b.five_hour && a.seven_day == b.seven_day && a.available == b.available && a.source == b.source
}

#[cfg(test)]
mod tests {
    use super::*;
    use nudge_proto::RateLimits;

    fn status(ts: u64, five: Option<f64>, available: Option<bool>) -> StatusEnvelope {
        StatusEnvelope {
            v: 1,
            ts_ms: ts,
            session_id: None,
            rate_limits_available: available,
            rate_limits: five.map(|p| RateLimits {
                five_hour: Some(RateWindow { used_percentage: p, resets_at: Some(100) }),
                seven_day: None,
            }),
            context_used_percentage: None,
        }
    }

    #[test]
    fn statusline_sets_numbers() {
        let mut u = UsageStore::default();
        assert!(u.apply_status(&status(1, Some(42.0), Some(true))));
        assert_eq!(u.snapshot().five_hour.as_ref().unwrap().used_percentage, 42.0);
        assert_eq!(u.snapshot().source, Some(UsageSource::Statusline));
        assert!(!u.apply_status(&status(2, Some(42.0), Some(true))), "same numbers = no change");
    }

    #[test]
    fn older_update_is_ignored() {
        let mut u = UsageStore::default();
        u.apply_status(&status(10, Some(50.0), None));
        assert!(!u.apply_status(&status(5, Some(1.0), None)));
        assert_eq!(u.snapshot().five_hour.as_ref().unwrap().used_percentage, 50.0);
    }

    #[test]
    fn api_key_user_marks_unavailable() {
        let mut u = UsageStore::default();
        assert!(u.apply_status(&status(1, None, Some(false))));
        assert_eq!(u.snapshot().available, Some(false));
        assert!(u.snapshot().five_hour.is_none());
    }

    #[test]
    fn render_without_limits_keeps_polled_numbers() {
        let mut u = UsageStore::default();
        u.apply_poll(Some(UsageWindow { used_percentage: 30.0, resets_at: None }), None, 5);
        u.apply_status(&status(6, None, Some(false)));
        assert_eq!(u.snapshot().five_hour.as_ref().unwrap().used_percentage, 30.0);
    }

    #[test]
    fn percentages_are_clamped() {
        let mut u = UsageStore::default();
        u.apply_status(&status(1, Some(140.0), None));
        assert_eq!(u.snapshot().five_hour.as_ref().unwrap().used_percentage, 100.0);
    }
}
