//! All session state: reads the session files and transcripts, orders the rows, keeps the NEW marks, send states and the
//! closed list, and hands out a `Snapshot`. Time is a parameter of `refresh`; nothing here reads the clock but `Store::new`.

mod links;
mod model;
mod reads;
mod refresh;
mod sends;

use self::links::{ButtonEntry, buttons, help, menu};
pub use self::links::{link_text, pr_item};
pub use self::model::*;
use crate::closed_sessions::{ClosedSession, ClosedSessions, ClosedStore};
use crate::credentials::{ConnectionState, Service};
use crate::feed::{self, BlockState, FeedMonitor, FeedQuestion};
use crate::file_stamp::FileStamp;
use crate::git_branch::GitBranchReader;
use crate::jira::{self, JiraIssue};
use crate::jira_monitor::{JiraMonitor, JiraTarget};
use crate::panel_state::{EditKind, Editing};
use crate::pr_monitor::{PRMonitor, PRTarget};
use crate::pull_requests::{self, PRRef, PRStatus};
use crate::quota::QuotaReader;
use crate::tasks::{self, Todo};
use crate::time::{add_secs, secs_since};
use crate::transcript::{self, MainTurn, PendingState};
use crate::transcript_scanner::{ScanResult, ScanTarget, TranscriptScanner};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_segmentation::UnicodeSegmentation;

/// A session that left the list is closed once its process is gone, for at most this long.
const DEPARTED_WINDOW: f64 = 10.0;
const SEND_TIMEOUT: f64 = 15.0;
const HIDE_AFTER: f64 = 30.0;
const PEEK_FOR: f64 = 5.0;
const PR_STALE_AFTER: f64 = 300.0;
const TAIL_BYTES: u64 = 262_144;

/// Reads the branch of a cwd.
pub type BranchFn = Box<dyn FnMut(&str) -> Option<String>>;

pub struct Paths {
    pub sessions: PathBuf,
    pub projects: PathBuf,
    pub tasks: PathBuf,
}

/// What `Store::new` takes besides the paths; `Default` is the app's setup without monitors.
pub struct StoreOptions {
    pub start_hidden: bool,
    pub prs: Option<PRMonitor>,
    pub jira: Option<JiraMonitor>,
    pub feed: Option<FeedMonitor>,
    pub quota: Option<QuotaReader>,
    pub scanner: TranscriptScanner,
    /// Reads the branch of a cwd; `None` uses a `GitBranchReader` on `.git/HEAD`.
    pub branch: Option<BranchFn>,
    pub closed_store: Option<ClosedStore>,
    /// Whether a pid is a live process; `None` uses `kill(pid, 0)` (a zombie counts as alive: the app injects the finer check).
    pub is_running: Option<Box<dyn Fn(i32) -> bool>>,
}

impl Default for StoreOptions {
    fn default() -> Self {
        StoreOptions {
            start_hidden: false,
            prs: None,
            jira: None,
            feed: None,
            quota: None,
            scanner: TranscriptScanner::new(true),
            branch: None,
            closed_store: None,
            is_running: None,
        }
    }
}

/// What the app still has to do after `Store::apply`.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Nothing,
    /// A send to the same session is in flight: the caller beeps.
    Beep,
    /// The state is claimed; the app runs the action.
    Run(Action),
}

enum Branches {
    Git(GitBranchReader),
    Custom(BranchFn),
}

#[derive(Default)]
struct Cached {
    since: Option<SystemTime>,
    size: i64,
    reply: Option<String>,
    ai_title: Option<String>,
    pending: Option<PendingState>,
    prompt: bool,
}

#[derive(Clone, PartialEq)]
struct TaskKey {
    dir: Option<FileStamp>,
    stamps: Vec<Option<FileStamp>>,
}

struct TodoCache {
    key: TaskKey,
    files: Vec<PathBuf>,
    todo: Option<Todo>,
}

struct Head {
    prompt: Option<String>,
    size: u64,
}

#[derive(Clone)]
struct Activity {
    offset: u64,
    text: Option<String>,
    turn: Option<MainTurn>,
    agents: Vec<(String, String)>,
}

