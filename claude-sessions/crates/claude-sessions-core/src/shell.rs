//! Runs a program and returns its stdout; plus the ISO 8601 helpers Swift keeps next to it (`ISODate`).

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Runs `path` (an absolute path, never looked up in `PATH`) and returns its stdout, or `None` when it cannot start.
/// A timeout gives `None` with `require_success`; without it the stdout read so far is returned (as Swift does).
/// With `require_success` a non-zero exit status also gives `None`.
/// `input`, when given, is written to the process's stdin, which is then closed. stderr goes to null.
pub fn run_process(
    path: &Path,
    args: &[&str],
    timeout: Duration,
    require_success: bool,
    input: Option<&[u8]>,
) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::null());
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    if let (Some(mut stdin), Some(input)) = (child.stdin.take(), input) {
        let _ = stdin.write_all(input);
        // Dropping `stdin` closes it.
    }
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                break child.wait().ok()?;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => return None,
        }
    };
    let data = reader.join().ok()?;
    if require_success && !status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&data).into_owned())
}

/// Parses an ISO 8601 timestamp with a time zone, with or without fractional seconds (`2026-11-06T11:00:00.171Z`).
pub fn parse_iso_date(s: &str) -> Option<SystemTime> {
    let b = s.as_bytes();
    let num = |from: usize, len: usize| -> Option<i64> {
        let d = b.get(from..from + len)?;
        d.iter()
            .all(u8::is_ascii_digit)
            .then(|| d.iter().fold(0, |n, c| n * 10 + i64::from(c - b'0')))
    };
    let sep = |at: usize, c: u8| (b.get(at) == Some(&c)).then_some(());
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    sep(4, b'-')?;
    sep(7, b'-')?;
    sep(10, b'T')?;
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    sep(13, b':')?;
    sep(16, b':')?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut i = 19;
    let mut fraction = "";
    if b.get(i) == Some(&b'.') {
        let start = i + 1;
        i = start;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
        fraction = &s[start..i];
    }
    let offset = match b.get(i)? {
        b'Z' if i + 1 == b.len() => 0,
        sign @ (b'+' | b'-') => {
            let (h, m) = match b.len() - i - 1 {
                5 => (num(i + 1, 2)?, num(i + 4, 2)?),
                4 => (num(i + 1, 2)?, num(i + 3, 2)?),
                _ => return None,
            };
            if b.len() - i - 1 == 5 {
                sep(i + 3, b':')?;
            }
            let secs = h * 3600 + m * 60;
            if *sign == b'+' { secs } else { -secs }
        }
        _ => return None,
    };
    let whole =
        days_from_civil(year, month, day) * 86400 + hour * 3600 + minute * 60 + second - offset;
    // Built from the decimal text so the result is the same f64 as the literal `whole.fraction`.
    let secs: f64 = if fraction.is_empty() {
        whole as f64
    } else {
        format!("{whole}.{fraction}").parse().ok()?
    };
    Some(UNIX_EPOCH + Duration::try_from_secs_f64(secs).ok()?)
}

/// `2026-11-06T11:00:00.171Z`: UTC with millisecond precision.
pub fn format_iso_date(t: SystemTime) -> String {
    let ms = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let total_ms = (ms * 1000.0).round() as i64;
    let (secs, millis) = (total_ms.div_euclid(1000), total_ms.rem_euclid(1000));
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_round_trip_and_offsets() {
        let t = parse_iso_date("2026-11-06T11:00:00.171Z").unwrap();
        assert_eq!(t, UNIX_EPOCH + Duration::from_secs_f64(1793962800.171));
        assert_eq!(format_iso_date(t), "2026-11-06T11:00:00.171Z");
        assert_eq!(
            parse_iso_date("2026-11-03T03:33:15+07:00"),
            parse_iso_date("2026-11-02T20:33:15Z")
        );
        assert!(parse_iso_date("2026-11-03T03:33:15.111541+00:00").is_some());
        assert_eq!(parse_iso_date("2026-11-03T03:33:15"), None);
        assert_eq!(parse_iso_date("nope"), None);
    }
}
