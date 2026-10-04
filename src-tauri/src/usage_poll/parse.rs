//! Parse the `/api/oauth/usage` response (D3 opt-in).
//!
//! Shape observed on 2026-10-04 (Claude Code 2.1.288, Max account), field names
//! and types only: `five_hour` / `seven_day` = object
//! `{ utilization: number (0–100), resets_at: ISO-8601 string, limit_dollars,
//! used_dollars, remaining_dollars, locked_reason: null }`, plus many other
//! keys we ignore. Everything is parsed leniently: a missing or malformed
//! window is `None`, never an error.

use crate::usage::UsageWindow;
use serde_json::Value;

/// `None` when the body has neither window (garbage, error body, new shape).
pub fn parse_usage(body: &Value) -> Option<(Option<UsageWindow>, Option<UsageWindow>)> {
    let five = raw_window(body.get("five_hour"));
    let seven = raw_window(body.get("seven_day"));
    if five.is_none() && seven.is_none() {
        return None;
    }
    // Always 0–100, the observed contract. A per-response "fraction" guess
    // (all values ≤ 1) misread low real usage, e.g. 0.4% / 0.7% as 40% / 70%.
    let mk = |w: Option<(f64, Option<i64>)>| {
        w.map(|(u, r)| UsageWindow { used_percentage: u.clamp(0.0, 100.0), resets_at: r })
    };
    Some((mk(five), mk(seven)))
}

fn raw_window(v: Option<&Value>) -> Option<(f64, Option<i64>)> {
    let v = v?;
    let u = v
        .get("utilization")
        .or_else(|| v.get("used_percentage"))
        .and_then(Value::as_f64)
        .filter(|u| u.is_finite() && *u >= 0.0)?;
    let resets = match v.get("resets_at") {
        Some(Value::String(s)) => iso_to_unix(s),
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).map(secs_from_maybe_ms),
        _ => None,
    };
    Some((u, resets))
}

/// Numeric timestamps above ~year 33658 in seconds are really milliseconds.
fn secs_from_maybe_ms(n: i64) -> i64 {
    if n > 1_000_000_000_000 {
        n / 1000
    } else {
        n
    }
}

/// `YYYY-MM-DDTHH:MM:SS[.frac](Z|±HH:MM|±HHMM)` → unix seconds. No chrono needed.
pub fn iso_to_unix(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || !matches!(b[10], b'T' | b't' | b' ') || b[4] != b'-' || b[7] != b'-' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = s.get(r)?;
        if t.bytes().all(|c| c.is_ascii_digit()) { t.parse().ok() } else { None }
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut rest = &s[19..];
    if let Some(r) = rest.strip_prefix('.') {
        let n = r.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            return None;
        }
        rest = &r[n..];
    }
    let offset = match rest {
        "Z" | "z" | "" => 0,
        _ => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let digits: String = rest[1..].chars().filter(|c| *c != ':').collect();
            if digits.len() != 4 || !digits.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let oh: i64 = digits[..2].parse().ok()?;
            let om: i64 = digits[2..].parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
    };
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se - offset)
}