struct ActivityRead {
    text: Option<String>,
    turn: Option<MainTurn>,
    agents: Vec<String>,
}

struct ParsedFile {
    stamp: FileStamp,
    base: Session,
    spare: bool,
}

#[derive(Clone, PartialEq)]
struct ButtonKey {
    version: u64,
    projects_generation: u64,
}

pub struct Store {
    pub sessions: Vec<Session>,
    /// Session id -> cmux surface UUID, filled by the app. Pruned to live ids and dropped when the session's pid changes.
    pub surfaces: HashMap<String, String>,
    /// Session id -> why it has a NEW mark (key present = marked). In memory only.
    pub unseen: HashMap<String, NewMark>,
    /// Session ids whose cmux surface lookup failed; dropped when the pid changes or the session dies.
    pub surface_misses: HashSet<String>,
    /// Tool ids whose long command the user expanded with Xem đủ.
    pub shown_commands: HashSet<String>,
    /// Typed text per `draft_key`: a reply and a deny reason of one session are kept apart.
    pub drafts: HashMap<String, String>,
    /// The OMC usage cache's 5h and weekly quota.
    pub quota: Option<crate::quota::Quota>,
    /// Sessions whose process ended, newest first; saved to the closed store whenever a refresh changes it.
    pub(crate) closed: ClosedSessions,
    pub peek_until: Option<SystemTime>,
    pub muted: bool,
    pub bg_opacity: f64,
    /// The chime of the last refresh's session transitions, per session, and the PR events' chime.
    pub chime: Option<Chime>,
    pub transitions: HashMap<String, Chime>,
    pub pr_chime: Option<Chime>,
    pub pr_monitor: Option<PRMonitor>,
    pub feed_monitor: Option<FeedMonitor>,
    pub jira_monitor: Option<JiraMonitor>,
    /// Session id -> request id of the question the widget answered; dropped once the session stops waiting.
    pub sent: HashMap<String, String>,
    /// Session id -> request id whose send failed; the card falls back to `Trả lời trong cmux`. Dropped like `sent`.
    pub failed: HashMap<String, String>,
    /// Keychain account (email) per service, set by the app.
    pub accounts: HashMap<Service, String>,
    /// Jira project prefix -> host, learned from `/browse/` and `selectedIssue=` links.
    pub jira_projects: HashMap<String, String>,
    pub pr_last_success: Option<SystemTime>,
    pub empty_since: Option<SystemTime>,
    /// File reads done for the running-tool line, for tests.
    pub activity_reads: usize,
    /// Transcript bytes read by the head, tail and activity reads, for tests.
    pub transcript_bytes_read: usize,
    /// Session `.json` files read and parsed, for tests.
    pub session_parses: usize,
    /// Task directory listings, for tests.
    pub task_listings: usize,

    now: SystemTime,
    sends: HashMap<SendKey, (SendState, SystemTime)>,
    issue_icons: HashMap<url::Url, Vec<u8>>,
    connections: HashMap<Service, ConnectionLine>,
    last_focused: Option<String>,
    pids: HashMap<String, i32>,
    last_status: HashMap<String, String>,
    last_question: HashMap<String, FeedQuestion>,
    announced_question: HashMap<String, String>,
    departed: HashMap<String, (ClosedSession, i32, SystemTime)>,
    dir: PathBuf,
    projects: PathBuf,
    tasks: PathBuf,
    cache: HashMap<String, Cached>,
    button_cache: HashMap<String, (ButtonKey, Vec<ButtonEntry>)>,
    projects_generation: u64,
    urls: HashMap<String, PathBuf>,
    url_tried: HashMap<String, Option<SystemTime>>,
    heads: HashMap<String, Head>,
    todos: HashMap<String, TodoCache>,
    session_json: HashMap<PathBuf, ParsedFile>,
    activities: HashMap<String, Activity>,
    closed_store: Option<ClosedStore>,
    quota_reader: Option<QuotaReader>,
    scanner: TranscriptScanner,
    branches: Branches,
    is_running: Box<dyn Fn(i32) -> bool>,
}

