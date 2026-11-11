//! Bitbucket pull requests: links, remotes, the status read from the API, the events between two statuses and the prompt template.

use crate::code_rabbit::{self, Rabbit};
use crate::shell::parse_iso_date;
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::SystemTime;
use url::Url;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PRRef {
    pub workspace: String,
    pub repo: String,
    pub id: i64,
}

static LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https://bitbucket\.org/([\w.-]+)/([\w.-]+)/pull-requests/(\d+)").unwrap()
});

/// The Bitbucket PRs linked in `text`, first appearance order, one per PR.
pub fn refs(text: &str) -> Vec<PRRef> {
    let mut out: Vec<PRRef> = Vec::new();
    for m in LINK_REGEX.captures_iter(text) {
        let Ok(id) = m[3].parse::<i64>() else {
            continue;
        };
        // The `url` crate resolves `.` and `..` segments: such a ref would steer a request to another API path.
        if [&m[1], &m[2]].iter().any(|seg| matches!(*seg, "." | "..")) {
            continue;
        }
        let r = PRRef {
            workspace: m[1].to_string(),
            repo: m[2].to_string(),
            id,
        };
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

/// `!` bash-mode and local-command output, which Claude Code stores as plain user lines.
pub fn is_shell_output(s: &str) -> bool {
    let t = s.trim();
    [
        "<bash-stdout>",
        "<bash-stderr>",
        "<local-command-stdout>",
        "<local-command-stderr>",
    ]
    .iter()
    .any(|p| t.starts_with(p))
}

#[derive(Clone, Debug, PartialEq)]
pub struct PRComment {
    pub name: String,
    pub at: SystemTime,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RabbitThreads {
    pub pending: i64,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PREvent {
    Comment,
    Approved,
    ChangesRequested,
    Merged,
    Declined,
    RabbitNeedsYou,
    RabbitDone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chime {
    Done,
    NeedsYou,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PRStatus {
    pub ref_: PRRef,
    pub url: Url,
    /// OPEN, MERGED, DECLINED or SUPERSEDED.
    pub state: String,
    pub reviewers: i64,
    pub approved: i64,
    pub changes_requested: i64,
    pub comments: i64,
    pub commit: Option<String>,
    /// `full_name` of the source repository.
    pub source_repo: Option<String>,
    pub branch: Option<String>,
    pub title: Option<String>,
    pub last_other: Option<PRComment>,
    /// uuids of REVIEWER participants with `approved == true`.
    pub approvers: HashSet<String>,
    /// uuid of the PR author.
    pub author: Option<String>,
    /// The displayed CodeRabbit state: `rabbit_status` resolved against approvers and comments.
    pub rabbit: Option<Rabbit>,
    /// The raw state from `/statuses` plus the findings count.
    pub rabbit_status: Option<Rabbit>,
    pub rabbit_since: Option<SystemTime>,
    /// CodeRabbit inline threads still to answer; `None` until a full comments fetch succeeded.
    pub rabbit_threads: Option<RabbitThreads>,
    /// A reviewer's comment is still unanswered by `me`.
    pub rabbit_others_pending: bool,
}

impl PRStatus {
    pub fn is_open(&self) -> bool {
        self.state == "OPEN"
    }

    pub fn state_word(&self) -> &'static str {
        if self.is_open() {
            "mở"
        } else if self.state == "MERGED" {
            "đã merge"
        } else {
            "đã đóng"
        }
    }
}

pub const DEFAULT_PROMPT: &str = "Đọc các bình luận chưa xử lý và kết quả pipeline của PR {url}, sửa code theo đó, push, rồi trả lời từng bình luận.";

static REMOTE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:git@bitbucket\.org:|https://(?:[^@/]+@)?bitbucket\.org/)([\w.-]+)/([\w.-]+?)(?:\.git)?/?$")
        .unwrap()
});

/// `(workspace, repo)` of a Bitbucket git remote URL.
pub fn repo_from_remote(s: &str) -> Option<(String, String)> {
    let m = REMOTE_REGEX.captures(s.trim())?;
    if matches!(&m[1], "." | "..") || matches!(&m[2], "." | "..") {
        return None;
    }
    Some((m[1].to_string(), m[2].to_string()))
}

fn str_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter().try_fold(v, |v, k| v.get(k))?.as_str()
}

pub fn status_from(o: &Value) -> Option<PRStatus> {
    let href = str_at(o, &["links", "html", "href"])?;
    let ref_ = refs(href).into_iter().next()?;
    let url = Url::parse(href).ok()?;
    let participants = o.get("participants").and_then(Value::as_array);
    let reviewers: Vec<&Value> = participants
        .into_iter()
        .flatten()
        .filter(|p| p.get("role").and_then(Value::as_str) == Some("REVIEWER"))
        .collect();
    let approving: Vec<&&Value> = reviewers
        .iter()
        .filter(|p| p.get("approved").and_then(Value::as_bool) == Some(true))
        .collect();
    let source = o.get("source");
    let src = |path: &[&str]| source.and_then(|s| str_at(s, path)).map(str::to_string);
    Some(PRStatus {
        ref_,
        url,
        state: str_at(o, &["state"]).unwrap_or("").to_string(),
        reviewers: reviewers.len() as i64,
        approved: approving.len() as i64,
        changes_requested: reviewers
            .iter()
            .filter(|p| p.get("state").and_then(Value::as_str) == Some("changes_requested"))
            .count() as i64,
        comments: o.get("comment_count").and_then(Value::as_i64).unwrap_or(0),
        commit: src(&["commit", "hash"]),
        source_repo: src(&["repository", "full_name"]),
        branch: src(&["branch", "name"]),
        title: str_at(o, &["title"]).map(str::to_string),
        last_other: None,
        approvers: approving
            .iter()
            .filter_map(|p| str_at(p, &["user", "uuid"]).map(str::to_string))
            .collect(),
        author: str_at(o, &["author", "uuid"]).map(str::to_string),
        rabbit: None,
        rabbit_status: None,
        rabbit_since: None,
        rabbit_threads: None,
        rabbit_others_pending: false,
    })
}

pub struct Page {
    pub values: Vec<PRStatus>,
    pub next: Option<String>,
}

pub fn page(data: &[u8]) -> Option<Page> {
    let o: Value = serde_json::from_slice(data).ok()?;
    o.as_object()?;
    let values = o
        .get("values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(status_from)
        .collect();
    Some(Page {
        values,
        next: str_at(&o, &["next"]).map(str::to_string),
    })
}

/// The newest comment of someone other than `me`, skipping deleted and CodeRabbit comments.
/// `None` when there is none, or when that comment has no readable name or date.
pub fn last_other_from_comments(data: &[u8], me: &str) -> Option<PRComment> {
    let o: Value = serde_json::from_slice(data).ok()?;
    let values = o.get("values")?.as_array()?;
    for v in values {
        if v.get("deleted").and_then(Value::as_bool) == Some(true) || code_rabbit::is_comment(v) {
            continue;
        }
        if str_at(v, &["user", "uuid"]) == Some(me) {
            continue;
        }
        let name = str_at(v, &["user", "display_name"])?;
        let at = parse_iso_date(str_at(v, &["created_on"])?)?;
        return Some(PRComment {
            name: name.to_string(),
            at,
        });
    }
    None
}

pub fn events(old: &PRStatus, new: &PRStatus) -> Vec<PREvent> {
    let mut out = Vec::new();
    if let Some(c) = &new.last_other
        && old.last_other.as_ref().is_none_or(|o| c.at > o.at)
    {
        out.push(PREvent::Comment);
    }
    if new.approved > old.approved {
        out.push(PREvent::Approved);
    }
    if new.changes_requested > old.changes_requested {
        out.push(PREvent::ChangesRequested);
    }
    if old.is_open() && new.state == "MERGED" {
        out.push(PREvent::Merged);
    }
    if old.is_open() && (new.state == "DECLINED" || new.state == "SUPERSEDED") {
        out.push(PREvent::Declined);
    }
    out
}

pub fn chime(events: &[PREvent]) -> Option<Chime> {
    use PREvent::*;
    if events
        .iter()
        .any(|e| matches!(e, Comment | ChangesRequested | Declined | RabbitNeedsYou))
    {
        return Some(Chime::NeedsYou);
    }
    if events
        .iter()
        .any(|e| matches!(e, Approved | Merged | RabbitDone))
    {
        return Some(Chime::Done);
    }
    None
}

/// The prompt typed into a session for a PR: `{url}` and `{id}` filled in, newlines turned into spaces.
pub fn prompt(template: &str, url: &Url, id: i64) -> String {
    let t = if template.trim().is_empty() {
        DEFAULT_PROMPT
    } else {
        template
    };
    t.replace("{url}", url.as_str())
        .replace("{id}", &id.to_string())
        .replace("\r\n", " ")
        .replace('\n', " ")
}
