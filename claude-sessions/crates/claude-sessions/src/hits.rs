//! What a `HitId` of a view's scene means. Views register a `Target` per clickable rect; its id is a stable hash of the target,
//! so an unchanged control keeps its id across rebuilds. `Target::click` resolves it against the current state into an `Action`
//! for the store or a `PanelChange` for the panel.

use crate::settings_model::SettingsAction;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::panel_state::{EditKind, Editing, PanelState};
use claude_sessions_core::pull_requests::{self, PRRef};
use claude_sessions_core::scene::HitId;
use claude_sessions_core::store::{Action, Session, Snapshot};
use claude_sessions_core::time::secs_since;
use std::collections::HashMap;
use std::time::SystemTime;
use url::Url;

/// A second `Dừng` click within this many seconds sends the stop.
pub const STOP_ARM_SECS: f64 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// The card body: jump to the session's pane.
    Jump(String),
    /// `Trả lời…` on a finished card: opens the reply field.
    Reply(String),
    /// `Gửi` of the reply field.
    ReplySend(String),
    /// `Dừng` on a busy card (two clicks).
    Stop(String),
    /// `Xem đủ`: shows a long command whole, which unlocks `Cho phép` / `Từ chối…`.
    ShowCommand {
        session: String,
        tool_id: String,
    },
    Approve {
        session: String,
        tool_id: String,
    },
    /// `Từ chối…`: opens the reason field.
    Deny {
        session: String,
        tool_id: String,
    },
    /// `Gửi` of the deny reason field.
    DenySend {
        session: String,
        tool_id: String,
    },
    PickOption {
        session: String,
        question: usize,
        label: String,
    },
    /// The `Khác…` row of a question.
    OpenOther {
        session: String,
        question: usize,
    },
    /// `Gửi` of the question form.
    Answer(String),
    /// A ticket / PR / link button or list row.
    Link {
        session: String,
        url: Url,
    },
    /// The menu count: toggles the inline link list.
    Menu(String),
    AskClaude {
        session: String,
        pr: PRRef,
    },
    Nudge {
        session: String,
        pr: PRRef,
    },
    Connect(Service),
    ToggleClosed,
    /// Resume a closed session by id.
    Resume(String),
    /// A `· 2 lệnh nền ›` / `N đang chạy ›` line: opens or closes the process list of a live session or closed row.
    ToggleProcs(String),
    /// `Tắt` / `Tắt N` / `Tắt cả N` / `✕` when not armed; `key` is a `ProcGroup::key`, `all:<session>` or `all`.
    ArmKill {
        session: String,
        key: String,
    },
    /// The same control while armed (shown as `Tắt?`): a click kills.
    Kill {
        session: String,
        key: String,
    },
    /// A control of the Settings window.
    Settings(SettingsAction),
}

/// A change of the panel's own state.
#[derive(Clone, Debug, PartialEq)]
pub enum PanelChange {
    OpenEditor(Editing),
    ToggleMenu(String),
    ToggleClosed,
    ArmStop(String),
    ToggleProcs(String),
    ArmKill(String),
    Pick {
        session: String,
        question: usize,
        label: String,
    },
    /// Opens the `Khác…` field of a question (and asks for focus).
    OpenOther {
        session: String,
        question: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Click {
    Action(Action),
    Panel(PanelChange),
    /// The control no longer applies to the current state.
    Stale,
}

pub struct ClickEnv<'a> {
    pub snap: &'a Snapshot,
    pub panel: &'a PanelState,
    pub now: SystemTime,
    /// The text of the field the click belongs to (deny reason, reply), empty otherwise.
    pub text: &'a str,
    /// The `prPrompt` template for `Nhờ Claude`.
    pub pr_prompt: &'a str,
}

impl Target {
    /// The session the control belongs to, when it has one.
    pub fn session(&self) -> Option<&str> {
        match self {
            Target::Jump(s)
            | Target::Reply(s)
            | Target::ReplySend(s)
            | Target::Stop(s)
            | Target::Answer(s)
            | Target::Menu(s)
            | Target::ToggleProcs(s) => Some(s),
            Target::ShowCommand { session, .. }
            | Target::Approve { session, .. }
            | Target::Deny { session, .. }
            | Target::DenySend { session, .. }
            | Target::PickOption { session, .. }
            | Target::OpenOther { session, .. }
            | Target::Link { session, .. }
            | Target::AskClaude { session, .. }
            | Target::Nudge { session, .. } => Some(session),
            Target::ArmKill { session, .. } | Target::Kill { session, .. } => {
                Some(session.as_str()).filter(|s| !s.is_empty())
            }
            Target::Resume(_) | Target::Connect(_) | Target::ToggleClosed | Target::Settings(_) => {
                None
            }
        }
    }