fn default_is_running(pid: i32) -> bool {
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};
    match Pid::from_raw(pid) {
        Some(p) => match test_kill_process(p) {
            Ok(()) => true,
            Err(e) => e == Errno::PERM,
        },
        None => false,
    }
}

fn is_busy_or_waiting(s: &Session) -> bool {
    s.status == "waiting" || s.status == "busy"
}

impl Store {
    pub fn new(paths: Paths, options: StoreOptions) -> Store {
        let now = SystemTime::now();
        let closed = options
            .closed_store
            .as_ref()
            .map(|c| c.load(now))
            .unwrap_or_default();
        let branches = match options.branch {
            Some(f) => Branches::Custom(f),
            None => Branches::Git(GitBranchReader::new()),
        };
        Store {
            sessions: Vec::new(),
            surfaces: HashMap::new(),
            unseen: HashMap::new(),
            surface_misses: HashSet::new(),
            shown_commands: HashSet::new(),
            drafts: HashMap::new(),
            quota: None,
            closed,
            peek_until: None,
            muted: false,
            bg_opacity: 0.25,
            chime: None,
            transitions: HashMap::new(),
            pr_chime: None,
            pr_monitor: options.prs,
            feed_monitor: options.feed,
            jira_monitor: options.jira,
            sent: HashMap::new(),
            failed: HashMap::new(),
            accounts: HashMap::new(),
            jira_projects: HashMap::new(),
            pr_last_success: None,
            empty_since: options.start_hidden.then_some(UNIX_EPOCH),
            activity_reads: 0,
            transcript_bytes_read: 0,
            session_parses: 0,
            task_listings: 0,
            now,
            sends: HashMap::new(),
            issue_icons: HashMap::new(),
            connections: HashMap::new(),
            last_focused: None,
            pids: HashMap::new(),
            last_status: HashMap::new(),
            last_question: HashMap::new(),
            announced_question: HashMap::new(),
            departed: HashMap::new(),
            dir: paths.sessions,
            projects: paths.projects,
            tasks: paths.tasks,
            cache: HashMap::new(),
            button_cache: HashMap::new(),
            projects_generation: 0,
            urls: HashMap::new(),
            url_tried: HashMap::new(),
            heads: HashMap::new(),
            todos: HashMap::new(),
            session_json: HashMap::new(),
            activities: HashMap::new(),
            closed_store: options.closed_store,
            quota_reader: options.quota,
            scanner: options.scanner,
            branches,
            is_running: options
                .is_running
                .unwrap_or_else(|| Box::new(default_is_running)),
        }
    }

    pub fn draft_key(id: &str, kind: EditKind) -> String {
        if kind == EditKind::Reply {
            format!("{id}#reply")
        } else {
            id.to_string()
        }
    }

    /// The pid of the live session with this id as of the last refresh.
    pub fn pid_of(&self, id: &str) -> Option<i32> {
        self.pids.get(id).copied()
    }

    /// Sessions with a NEW mark that are still live.
    pub fn new_count(&self) -> usize {
        let live: HashSet<&str> = self.sessions.iter().map(|s| s.id.as_str()).collect();
        self.unseen
            .keys()
            .filter(|k| live.contains(k.as_str()))
            .count()
    }

    /// `HEAD` file reads of the store's own branch reader; 0 when a custom reader was injected.
    pub fn head_reads(&self) -> usize {
        match &self.branches {
            Branches::Git(g) => g.head_reads(),
            Branches::Custom(_) => 0,
        }
    }

    /// Lines the transcript scanner has parsed so far, for tests.
    pub fn parsed_lines(&self) -> usize {
        self.scanner.parsed_lines()
    }