/// Howard Hinnant's days_from_civil.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Mirrors the observed response shape (values are made up).
    fn observed() -> Value {
        let win = |u: f64, r: &str| {
            json!({"utilization": u, "resets_at": r, "limit_dollars": null, "used_dollars": null,
                   "remaining_dollars": null, "locked_reason": null})
        };
        json!({
            "five_hour": win(37.0, "2026-10-04T18:00:00.512345+00:00"),
            "seven_day": win(12.5, "2026-10-09T00:00:00Z"),
            "seven_day_opus": null,
            "seven_day_sonnet": null,
            "iguana_necktie": {"utilization": 3.0, "resets_at": "2026-11-01T00:00:00Z",
                               "limit_dollars": 50.0, "used_dollars": 1.5, "remaining_dollars": 48.5, "locked_reason": null},
            "extra_usage": {"is_enabled": false, "utilization": null},
            "limits": [{"kind": "x", "group": "y", "percent": 37.0, "severity": "ok",
                        "resets_at": "2026-10-04T18:00:00Z", "scope": null, "is_active": true}],
            "member_dashboard_available": false
        })
    }

    #[test]
    fn parses_observed_shape() {
        let (five, seven) = parse_usage(&observed()).unwrap();
        let five = five.unwrap();
        assert_eq!(five.used_percentage, 37.0);
        assert_eq!(five.resets_at, Some(1_791_136_800));
        let seven = seven.unwrap();
        assert_eq!(seven.used_percentage, 12.5);
        assert_eq!(seven.resets_at, Some(1_791_504_000));
    }

    #[test]
    fn low_usage_is_not_mistaken_for_fractions() {
        // Early in a window both values are below 1 percent. They must stay percent.
        let b = json!({"five_hour": {"utilization": 0.4}, "seven_day": {"utilization": 0.7}});
        let (f, s) = parse_usage(&b).unwrap();
        assert_eq!(f.unwrap().used_percentage, 0.4);
        assert_eq!(s.unwrap().used_percentage, 0.7);
    }

    #[test]
    fn percent_scale_kept_when_any_value_above_one() {
        let b = json!({"five_hour": {"utilization": 0.5}, "seven_day": {"utilization": 20.0}});
        let (f, _) = parse_usage(&b).unwrap();
        assert_eq!(f.unwrap().used_percentage, 0.5);
        let b = json!({"five_hour": {"utilization": 1.0}, "seven_day": {"utilization": 0.0}});
        assert_eq!(parse_usage(&b).unwrap().0.unwrap().used_percentage, 1.0, "integers stay percent");
        let b = json!({"five_hour": {"utilization": 250.0}});
        assert_eq!(parse_usage(&b).unwrap().0.unwrap().used_percentage, 100.0, "clamped");
    }

    #[test]
    fn numeric_and_iso_resets_at() {
        let b = json!({"five_hour": {"utilization": 5, "resets_at": 1_791_136_800},
                       "seven_day": {"utilization": 5, "resets_at": 1_791_504_000_000u64}});
        let (f, s) = parse_usage(&b).unwrap();
        assert_eq!(f.unwrap().resets_at, Some(1_791_136_800));
        assert_eq!(s.unwrap().resets_at, Some(1_791_504_000), "ms → s");
        assert_eq!(iso_to_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_to_unix("2026-10-04T20:00:00+02:00"), Some(1_791_136_800));
        assert_eq!(iso_to_unix("2026-10-04T13:00:00-0500"), Some(1_791_136_800));
        assert_eq!(iso_to_unix("2000-02-29T00:00:00"), Some(951_782_400));
        assert_eq!(iso_to_unix("not a date"), None);
        assert_eq!(iso_to_unix("2026-13-04T00:00:00Z"), None);
        assert_eq!(iso_to_unix("2026-10-04T00:00:00Q"), None);
    }

    #[test]
    fn missing_seven_day_and_bad_resets_tolerated() {
        let b = json!({"five_hour": {"utilization": 9, "resets_at": "soon"}, "seven_day": null});
        let (f, s) = parse_usage(&b).unwrap();
        assert_eq!(f.unwrap(), UsageWindow { used_percentage: 9.0, resets_at: None });
        assert!(s.is_none());
    }

    #[test]
    fn garbage_is_none() {
        assert!(parse_usage(&json!("nope")).is_none());
        assert!(parse_usage(&json!({})).is_none());
        assert!(parse_usage(&json!({"error": {"type": "authentication_error"}})).is_none());
        assert!(parse_usage(&json!({"five_hour": {"utilization": "high"}})).is_none());
        assert!(parse_usage(&json!({"five_hour": {"utilization": -3}})).is_none());
    }
}
