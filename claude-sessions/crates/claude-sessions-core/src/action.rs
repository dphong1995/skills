//! What a click or key in the widget asks the app to do (Swift `WidgetAction`), and the send button each action belongs to.

use crate::closed_sessions::ClosedSession;
use crate::credentials::Service;
use crate::pull_requests::PRRef;
use std::collections::HashMap;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SendKind {
    AskClaude,
    Nudge,
    Approve,
    Deny,
    Reply,
    Stop,
    Resume,
    Answer,
}

/// One send button of one session; `pr` tells the PR buttons of a session apart, `detail` the prompt of an approve/deny (its tool id).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SendKey {
    pub session: String,
    pub kind: SendKind,
    pub pr: Option<PRRef>,
    pub detail: Option<String>,
}

impl SendKey {
    pub fn new(session: &str, kind: SendKind) -> Self {
        SendKey {
            session: session.to_string(),
            kind,
            pr: None,
            detail: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Jump(String),
    Approve {
        session: String,
        tool_id: String,
    },
    Deny {
        session: String,
        tool_id: String,
        reason: String,
    },
    OpenLink(String, Url),
    AskClaude {
        session: String,
        prompt: String,
        pr: PRRef,
    },
    /// Types a prompt of the user's own into a finished session.
    Reply {
        session: String,
        text: String,
    },
    /// Sends Esc to a busy session.
    Stop(String),
    /// Answers the Feed question `request_id` through cmux's socket; picks per question and Other text by question index.
    Answer {
        session: String,
        request_id: String,
        answers: Vec<Vec<String>>,
        other: HashMap<usize, String>,
    },
    NudgeRabbit(String, PRRef),
    /// Opens a cmux workspace that resumes a closed session.
    Resume(ClosedSession),
    /// Opens the token dialog of a service; belongs to no session.
    Connect(Service),
    /// The mouse rested on a fresh row: clears its NEW mark (the hover dwell of the card).
    MarkSeen(String),
    /// `Xem đủ`: expands a long command so it can be approved or denied from the widget.
    ShowCommand {
        session: String,
        tool_id: String,
    },
}

impl Action {
    /// The send button this action belongs to; `None` for actions that only open things.
    pub fn send_key(&self) -> Option<SendKey> {
        let key = |session: &str, kind| SendKey::new(session, kind);
        match self {
            Action::AskClaude { session, pr, .. } => Some(SendKey {
                pr: Some(pr.clone()),
                ..key(session, SendKind::AskClaude)
            }),
            Action::NudgeRabbit(session, pr) => Some(SendKey {
                pr: Some(pr.clone()),
                ..key(session, SendKind::Nudge)
            }),
            Action::Approve { session, tool_id } => Some(SendKey {
                detail: Some(tool_id.clone()),
                ..key(session, SendKind::Approve)
            }),
            Action::Deny {
                session, tool_id, ..
            } => Some(SendKey {
                detail: Some(tool_id.clone()),
                ..key(session, SendKind::Deny)
            }),
            Action::Reply { session, .. } => Some(key(session, SendKind::Reply)),
            Action::Stop(session) => Some(key(session, SendKind::Stop)),
            Action::Resume(c) => Some(key(&c.id, SendKind::Resume)),
            Action::Answer { session, .. } => Some(key(session, SendKind::Answer)),
            Action::Jump(_)
            | Action::OpenLink(..)
            | Action::Connect(_)
            | Action::MarkSeen(_)
            | Action::ShowCommand { .. } => None,
        }
    }

    /// The session the action is about; empty for `Connect`.
    pub fn session_id(&self) -> &str {
        match self {
            Action::Jump(id)
            | Action::Stop(id)
            | Action::MarkSeen(id)
            | Action::OpenLink(id, _)
            | Action::NudgeRabbit(id, _) => id,
            Action::Approve { session, .. }
            | Action::Deny { session, .. }
            | Action::AskClaude { session, .. }
            | Action::Reply { session, .. }
            | Action::Answer { session, .. }
            | Action::ShowCommand { session, .. } => session,
            Action::Resume(c) => &c.id,
            Action::Connect(_) => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr() -> PRRef {
        PRRef {
            workspace: "w".into(),
            repo: "r".into(),
            id: 7,
        }
    }

    #[test]
    fn send_keys_tell_buttons_apart() {
        let ask = Action::AskClaude {
            session: "a".into(),
            prompt: "p".into(),
            pr: pr(),
        };
        let nudge = Action::NudgeRabbit("a".into(), pr());
        assert_ne!(ask.send_key(), nudge.send_key());
        let approve = Action::Approve {
            session: "a".into(),
            tool_id: "t1".into(),
        };
        let deny = Action::Deny {
            session: "a".into(),
            tool_id: "t1".into(),
            reason: "no".into(),
        };
        assert_eq!(approve.send_key().unwrap().detail.as_deref(), Some("t1"));
        assert_ne!(approve.send_key(), deny.send_key());
        assert_eq!(
            Action::Stop("a".into()).send_key(),
            Some(SendKey::new("a", SendKind::Stop))
        );
    }

    #[test]
    fn opening_actions_have_no_send_key() {
        assert_eq!(Action::Jump("a".into()).send_key(), None);
        assert_eq!(Action::MarkSeen("a".into()).send_key(), None);
        assert_eq!(Action::Connect(Service::Jira).send_key(), None);
        assert_eq!(Action::Connect(Service::Jira).session_id(), "");
        assert_eq!(Action::MarkSeen("a".into()).session_id(), "a");
    }
}
