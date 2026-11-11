//! Signed `f64` seconds between `SystemTime`s, where Swift subtracts `Date`s (`timeIntervalSince`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// `later - earlier` in seconds; negative when `later` is before `earlier`.
pub fn secs_since(later: SystemTime, earlier: SystemTime) -> f64 {
    match later.duration_since(earlier) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    }
}

/// `t` moved by `secs` (negative moves back); `None` for NaN, infinite or out-of-range values.
pub fn checked_add_secs(t: SystemTime, secs: f64) -> Option<SystemTime> {
    if secs >= 0.0 {
        t.checked_add(Duration::try_from_secs_f64(secs).ok()?)
    } else {
        t.checked_sub(Duration::try_from_secs_f64(-secs).ok()?)
    }
}

/// `t` moved by `secs`; `t` itself when the value is unusable (see `checked_add_secs`).
pub fn add_secs(t: SystemTime, secs: f64) -> SystemTime {
    checked_add_secs(t, secs).unwrap_or(t)
}

/// Seconds since 1970-01-01.
pub fn unix_secs(t: SystemTime) -> f64 {
    secs_since(t, UNIX_EPOCH)
}

/// The time `secs` seconds after 1970-01-01; `None` for NaN, infinite or out-of-range values.
pub fn checked_from_unix_secs(secs: f64) -> Option<SystemTime> {
    checked_add_secs(UNIX_EPOCH, secs)
}

/// The time `secs` seconds after 1970-01-01; the epoch when the value is unusable.
pub fn from_unix_secs(secs: f64) -> SystemTime {
    add_secs(UNIX_EPOCH, secs)
}
