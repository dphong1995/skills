use crate::support::make_temp_dir;
use claude_sessions_core::quota::{QuotaLevel, QuotaReader};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LAST: f64 = 1793961364363.0;

struct Cache {
    five: f64,
    five_reset: &'static str,
    weekly: f64,
    error: bool,
    last: f64,
}

impl Default for Cache {
    fn default() -> Self {
        Cache {
            five: 30.0,
            five_reset: "2026-11-06T11:00:00.171Z",
            weekly: 5.0,
            error: false,
            last: LAST,
        }
    }
}

fn write_cache(url: &Path, c: Cache) {
    let o = json!({"timestamp": c.last, "data": {"fiveHourPercent": c.five, "fiveHourResetsAt": c.five_reset, "weeklyPercent": c.weekly,
        "weeklyResetsAt": "2026-11-13T06:00:00.171Z", "scopedWeeklyBuckets": []},
        "error": c.error, "source": "anthropic", "lastSuccessAt": c.last});
    fs::write(url, serde_json::to_vec(&o).unwrap()).unwrap();
}

/// The temp dir guard is returned so the file outlives the call.
fn cache_url() -> (tempfile::TempDir, PathBuf) {
    let d = make_temp_dir();
    let p = d.path().join("usage.json");
    (d, p)
}

fn at(secs: f64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs_f64(secs)
}

#[test]
fn parses_real_file() {
    let (_d, url) = cache_url();
    write_cache(&url, Cache::default());
    let q = QuotaReader::new(url).read(at(1793961364.0)).unwrap();
    assert_eq!(q.five_hour, 30);
    assert_eq!(q.weekly, 5);
    assert_eq!(q.five_hour_reset, at(1793962800.171));
    assert_eq!(q.weekly_reset, at(1794549600.171));
}

#[test]
fn fractional_percent_is_rounded() {
    let (_d, url) = cache_url();
    write_cache(
        &url,
        Cache {
            five: 37.5,
            weekly: 4.2,
            ..Cache::default()
        },
    );
    let q = QuotaReader::new(url).read(at(1793961364.0)).unwrap();
    assert_eq!(q.five_hour, 38);
    assert_eq!(q.weekly, 4);
}

#[test]
fn missing_and_stale() {
    let now = at(1793961364.0);
    let (_d1, missing) = cache_url();
    assert_eq!(QuotaReader::new(missing).read(now), None);
    let (_d2, bad) = cache_url();
    write_cache(
        &bad,
        Cache {
            error: true,
            ..Cache::default()
        },
    );
    assert_eq!(QuotaReader::new(bad).read(now), None);
    let (_d3, url) = cache_url();
    write_cache(&url, Cache::default());
    let later = now + Duration::from_secs(16 * 60);
    assert!(QuotaReader::new(url).read(later).is_some());
}

#[test]
fn forecast_linear() {
    let t0 = at(1_000_000.0);
    let t1 = t0 + Duration::from_secs(20 * 60);
    let f = QuotaReader::forecast(
        &[(t0, 20), (t1, 40)],
        t1,
        t0 + Duration::from_secs(3 * 3600),
    );
    assert_eq!(f, Some(t0 + Duration::from_secs(80 * 60)));
}

#[test]
fn forecast_needs_two_samples() {
    let t0 = at(1_000_000.0);
    let reset = t0 + Duration::from_secs(3 * 3600);
    assert_eq!(QuotaReader::forecast(&[(t0, 20)], t0, reset), None);
    let t1 = t0 + Duration::from_secs(4 * 60);
    assert_eq!(
        QuotaReader::forecast(&[(t0, 20), (t1, 40)], t1, reset),
        None
    );
}

#[test]
fn forecast_not_before_reset() {
    let t0 = at(1_000_000.0);
    let t1 = t0 + Duration::from_secs(20 * 60);
    assert_eq!(
        QuotaReader::forecast(&[(t0, 20), (t1, 40)], t1, t0 + Duration::from_secs(60 * 60)),
        None
    );
}

#[test]
fn window_change_clears_samples() {
    let (_d, url) = cache_url();
    let t0 = LAST;
    let mut reader = QuotaReader::new(url.clone());
    write_cache(
        &url,
        Cache {
            five: 20.0,
            five_reset: "2026-11-06T14:00:00.171Z",
            last: t0,
            ..Cache::default()
        },
    );
    reader.read(at(t0 / 1000.0));
    write_cache(
        &url,
        Cache {
            five: 40.0,
            five_reset: "2026-11-06T14:00:00.171Z",
            last: t0 + 20.0 * 60.0 * 1000.0,
            ..Cache::default()
        },
    );
    let now = at(t0 / 1000.0 + 20.0 * 60.0 + 11.0);
    assert!(reader.read(now).unwrap().forecast.is_some());
    write_cache(
        &url,
        Cache {
            five: 50.0,
            five_reset: "2026-11-06T19:00:00.171Z",
            last: t0 + 21.0 * 60.0 * 1000.0,
            ..Cache::default()
        },
    );
    assert!(
        reader
            .read(now + Duration::from_secs(11))
            .unwrap()
            .forecast
            .is_none()
    );
}

#[test]
fn levels() {
    assert_eq!(QuotaLevel::level(79), QuotaLevel::Normal);
    assert_eq!(QuotaLevel::level(80), QuotaLevel::Warn);
    assert_eq!(QuotaLevel::level(94), QuotaLevel::Warn);
    assert_eq!(QuotaLevel::level(95), QuotaLevel::High);
}
