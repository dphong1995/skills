//! CodeRabbit's state on a pull request, derived from Bitbucket's build statuses and comments. `derive` is pure.

use crate::credentials::is_newline;
use crate::pull_requests::RabbitThreads;
use crate::shell::parse_iso_date;
use crate::time::secs_since;
use regex::{Regex, RegexBuilder};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rabbit {
    Reviewing,
    Reviewed(Option<i64>),
    Approved,
    Skipped,
    Failed,
    AwaitingApproval,
    Nudged,
    ApprovalFailed(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RabbitFacts {
    /// uuid of the author of the newest marked comment.
    pub rabbit_user: Option<String>,
    /// CodeRabbit inline threads (a live top-level inline comment and its descendants), and those not answered:
    /// no human reply, or the newest comment is not CodeRabbit's.
    pub threads: RabbitThreads,
    /// Inline threads nobody but CodeRabbit wrote in and CodeRabbit has not marked addressed.
    pub unreplied: i64,
    /// The newest approve command is newer than every top-level CodeRabbit comment.
    pub nudge_pending: bool,
    /// When the newest approve command was posted.
    pub command: Option<SystemTime>,
    /// When CodeRabbit last posted a top-level comment saying the approval failed, and its line saying so.
    pub approval_failed_at: Option<SystemTime>,
    pub approval_failed_reason: Option<String>,
    /// A reviewer (not `me`, not the PR author, not the CodeRabbit account) has a comment `me` has not answered in its thread.
    pub others_pending: bool,
}

impl RabbitFacts {
    /// Every top-level inline CodeRabbit finding has been answered.
    pub fn all_answered(&self) -> bool {
        self.threads.pending == 0
    }
}

/// The raw data `derive` reads; `statuses` and `comments` are `None` until fetched.
#[derive(Clone, Debug, Default)]
pub struct RabbitInputs {
    pub commit: Option<String>,
    pub statuses: Option<Vec<Value>>,
    pub comments: Option<Vec<Value>>,
    /// The newest summary comment seen for `commit`, kept for when `comments` no longer holds it.
    pub summary: Option<Value>,
    pub approvers: HashSet<String>,
    pub me: Option<String>,
    pub author: Option<String>,
    /// When the widget itself posted the approve command; the 600 s timeout counts from the later of that and the command's
    /// own time, so a clock that runs ahead of Bitbucket's cannot end it early. Commands first seen in a fetch get no floor.
    pub nudged_at: Option<SystemTime>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RabbitView {
    /// The displayed state.
    pub rabbit: Option<Rabbit>,
    /// The state from the statuses and summary alone.
    pub status: Option<Rabbit>,
    pub since: Option<SystemTime>,
    pub threads: Option<RabbitThreads>,
    pub others_pending: bool,
}

pub const MARKER: &str = "coderabbit.ai/#html-comment";
const NUDGE_TIMEOUT: f64 = 600.0;

/// Swift's `Date.distantPast` (0001-01-01).
fn distant_past() -> SystemTime {
    UNIX_EPOCH - Duration::from_secs(62_135_596_800)
}

fn ci(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .unwrap()
}

static ACTIONABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Actionable comments posted: (\d+)").unwrap());
static MENTION: LazyLock<Regex> = LazyLock::new(|| ci(r"@coderabbit(ai)?\b"));
static NUDGE_COMMAND: LazyLock<Regex> = LazyLock::new(|| ci(r"@coderabbit(ai)?\s+approve"));
static APPROVAL_SKIPPED: LazyLock<Regex> = LazyLock::new(|| ci("Approval skipped: "));
static APPROVAL_FAILED: LazyLock<Regex> = LazyLock::new(|| ci("approval failed"));

/// The first maximum, like Swift's `max(by:)` (Rust's `max_by` keeps the last of equal elements).
fn max_first<T>(items: impl IntoIterator<Item = T>, less: impl Fn(&T, &T) -> bool) -> Option<T> {
    let mut best: Option<T> = None;
    for e in items {
        best = match best {
            Some(b) if !less(&b, &e) => Some(b),
            _ => Some(e),
        };
    }
    best
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

fn is_status(v: &Value) -> bool {
    str_of(v, "key").is_some_and(|k| k.starts_with("coderabbitai/"))
        || str_of(v, "name") == Some("CodeRabbit")
}

fn raw(v: &Value) -> &str {
    v.get("content")
        .and_then(|c| str_of(c, "raw"))
        .unwrap_or("")
}

fn stamp(v: &Value, key: &str) -> Option<SystemTime> {
    str_of(v, key).and_then(parse_iso_date)
}

fn deleted(v: &Value) -> bool {
    v.get("deleted").and_then(Value::as_bool) == Some(true)
}

pub fn is_comment(v: &Value) -> bool {
    raw(v).contains(MARKER)
}

/// A status without `commit.hash`, or a `None` head commit, counts as on the head commit.
fn on_commit(v: &Value, commit: Option<&str>) -> bool {
    let hash = v.get("commit").and_then(|c| str_of(c, "hash"));
    match (commit, hash) {
        (Some(commit), Some(hash)) => hash.starts_with(commit),
        _ => true,
    }
}

/// A non-CodeRabbit build status of `commit` is in `state`.
fn pipeline_in(values: &[Value], commit: Option<&str>, state: &str) -> bool {
    values
        .iter()
        .any(|v| !is_status(v) && on_commit(v, commit) && str_of(v, "state") == Some(state))
}

/// A pipeline (non-CodeRabbit) status of `commit` is FAILED.
pub fn pipeline_failed(values: &[Value], commit: Option<&str>) -> bool {
    pipeline_in(values, commit, "FAILED")
}

/// A pipeline (non-CodeRabbit) status of `commit` is INPROGRESS.
pub fn pipeline_running(values: &[Value], commit: Option<&str>) -> bool {
    pipeline_in(values, commit, "INPROGRESS")
}

/// The newest CodeRabbit status of `commit`.
pub fn newest_status<'a>(values: &'a [Value], commit: Option<&str>) -> Option<&'a Value> {
    let matching = values
        .iter()
        .filter(|v| is_status(v) && on_commit(v, commit));
    max_first(matching, |a, b| {
        stamp(a, "updated_on").unwrap_or_else(distant_past)
            < stamp(b, "updated_on").unwrap_or_else(distant_past)
    })
}

pub fn state_from_values(values: &[Value], commit: Option<&str>) -> Option<(Rabbit, SystemTime)> {
    newest_status(values, commit).and_then(state_from_status)
}

/// The `findings` of the result stay `None` here; `findings_from_values` fills them in.
pub fn state_from_status(s: &Value) -> Option<(Rabbit, SystemTime)> {
    let state = str_of(s, "state")?;
    // CodeRabbit's INPROGRESS status lingers after its summary is posted, so its start is its last update.
    let since = (if state == "INPROGRESS" {
        stamp(s, "updated_on")
    } else {
        None
    })
    .or_else(|| stamp(s, "created_on"))
    .or_else(|| stamp(s, "updated_on"))
    .unwrap_or_else(distant_past);
    let description = str_of(s, "description").unwrap_or("");
    let text = description.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| text.contains(w));
    let rabbit = match state {
        "INPROGRESS" => Rabbit::Reviewing,
        "FAILED" => Rabbit::Failed,
        "STOPPED" => Rabbit::Skipped,
        "SUCCESSFUL" => {
            if text.starts_with("approve command performed") {
                match APPROVAL_SKIPPED.find(description) {
                    Some(m) if !text.contains("already approved") => {
                        Rabbit::ApprovalFailed(description[m.end()..].trim().to_string())
                    }
                    _ if has(&["approval completed", "already approved"]) => Rabbit::Approved,
                    _ => Rabbit::Reviewed(None),
                }
            } else if has(&[
                "failed",
                "could not",
                "incomplete",
                "not published",
                "error",
            ]) {
                Rabbit::Failed
            } else if has(&[
                "skipped",
                "paused",
                "no files",
                "too many files",
                "rate limited",
                "no reviewable",
            ]) {
                Rabbit::Skipped
            } else if text.contains("approv") {
                Rabbit::Approved
            } else {
                Rabbit::Reviewed(None)
            }
        }
        _ => return None,
    };
    Some((rabbit, since))
}

/// The finding count of a summary comment, `None` for any other comment.
fn summary_count(v: &Value) -> Option<i64> {
    if deleted(v) || !is_comment(v) {
        return None;
    }
    let raw = raw(v);
    if let Some(n) = ACTIONABLE
        .captures(raw)
        .and_then(|m| m[1].parse::<i64>().ok())
    {
        return Some(n);
    }
    raw.contains("No actionable comments").then_some(0)
}

/// The first (newest, in `sort=-created_on` order) summary comment of `values`.
pub fn newest_summary(values: &[Value]) -> Option<&Value> {
    values.iter().find(|v| summary_count(v).is_some())
}

/// Count from the newest summary comment (marked, with a count line); `None` when there is none or it predates `since`.
/// `created_only` ignores `updated_on`, which CodeRabbit bumps when it edits an old summary.
pub fn findings_from_values(
    values: &[Value],
    since: SystemTime,
    created_only: bool,
) -> Option<i64> {
    let v = newest_summary(values)?;
    let count = summary_count(v)?;
    let keys: &[&str] = if created_only {
        &["created_on"]
    } else {
        &["created_on", "updated_on"]
    };
    keys.iter()
        .filter_map(|k| stamp(v, k))
        .any(|d| d >= since)
        .then_some(count)
}

/// Pure: the same inputs always give the same view.
pub fn derive(i: &RabbitInputs, now: SystemTime) -> RabbitView {
    let mut facts = i
        .comments
        .as_ref()
        .map(|c| facts(c, i.me.as_deref(), i.author.as_deref()));
    let mut status: Option<Rabbit> = None;
    let mut since = None;
    let mut status_updated: Option<SystemTime> = None;
    let mut status_description = String::new();
    if let Some(values) = &i.statuses
        && let Some(newest) = newest_status(values, i.commit.as_deref())
        && let Some(fresh) = state_from_status(newest)
    {
        since = Some(fresh.1);
        status_updated = stamp(newest, "updated_on");
        status_description = str_of(newest, "description").unwrap_or("").to_lowercase();
        let pool: Vec<Value> = i
            .comments
            .iter()
            .flatten()
            .chain(i.summary.iter())
            .cloned()
            .collect();
        let mut r = fresh.0;
        // An approve command newer than the failed approval starts over: the answer to it is still to come.
        if matches!(r, Rabbit::ApprovalFailed(_))
            && let Some(c) = facts.as_ref().and_then(|f| f.command)
            && let Some(u) = status_updated
            && c > u
        {
            r = Rabbit::Reviewed(None);
        }
        if r == Rabbit::Reviewing {
            // The summary is out; the status lags behind it.
            if let Some(n) = findings_from_values(&pool, fresh.1, true) {
                r = Rabbit::Reviewed(Some(n));
            }
        } else if matches!(r, Rabbit::Reviewed(_))
            && let Some(n) = findings_from_values(&pool, fresh.1, false)
        {
            r = Rabbit::Reviewed(Some(n));
        }
        status = Some(r);
    }
    // The newest approve command ends once CodeRabbit answered in a comment (kept by `facts`), updated its status, or 600 s passed.
    if let Some(f) = facts.as_mut()
        && let Some(command) = f.command
        && f.nudge_pending
    {
        let floor = i.nudged_at.map_or(command, |n| command.max(n));
        if status_updated.is_some_and(|u| u > command) || secs_since(now, floor) >= NUDGE_TIMEOUT {
            f.nudge_pending = false;
        }
    }
    let mut shown = resolve(status.as_ref(), &i.approvers, facts.as_ref());
    // The failure CodeRabbit wrote in a comment after the newest command stays until a new review (a status that is not
    // an approve command) or a newer command replaces it.
    if matches!(status, Some(Rabbit::Reviewed(_) | Rabbit::Skipped))
        && let Some(f) = &facts
        && let Some(command) = f.command
        && let Some(failed) = f.approval_failed_at
        && failed > command
        && (!status_updated.is_some_and(|u| u > failed)
            || status_description.starts_with("approve command performed"))
        && matches!(shown, Some(Rabbit::Reviewed(_) | Rabbit::AwaitingApproval))
    {
        shown = Some(Rabbit::ApprovalFailed(
            f.approval_failed_reason
                .clone()
                .unwrap_or_else(|| "approval failed".into()),
        ));
    }
    RabbitView {
        rabbit: shown,
        status,
        since,
        threads: facts.as_ref().map(|f| f.threads),
        others_pending: facts.as_ref().is_some_and(|f| f.others_pending),
    }
}

pub fn label(r: &Rabbit, threads: Option<RabbitThreads>) -> String {
    if let (Rabbit::Reviewed(_), Some(t)) = (r, threads)
        && t.total > 0
        && t.pending > 0
    {
        return format!("còn {}/{}", t.pending, t.total);
    }
    match r {
        Rabbit::Reviewing => "đang review".into(),
        Rabbit::Reviewed(Some(n)) if *n > 0 => format!("{n} góp ý"),
        Rabbit::Reviewed(_) => "xong".into(),
        Rabbit::Approved => "đã duyệt".into(),
        Rabbit::Skipped => "bỏ qua".into(),
        Rabbit::Failed => "lỗi".into(),
        Rabbit::AwaitingApproval => "chờ duyệt".into(),
        Rabbit::Nudged => "đang nhắc".into(),
        Rabbit::ApprovalFailed(_) => "duyệt lỗi".into(),
    }
}

/// Which buttons an open PR item shows: `(Nhờ Claude, Nhắc duyệt)`. The result of a send whose button is gone is shown
/// by the view, not here.
pub fn pr_buttons(pr: &crate::pull_requests::PRStatus) -> (bool, bool) {
    let open = pr.is_open();
    (
        open && (pr.pipeline_failed
            || !hides_ask_claude(
                pr.rabbit.as_ref(),
                pr.rabbit_threads,
                pr.rabbit_others_pending,
            )),
        open && pr.rabbit == Some(Rabbit::AwaitingApproval),
    )
}

/// `Nhờ Claude` has nothing to ask while CodeRabbit is done and every thread is answered.
pub fn hides_ask_claude(
    r: Option<&Rabbit>,
    threads: Option<RabbitThreads>,
    others_pending: bool,
) -> bool {
    let Some(threads) = threads else {
        return false;
    };
    if threads.pending != 0 || others_pending {
        return false;
    }
    matches!(
        r,
        Some(
            Rabbit::Reviewed(_)
                | Rabbit::AwaitingApproval
                | Rabbit::Nudged
                | Rabbit::Approved
                | Rabbit::ApprovalFailed(_)
        )
    )
}

pub fn is_final(r: &Rabbit) -> bool {
    matches!(
        r,
        Rabbit::Reviewed(_)
            | Rabbit::Approved
            | Rabbit::Failed
            | Rabbit::AwaitingApproval
            | Rabbit::ApprovalFailed(_)
    )
}

/// What the full comment list says about the CodeRabbit account, its inline threads and a pending approve command.
pub fn facts(values: &[Value], me: Option<&str>, author: Option<&str>) -> RabbitFacts {
    let live: Vec<&Value> = values.iter().filter(|v| !deleted(v)).collect();
    let created = |v: &Value| stamp(v, "created_on").unwrap_or_else(distant_past);
    let parent = |v: &Value| {
        v.get("parent")
            .and_then(|p| p.get("id"))
            .and_then(Value::as_i64)
    };
    let uuid = |v: &Value| {
        v.get("user")
            .and_then(|u| str_of(u, "uuid"))
            .map(str::to_string)
    };
    let id = |v: &Value| v.get("id").and_then(Value::as_i64);
    let is_top_level = |v: &Value| parent(v).is_none() && v.get("inline").is_none();
    let marked: Vec<&Value> = live.iter().copied().filter(|v| is_comment(v)).collect();
    let rabbit_user =
        max_first(marked.iter().copied(), |a, b| created(a) < created(b)).and_then(uuid);

    // Each comment's thread root, found once; comments without an id stand alone.
    let mut parent_of: HashMap<i64, i64> = HashMap::new();
    for v in &live {
        if let (Some(i), Some(p)) = (id(v), parent(v)) {
            parent_of.insert(i, p);
        }
    }
    let mut threads: HashMap<i64, Vec<&Value>> = HashMap::new();
    let mut alone: Vec<&Value> = Vec::new();
    for v in &live {
        let Some(mut i) = id(v) else {
            alone.push(v);
            continue;
        };
        // A cycle of parents cannot reach a root; the step bound keeps it from looping.
        for _ in 0..parent_of.len() {
            match parent_of.get(&i) {
                Some(p) => i = *p,
                None => break,
            }
        }
        threads.entry(i).or_default().push(v);
    }

    let roots: Vec<&Value> = marked
        .iter()
        .copied()
        .filter(|v| v.get("inline").is_some() && parent(v).is_none())
        .collect();
    // Per open root (CodeRabbit rewrites a root it has dealt with to `✅ Addressed in commit …`): whether anyone else
    // replied, and whether the newest comment is someone else's.
    let open: Vec<(bool, bool)> = roots
        .iter()
        .filter(|root| !raw(root).contains("Addressed in commit"))
        .map(|root| {
            let thread: Vec<&Value> = id(root)
                .and_then(|i| threads.get(&i).cloned())
                .unwrap_or_else(|| vec![root]);
            let replied = thread.iter().any(|v| !is_comment(v));
            let newest_other = max_first(thread.iter().copied(), |a, b| created(a) < created(b))
                .is_none_or(|newest| !is_comment(newest));
            (replied, newest_other)
        })
        .collect();
    let pending = open
        .iter()
        .filter(|(replied, newest_other)| !replied || *newest_other)
        .count() as i64;
    let unreplied = open.iter().filter(|(replied, _)| !replied).count() as i64;

    let command = live
        .iter()
        .filter(|v| !is_comment(v) && NUDGE_COMMAND.is_match(raw(v)))
        .map(|v| created(v))
        .max();
    let top_level = marked
        .iter()
        .filter(|v| is_top_level(v))
        .map(|v| created(v))
        .max();
    let nudge_pending = command.is_some_and(|c| top_level.is_none_or(|t| c > t));
    let failed = max_first(
        marked
            .iter()
            .copied()
            .filter(|v| is_top_level(v) && APPROVAL_FAILED.is_match(raw(v))),
        |a, b| created(a) < created(b),
    );
    let approval_failed_reason = failed.and_then(|v| {
        raw(v)
            .split(is_newline)
            .find(|l| APPROVAL_FAILED.is_match(l))
            .map(|l| l.trim().to_string())
    });

    // Reviewer comments `me` has not answered: nothing from `me` after them in their thread.
    let mut others_pending = false;
    if let Some(me) = me {
        let unanswered = |thread: &[&Value], answered: &dyn Fn(SystemTime) -> bool| {
            thread.iter().any(|v| {
                let Some(user) = uuid(v) else {
                    return false;
                };
                if user == me
                    || Some(user.as_str()) == author
                    || Some(&user) == rabbit_user.as_ref()
                    || is_comment(v)
                    || MENTION.is_match(raw(v))
                {
                    return false;
                }
                !answered(created(v))
            })
        };
        others_pending = unanswered(&alone, &|_| false)
            || threads.values().any(|thread| {
                let latest_me = thread
                    .iter()
                    .filter(|v| uuid(v).as_deref() == Some(me))
                    .map(|v| created(v))
                    .max();
                unanswered(thread, &|at| latest_me.is_some_and(|m| m > at))
            });
    }
    RabbitFacts {
        rabbit_user,
        threads: RabbitThreads {
            pending,
            total: roots.len() as i64,
        },
        unreplied,
        nudge_pending,
        command,
        approval_failed_at: failed.map(created),
        approval_failed_reason,
        others_pending,
    }
}

/// The displayed state from the raw status, the PR's approvers and the comment facts.
pub fn resolve(
    status: Option<&Rabbit>,
    approvers: &HashSet<String>,
    facts: Option<&RabbitFacts>,
) -> Option<Rabbit> {
    if let Some(u) = facts.and_then(|f| f.rabbit_user.as_ref())
        && approvers.contains(u)
    {
        return Some(Rabbit::Approved);
    }
    if status == Some(&Rabbit::Reviewing) {
        return Some(Rabbit::Reviewing);
    }
    match status {
        // CodeRabbit can skip the review of the commit that fixes its findings and then never answer the replies to them;
        // with every finding replied to, the PR waits for approval as if it had.
        Some(Rabbit::Skipped)
            if facts.is_some_and(|f| {
                f.rabbit_user.is_some() && f.threads.total > 0 && f.unreplied == 0
            }) =>
        {
            Some(if facts.is_some_and(|f| f.nudge_pending) {
                Rabbit::Nudged
            } else {
                Rabbit::AwaitingApproval
            })
        }
        Some(Rabbit::Failed | Rabbit::Skipped | Rabbit::ApprovalFailed(_)) => status.cloned(),
        Some(Rabbit::Reviewed(_) | Rabbit::Approved) => {
            let Some(facts) = facts.filter(|f| f.rabbit_user.is_some()) else {
                return status.cloned();
            };
            if facts.all_answered() {
                return Some(if facts.nudge_pending {
                    Rabbit::Nudged
                } else {
                    Rabbit::AwaitingApproval
                });
            }
            if status == Some(&Rabbit::Approved) {
                return Some(Rabbit::Reviewed(None));
            }
            status.cloned()
        }
        _ => status.cloned(),
    }
}

/// `r` without its findings count, to tell a new outcome from a recount.
pub fn kind(r: Option<&Rabbit>) -> Option<Rabbit> {
    match r {
        Some(Rabbit::Reviewed(_)) => Some(Rabbit::Reviewed(None)),
        other => other.cloned(),
    }
}
