//! The elapsed-time label (Swift `elapsed`, `WidgetView.swift`) and when it next changes.

use claude_sessions_core::text::{closed_ago, elapsed, next_closed_change, next_label_change};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn t(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn label(age: u64) -> String {
    elapsed(Some(t(1_000_000)), t(1_000_000 + age))
}

#[test]
fn elapsed_matches_swift_literals() {
    assert_eq!(label(0), "0s");
    assert_eq!(label(59), "59s");
    assert_eq!(label(60), "1m");
    assert_eq!(label(59 * 60), "59m");
    assert_eq!(label(2 * 3600), "2h00");
    assert_eq!(label(3600 + 5 * 60 + 59), "1h05");
    assert_eq!(label(86_400), "24h00");
    assert_eq!(elapsed(None, t(5)), "");
}

#[test]
fn next_change_is_one_second_under_a_minute() {
    let since = t(1_000_000);
    for age in [0, 1, 30, 59] {
        assert_eq!(
            next_label_change(since, t(1_000_000 + age)),
            Duration::from_secs(1)
        );
    }
}

#[test]
fn next_change_is_the_next_minute_boundary_after() {
    let since = t(1_000_000);
    assert_eq!(
        next_label_change(since, t(1_000_060)),
        Duration::from_secs(60)
    );
    assert_eq!(
        next_label_change(since, t(1_000_061)),
        Duration::from_secs(59)
    );
    assert_eq!(
        next_label_change(since, t(1_000_119)),
        Duration::from_secs(1)
    );
    let half = since + Duration::from_millis(90_500);
    assert_eq!(
        next_label_change(since, half),
        Duration::from_millis(29_500)
    );
    assert_eq!(
        next_label_change(since, t(1_000_000 + 7200 + 10)),
        Duration::from_secs(50)
    );
}

#[test]
fn the_label_really_changes_at_the_reported_time() {
    let since = t(1_000_000);
    // From a minute on the bound is exact; under a minute it is the 1 s tick, an upper bound.
    for age_ms in [60_000u64, 61_250, 3_599_500, 7_200_001] {
        let now = since + Duration::from_millis(age_ms);
        let wait = next_label_change(since, now);
        assert_eq!(
            elapsed(Some(since), now + wait - Duration::from_millis(1)),
            elapsed(Some(since), now)
        );
        assert_ne!(elapsed(Some(since), now + wait), elapsed(Some(since), now));
    }
    for age_ms in [0u64, 400, 59_999] {
        let now = since + Duration::from_millis(age_ms);
        assert_ne!(
            elapsed(Some(since), now + next_label_change(since, now)),
            elapsed(Some(since), now)
        );
    }
}

#[test]
fn closed_label_changes_at_the_reported_time() {
    let since = t(1_000_000);
    for age_ms in [0u64, 59_999, 60_000, 3_599_500, 3_600_000, 7_300_000] {
        let now = since + Duration::from_millis(age_ms);
        let wait = next_closed_change(since, now);
        let same =
            closed_ago(since, now + wait - Duration::from_millis(1)) == closed_ago(since, now);
        // Under the first minute `12p` stays `1p` (a floor of one), so only later boundaries must change.
        if age_ms >= 60_000 {
            assert!(same);
            assert_ne!(closed_ago(since, now + wait), closed_ago(since, now));
        }
    }
}
