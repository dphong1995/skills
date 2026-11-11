//! The recently closed sessions, kept so they can be resumed with `claude --resume`.

use crate::kv::KeyValueStore;
use crate::time::{from_unix_secs, secs_since, unix_secs};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::time::SystemTime;

/// Seconds between 1970-01-01 and Swift's reference date 2001-01-01, which `JSONEncoder` counts `Date`s from.
const REFERENCE_OFFSET: f64 = 978_307_200.0;
const CLOSED_KEY: &str = "closedSessions";

fn serialize_date<S: Serializer>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f64(unix_secs(*t) - REFERENCE_OFFSET)
}

fn deserialize_date<'de, D: Deserializer<'de>>(d: D) -> Result<SystemTime, D::Error> {
    Ok(from_unix_secs(f64::deserialize(d)? + REFERENCE_OFFSET))
}

/// A session whose process ended. Serialized as Swift's `JSONEncoder` does, so both builds read each other's data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClosedSession {
    pub id: String,
    pub title: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(
        rename = "closedAt",
        serialize_with = "serialize_date",
        deserialize_with = "deserialize_date"
    )]
    pub closed_at: SystemTime,
}

impl ClosedSession {
    pub fn folder(&self) -> String {
        folder(&self.cwd)
    }

    /// `claude --resume '<id>'` for a cmux workspace; `None` unless the id is a canonical UUID, so it is never shell-interpreted.
    pub fn resume_command(&self) -> Option<String> {
        is_canonical_uuid(&self.id).then(|| format!("claude --resume '{}'", self.id))
    }

    /// What the user can paste into a terminal when cmux cannot open the workspace; `None` for a non-UUID id.
    pub fn shell_command(&self) -> Option<String> {
        let resume = self.resume_command()?;
        Some(format!(
            "cd '{}' && {resume}",
            self.cwd.replace('\'', "'\\''")
        ))
    }
}

fn is_canonical_uuid(id: &str) -> bool {
    id.len() == 36 && uuid::Uuid::try_parse(id).is_ok()
}

/// The folder name of `cwd` (Foundation's `lastPathComponent`).
pub fn folder(cwd: &str) -> String {
    let t = cwd.trim_end_matches('/');
    if t.is_empty() && !cwd.is_empty() {
        return "/".into();
    }
    t.rsplit('/').next().unwrap_or("").to_string()
}

/// Newest first, at most `LIMIT`, none older than `MAX_AGE`, one per id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClosedSessions {
    items: Vec<ClosedSession>,
}

impl ClosedSessions {
    const LIMIT: usize = 5;
    const MAX_AGE: f64 = 24.0 * 3600.0;

    pub fn new(items: Vec<ClosedSession>) -> Self {
        Self { items }
    }

    pub fn items(&self) -> &[ClosedSession] {
        &self.items
    }

    pub fn add(&mut self, s: ClosedSession, now: SystemTime) {
        self.items.retain(|i| i.id != s.id);
        self.items.insert(0, s);
        self.prune(now);
    }

    pub fn revive(&mut self, ids: &HashSet<String>) {
        self.items.retain(|i| !ids.contains(&i.id));
    }

    pub fn prune(&mut self, now: SystemTime) {
        self.items
            .retain(|i| secs_since(now, i.closed_at) <= Self::MAX_AGE);
        self.items.truncate(Self::LIMIT);
    }
}

/// Saves and loads `ClosedSessions` under the `closedSessions` key: a JSON array, `Date`s as seconds since 2001-01-01.
pub struct ClosedStore {
    kv: Box<dyn KeyValueStore>,
}

impl ClosedStore {
    pub fn new(kv: Box<dyn KeyValueStore>) -> Self {
        Self { kv }
    }

    /// The saved list with what is older than 24 h dropped; empty when nothing usable is saved.
    pub fn load(&self, now: SystemTime) -> ClosedSessions {
        let items = self
            .kv
            .get_bytes(CLOSED_KEY)
            .and_then(|b| serde_json::from_slice::<Vec<ClosedSession>>(&b).ok());
        let mut loaded = ClosedSessions::new(items.unwrap_or_default());
        loaded.prune(now);
        loaded
    }

    pub fn save(&self, closed: &ClosedSessions) {
        if let Ok(bytes) = serde_json::to_vec(closed.items()) {
            self.kv.set_bytes(CLOSED_KEY, &bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_is_last_path_component() {
        assert_eq!(folder("/Users/x/proj"), "proj");
        assert_eq!(folder("/Users/x/proj/"), "proj");
        assert_eq!(folder("/"), "/");
    }
}