    /// Entry count of every per-session cache, by name, for tests.
    pub fn cache_counts(&self) -> HashMap<&'static str, usize> {
        HashMap::from([
            ("cache", self.cache.len()),
            ("buttonCache", self.button_cache.len()),
            ("urls", self.urls.len()),
            ("urlTried", self.url_tried.len()),
            ("heads", self.heads.len()),
            ("todos", self.todos.len()),
            ("activities", self.activities.len()),
            ("pids", self.pids.len()),
            ("surfaces", self.surfaces.len()),
            ("sessionJSON", self.session_json.len()),
            (
                "branches",
                match &self.branches {
                    Branches::Git(g) => g.cached_cwds(),
                    Branches::Custom(_) => 0,
                },
            ),
        ])
    }

    /// The Jira site to check a token against: the first learned `*.atlassian.net` host.
    pub fn jira_host(&self) -> Option<String> {
        let mut hosts: Vec<&String> = self
            .jira_projects
            .values()
            .filter(|h| jira::is_atlassian_host(h))
            .collect();
        hosts.sort();
        hosts.first().map(|h| (*h).clone())
    }

    /// No successful open-list call for 300 s while the feature is live.
    pub fn pr_stale(&self) -> bool {
        self.pr_last_success
            .is_some_and(|s| secs_since(self.now, s) > PR_STALE_AFTER)
    }

    /// The last successful open-list call, only while `pr_stale`: a fresh success changes nothing a panel shows.
    fn pr_stale_since(&self) -> Option<SystemTime> {
        self.pr_last_success.filter(|_| self.pr_stale())
    }

    /// Takes the given ids out of the closed list (they run again) and saves the list.
    pub fn revive_closed(&mut self, ids: &HashSet<String>) {
        self.closed.revive(ids);
        if let Some(store) = &self.closed_store {
            store.save(&self.closed);
        }
    }

    pub fn hidden(&self) -> bool {
        self.empty_since
            .is_some_and(|e| secs_since(self.now, e) >= HIDE_AFTER)
    }

    pub fn peeking(&self) -> bool {
        self.peek_until.is_some_and(|u| self.now < u)
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            sessions: self.sessions.clone(),
            closed: self.closed.items().to_vec(),
            quota: self.quota.clone(),
            unseen: self.unseen.clone(),
            sends: self.sends.clone(),
            shown_commands: self.shown_commands.clone(),
            surface_misses: self.surface_misses.clone(),
            connections: self.connections.clone(),
            issue_icons: self.issue_icons.clone(),
            pr_stale_since: self.pr_stale_since(),
            pr_stale: self.pr_stale(),
            hidden: self.hidden(),
            peeking: self.peeking(),
            muted: self.muted,
            bg_opacity: self.bg_opacity,
        }
    }

    /// The chime and peek left after dropping the transitions of `excluding`. `NeedsYou` wins.
    pub fn alert(
        transitions: &HashMap<String, Chime>,
        pr_chime: Option<Chime>,
        excluding: &HashSet<String>,
    ) -> (Option<Chime>, bool) {
        let kept: Vec<Chime> = transitions
            .iter()
            .filter(|(k, _)| !excluding.contains(*k))
            .map(|(_, c)| *c)
            .collect();
        let peek = !kept.is_empty();
        let all: Vec<Chime> = kept.into_iter().chain(pr_chime).collect();
        let chime = if all.contains(&Chime::NeedsYou) {
            Some(Chime::NeedsYou)
        } else if all.is_empty() {
            None
        } else {
            Some(Chime::Done)
        };
        (chime, peek)
    }

    /// Applies `alert`: opens the peek when a transition remains and returns the chime to play.
    pub fn apply_alert(
        &mut self,
        transitions: &HashMap<String, Chime>,
        pr_chime: Option<Chime>,
        excluding: &HashSet<String>,
        at: SystemTime,
    ) -> Option<Chime> {
        let (chime, peek) = Store::alert(transitions, pr_chime, excluding);
        if peek {
            self.peek_until = Some(add_secs(at, PEEK_FOR));
        }
        chime
    }

    /// Whether the session `e.id` still exists and still allows what `e` is editing.
    pub fn allows(&self, e: &Editing) -> bool {
        self.sessions
            .iter()
            .find(|s| s.id == e.id)
            .is_some_and(|s| s.allows(e.kind, e.tool_id.as_deref()))
    }

    /// Session ids to cycle through: waiting ones (both kinds) by oldest `since`, then finished ones with a fresh Claude mark by oldest `since`.
    pub fn waiting_order(&self) -> Vec<String> {
        fn oldest(mut list: Vec<&Session>) -> Vec<String> {
            // `None` (no `since`) sorts last, like Swift's `.distantFuture`.
            list.sort_by(|a, b| {
                (a.since.is_none(), a.since, &a.id).cmp(&(b.since.is_none(), b.since, &b.id))
            });
            list.into_iter().map(|s| s.id.clone()).collect()
        }
        let waiting = self.sessions.iter().filter(|s| s.status == "waiting");
        let finished = self.sessions.iter().filter(|s| {
            s.status != "waiting"
                && s.shown_status() == "idle"
                && self.unseen.get(&s.id).is_some_and(|m| m.claude)
        });
        let mut out = oldest(waiting.collect());
        out.extend(oldest(finished.collect()));
        out
    }

    pub fn mark_seen(&mut self, id: &str) {
        self.unseen.remove(id);
    }

    /// Called with the focused cmux surface (`None` when unknown). Clears the marks of the session shown there once it is seen on two consecutive calls.
    pub fn note_focused(&mut self, surface: Option<&str>) {
        if let (Some(surface), Some(last)) = (surface, self.last_focused.as_deref())
            && last.to_lowercase() == surface.to_lowercase()
        {
            let matches: Vec<String> = self
                .unseen
                .keys()
                .filter(|k| {
                    self.surfaces
                        .get(*k)
                        .is_some_and(|s| s.to_lowercase() == surface.to_lowercase())
                })
                .cloned()
                .collect();
            for k in matches {
                self.unseen.remove(&k);
            }
        }
        self.last_focused = surface.map(String::from);
    }

    /// Whether the card shows the whole of `p`'s command; see `command_is_shown`.
    pub fn is_command_shown(&self, p: &transcript::Pending) -> bool {
        command_is_shown(&self.shown_commands, p)
    }

    /// The context-menu line of one service.
    pub fn connection_text(&self, s: Service) -> String {
        let name = s.display_name();
        match self.connections.get(&s).unwrap_or(&ConnectionLine::Missing) {
            ConnectionLine::Missing => format!("{name}: ✗ chưa có token"),
            ConnectionLine::Checking(a) => format!("{name}: {a} (đang kiểm tra)"),
            ConnectionLine::Ok(a) => format!("{name}: ✓ {a}"),
            ConnectionLine::Rejected => format!("{name}: ✗ token bị từ chối (401)"),
            ConnectionLine::Unreachable => format!("{name}: ✗ không kết nối được"),
            ConnectionLine::NoSite => format!("{name}: ✗ chưa biết site Jira"),
        }
    }

    /// The Feed question the session `id` is asking, as of the last refresh.
    pub fn question_of(&self, id: &str) -> Option<&FeedQuestion> {
        self.sessions
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.question.as_ref())
    }
}

