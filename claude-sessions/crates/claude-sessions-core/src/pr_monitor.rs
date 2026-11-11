//! Polls Bitbucket for the pull requests of the sessions (link refs from transcripts, branch matches from the cwd's
//! remote) and CodeRabbit's state on them, off the main thread.

use crate::bitbucket::{BitbucketApi, BitbucketError};
use crate::code_rabbit::{self, Rabbit, RabbitInputs};
use crate::credentials::ConnectionState;
use crate::pull_requests::{self, PREvent, PRRef, PRStatus};
use crate::shell::{format_iso_date, parse_iso_date, run_process};
use crate::time::{add_secs, secs_since};
use crate::worker::Worker;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, PartialEq)]
pub struct PRTarget {
    pub session_id: String,
    /// PR refs found in the transcript by `transcript_scanner`, first appearance order.
    pub link_refs: Vec<PRRef>,
    pub cwd: String,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PRSnapshot {
    /// Shown PRs per session in display order: link refs by first appearance, then branch refs. Not capped.
    pub prs: HashMap<String, Vec<PRStatus>>,
    pub events: Vec<PREvent>,
    /// The PRs that produced `events` since the last `take_snapshot`.
    pub event_refs: HashSet<PRRef>,
    pub last_success: Option<SystemTime>,
    pub connection: ConnectionState,
}

const MAX_PAGES: usize = 20;
const FORGET_AFTER: f64 = 30.0;
const REMOTE_RETRY: f64 = 300.0;
/// How long a PR head without a CodeRabbit status keeps asking for one.
const RABBIT_WAIT: f64 = 600.0;
const STATUSES_SLOW_POLL: f64 = 300.0;
/// How often the comments are refetched while CodeRabbit threads are pending (CodeRabbit edits them to `Addressed`
/// without changing `comment_count`).
const COMMENTS_SLOW_POLL: f64 = 300.0;
const INTERVAL: f64 = 45.0;
const BASE: &str = "https://api.bitbucket.org";
const SINGLE_FIELDS: &str = "id,title,state,comment_count,updated_on,source.branch.name,source.repository.full_name,source.commit.hash,participants.approved,participants.state,participants.role,participants.user.uuid,author.uuid,links.html.href";
const STATUS_FIELDS: &str = "values.state,values.commit.hash,values.key,values.name,values.description,values.created_on,values.updated_on";
const COMMENT_FIELDS: &str = "values.user.uuid,values.user.display_name,values.created_on,values.updated_on,values.deleted,values.content.raw,values.id,values.parent.id,values.inline.path,next";

/// The commit, the review run and the chimed labels of one PR.
type RabbitSeen = (Option<String>, Option<SystemTime>, HashSet<String>);
type Remote = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

#[derive(Clone, PartialEq)]
struct Repo {
    workspace: String,
    name: String,
}

#[derive(Default)]
struct SessionState {
    link_refs: Vec<PRRef>,
    branch_refs: Vec<PRRef>,
    branch: Option<String>,
}

impl SessionState {
    /// Link refs first, then branch refs, without duplicates.
    fn refs(&self) -> Vec<PRRef> {
        let mut seen = HashSet::new();
        self.link_refs
            .iter()
            .chain(&self.branch_refs)
            .filter(|r| seen.insert((*r).clone()))
            .cloned()
            .collect()
    }
}

/// Shared between the main thread and the worker.
struct Shared {
    snapshot: PRSnapshot,
    pending: Option<(Vec<PRTarget>, SystemTime)>,
    /// PRs with a nudge post in flight.
    nudging: HashSet<PRRef>,
}

/// Touched only by `tick` and the nudge work (the worker, or inline without one).
struct Ticker {
    sessions: HashMap<String, SessionState>,
    remotes: HashMap<String, (Option<Repo>, SystemTime)>,
    absent_since: HashMap<String, SystemTime>,
    statuses: HashMap<PRRef, PRStatus>,
    /// Per PR: the commit, the review run (`rabbit_since`) and the labels already chimed for it (a new commit or a new run
    /// starts a new set).
    rabbit_seen: HashMap<PRRef, RabbitSeen>,
    /// PRs whose `/statuses` was fetched successfully at least once.
    statuses_fetched: HashSet<PRRef>,
    /// Raw values of each PR's last successful `/statuses` fetch.
    status_values: HashMap<PRRef, Vec<Value>>,
    /// Raw values of each PR's last successful full `/comments` fetch.
    comment_values: HashMap<PRRef, Vec<Value>>,
    /// The newest summary comment seen for each PR's commit.
    summaries: HashMap<PRRef, (Option<String>, Value)>,
    /// When each PR's `/statuses` was last fetched successfully (paces the slow poll after the window).
    statuses_at: HashMap<PRRef, SystemTime>,
    /// When each PR's full `/comments` was last fetched successfully (paces the slow poll while threads are pending).
    comments_at: HashMap<PRRef, SystemTime>,
    /// Per PR: the clock of the cycle `nudge` posted its approve command in; the 600 s nudge timeout never starts before it.
    nudged_at: HashMap<PRRef, SystemTime>,
    /// PRs whose first `/comments` fetch failed and is still owed.
    comments_owed: HashSet<PRRef>,
    head_seen: HashMap<PRRef, (Option<String>, SystemTime)>,
    hidden: HashSet<PRRef>,
    closed: HashSet<PRRef>,
    me: Option<String>,
    last_cycle: Option<SystemTime>,
    connection: ConnectionState,
}

struct Inner {
    api: Arc<dyn BitbucketApi>,
    remote: Remote,
    interval: f64,
    shared: Mutex<Shared>,
    ticker: Mutex<Ticker>,
}

pub struct PRMonitor {
    inner: Arc<Inner>,
    worker: Worker,
}

/// `git remote get-url origin` of `cwd`, trimmed; `None` when git fails or takes over 2 s.
fn git_remote(cwd: &str) -> Option<String> {
    run_process(
        Path::new("/usr/bin/git"),
        &["-C", cwd, "remote", "get-url", "origin"],
        Duration::from_secs(2),
        false,
        None,
    )
    .map(|s| s.trim().to_string())
}

impl PRMonitor {
    /// `threaded` runs the polling on its own worker thread (`claude-sessions.pr-monitor`); without it `update` polls inline (tests).
    pub fn new(api: Arc<dyn BitbucketApi>, threaded: bool) -> Self {
        Self::with_options(api, threaded, Arc::new(git_remote), INTERVAL)
    }