    /// The PR the control stands for: a PR link's, or the one `Nhắc duyệt` / `Nhờ Claude` is about.
    pub fn pr(&self) -> Option<PRRef> {
        match self {
            Target::Nudge { pr, .. } | Target::AskClaude { pr, .. } => Some(pr.clone()),
            Target::Link { url, .. } => pull_requests::refs(url.as_str()).into_iter().next(),
            _ => None,
        }
    }

    /// Stable across rebuilds: FNV-1a of the debug form.
    pub fn id(&self) -> HitId {
        let mut h: u32 = 0x811c_9dc5;
        for b in format!("{self:?}").bytes() {
            h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
        }
        HitId(h)
    }

    pub fn click(&self, env: &ClickEnv) -> Click {
        let session = |id: &str| env.snap.sessions.iter().find(|s| s.id == id);
        match self {
            Target::Jump(id) => Click::Action(Action::Jump(id.clone())),
            Target::Reply(id) => Click::Panel(PanelChange::OpenEditor(Editing {
                id: id.clone(),
                kind: EditKind::Reply,
                tool_id: None,
            })),
            Target::ReplySend(id) if !env.text.trim().is_empty() => Click::Action(Action::Reply {
                session: id.clone(),
                text: env.text.to_string(),
            }),
            Target::ReplySend(_) => Click::Stale,
            Target::Stop(id) => {
                let armed = env
                    .panel
                    .stop_armed
                    .get(id)
                    .is_some_and(|t| secs_since(env.now, *t) < STOP_ARM_SECS);
                if armed {
                    Click::Action(Action::Stop(id.clone()))
                } else {
                    Click::Panel(PanelChange::ArmStop(id.clone()))
                }
            }
            Target::ShowCommand { session, tool_id } => Click::Action(Action::ShowCommand {
                session: session.clone(),
                tool_id: tool_id.clone(),
            }),
            Target::Approve { session, tool_id } => Click::Action(Action::Approve {
                session: session.clone(),
                tool_id: tool_id.clone(),
            }),
            Target::Deny { session, tool_id } => Click::Panel(PanelChange::OpenEditor(Editing {
                id: session.clone(),
                kind: EditKind::Deny,
                tool_id: Some(tool_id.clone()),
            })),
            Target::DenySend { session, tool_id } => Click::Action(Action::Deny {
                session: session.clone(),
                tool_id: tool_id.clone(),
                reason: env.text.to_string(),
            }),
            Target::PickOption {
                session,
                question,
                label,
            } => Click::Panel(PanelChange::Pick {
                session: session.clone(),
                question: *question,
                label: label.clone(),
            }),
            Target::OpenOther { session, question } => Click::Panel(PanelChange::OpenOther {
                session: session.clone(),
                question: *question,
            }),
            Target::Answer(id) => {
                let Some(q) = session(id).and_then(|s| s.question.as_ref()) else {
                    return Click::Stale;
                };
                Click::Action(Action::Answer {
                    session: id.clone(),
                    request_id: q.request_id.clone(),
                    answers: env
                        .panel
                        .answers
                        .get(&q.request_id)
                        .cloned()
                        .unwrap_or_default(),
                    other: env
                        .panel
                        .others
                        .get(&q.request_id)
                        .cloned()
                        .unwrap_or_default(),
                })
            }
            Target::Link { session, url } => {
                Click::Action(Action::OpenLink(session.clone(), url.clone()))
            }
            Target::Menu(id) => Click::Panel(PanelChange::ToggleMenu(id.clone())),
            Target::AskClaude { session: id, pr } => {
                let Some(status) = session(id).and_then(|s| s.prs.iter().find(|p| &p.ref_ == pr))
                else {
                    return Click::Stale;
                };
                Click::Action(Action::AskClaude {
                    session: id.clone(),
                    prompt: pull_requests::prompt(env.pr_prompt, &status.url, pr.id),
                    pr: pr.clone(),
                })
            }
            Target::Nudge { session, pr } => {
                Click::Action(Action::NudgeRabbit(session.clone(), pr.clone()))
            }
            Target::Connect(service) => Click::Action(Action::Connect(*service)),
            Target::ToggleClosed => Click::Panel(PanelChange::ToggleClosed),
            Target::Resume(id) => match env.snap.closed.iter().find(|c| &c.id == id) {
                Some(c) => Click::Action(Action::Resume(c.clone())),
                None => Click::Stale,
            },
            Target::ToggleProcs(id) => Click::Panel(PanelChange::ToggleProcs(id.clone())),
            Target::ArmKill { key, .. } => Click::Panel(PanelChange::ArmKill(key.clone())),
            Target::Kill { session, key } => {
                let armed = env
                    .panel
                    .kill_armed
                    .get(key)
                    .is_some_and(|t| secs_since(env.now, *t) < STOP_ARM_SECS);
                if !armed {
                    return Click::Panel(PanelChange::ArmKill(key.clone()));
                }
                match kill_targets(env.snap, key) {
                    Some(targets) if !targets.is_empty() => Click::Action(Action::KillProcs {
                        session: session.clone(),
                        key: key.clone(),
                        targets,
                    }),
                    _ => Click::Stale,
                }
            }
            // The Settings window's `Targets` carry this variant so it can reuse the panel's hit ids and `node_of`, but its
            // clicks are resolved by the app's own handler and never reach here: stale if one ever does.
            Target::Settings(_) => Click::Stale,
        }
    }
}

impl PanelChange {
    pub fn apply(&self, panel: &mut PanelState, snap: &Snapshot, now: SystemTime) {
        let question = |s: &str| -> Option<&Session> { snap.sessions.iter().find(|x| x.id == s) };
        match self {
            PanelChange::OpenEditor(e) => panel.editing = Some(e.clone()),
            PanelChange::ToggleMenu(id) => {
                if !panel.open_menus.remove(id) {
                    panel.open_menus.insert(id.clone());
                }
            }
            PanelChange::ToggleClosed => panel.closed_open = !panel.closed_open,
            PanelChange::ArmStop(id) => {
                panel.stop_armed.insert(id.clone(), now);
            }
            PanelChange::ToggleProcs(id) => {
                if !panel.open_procs.remove(id) {
                    panel.open_procs.insert(id.clone());
                }
            }
            PanelChange::ArmKill(key) => {
                panel.kill_armed.insert(key.clone(), now);
            }
            PanelChange::Pick {
                session,
                question: i,
                label,
            } => {
                if let Some(q) = question(session).and_then(|s| s.question.as_ref()) {
                    panel.pick(q, *i, label);
                }
            }
            PanelChange::OpenOther {
                session,
                question: i,
            } => {
                panel.other_key = Some(format!("{session}#{i}"));
                panel.editing = Some(Editing {
                    id: session.clone(),
                    kind: EditKind::Answer,
                    tool_id: None,
                });
            }
        }
    }
}

/// The `(pid, start)` pairs a kill key stands for in `snap`: a group's key, `all:<session>` for one closed row, `all` for every
/// closed row with leftovers. `None` when nothing in the snapshot has that key.
fn kill_targets(snap: &Snapshot, key: &str) -> Option<Vec<(i32, u64)>> {
    let closed = || snap.closed_rows.iter().filter_map(|r| r.procs.as_ref());
    if key == "all" {
        return Some(closed().flat_map(|p| p.targets()).collect());
    }
    if let Some(id) = key.strip_prefix("all:") {
        return closed().find(|p| p.session == id).map(|p| p.targets());
    }
    snap.procs
        .values()
        .chain(closed())
        .flat_map(|p| p.groups())
        .find(|g| g.key == key)
        .map(|g| g.members.clone())
}

/// The targets of one built scene, by hit id.
#[derive(Clone, Debug, Default)]
pub struct Targets(pub HashMap<HitId, Target>);

impl Targets {
    pub fn add(&mut self, t: Target) -> HitId {
        let id = t.id();
        debug_assert!(
            self.0.get(&id).is_none_or(|old| *old == t),
            "hit id collision between {t:?} and {:?}",
            self.0[&id]
        );
        self.0.insert(id, t);
        id
    }

    pub fn get(&self, id: HitId) -> Option<&Target> {
        self.0.get(&id)
    }
}