/// What the card of `s` shows for its question; `None` without one.
fn block_of(
    s: &Session,
    sent: &HashMap<String, String>,
    failed: &HashMap<String, String>,
    now: SystemTime,
) -> Option<BlockState> {
    s.question.as_ref().map(|q| {
        feed::block_state(
            q,
            s.question_live,
            sent.get(&s.id) == Some(&q.request_id),
            failed.get(&s.id) == Some(&q.request_id),
            now,
        )
    })
}

fn connection_line(
    present: bool,
    account: Option<&String>,
    state: ConnectionState,
) -> ConnectionLine {
    let (true, Some(account)) = (present, account) else {
        return ConnectionLine::Missing;
    };
    match state {
        ConnectionState::Checking => ConnectionLine::Checking(account.clone()),
        ConnectionState::Ok => ConnectionLine::Ok(account.clone()),
        ConnectionState::Rejected => ConnectionLine::Rejected,
        ConnectionState::Unreachable => ConnectionLine::Unreachable,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandFit {
    Short,
    Long,
    TooLong,
}

/// How much of a command the card can show (Swift `Layout.commandFit`).
pub fn command_fit(s: &str) -> CommandFit {
    let lines = s.split('\n').count();
    let chars = s.graphemes(true).count();
    if lines > 12 || chars > 1200 {
        CommandFit::TooLong
    } else if lines <= 4 && chars <= 240 {
        CommandFit::Short
    } else {
        CommandFit::Long
    }
}
