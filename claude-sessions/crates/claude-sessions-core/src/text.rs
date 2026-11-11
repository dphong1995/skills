//! The text the cards and the header show, derived from state (Swift `SessionCard.swift` / `WidgetView.swift`).

use crate::code_rabbit::{self, Rabbit};
use crate::pull_requests::PRStatus;
use crate::scene::Run;
use crate::store::{ItemTone, Session};
use crate::time::secs_since;
use std::time::{Duration, SystemTime};

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

/// How long until `elapsed(Some(since), now)` shows a different string: 1 s under a minute, then the time to the next
/// minute boundary (`12m` and `1h05` change on whole minutes).
pub fn next_label_change(since: SystemTime, now: SystemTime) -> Duration {
    let secs = secs_since(now, since).max(0.0);
    if secs < 60.0 {
        return Duration::from_secs(1);
    }
    let boundary = ((secs / 60.0).floor() + 1.0) * 60.0;
    Duration::from_secs_f64(boundary - secs)
}

/// How long until `closed_ago(since, now)` shows a different string: the next minute under an hour, the next hour after.
pub fn next_closed_change(since: SystemTime, now: SystemTime) -> Duration {
    let secs = secs_since(now, since).max(0.0);
    let step = if secs < 3600.0 { 60.0 } else { 3600.0 };
    let boundary = ((secs / step).floor() + 1.0) * step;
    Duration::from_secs_f64(boundary - secs)
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

/// Inline markdown of a reply as runs (Swift `markdown(_:)`): `**bold**`, `*italic*`, `` `code` `` and `[text](http://..)`, whose
/// link is dropped and its text kept. Whitespace is preserved and an unclosed marker stays as typed. Adjacent runs with the same
/// style are merged.
pub fn markdown_runs(s: &str) -> Vec<Run> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    md_parse(&chars, Run::default(), &mut out);
    let mut merged: Vec<Run> = Vec::new();
    for r in out {
        match merged.last_mut() {
            Some(l) if (l.bold, l.italic, l.mono) == (r.bold, r.italic, r.mono) => {
                l.text.push_str(&r.text)
            }
            _ => merged.push(r),
        }
    }
    merged
}

fn md_find(chars: &[char], from: usize, pat: &[char]) -> Option<usize> {
    (from..chars.len().saturating_sub(pat.len() - 1)).find(|&i| chars[i..].starts_with(pat))
}

/// The end of the emphasis opened at `start` with `pat`: the closing marker follows non-space text.
fn md_close(chars: &[char], start: usize, pat: &[char]) -> Option<usize> {
    let open = start + pat.len();
    if chars.get(open).is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    let mut from = open + 1;
    while let Some(i) = md_find(chars, from, pat) {
        if !chars[i - 1].is_whitespace() {
            return Some(i);
        }
        from = i + 1;
    }
    None
}

fn md_parse(chars: &[char], style: Run, out: &mut Vec<Run>) {
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Run>| {
        if !plain.is_empty() {
            out.push(Run {
                text: std::mem::take(plain),
                ..style.clone()
            });
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let rest = &chars[i..];
        let mut handled = None;
        if chars[i] == '`' {
            if let Some(end) = md_find(chars, i + 1, &['`']).filter(|&e| e > i + 1) {
                flush(&mut plain, out);
                out.push(Run {
                    text: chars[i + 1..end].iter().collect(),
                    mono: true,
                    ..style.clone()
                });
                handled = Some(end + 1);
            }
        } else if rest.starts_with(&['*', '*']) {
            if let Some(end) = md_close(chars, i, &['*', '*']) {
                flush(&mut plain, out);
                let inner = Run {
                    bold: true,
                    ..style.clone()
                };
                md_parse(&chars[i + 2..end], inner, out);
                handled = Some(end + 2);
            }
        } else if chars[i] == '*' {
            if let Some(end) = md_close(chars, i, &['*']) {
                flush(&mut plain, out);
                let inner = Run {
                    italic: true,
                    ..style.clone()
                };
                md_parse(&chars[i + 1..end], inner, out);
                handled = Some(end + 1);
            }
        } else if chars[i] == '['
            && let Some(mid) = md_find(chars, i + 1, &[']', '('])
        {
            let url_start = mid + 2;
            if let Some(end) = md_find(chars, url_start, &[')']) {
                let url: String = chars[url_start..end].iter().collect();
                if (url.starts_with("http://") || url.starts_with("https://"))
                    && !url.contains(char::is_whitespace)
                {
                    flush(&mut plain, out);
                    md_parse(&chars[i + 1..mid], style.clone(), out);
                    handled = Some(end + 1);
                }
            }
        }
        match handled {
            Some(next) => i = next,
            None => {
                plain.push(chars[i]);
                i += 1;
            }
        }
    }
    flush(&mut plain, out);
}
