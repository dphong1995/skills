//! The text the cards and the header show, derived from state (Swift `SessionCard.swift` / `WidgetView.swift`).

use crate::code_rabbit::{self, Rabbit};
use crate::pull_requests::PRStatus;
use crate::store::{ItemTone, Session};
use crate::time::secs_since;
use std::time::SystemTime;

/// The state line of a card.
pub fn status_text(s: &Session) -> String {
    if let Some(n) = s.background_agents {
        return if n > 0 {
            format!("chờ {n} agent nền")
        } else if s.busy_after_turn {
            "chờ agent nền".into()
        } else {
            "xong".into()
        };
    }
    match s.status.as_str() {
        "busy" => "đang chạy".into(),
        "idle" => "xong".into(),
        "shell" => "chờ lệnh nền".into(),
        "waiting" => if s.is_permission_prompt() {
            "chờ duyệt"
        } else {
            "chờ trả lời"
        }
        .into(),
        "" => "?".into(),
        other => other.to_string(),
    }
}

/// `45s`, `12m`, `1h05`; empty without a start.
pub fn elapsed(since: Option<SystemTime>, now: SystemTime) -> String {
    let Some(since) = since else {
        return String::new();
    };
    let s = secs_since(now, since).max(0.0) as i64;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else {
        format!("{}h{:02}", s / 3600, (s % 3600) / 60)
    }
}

/// `12p` / `3h` since a session closed.
pub fn closed_ago(since: SystemTime, now: SystemTime) -> String {
    let s = secs_since(now, since).max(0.0) as i64;
    if s < 3600 {
        format!("{}p", (s / 60).max(1))
    } else {
        format!("{}h", s / 3600)
    }
}

/// Running subagents, foreground and background.
pub fn agent_count(s: &Session) -> i64 {
    s.agent_count()
}

/// What a part of a PR line shows next to its text; the view maps it to a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    Pr,
    State(ItemTone),
    ChangesRequested,
    Approved,
    Rabbit,
    Comments,
    Person,
    Clock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartTint {
    Red,
    Green,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrPart {
    pub text: String,
    pub kind: PartKind,
    pub tint: Option<PartTint>,
    /// 0 for a part that stays; a part with rank r is dropped once the row has to drop r tags (lower ranks go first).
    pub drop_rank: u8,
    pub help: Option<String>,
}

fn part(text: String, kind: PartKind, tint: Option<PartTint>, drop_rank: u8) -> PrPart {
    PrPart {
        text,
        kind,
        tint,
        drop_rank,
        help: None,
    }
}

/// The parts of a PR line in order; `pr_line` joins their text. `stale` is when the last successful PR poll was, if it is overdue.
pub fn pr_parts(pr: &PRStatus, now: SystemTime, stale: Option<SystemTime>) -> Vec<PrPart> {
    let mut parts = vec![part(format!("PR #{}", pr.ref_.id), PartKind::Pr, None, 0)];
    if !pr.is_open() {
        let tone = if pr.state == "MERGED" {
            ItemTone::Merged
        } else {
            ItemTone::Closed
        };
        parts.push(part(
            pr.state_word().to_string(),
            PartKind::State(tone),
            None,
            0,
        ));
        return parts;
    }
    if pr.changes_requested > 0 {
        parts.push(part(
            format!("{} yêu cầu sửa", pr.changes_requested),
            PartKind::ChangesRequested,
            Some(PartTint::Red),
            0,
        ));
    }
    if pr.reviewers > 0 {
        parts.push(part(
            format!("{}/{}", pr.approved, pr.reviewers),
            PartKind::Approved,
            (pr.approved > 0).then_some(PartTint::Green),
            0,
        ));
    }
    if let Some(rabbit) = &pr.rabbit {
        let mut p = part(
            code_rabbit::label(rabbit, pr.rabbit_threads),
            PartKind::Rabbit,
            None,
            0,
        );
        if let Rabbit::ApprovalFailed(reason) = rabbit {
            p.tint = Some(PartTint::Red);
            p.help = Some(reason.clone());
        }
        parts.push(p);
    }
    if pr.comments > 0 {
        parts.push(part(pr.comments.to_string(), PartKind::Comments, None, 0));
    }
    if let Some(c) = &pr.last_other {
        parts.push(part(
            format!("{} {}", c.name, elapsed(Some(c.at), now)),
            PartKind::Person,
            None,
            2,
        ));
    }
    if let Some(stale) = stale {
        parts.push(part(
            format!("cũ {}", elapsed(Some(stale), now)),
            PartKind::Clock,
            None,
            1,
        ));
    }
    parts
}

pub fn pr_line(pr: &PRStatus, now: SystemTime, stale: Option<SystemTime>) -> String {
    pr_parts(pr, now, stale)
        .into_iter()
        .map(|p| p.text)
        .collect::<Vec<_>>()
        .join(" · ")
}
