//! Polls Jira for the titles and statuses of the issue keys the sessions mention, off the main thread.

use crate::credentials::ConnectionState;
use crate::jira::{self, JiraApi, JiraError, JiraIssue};
use crate::time::{add_secs, secs_since};
use crate::worker::Worker;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JiraTarget {
    pub key: String,
    pub host: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JiraSnapshot {
    pub issues: HashMap<String, JiraIssue>,
    pub icons: HashMap<Url, Vec<u8>>,
    pub connection: ConnectionState,
}

pub const NEW_KEY_GAP: f64 = 5.0;
pub const REFRESH_EVERY: f64 = 900.0;
pub const RETRY_AFTER: f64 = 60.0;
pub const FORGET_AFTER: f64 = 30.0;
const CHUNK: usize = 100;

/// Shared between the main thread and the worker.
struct Shared {
    snapshot: JiraSnapshot,
    pending: Option<(Vec<JiraTarget>, SystemTime)>,
}

/// Touched only by `tick` (the worker, or inline without one).
struct Ticker {
    known: HashSet<JiraTarget>,
    cache: HashMap<JiraTarget, JiraIssue>,
    absent_since: HashMap<JiraTarget, SystemTime>,
    /// Keys that failed or were never asked wait here until their retry time.
    retry_at: HashMap<JiraTarget, SystemTime>,
    icons: HashMap<Url, Vec<u8>>,
    icon_retry_at: HashMap<Url, SystemTime>,
    last_request: Option<SystemTime>,
    last_refresh: Option<SystemTime>,
    connection: ConnectionState,
}

struct Inner {
    api: Arc<dyn JiraApi>,
    shared: Mutex<Shared>,
    ticker: Mutex<Ticker>,
}

pub struct JiraMonitor {
    inner: Arc<Inner>,
    worker: Worker,
}

impl JiraMonitor {
    /// `threaded` runs the polling on its own worker thread (`claude-sessions.jira-monitor`); without it `update` polls inline (tests).
    pub fn new(api: Arc<dyn JiraApi>, threaded: bool) -> Self {
        JiraMonitor {
            inner: Arc::new(Inner {
                api,
                shared: Mutex::new(Shared {
                    snapshot: JiraSnapshot {
                        issues: HashMap::new(),
                        icons: HashMap::new(),
                        connection: ConnectionState::Checking,
                    },
                    pending: None,
                }),
                ticker: Mutex::new(Ticker {
                    known: HashSet::new(),
                    cache: HashMap::new(),
                    absent_since: HashMap::new(),
                    retry_at: HashMap::new(),
                    icons: HashMap::new(),
                    icon_retry_at: HashMap::new(),
                    last_request: None,
                    last_refresh: None,
                    connection: ConnectionState::Checking,
                }),
            }),
            worker: if threaded {
                Worker::spawn("claude-sessions.jira-monitor")
            } else {
                Worker::inline()
            },
        }
    }

    /// Cheap: queues one `tick` with the latest targets; calls made while one is queued replace its arguments.
    pub fn update(&self, targets: &[JiraTarget], now: SystemTime) {
        if self.worker.is_inline() {
            return self.inner.tick(targets, now);
        }
        let queued = self
            .inner
            .shared()
            .pending
            .replace((targets.to_vec(), now))
            .is_some();
        if queued {
            return;
        }
        let inner = self.inner.clone();
        self.worker.submit(move || {
            let p = inner.shared().pending.take();
            if let Some((targets, now)) = p {
                inner.tick(&targets, now);
            }
        });
    }

    pub fn take_snapshot(&self) -> JiraSnapshot {
        self.inner.shared().snapshot.clone()
    }
}

fn sort_key(t: &JiraTarget) -> (&str, &str) {
    (&t.host, &t.key)
}

impl Inner {
    fn shared(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tick(&self, targets: &[JiraTarget], now: SystemTime) {
        let mut t = self.ticker.lock().unwrap_or_else(|e| e.into_inner());
        let live: HashSet<JiraTarget> = targets.iter().cloned().collect();
        let gone: Vec<JiraTarget> = t
            .known
            .iter()
            .filter(|k| !live.contains(k))
            .cloned()
            .collect();
        for k in gone {
            t.absent_since.entry(k).or_insert(now);
        }
        for k in &live {
            t.absent_since.remove(k);
        }
        let mut changed = false;
        let forgotten: Vec<JiraTarget> = t
            .absent_since
            .iter()
            .filter(|(_, since)| secs_since(now, **since) > FORGET_AFTER)
            .map(|(k, _)| k.clone())
            .collect();
        for k in forgotten {
            t.known.remove(&k);
            t.cache.remove(&k);
            t.retry_at.remove(&k);
            t.absent_since.remove(&k);
            changed = true;
        }
        t.known.extend(live.iter().cloned());

        let gap_open = t
            .last_request
            .is_none_or(|l| secs_since(now, l) >= NEW_KEY_GAP);
        let refresh_due = t
            .last_refresh
            .is_some_and(|l| secs_since(now, l) >= REFRESH_EVERY);
        let mut due: Vec<JiraTarget> = Vec::new();
        if refresh_due {
            due = live.iter().cloned().collect();
        } else if gap_open {
            due = live
                .iter()
                .filter(|k| {
                    (!t.cache.contains_key(*k) || t.retry_at.contains_key(*k))
                        && t.retry_at.get(*k).is_none_or(|r| now >= *r)
                })
                .cloned()
                .collect();
        }
        if !due.is_empty() {
            t.last_request = Some(now);
            if refresh_due || t.last_refresh.is_none() {
                t.last_refresh = Some(now);
            }
            due.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
            self.fetch(&mut t, &due, now);
            changed = true;
        }

        if changed {
            // Sorted by host so a key shared by two hosts always resolves to the same one.
            let mut entries: Vec<(&JiraTarget, &JiraIssue)> = t.cache.iter().collect();
            entries.sort_by(|a, b| sort_key(a.0).cmp(&sort_key(b.0)));
            let mut issues: HashMap<String, JiraIssue> = HashMap::new();
            for (k, v) in entries {
                issues.entry(k.key.clone()).or_insert_with(|| v.clone());
            }
            let mut s = self.shared();
            s.snapshot.issues = issues;
            s.snapshot.connection = t.connection;
        }

        // Titles are published above, so a slow icon download never delays them.
        if t.connection != ConnectionState::Rejected && self.fetch_icons(&mut t, now) {
            self.shared().snapshot.icons = t.icons.clone();
        }
    }

    /// One request per distinct Atlassian icon URL; a failure waits `RETRY_AFTER`. Returns whether an icon arrived.
    fn fetch_icons(&self, t: &mut Ticker, now: SystemTime) -> bool {
        let mut arrived = false;
        let mut urls: Vec<Url> = t
            .cache
            .values()
            .filter_map(|i| i.icon_url.clone())
            .filter(|u| {
                u.scheme() == "https"
                    && jira::is_atlassian_host(u.host_str().unwrap_or(""))
                    && !t.icons.contains_key(u)
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        urls.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        for url in urls {
            if t.icon_retry_at.get(&url).is_some_and(|r| now < *r) {
                continue;
            }
            match self.api.icon(&url) {
                Ok(data) => {
                    t.icons.insert(url.clone(), data);
                    t.icon_retry_at.remove(&url);
                    arrived = true;
                }
                Err(_) => {
                    t.icon_retry_at.insert(url, add_secs(now, RETRY_AFTER));
                }
            }
        }
        arrived
    }

    fn fetch(&self, t: &mut Ticker, due: &[JiraTarget], now: SystemTime) {
        let mut by_host: BTreeMap<&str, Vec<&JiraTarget>> = BTreeMap::new();
        for target in due {
            by_host.entry(&target.host).or_default().push(target);
        }
        for (host, all) in by_host {
            for part in all.chunks(CHUNK) {
                let keys: Vec<String> = part.iter().map(|p| p.key.clone()).collect();
                let result = self
                    .api
                    .bulk_fetch(host, &keys)
                    .and_then(|data| jira::issues(&data, &keys).ok_or(JiraError::Transport));
                match result {
                    Ok(issues) => {
                        t.connection = ConnectionState::after(None, t.connection);
                        for target in part {
                            match issues.get(&target.key) {
                                Some(i) => t.cache.insert((*target).clone(), i.clone()),
                                None => t.cache.remove(*target),
                            };
                            t.retry_at.remove(*target);
                        }
                    }
                    Err(e) => {
                        t.connection = ConnectionState::after(Some(&e), t.connection);
                        for target in part {
                            t.retry_at
                                .insert((*target).clone(), add_secs(now, RETRY_AFTER));
                        }
                    }
                }
            }
        }
    }
}