    /// Like `new`, with the remote lookup and the cycle interval (seconds) injected.
    pub fn with_options(
        api: Arc<dyn BitbucketApi>,
        threaded: bool,
        remote: Remote,
        interval: f64,
    ) -> Self {
        PRMonitor {
            inner: Arc::new(Inner {
                api,
                remote,
                interval,
                shared: Mutex::new(Shared {
                    snapshot: PRSnapshot {
                        prs: HashMap::new(),
                        events: Vec::new(),
                        event_refs: HashSet::new(),
                        last_success: None,
                        connection: ConnectionState::Checking,
                    },
                    pending: None,
                    nudging: HashSet::new(),
                }),
                ticker: Mutex::new(Ticker {
                    sessions: HashMap::new(),
                    remotes: HashMap::new(),
                    absent_since: HashMap::new(),
                    statuses: HashMap::new(),
                    rabbit_seen: HashMap::new(),
                    statuses_fetched: HashSet::new(),
                    status_values: HashMap::new(),
                    comment_values: HashMap::new(),
                    summaries: HashMap::new(),
                    statuses_at: HashMap::new(),
                    comments_at: HashMap::new(),
                    nudged_at: HashMap::new(),
                    comments_owed: HashSet::new(),
                    head_seen: HashMap::new(),
                    hidden: HashSet::new(),
                    closed: HashSet::new(),
                    me: None,
                    last_cycle: None,
                    connection: ConnectionState::Checking,
                }),
            }),
            worker: if threaded {
                Worker::spawn("claude-sessions.pr-monitor")
            } else {
                Worker::inline()
            },
        }
    }

