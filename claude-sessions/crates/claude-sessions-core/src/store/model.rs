//! The data the store hands out: `Session`, the link menu, and the `Snapshot` the views read.

use crate::closed_sessions::{self, ClosedSession};
use crate::credentials::Service;
use crate::feed::{BlockState, FeedQuestion};
use crate::jira::JiraTone;
use crate::panel_state::EditKind;
use crate::pull_requests::{PRRef, PRStatus};
use crate::quota::Quota;
use crate::tasks::Todo;
use crate::transcript::{Pending, PendingState};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;
use url::Url;

pub use crate::action::{Action, SendKey, SendKind};
pub use crate::pull_requests::Chime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemTone {
    Todo,
    Doing,
    Done,
    Open,
    Merged,
    Closed,
}

impl From<JiraTone> for ItemTone {
    fn from(t: JiraTone) -> Self {
        match t {
            JiraTone::Todo => ItemTone::Todo,
            JiraTone::Doing => ItemTone::Doing,
            JiraTone::Done => ItemTone::Done,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Ticket,
    Pr,
    Link,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkItem {
    pub text: String,
    pub url: Url,
    pub kind: LinkKind,
    pub code: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub tone: Option<ItemTone>,
    pub icon_url: Option<Url>,
    pub help: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkSection {
    pub title: String,
    pub items: Vec<LinkItem>,
}

/// A ticket or PR button of a session (Swift `SessionLink`).
#[derive(Clone, Debug, PartialEq)]
pub struct SessionLink {
    pub label: String,
    pub url: Url,
    /// Hover text; a ticket button shows its Jira title and status here.
    pub help: Option<String>,
    pub icon_url: Option<Url>,
    pub tone: Option<ItemTone>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionLine {
    Missing,
    Checking(String),
    Ok(String),
    Rejected,
    Unreachable,
    NoSite,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewMark {
    pub claude: bool,
    pub prs: HashSet<PRRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendState {
    Sending,
    Sent,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub cwd: String,
    pub status: String,
    pub waiting_for: Option<String>,
    pub since: Option<SystemTime>,
    pub last_reply: Option<String>,
    pub pending: Option<PendingState>,
    pub pid: i32,
    pub name_source: Option<String>,
    pub ai_title: Option<String>,
    pub first_prompt: Option<String>,
    pub branch: Option<String>,
    pub todo: Option<Todo>,
    pub links: Vec<SessionLink>,
    pub activity: Option<String>,
    /// Descriptions of the foreground subagents still without a result, in launch order; only while the session is `busy`.
    pub agents: Vec<String>,
    /// Pending background agents and workflows; `Some` only while the session is `busy` but its main turn has ended.
    pub background_agents: Option<i64>,
    /// The status file was rewritten `busy` after the last main turn ended with no pending background count.
    pub busy_after_turn: bool,
    pub prs: Vec<PRStatus>,
    /// Every ticket and PR of the session, grouped for the inline menu.
    pub menu: Vec<LinkSection>,
    /// The menu holds a ticket and Jira is not connected: the menu ends with a `Kết nối Jira…` item.
    pub menu_needs_jira: bool,
    /// The Feed question this session asked; kept (with `question_live` false) while the session still waits after the item left the list.
    pub question: Option<FeedQuestion>,
    pub question_live: bool,
    /// What the card shows for `question`, as of the last refresh or send note.
    pub block: Option<BlockState>,
}

impl Session {
    pub fn new(id: &str, name: &str, cwd: &str, status: &str) -> Self {
        Session {
            id: id.into(),
            name: name.into(),
            cwd: cwd.into(),
            status: status.into(),
            waiting_for: None,
            since: None,
            last_reply: None,
            pending: None,
            pid: 0,
            name_source: None,
            ai_title: None,
            first_prompt: None,
            branch: None,
            todo: None,
            links: Vec::new(),
            activity: None,
            agents: Vec::new(),
            background_agents: None,
            busy_after_turn: false,
            prs: Vec::new(),
            menu: Vec::new(),
            menu_needs_jira: false,
            question: None,
            question_live: false,
            block: None,
        }
    }

    /// `idle` while only background agents or a background shell command run, else the raw `status`.
    pub fn shown_status(&self) -> &str {
        if self.background_agents.is_some() || self.status == "shell" {
            "idle"
        } else {
            &self.status
        }
    }

    /// The colour, group and sort order: `busy` while `waits_in_background`, else `shown_status`.
    pub fn color_status(&self) -> &str {
        if self.waits_in_background() {
            "busy"
        } else {
            self.shown_status()
        }
    }

    /// Waiting on background agents, a background shell command, or uncounted work after the turn.
    pub fn waits_in_background(&self) -> bool {
        self.background_agents.unwrap_or(0) > 0 || self.busy_after_turn || self.status == "shell"
    }

    pub fn title(&self) -> &str {
        if self.name_source.as_deref() == Some("user") {
            &self.name
        } else {
            self.ai_title
                .as_deref()
                .or(self.first_prompt.as_deref())
                .unwrap_or(&self.name)
        }
    }

    pub fn folder(&self) -> String {
        closed_sessions::folder(&self.cwd)
    }

    /// Running subagents, foreground and background (the two are never non-zero together).
    pub fn agent_count(&self) -> i64 {
        self.agents.len() as i64 + self.background_agents.unwrap_or(0)
    }

    pub fn menu_count(&self) -> usize {
        self.menu.iter().map(|s| s.items.len()).sum()
    }

    /// Shown as done or waiting, including a main turn that ended while background agents still run.
    pub fn is_finished(&self) -> bool {
        self.shown_status() == "idle" || self.status == "waiting"
    }

    /// Can take a message now: the main turn is over.
    pub fn can_ask_claude(&self) -> bool {
        self.shown_status() == "idle"
    }

    pub fn is_permission_prompt(&self) -> bool {
        self.status == "waiting" && self.waiting_for.as_deref() == Some("permission prompt")
    }

    pub fn approvable(&self) -> Option<&Pending> {
        match &self.pending {
            Some(PendingState::One(p)) => Some(p),
            _ => None,
        }
    }

    /// Whether an editor of this kind, or an approve/deny of `tool_id` (approve and deny are both `Deny`), still applies.
    pub fn allows(&self, kind: EditKind, tool_id: Option<&str>) -> bool {
        match kind {
            EditKind::Deny => {
                tool_id.is_some() && self.approvable().map(|p| p.tool_id.as_str()) == tool_id
            }
            EditKind::Reply => self.can_ask_claude(),
            EditKind::Answer => self.question_live,
        }
    }

    pub fn closed(&self, at: SystemTime) -> ClosedSession {
        ClosedSession {
            id: self.id.clone(),
            title: self.title().to_string(),
            cwd: self.cwd.clone(),
            branch: self.branch.clone(),
            closed_at: at,
        }
    }
}

/// The three groups of the list, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    NeedsYou,
    Done,
    Running,
}

/// Everything the views read. Holds timestamps, never "now" or formatted elapsed text, so it stays equal across idle ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub sessions: Vec<Session>,
    pub closed: Vec<ClosedSession>,
    pub quota: Option<Quota>,
    pub unseen: HashMap<String, NewMark>,
    pub sends: HashMap<SendKey, (SendState, SystemTime)>,
    pub shown_commands: HashSet<String>,
    pub surface_misses: HashSet<String>,
    pub connections: HashMap<Service, ConnectionLine>,
    pub issue_icons: HashMap<Url, Vec<u8>>,
    pub pr_last_success: Option<SystemTime>,
    pub pr_stale: bool,
    pub hidden: bool,
    pub peeking: bool,
    pub muted: bool,
    pub bg_opacity: f64,
}

impl Snapshot {
    /// Session ids grouped by need, in display order: waiting for approval or an answer, done, running (busy and background work).
    pub fn sections(&self) -> Vec<(Section, Vec<String>)> {
        let pick = |want: &str| -> Vec<String> {
            self.sessions
                .iter()
                .filter(|s| s.color_status() == want)
                .map(|s| s.id.clone())
                .collect()
        };
        let running = self
            .sessions
            .iter()
            .filter(|s| !matches!(s.color_status(), "waiting" | "idle"))
            .map(|s| s.id.clone())
            .collect();
        vec![
            (Section::NeedsYou, pick("waiting")),
            (Section::Done, pick("idle")),
            (Section::Running, running),
        ]
    }

    /// What the button of `k` shows at `now`: `Sending` until the store times it out, `Sent`/`Failed` for 3 s, then `None`.
    pub fn send_state(&self, k: &SendKey, now: SystemTime) -> Option<SendState> {
        shown_send(self.sends.get(k)?, now)
    }
}

pub(super) const SEND_SHOWN: f64 = 3.0;

pub(super) fn shown_send(e: &(SendState, SystemTime), now: SystemTime) -> Option<SendState> {
    match e.0 {
        SendState::Sending => Some(SendState::Sending),
        s => (crate::time::secs_since(now, e.1) < SEND_SHOWN).then_some(s),
    }
}
