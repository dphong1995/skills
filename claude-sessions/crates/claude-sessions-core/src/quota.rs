//! The OMC HUD usage cache: 5-hour and weekly quota, plus a forecast of when the 5-hour quota runs out.

use crate::file_stamp::FileStamp;
use crate::shell::parse_iso_date;
use crate::time::{add_secs, from_unix_secs, secs_since};
use serde_json::Value;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq)]
pub struct Quota {
    pub five_hour: i64,
    pub five_hour_reset: SystemTime,
    pub weekly: i64,
    pub weekly_reset: SystemTime,
    pub forecast: Option<SystemTime>,
}

impl Quota {
    /// The percent shown on the collapsed pill: the larger of the two, only from 80.
    pub fn pill_percent(&self) -> Option<i64> {
        let m = self.five_hour.max(self.weekly);
        (m >= 80).then_some(m)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaLevel {
    Normal,
    Warn,
    High,
}

impl QuotaLevel {
    pub fn level(percent: i64) -> QuotaLevel {
        if percent >= 95 {
            QuotaLevel::High
        } else if percent >= 80 {
            QuotaLevel::Warn
        } else {
            QuotaLevel::Normal
        }
    }
}

/// Reads the OMC HUD usage cache; re-reads only when the file changed and at most every 10 s.
pub struct QuotaReader {
    url: PathBuf,
    stamp: Option<FileStamp>,
    last_read: Option<SystemTime>,
    cached: Option<Quota>,
    samples: Vec<(SystemTime, i64)>,
    window: Option<SystemTime>,
}

impl QuotaReader {
    pub fn new(url: PathBuf) -> Self {
        Self {
            url,
            stamp: None,
            last_read: None,
            cached: None,
            samples: Vec::new(),
            window: None,
        }
    }

    pub fn read(&mut self, now: SystemTime) -> Option<Quota> {
        if let Some(last) = self.last_read
            && secs_since(now, last) < 10.0
        {
            return self.cached.clone();
        }
        let st = FileStamp::new(&self.url);
        if st.is_some() && st == self.stamp {
            self.last_read = Some(now);
            return self.cached.clone();
        }
        self.last_read = Some(now);
        self.stamp = st;
        self.cached = self.parse(now);
        self.cached.clone()
    }

    fn parse(&mut self, now: SystemTime) -> Option<Quota> {
        let data = std::fs::read(&self.url).ok()?;
        let Value::Object(o) = serde_json::from_slice(&data).ok()? else {
            return None;
        };
        if o.get("error").and_then(Value::as_bool) == Some(true) {
            return None;
        }
        let d = o.get("data")?.as_object()?;
        let percent = |key: &str| d.get(key).and_then(Value::as_f64).map(|p| p.round() as i64);
        let five = percent("fiveHourPercent")?;
        let weekly = percent("weeklyPercent")?;
        let five_reset = d
            .get("fiveHourResetsAt")
            .and_then(Value::as_str)
            .and_then(parse_iso_date)?;
        let weekly_reset = d
            .get("weeklyResetsAt")
            .and_then(Value::as_str)
            .and_then(parse_iso_date)?;
        let last = o.get("lastSuccessAt").and_then(Value::as_f64)?;
        let updated = from_unix_secs(last / 1000.0);
        if self.window != Some(five_reset) {
            self.window = Some(five_reset);
            self.samples.clear();
        }
        if self.samples.last().map(|s| s.0) != Some(updated) {
            self.samples.push((updated, five));
            if self.samples.len() > 60 {
                self.samples.drain(..self.samples.len() - 60);
            }
        }
        Some(Quota {
            five_hour: five,
            five_hour_reset: five_reset,
            weekly,
            weekly_reset,
            forecast: Self::forecast(&self.samples, now, five_reset),
        })
    }

    /// When the 5h percent reaches 100 at the pace of the last 30 minutes; `None` without two samples 5+ minutes apart,
    /// a rising pace, or a hit after `reset`.
    pub fn forecast(
        samples: &[(SystemTime, i64)],
        now: SystemTime,
        reset: SystemTime,
    ) -> Option<SystemTime> {
        let recent: Vec<_> = samples
            .iter()
            .filter(|s| secs_since(now, s.0) <= 1800.0)
            .collect();
        let (first, last) = (recent.first()?, recent.last()?);
        let span = secs_since(last.0, first.0);
        if span < 300.0 {
            return None;
        }
        let rate = (last.1 - first.1) as f64 / span;
        if rate <= 0.0 {
            return None;
        }
        let hit = add_secs(now, (100 - last.1) as f64 / rate);
        (hit < reset).then_some(hit)
    }
}