    /// Cheap: queues one `tick` with the latest targets; calls made while one is queued replace its arguments.
    pub fn update(&self, targets: &[PRTarget], now: SystemTime) {
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

    /// The current state; drains `events` and `event_refs`.
    pub fn take_snapshot(&self) -> PRSnapshot {
        let mut s = self.inner.shared();
        let out = s.snapshot.clone();
        s.snapshot.events.clear();
        s.snapshot.event_refs.clear();
        out
    }

    /// Posts `@coderabbitai approve` on a PR that awaits approval (`chờ duyệt`). `done(true)` once posted; `done(false)` when
    /// refused (not awaiting, or a post for it is already in flight) or the post failed.
    /// `done` runs on the worker (inline without one), except for a refusal because a post is in flight, which runs on the
    /// caller at once. The app re-posts it to the main queue.
    pub fn nudge(&self, ref_: &PRRef, done: impl FnOnce(bool) + Send + 'static) {
        let claimed = self.inner.shared().nudging.insert(ref_.clone());
        if !claimed {
            return done(false);
        }
        let inner = self.inner.clone();
        let ref_ = ref_.clone();
        self.worker.submit(move || {
            let ok = inner.nudge_work(&ref_);
            inner.shared().nudging.remove(&ref_);
            done(ok);
        });
    }
}

/// Percent-encodes what `urlPathAllowed` does not allow.
fn path_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(d: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(d)
        .ok()
        .filter(Value::is_object)
}

fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

impl Inner {
    fn shared(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ticker(&self) -> MutexGuard<'_, Ticker> {
        self.ticker.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tick(&self, targets: &[PRTarget], now: SystemTime) {
        let mut t = self.ticker();
        let live: HashSet<&str> = targets.iter().map(|t| t.session_id.as_str()).collect();
        let gone: Vec<String> = t
            .sessions
            .keys()
            .filter(|id| !live.contains(id.as_str()))
            .cloned()
            .collect();
        for id in gone {
            t.absent_since.entry(id).or_insert(now);
        }
        for id in &live {
            t.absent_since.remove(*id);
        }
        let forgotten: Vec<String> = t
            .absent_since
            .iter()
            .filter(|(_, since)| secs_since(now, **since) > FORGET_AFTER)
            .map(|(id, _)| id.clone())
            .collect();
        for id in forgotten {
            t.sessions.remove(&id);
            t.absent_since.remove(&id);
        }
        let mut need_cycle = t
            .last_cycle
            .is_none_or(|l| secs_since(now, l) >= self.interval);
        for target in targets {
            let existing = t.sessions.contains_key(&target.session_id);
            let s = t.sessions.entry(target.session_id.clone()).or_default();
            if if existing {
                s.branch != target.branch
            } else {
                target.branch.is_some()
            } {
                need_cycle = true;
            }
            s.branch = target.branch.clone();
            for r in &target.link_refs {
                if !s.link_refs.contains(r) {
                    s.link_refs.push(r.clone());
                    need_cycle = true;
                }
            }
            if let Some((repo, at)) = t.remotes.get(&target.cwd)
                && (repo.is_some() || secs_since(now, *at) < REMOTE_RETRY)
            {
                continue;
            }
            let repo = (self.remote)(&target.cwd)
                .and_then(|r| pull_requests::repo_from_remote(&r))
                .map(|(workspace, name)| Repo { workspace, name });
            t.remotes.insert(target.cwd.clone(), (repo, now));
        }
        let used: HashSet<PRRef> = t.sessions.values().flat_map(|s| s.refs()).collect();
        t.statuses.retain(|k, _| used.contains(k));
        t.rabbit_seen.retain(|k, _| used.contains(k));
        t.status_values.retain(|k, _| used.contains(k));
        t.comment_values.retain(|k, _| used.contains(k));
        t.summaries.retain(|k, _| used.contains(k));
        t.head_seen.retain(|k, _| used.contains(k));
        t.statuses_at.retain(|k, _| used.contains(k));
        t.comments_at.retain(|k, _| used.contains(k));
        t.nudged_at.retain(|k, _| used.contains(k));
        t.statuses_fetched.retain(|k| used.contains(k));
        t.comments_owed.retain(|k| used.contains(k));
        t.hidden.retain(|k| used.contains(k));
        t.closed.retain(|k| used.contains(k));

        let mut events = Vec::new();
        let mut event_refs = HashSet::new();
        let mut succeeded = false;
        if need_cycle {
            t.last_cycle = Some(now);
            (events, event_refs, succeeded) = self.cycle(&mut t, targets, now);
        }
        self.publish(&t, events, event_refs, succeeded.then_some(now));
    }

    fn shown_prs(t: &Ticker) -> HashMap<String, Vec<PRStatus>> {
        let mut prs = HashMap::new();
        for (id, s) in &t.sessions {
            let list: Vec<PRStatus> = s
                .refs()
                .iter()
                .filter_map(|r| t.statuses.get(r).cloned())
                .collect();
            if !list.is_empty() {
                prs.insert(id.clone(), list);
            }
        }
        prs
    }

    fn publish(
        &self,
        t: &Ticker,
        events: Vec<PREvent>,
        refs: HashSet<PRRef>,
        success: Option<SystemTime>,
    ) {
        let mut s = self.shared();
        s.snapshot.prs = Self::shown_prs(t);
        s.snapshot.connection = t.connection;
        s.snapshot.events.extend(events);
        s.snapshot.event_refs.extend(refs);
        if success.is_some() {
            s.snapshot.last_success = success;
        }
    }

    fn nudge_work(&self, ref_: &PRRef) -> bool {
        let mut t = self.ticker();
        let nudged_at = t.last_cycle.unwrap_or_else(SystemTime::now);
        if t.statuses.get(ref_).and_then(|p| p.rabbit.as_ref()) != Some(&Rabbit::AwaitingApproval) {
            return false;
        }
        let path = format!(
            "/2.0/repositories/{}/{}/pullrequests/{}/comments",
            ref_.workspace, ref_.repo, ref_.id
        );
        let body = json!({"content": {"raw": "@coderabbitai approve"}});
        if self.recorded(&mut t, |api| api.post(&path, &body)).is_err() {
            self.publish(&t, vec![], HashSet::new(), None);
            return false;
        }
        // Stamped with the clock of the last cycle, the one `now` is compared against, and after the newest status so
        // that status cannot end the nudge at once.
        let mut stamped = nudged_at;
        let commit = t.statuses.get(ref_).and_then(|p| p.commit.clone());
        let updated = t.status_values.get(ref_).and_then(|v| {
            code_rabbit::newest_status(v, commit.as_deref())
                .and_then(|s| str_at(s, "updated_on"))
                .and_then(parse_iso_date)
        });
        if let Some(u) = updated {
            stamped = stamped.max(add_secs(u, 0.001));
        }
        let posted = json!({"id": -1, "user": {"uuid": t.me.clone().unwrap_or_default()},
            "created_on": format_iso_date(stamped), "content": {"raw": "@coderabbitai approve"}, "deleted": false});
        t.comment_values
            .entry(ref_.clone())
            .or_default()
            .push(posted);
        t.nudged_at.insert(ref_.clone(), nudged_at);
        if let Some(pr) = t.statuses.get(ref_).cloned() {
            let rabbit = Self::derive(&t, &pr, ref_, nudged_at).rabbit;
            if let Some(s) = t.statuses.get_mut(ref_) {
                s.rabbit = rabbit;
            }
        }
        t.last_cycle = None;
        self.publish(&t, vec![], HashSet::new(), None);
        true
    }

    fn derive(t: &Ticker, pr: &PRStatus, ref_: &PRRef, now: SystemTime) -> code_rabbit::RabbitView {
        let summary = t
            .summaries
            .get(ref_)
            .and_then(|(c, v)| (*c == pr.commit).then(|| v.clone()));
        code_rabbit::derive(
            &RabbitInputs {
                commit: pr.commit.clone(),
                statuses: t.status_values.get(ref_).cloned(),
                comments: t.comment_values.get(ref_).cloned(),
                summary,
                approvers: pr.approvers.clone(),
                me: t.me.clone(),
                author: pr.author.clone(),
                nudged_at: t.nudged_at.get(ref_).copied(),
            },
            now,
        )
    }

    /// Sets the five rabbit fields from the raw data kept for `ref_`; the thread counts stay `None` until a comments fetch succeeded.
    fn apply_rabbit(t: &Ticker, pr: &mut PRStatus, ref_: &PRRef, now: SystemTime) {
        let v = Self::derive(t, pr, ref_, now);
        pr.rabbit = v.rabbit;
        pr.rabbit_status = v.status;
        pr.rabbit_since = v.since;
        pr.rabbit_threads = v.threads;
        pr.rabbit_others_pending = v.others_pending;
    }

    /// Remembers the label of a finished review so only a new one (or a new commit) chimes.
    fn record_rabbit(t: &mut Ticker, ref_: &PRRef, pr: &PRStatus) {
        let Some(r) = &pr.rabbit else { return };
        if !code_rabbit::is_final(r) {
            return;
        }
        let mut labels = match t.rabbit_seen.get(ref_) {
            Some((c, s, l)) if *c == pr.commit && *s == pr.rabbit_since => l.clone(),
            _ => HashSet::new(),
        };
        labels.insert(code_rabbit::label(r, None));
        t.rabbit_seen
            .insert(ref_.clone(), (pr.commit.clone(), pr.rabbit_since, labels));
    }

    /// Every API call goes through here so each path records the connection state.
    fn recorded<T>(
        &self,
        t: &mut Ticker,
        call: impl FnOnce(&dyn BitbucketApi) -> Result<T, BitbucketError>,
    ) -> Result<T, BitbucketError> {
        match call(self.api.as_ref()) {
            Ok(r) => {
                t.connection = ConnectionState::after(None, t.connection);
                Ok(r)
            }
            Err(e) => {
                t.connection = ConnectionState::after(Some(&e), t.connection);
                Err(e)
            }
        }
    }

    fn fetch(&self, t: &mut Ticker, path: &str) -> Result<Vec<u8>, BitbucketError> {
        self.recorded(t, |api| api.get(path))
    }

    /// The `values` of every page from `first`, following `next` up to `MAX_PAGES`.
    fn all_values(&self, t: &mut Ticker, first: &str) -> Result<Vec<Value>, BitbucketError> {
        let mut next = Some(first.to_string());
        let mut values = Vec::new();
        let mut pages = 0;
        while let Some(p) = next {
            if pages >= MAX_PAGES {
                break;
            }
            let o = decode(&self.fetch(t, &p)?).ok_or(BitbucketError::Transport)?;
            if let Some(a) = o.get("values").and_then(Value::as_array) {
                values.extend(a.iter().cloned());
            }
            next = str_at(&o, "next").map(|n| n.strip_prefix(BASE).unwrap_or(n).to_string());
            pages += 1;
        }
        Ok(values)
    }

    fn open_list(
        &self,
        t: &mut Ticker,
        workspace: &str,
        me: &str,
    ) -> Result<Vec<PRStatus>, BitbucketError> {
        let fields: String = SINGLE_FIELDS
            .split(',')
            .map(|f| format!("values.{f}"))
            .collect::<Vec<_>>()
            .join(",");
        let path = format!(
            "/2.0/workspaces/{workspace}/pullrequests/{}?state=OPEN&pagelen=50&fields={fields},next",
            path_encode(me)
        );
        Ok(self
            .all_values(t, &path)?
            .iter()
            .filter_map(pull_requests::status_from)
            .collect())
    }

    /// Every page of `/comments`, merged into one `{"values": [...]}` document.
    fn fetch_comments(&self, t: &mut Ticker, path: &str) -> Result<Vec<u8>, BitbucketError> {
        let values = self.all_values(
            t,
            &format!("{path}/comments?sort=-created_on&pagelen=100&fields={COMMENT_FIELDS}"),
        )?;
        serde_json::to_vec(&json!({"values": values})).map_err(|_| BitbucketError::Transport)
    }

    fn cycle(
        &self,
        t: &mut Ticker,
        targets: &[PRTarget],
        now: SystemTime,
    ) -> (Vec<PREvent>, HashSet<PRRef>, bool) {
        let mut workspaces: HashSet<String> = t
            .sessions
            .values()
            .flat_map(|s| s.refs())
            .map(|r| r.workspace)
            .collect();
        for target in targets {
            if let Some((Some(repo), _)) = t.remotes.get(&target.cwd) {
                workspaces.insert(repo.workspace.clone());
            }
        }
        if workspaces.is_empty() {
            return (vec![], HashSet::new(), false);
        }

        let mut open: HashMap<PRRef, PRStatus> = HashMap::new();
        let mut listed = 0;
        if t.me.is_none() {
            let uuid = self
                .fetch(t, "/2.0/user?fields=uuid")
                .ok()
                .and_then(|d| decode(&d))
                .and_then(|o| str_at(&o, "uuid").map(String::from));
            match uuid {
                Some(u) => t.me = Some(u),
                None => return (vec![], HashSet::new(), false),
            }
        }
        let me = t.me.clone().unwrap_or_default();
        let mut sorted: Vec<String> = workspaces.into_iter().collect();
        sorted.sort();
        for ws in &sorted {
            match self.open_list(t, ws, &me) {
                Ok(list) => {
                    for pr in list {
                        open.insert(pr.ref_.clone(), pr);
                    }
                    listed += 1;
                }
                // Foreign or unknown workspace: its refs go through the single-PR path.
                Err(BitbucketError::Http(403 | 404)) => {}
                Err(_) => return (vec![], HashSet::new(), false),
            }
        }
        if listed == 0 {
            return (vec![], HashSet::new(), false);
        }

        for target in targets {
            let Some(branch) = &target.branch else {
                continue;
            };
            let Some((Some(repo), _)) = t.remotes.get(&target.cwd) else {
                continue;
            };
            let full = format!("{}/{}", repo.workspace, repo.name);
            let mut matching: Vec<&PRStatus> = open
                .values()
                .filter(|pr| {
                    pr.source_repo.as_deref() == Some(full.as_str())
                        && pr.branch.as_deref() == Some(branch.as_str())
                })
                .collect();
            matching.sort_by_key(|pr| pr.ref_.id);
            for pr in matching {
                if let Some(s) = t.sessions.get_mut(&target.session_id)
                    && !s.refs().contains(&pr.ref_)
                {
                    s.branch_refs.push(pr.ref_.clone());
                }
            }
        }

        let mut seen_refs = HashSet::new();
        let mut ids: Vec<&String> = t.sessions.keys().collect();
        ids.sort();
        let refs: Vec<PRRef> = ids
            .into_iter()
            .flat_map(|id| t.sessions[id].refs())
            .filter(|r| seen_refs.insert(r.clone()))
            .collect();

        let mut events = Vec::new();
        let mut event_refs = HashSet::new();
        for ref_ in refs {
            if t.hidden.contains(&ref_) || t.closed.contains(&ref_) {
                continue;
            }
            let found = self.cycle_pr(t, &ref_, open.get(&ref_).cloned(), now);
            if let Some(found) = found
                && !found.is_empty()
            {
                event_refs.insert(ref_);
                events.extend(found);
            }
        }
        (events, event_refs, true)
    }

    /// One PR of a cycle; `None` when it is skipped (its stored state stays as it was).
    fn cycle_pr(
        &self,
        t: &mut Ticker,
        ref_: &PRRef,
        open: Option<PRStatus>,
        now: SystemTime,
    ) -> Option<Vec<PREvent>> {
        let prev = t.statuses.get(ref_).cloned();
        let path = format!(
            "/2.0/repositories/{}/{}/pullrequests/{}",
            ref_.workspace, ref_.repo, ref_.id
        );
        let mut pr = match open {
            Some(o) => o,
            None => {
                let d = match self.fetch(t, &format!("{path}?fields={SINGLE_FIELDS}")) {
                    Ok(d) => d,
                    Err(BitbucketError::Http(403 | 404)) if prev.is_none() => {
                        t.hidden.insert(ref_.clone()); // foreign or placeholder workspace: never ask again
                        return None;
                    }
                    Err(_) => return None,
                };
                let fetched = decode(&d).and_then(|o| pull_requests::status_from(&o))?;
                if !fetched.is_open() {
                    if prev.is_none() {
                        t.hidden.insert(ref_.clone());
                        return None;
                    }
                    t.closed.insert(ref_.clone());
                }
                fetched
            }
        };
        pr.last_other = prev.as_ref().and_then(|p| p.last_other.clone());
        let mut rabbit_finished = false;
        let mut first_statuses = false;
        let mut owed_fetched = false;
        let prev_rabbit_status = prev.as_ref().and_then(|p| p.rabbit_status.clone());
        if pr.is_open() {
            if t.head_seen.get(ref_).is_none_or(|(c, _)| *c != pr.commit) {
                t.head_seen.insert(ref_.clone(), (pr.commit.clone(), now));
            }
            let in_window = secs_since(now, t.head_seen[ref_].1) < RABBIT_WAIT;
            let rabbit_pending = prev_rabbit_status == Some(Rabbit::Reviewing)
                || (prev_rabbit_status.is_none() && in_window);
            // A missing CodeRabbit status (after the window) and a finished review (a later approval or
            // failure) are still looked for, at a slow rate.
            let reviewed = matches!(
                prev_rabbit_status,
                Some(Rabbit::Reviewed(_) | Rabbit::ApprovalFailed(_))
            );
            let slow_poll = (reviewed || (!in_window && prev_rabbit_status.is_none()))
                && t.statuses_at
                    .get(ref_)
                    .is_none_or(|a| secs_since(now, *a) >= STATUSES_SLOW_POLL);
            if prev.is_none()
                || prev.as_ref().map(|p| &p.commit) != Some(&pr.commit)
                || rabbit_pending
                || prev.as_ref().map(|p| p.comments) != Some(pr.comments)
                || !t.statuses_fetched.contains(ref_)
                || slow_poll
            {
                let fetched = self.fetch(
                    t,
                    &format!("{path}/statuses?fields={STATUS_FIELDS}&pagelen=50&sort=-created_on"),
                );
                match fetched {
                    Ok(d) => {
                        first_statuses = !t.statuses_fetched.contains(ref_);
                        t.statuses_at.insert(ref_.clone(), now);
                        let values = decode(&d)
                            .and_then(|o| o.get("values").and_then(Value::as_array).cloned())
                            .unwrap_or_default();
                        t.status_values.insert(ref_.clone(), values);
                        // Derived against derived: a failed approval a newer command supersedes is `reviewed` on both sides.
                        if let Some(r) = Self::derive(t, &pr, ref_, now).status {
                            rabbit_finished = code_rabbit::is_final(&r)
                                && !(prev.as_ref().is_some_and(|p| p.commit == pr.commit)
                                    && code_rabbit::kind(prev_rabbit_status.as_ref())
                                        == code_rabbit::kind(Some(&r)));
                        }
                    }
                    Err(_) if prev.is_some() => return None,
                    Err(_) => {}
                }
            }
            let owed = t.comments_owed.contains(ref_);
            // Not while the status itself still says `reviewing`: that case is polled through the statuses.
            let comments_slow = prev
                .as_ref()
                .and_then(|p| p.rabbit_threads)
                .is_some_and(|th| th.pending > 0)
                && t.status_values
                    .get(ref_)
                    .and_then(|v| code_rabbit::state_from_values(v, pr.commit.as_deref()))
                    .map(|(r, _)| r)
                    != Some(Rabbit::Reviewing)
                && t.comments_at
                    .get(ref_)
                    .is_none_or(|a| secs_since(now, *a) >= COMMENTS_SLOW_POLL);
            if prev.is_none()
                || prev.as_ref().map(|p| p.comments) != Some(pr.comments)
                || rabbit_finished
                || owed
                || comments_slow
            {
                let Ok(d) = self.fetch_comments(t, &path) else {
                    if prev.is_some() {
                        return None;
                    }
                    t.comments_owed.insert(ref_.clone());
                    Self::apply_rabbit(t, &mut pr, ref_, now);
                    Self::record_rabbit(t, ref_, &pr);
                    if first_statuses {
                        t.statuses_fetched.insert(ref_.clone());
                    }
                    t.statuses.insert(ref_.clone(), pr);
                    return None;
                };
                t.comments_owed.remove(ref_);
                t.comments_at.insert(ref_.clone(), now);
                owed_fetched = owed;
                if let Some(me) = t.me.clone()
                    && let Some(c) = pull_requests::last_other_from_comments(&d, &me)
                {
                    pr.last_other = Some(c);
                }
                if let Some(values) = decode(&d).and_then(|o| o.get("values")?.as_array().cloned())
                {
                    if let Some(found) = code_rabbit::newest_summary(&values) {
                        t.summaries
                            .insert(ref_.clone(), (pr.commit.clone(), found.clone()));
                    } else if t.summaries.get(ref_).is_some_and(|(c, _)| *c != pr.commit) {
                        t.summaries.remove(ref_);
                    }
                    t.comment_values.insert(ref_.clone(), values);
                }
            }
        }
        Self::apply_rabbit(t, &mut pr, ref_, now);
        if owed_fetched {
            // Re-record only a count filled in for the review already recorded; a new kind must still chime.
            let same_review = t.rabbit_seen.get(ref_).and_then(|(c, _, _)| c.clone()) == pr.commit
                && code_rabbit::kind(prev.as_ref().and_then(|p| p.rabbit.as_ref()))
                    == code_rabbit::kind(pr.rabbit.as_ref())
                && prev.as_ref().and_then(|p| p.rabbit_since) == pr.rabbit_since;
            if same_review {
                Self::record_rabbit(t, ref_, &pr);
            }
            // First sight whose comments fetch failed: a summary posted before the widget saw the PR stays silent.
            if !t.rabbit_seen.contains_key(ref_) {
                Self::record_rabbit(t, ref_, &pr);
            }
        }
        let mut result = Vec::new();
        if let Some(prev) = &prev {
            let mut found = pull_requests::events(prev, &pr);
            if first_statuses {
                Self::record_rabbit(t, ref_, &pr);
            } else if let Some(r) = pr
                .rabbit
                .clone()
                .filter(|r| pr.is_open() && code_rabbit::is_final(r))
                && t.rabbit_seen.get(ref_).is_none_or(|(c, s, labels)| {
                    !(*c == pr.commit
                        && *s == pr.rabbit_since
                        && labels.contains(&code_rabbit::label(&r, None)))
                })
            {
                // An approval already shown stays an approval on the same commit: `since` moved, not the review.
                if !(r == Rabbit::Approved
                    && prev.rabbit == Some(Rabbit::Approved)
                    && prev.commit == pr.commit)
                {
                    match r {
                        Rabbit::Reviewed(Some(n)) if n > 0 => found.push(PREvent::RabbitNeedsYou),
                        Rabbit::Failed | Rabbit::ApprovalFailed(_) => {
                            found.push(PREvent::RabbitNeedsYou)
                        }
                        _ => found.push(PREvent::RabbitDone),
                    }
                }
                Self::record_rabbit(t, ref_, &pr);
            }
            result = found;
        } else {
            Self::record_rabbit(t, ref_, &pr);
        }
        if first_statuses {
            t.statuses_fetched.insert(ref_.clone());
        }
        t.statuses.insert(ref_.clone(), pr);
        Some(result)
    }
}
