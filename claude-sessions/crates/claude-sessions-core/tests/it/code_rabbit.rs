use crate::support::*;
use claude_sessions_core::code_rabbit::{
    self as cr, Rabbit, RabbitFacts, RabbitInputs, RabbitView,
};
use claude_sessions_core::pull_requests::RabbitThreads;
use claude_sessions_core::shell::format_iso_date;
use claude_sessions_core::time::{add_secs, from_unix_secs};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::time::SystemTime;

fn rs(state: &str, description: &str) -> Value {
    rabbit_status(state, description, "abc123", RABBIT_CREATED, None)
}

/// A SUCCESSFUL status of commit `abc123`.
fn done(description: &str, created: &str, updated: Option<&str>) -> Value {
    rabbit_status("SUCCESSFUL", description, "abc123", created, updated)
}

fn rabbit(state: &str, description: &str) -> Option<Rabbit> {
    cr::state_from_values(&[rs(state, description)], Some("abc123")).map(|s| s.0)
}

fn threads(pending: i64, total: i64) -> RabbitThreads {
    RabbitThreads { pending, total }
}

fn reviewed(n: Option<i64>) -> Rabbit {
    Rabbit::Reviewed(n)
}

fn set(v: &[&str]) -> HashSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn state_table() {
    assert_eq!(
        rabbit("INPROGRESS", "Review in progress"),
        Some(Rabbit::Reviewing)
    );
    assert_eq!(rabbit("FAILED", "Review failed"), Some(Rabbit::Failed));
    assert_eq!(rabbit("SUCCESSFUL", "Review failed"), Some(Rabbit::Failed));
    assert_eq!(
        rabbit("SUCCESSFUL", "Review skipped"),
        Some(Rabbit::Skipped)
    );
    assert_eq!(
        rabbit("SUCCESSFUL", "Review rate limited"),
        Some(Rabbit::Skipped)
    );
    assert_eq!(
        rabbit("SUCCESSFUL", "No reviewable changes"),
        Some(Rabbit::Skipped)
    );
    assert_eq!(rabbit("STOPPED", "Stopped"), Some(Rabbit::Skipped));
    assert_eq!(
        rabbit(
            "SUCCESSFUL",
            "Approve command performed: Comments resolved. Approval completed"
        ),
        Some(Rabbit::Approved)
    );
    assert_eq!(
        rabbit(
            "SUCCESSFUL",
            "Approve command performed: Approval skipped: already approved"
        ),
        Some(Rabbit::Approved)
    );
    assert_eq!(
        rabbit(
            "SUCCESSFUL",
            "Approve command performed: Approval skipped: durable Cloud recovery is unavailable"
        ),
        Some(Rabbit::ApprovalFailed(
            "durable Cloud recovery is unavailable".into()
        ))
    );
    assert_eq!(
        rabbit("SUCCESSFUL", "Review completed"),
        Some(reviewed(None))
    );

    let other = json!({"key": "pipelines", "name": "Pipeline", "state": "SUCCESSFUL", "commit": {"hash": "abc123"}});
    assert!(cr::state_from_values(&[other], Some("abc123")).is_none());
    let old = rabbit_status(
        "SUCCESSFUL",
        "Review completed",
        "old999",
        RABBIT_CREATED,
        None,
    );
    assert!(cr::state_from_values(&[old], Some("abc123")).is_none());
}

#[test]
fn newest_status_wins() {
    let older = rabbit_status(
        "INPROGRESS",
        "Review in progress",
        "abc123",
        "2026-11-03T03:00:00.000000+00:00",
        None,
    );
    let newer = done("Review completed", "2026-11-03T04:00:00.000000+00:00", None);
    let s = cr::state_from_values(&[older, newer], Some("abc123")).unwrap();
    assert_eq!(s.0, reviewed(None));
    assert_eq!(s.1, from_unix_secs(1_793_678_400.0));
}

#[test]
fn in_progress_since_is_updated_on() {
    let s = rabbit_status(
        "INPROGRESS",
        "Review in progress",
        "abc123",
        "2026-11-03T03:00:00.000000+00:00",
        Some("2026-11-03T04:00:00.000000+00:00"),
    );
    assert_eq!(
        cr::state_from_values(&[s], Some("abc123")).unwrap().1,
        from_unix_secs(1_793_678_400.0)
    );
}

#[test]
fn created_only_ignores_edited_summary() {
    let since = from_unix_secs(1_793_678_400.0);
    let mut edited = rabbit_comment(
        "**Actionable comments posted: 2**",
        "2026-11-03T03:00:00.000000+00:00",
    );
    edited["updated_on"] = json!("2026-11-03T04:05:00.000000+00:00");
    assert_eq!(
        cr::findings_from_values(&[edited.clone()], since, false),
        Some(2)
    );
    assert_eq!(cr::findings_from_values(&[edited], since, true), None);
    let fresh = rabbit_comment(
        "**Actionable comments posted: 2**",
        "2026-11-03T04:05:00.000000+00:00",
    );
    assert_eq!(cr::findings_from_values(&[fresh], since, true), Some(2));
}

#[test]
fn findings_from_summary() {
    let since = from_unix_secs(1_793_678_400.0); // 2026-11-03T04:00:00Z
    let summary = rabbit_comment(
        "**Actionable comments posted: 2**",
        "2026-11-03T04:05:00.000000+00:00",
    );
    let inline = rabbit_comment("nit: rename this", "2026-11-03T04:06:00.000000+00:00");
    assert_eq!(
        cr::findings_from_values(&[inline.clone(), summary], since, false),
        Some(2)
    );

    let none = rabbit_comment(
        "No actionable comments were generated in the recent review.",
        "2026-11-03T04:05:00.000000+00:00",
    );
    assert_eq!(cr::findings_from_values(&[none], since, false), Some(0));

    let stale = rabbit_comment(
        "**Actionable comments posted: 2**",
        "2026-11-03T03:00:00.000000+00:00",
    );
    assert_eq!(cr::findings_from_values(&[stale], since, false), None);
    assert_eq!(cr::findings_from_values(&[], since, false), None);
    assert_eq!(cr::findings_from_values(&[inline], since, false), None);
}

#[test]
fn labels() {
    let l = |r: Rabbit| cr::label(&r, None);
    assert_eq!(l(reviewed(Some(2))), "2 góp ý");
    assert_eq!(l(reviewed(Some(0))), "xong");
    assert_eq!(l(reviewed(None)), "xong");
    assert_eq!(l(Rabbit::Reviewing), "đang review");
    assert_eq!(l(Rabbit::Approved), "đã duyệt");
    assert_eq!(l(Rabbit::Skipped), "bỏ qua");
    assert_eq!(l(Rabbit::Failed), "lỗi");
    assert_eq!(l(Rabbit::AwaitingApproval), "chờ duyệt");
    assert_eq!(l(Rabbit::Nudged), "đang nhắc");
    assert!(cr::is_final(&Rabbit::AwaitingApproval) && !cr::is_final(&Rabbit::Nudged));
    assert!(
        cr::is_final(&reviewed(None))
            && cr::is_final(&Rabbit::Approved)
            && cr::is_final(&Rabbit::Failed)
    );
    assert!(!cr::is_final(&Rabbit::Reviewing) && !cr::is_final(&Rabbit::Skipped));
}

const T1: &str = "2026-11-04T10:00:00.000000+00:00";
const T2: &str = "2026-11-04T10:01:00.000000+00:00";
const T3: &str = "2026-11-04T10:02:00.000000+00:00";
const T4: &str = "2026-11-04T10:03:00.000000+00:00";

fn answered() -> Vec<Value> {
    vec![
        rabbit_inline(1, "nit", T1, None),
        human_reply(2, Some(1), "{me}", "won't fix", T2, true),
        rabbit_inline(3, "ok, withdrawn", T3, Some(1)),
    ]
}

fn command(raw: &str, at: &str) -> Value {
    human_reply(10, None, "{me}", raw, at, false)
}

fn approve(at: &str) -> Value {
    command("@coderabbitai approve", at)
}

fn plus(mut v: Vec<Value>, more: Vec<Value>) -> Vec<Value> {
    v.extend(more);
    v
}

#[test]
fn facts_withdrawn_thread_is_answered() {
    let f = cr::facts(&answered(), None, None);
    assert!(f.all_answered() && f.rabbit_user.as_deref() == Some("{tin}") && !f.nudge_pending);
}

#[test]
fn facts_unanswered_thread() {
    assert!(!cr::facts(&[rabbit_inline(1, "nit", T1, None)], None, None).all_answered());
    let f = cr::facts(
        &[
            rabbit_inline(1, "nit", T1, None),
            human_reply(2, Some(1), "{me}", "why?", T2, true),
        ],
        None,
        None,
    );
    assert!(!f.all_answered());
}

#[test]
fn facts_no_findings() {
    let f = cr::facts(
        &[rabbit_comment("**Actionable comments posted: 0**", T1)],
        None,
        None,
    );
    assert!(f.all_answered() && f.rabbit_user.as_deref() == Some("{tin}"));
}

#[test]
fn facts_nudge_pending() {
    let base = plus(answered(), vec![rabbit_comment("summary", T1)]);
    let pending = |extra: Option<Value>| {
        cr::facts(&plus(base.clone(), extra.into_iter().collect()), None, None).nudge_pending
    };
    assert!(pending(Some(approve(T4))));
    assert!(pending(Some(command("@CodeRabbit  Approve", T4))));
    assert!(!pending(Some(approve(T1))));
    assert!(!pending(None));
}

#[test]
fn nudge_survives_inline_reply() {
    let reply = rabbit_inline(4, "still fine", "2026-11-04T10:04:00.000000+00:00", Some(1));
    let f = cr::facts(
        &plus(
            answered(),
            vec![rabbit_comment("summary", T1), approve(T4), reply],
        ),
        None,
        None,
    );
    assert!(f.nudge_pending && f.all_answered());
}

#[test]
fn nudge_ends_on_action_performed() {
    let done = rabbit_comment("Action performed", "2026-11-04T10:05:00.000000+00:00");
    assert!(!cr::facts(&plus(answered(), vec![approve(T4), done]), None, None).nudge_pending);
}

#[test]
fn facts_ignores_deleted() {
    let mut gone = rabbit_inline(9, "nit", T1, None);
    gone["deleted"] = json!(true);
    assert!(cr::facts(&plus(answered(), vec![gone]), None, None).all_answered());
}

#[test]
fn facts_counts_threads() {
    let f = cr::facts(
        &[
            rabbit_inline(1, "a", T1, None),
            rabbit_inline(4, "b", T1, None),
            human_reply(5, Some(4), "{me}", "ok", T2, true),
            rabbit_inline(6, "done", T3, Some(4)),
            rabbit_inline(7, "c", T1, None),
            human_reply(8, Some(7), "{me}", "why", T3, true),
        ],
        None,
        None,
    );
    assert!(f.threads == threads(2, 3) && !f.all_answered());
}

#[test]
fn facts_deleted_root_not_counted() {
    let mut gone = rabbit_inline(9, "nit", T1, None);
    gone["deleted"] = json!(true);
    let f = cr::facts(&plus(answered(), vec![gone]), None, None);
    assert_eq!(f.threads, threads(0, 1));
}

#[test]
fn label_remaining() {
    let five = reviewed(Some(5));
    assert_eq!(cr::label(&five, Some(threads(2, 5))), "còn 2/5");
    assert_eq!(cr::label(&five, Some(threads(0, 0))), "5 góp ý");
    assert_eq!(cr::label(&five, None), "5 góp ý");
}

#[test]
fn hides_ask_claude_table() {
    let (none, empty, some) = (threads(0, 3), threads(0, 0), threads(2, 5));
    let hides =
        |r: Option<Rabbit>, t: Option<RabbitThreads>| cr::hides_ask_claude(r.as_ref(), t, false);
    assert!(hides(Some(Rabbit::AwaitingApproval), Some(none)));
    assert!(hides(Some(Rabbit::Nudged), Some(none)));
    assert!(hides(Some(Rabbit::Approved), Some(none)));
    assert!(hides(Some(reviewed(Some(0))), Some(empty)));
    assert!(!hides(Some(reviewed(Some(5))), Some(some)));
    assert!(!hides(Some(Rabbit::Reviewing), Some(none)));
    assert!(!hides(Some(Rabbit::AwaitingApproval), None));
    assert!(!hides(Some(Rabbit::Failed), Some(none)));
    assert!(!hides(Some(Rabbit::Skipped), Some(none)));
    assert!(!hides(None, Some(none)));
}

#[test]
fn pipeline_failed_head_commit_only() {
    let st = |key: &str, state: &str, hash: &str| json!({"key": key, "name": key, "state": state, "commit": {"hash": hash}});
    let head = Some("abc123");
    // (key, state, hash, want_failed, want_running)
    let rows = [
        ("pipe", "FAILED", "abc123def", true, false),
        ("pipe", "FAILED", "zzz999", false, false),
        ("pipe", "SUCCESSFUL", "abc123", false, false),
        ("pipe", "INPROGRESS", "abc123", false, true),
        ("pipe", "INPROGRESS", "zzz999", false, false),
        ("coderabbitai/1", "FAILED", "abc123", false, false),
        ("coderabbitai/1", "INPROGRESS", "abc123", false, false),
    ];
    for (key, state, hash, want_failed, want_running) in rows {
        let values = [st(key, state, hash)];
        assert_eq!(
            cr::pipeline_failed(&values, head),
            want_failed,
            "{key} {state} {hash}"
        );
        assert_eq!(
            cr::pipeline_running(&values, head),
            want_running,
            "{key} {state} {hash}"
        );
    }
    // No `commit` on the status, or no head commit: counts as on the head commit.
    let no_commit = json!({"key": "pipe", "name": "pipe", "state": "FAILED"});
    assert!(cr::pipeline_failed(&[no_commit], head));
    assert!(cr::pipeline_failed(&[st("pipe", "FAILED", "zzz999")], None));
}

#[test]
fn pr_buttons_table() {
    use claude_sessions_core::pull_requests::{PRRef, PRStatus};
    let pr = |id: i64, state: &str| PRStatus {
        ref_: PRRef {
            workspace: "w".into(),
            repo: "r".into(),
            id,
        },
        url: format!("https://bitbucket.org/w/r/pull-requests/{id}")
            .parse()
            .unwrap(),
        state: state.into(),
        reviewers: 5,
        approved: 0,
        changes_requested: 1,
        comments: 23,
        commit: None,
        source_repo: None,
        branch: None,
        title: None,
        last_other: None,
        approvers: Default::default(),
        author: None,
        rabbit: None,
        rabbit_status: None,
        rabbit_since: None,
        rabbit_threads: None,
        rabbit_others_pending: false,
        pipeline_failed: false,
    };
    let open = |r: Option<Rabbit>, t: Option<RabbitThreads>| {
        let mut p = pr(1, "OPEN");
        p.rabbit = r;
        p.rabbit_threads = t;
        p
    };
    let failing = |mut p: PRStatus| {
        p.pipeline_failed = true;
        p
    };
    let rows: Vec<(PRStatus, (bool, bool))> = vec![
        (open(None, None), (true, false)),
        (open(Some(Rabbit::AwaitingApproval), None), (true, true)),
        (
            open(Some(Rabbit::AwaitingApproval), Some(threads(0, 1))),
            (false, true),
        ),
        (pr(2, "MERGED"), (false, false)),
        (pr(3, "CLOSED"), (false, false)),
        (
            open(Some(Rabbit::Approved), Some(threads(0, 3))),
            (false, false),
        ),
        (
            failing(open(Some(Rabbit::Approved), Some(threads(0, 3)))),
            (true, false),
        ),
        (failing(pr(2, "MERGED")), (false, false)),
    ];
    for (p, want) in rows {
        assert_eq!(cr::pr_buttons(&p), want, "{:?} {:?}", p.state, p.rabbit);
    }
}

#[test]
fn resolve_order() {
    let known = RabbitFacts {
        rabbit_user: Some("{tin}".into()),
        threads: threads(1, 1),
        nudge_pending: false,
        ..Default::default()
    };
    let mut answered_facts = known.clone();
    answered_facts.threads = threads(0, 1);
    let mut nudged_facts = answered_facts.clone();
    nudged_facts.nudge_pending = true;
    let tin = set(&["{tin}"]);
    let none = HashSet::new();
    let res = |s: Option<Rabbit>, a: &HashSet<String>, f: Option<&RabbitFacts>| {
        cr::resolve(s.as_ref(), a, f)
    };
    assert_eq!(
        res(Some(Rabbit::Reviewing), &tin, Some(&known)),
        Some(Rabbit::Approved)
    );
    assert_eq!(
        res(Some(Rabbit::Reviewing), &none, Some(&known)),
        Some(Rabbit::Reviewing)
    );
    assert_eq!(
        res(Some(Rabbit::Failed), &tin, Some(&known)),
        Some(Rabbit::Approved)
    );
    assert_eq!(
        res(Some(Rabbit::Skipped), &tin, Some(&known)),
        Some(Rabbit::Approved)
    );
    assert_eq!(
        res(Some(Rabbit::Failed), &none, Some(&known)),
        Some(Rabbit::Failed)
    );
    assert_eq!(
        res(Some(Rabbit::Skipped), &none, Some(&answered_facts)),
        Some(Rabbit::AwaitingApproval)
    );
    let mut unreplied_facts = known.clone();
    unreplied_facts.unreplied = 1;
    assert_eq!(
        res(Some(Rabbit::Skipped), &none, Some(&unreplied_facts)),
        Some(Rabbit::Skipped)
    );
    let unknown = RabbitFacts {
        rabbit_user: None,
        threads: threads(0, 1),
        nudge_pending: false,
        ..Default::default()
    };
    assert_eq!(
        res(Some(reviewed(Some(2))), &none, Some(&unknown)),
        Some(reviewed(Some(2)))
    );
    assert_eq!(
        res(Some(Rabbit::Approved), &none, None),
        Some(Rabbit::Approved)
    );
    assert_eq!(
        res(Some(reviewed(Some(2))), &none, Some(&answered_facts)),
        Some(Rabbit::AwaitingApproval)
    );
    assert_eq!(
        res(Some(reviewed(Some(2))), &none, Some(&nudged_facts)),
        Some(Rabbit::Nudged)
    );
    assert_eq!(
        res(Some(reviewed(Some(2))), &none, Some(&known)),
        Some(reviewed(Some(2)))
    );
    assert_eq!(
        res(Some(Rabbit::Approved), &none, Some(&known)),
        Some(reviewed(None))
    );
    assert_eq!(res(None, &tin, Some(&known)), Some(Rabbit::Approved));
    assert_eq!(res(None, &none, Some(&known)), None);
}

const SINCE: &str = "2026-11-03T04:00:00.000000+00:00";

fn now() -> SystemTime {
    from_unix_secs(1_793_682_000.0)
}

fn inputs(status: Value, comments: Option<Vec<Value>>, summary: Option<Value>) -> RabbitInputs {
    RabbitInputs {
        commit: Some("abc123".into()),
        statuses: Some(vec![status]),
        comments,
        summary,
        approvers: HashSet::new(),
        me: None,
        author: None,
        nudged_at: None,
    }
}

#[test]
fn derive_reviewing_with_fresh_summary_is_reviewed() {
    let status = rabbit_status("INPROGRESS", "Review in progress", "abc123", SINCE, None);
    let summary = rabbit_comment(
        "**Actionable comments posted: 2**",
        "2026-11-03T04:01:00.000000+00:00",
    );
    let v = cr::derive(&inputs(status, None, Some(summary)), now());
    assert_eq!(v.rabbit, Some(reviewed(Some(2))));
    assert_eq!(v.status, Some(reviewed(Some(2))));
}

#[test]
fn derive_ignores_summary_before_since() {
    let status = rabbit_status("INPROGRESS", "Review in progress", "abc123", SINCE, None);
    let summary = rabbit_comment(
        "**Actionable comments posted: 2**",
        "2026-11-03T03:59:00.000000+00:00",
    );
    let v = cr::derive(&inputs(status, None, Some(summary)), now());
    assert_eq!(v.rabbit, Some(Rabbit::Reviewing));
}

#[test]
fn derive_uses_cached_summary_when_comments_lack_it() {
    let status = done("Review completed", SINCE, None);
    let summary = rabbit_comment(
        "**Actionable comments posted: 8**",
        "2026-11-03T04:01:00.000000+00:00",
    );
    let v = cr::derive(&inputs(status, Some(vec![]), Some(summary)), now());
    assert_eq!(v.rabbit, Some(reviewed(Some(8))));
}

#[test]
fn derive_without_comments_has_no_threads() {
    let status = done("Review completed", SINCE, None);
    let v = cr::derive(&inputs(status, None, None), now());
    assert_eq!(v.threads, None);
    assert_eq!(v.rabbit, Some(reviewed(None)));
}

#[test]
fn derive_is_pure() {
    let status = done("Review completed", SINCE, None);
    let summary = rabbit_comment(
        "**Actionable comments posted: 1**",
        "2026-11-03T04:01:00.000000+00:00",
    );
    let i = inputs(status, Some(vec![summary]), None);
    assert_eq!(cr::derive(&i, now()), cr::derive(&i, now()));
}

// Rules 7-10

fn tc() -> SystemTime {
    from_unix_secs(1_793_682_000.0) // 2026-11-03T05:00:00Z
}

fn at(seconds: f64) -> String {
    format_iso_date(add_secs(tc(), seconds))
}

fn derived(statuses: Vec<Value>, comments: Vec<Value>, now: f64) -> RabbitView {
    cr::derive(
        &RabbitInputs {
            commit: Some("abc123".into()),
            statuses: Some(statuses),
            comments: Some(comments),
            summary: None,
            approvers: HashSet::new(),
            me: None,
            author: None,
            nudged_at: None,
        },
        add_secs(tc(), now),
    )
}

fn reviewed_status() -> Value {
    done("Review completed", &at(-600.0), None)
}

fn approve_command() -> Value {
    human_reply(10, None, "{me}", "@coderabbitai approve", &at(0.0), false)
}

#[test]
fn approval_skipped_is_approval_failed() {
    let status = done(
        "Approve command performed: Comments resolved. Approval skipped: durable Cloud recovery is unavailable",
        RABBIT_CREATED,
        None,
    );
    let v = derived(vec![status], vec![], 60.0);
    assert_eq!(
        v.rabbit,
        Some(Rabbit::ApprovalFailed(
            "durable Cloud recovery is unavailable".into()
        ))
    );
    assert_eq!(
        v.rabbit.as_ref().map(|r| cr::label(r, None)).as_deref(),
        Some("duyệt lỗi")
    );
    let already = done(
        "Approve command performed: Approval skipped: already approved",
        RABBIT_CREATED,
        None,
    );
    assert_eq!(
        derived(vec![already], vec![], 60.0).rabbit,
        Some(Rabbit::Approved)
    );
}

fn failed_comment() -> Value {
    rabbit_comment("Comments resolved, but approval failed.", &at(30.0))
}

fn approval_failed() -> Option<Rabbit> {
    Some(Rabbit::ApprovalFailed(
        "Comments resolved, but approval failed.".into(),
    ))
}

#[test]
fn approval_failed_from_comment() {
    let v = derived(
        vec![reviewed_status()],
        vec![approve_command(), failed_comment()],
        60.0,
    );
    assert_eq!(v.rabbit, approval_failed());
}

#[test]
fn approval_failed_comment_survives_approve_status_update() {
    let status = done(
        "Approve command performed: Comments resolved.",
        &at(-600.0),
        Some(&at(60.0)),
    );
    let v = derived(
        vec![status],
        vec![approve_command(), failed_comment()],
        90.0,
    );
    assert_eq!(v.rabbit, approval_failed());
}

#[test]
fn approval_failed_comment_cleared_by_new_review() {
    let fresh = done("Review completed", &at(120.0), None);
    let v = derived(
        vec![reviewed_status(), fresh],
        vec![approve_command(), failed_comment()],
        150.0,
    );
    assert_eq!(v.rabbit, Some(Rabbit::AwaitingApproval));
}

#[test]
fn newer_command_supersedes_approval_failed() {
    let again = human_reply(11, None, "{me}", "@coderabbitai approve", &at(90.0), false);
    let v = derived(
        vec![reviewed_status()],
        vec![approve_command(), failed_comment(), again],
        100.0,
    );
    assert_eq!(v.rabbit, Some(Rabbit::Nudged));
}

fn answered_thread() -> Vec<Value> {
    vec![
        rabbit_inline(1, "nit", &at(-500.0), None),
        human_reply(2, Some(1), "{me}", "fixed", &at(-400.0), true),
        rabbit_inline(3, "ok", &at(-300.0), Some(1)),
    ]
}

#[test]
fn nudge_ends_on_status_update() {
    let comments = plus(answered_thread(), vec![approve_command()]);
    assert_eq!(
        derived(vec![reviewed_status()], comments.clone(), 40.0).rabbit,
        Some(Rabbit::Nudged)
    );
    let updated = done("Review completed", &at(-600.0), Some(&at(30.0)));
    assert_eq!(
        derived(vec![updated], comments, 40.0).rabbit,
        Some(Rabbit::AwaitingApproval)
    );
}

#[test]
fn nudge_times_out_after_600s() {
    let comments = plus(answered_thread(), vec![approve_command()]);
    assert_eq!(
        derived(vec![reviewed_status()], comments.clone(), 599.0).rabbit,
        Some(Rabbit::Nudged)
    );
    assert_eq!(
        derived(vec![reviewed_status()], comments, 601.0).rabbit,
        Some(Rabbit::AwaitingApproval)
    );
}

#[test]
fn addressed_root_is_done() {
    let root = rabbit_inline(
        1,
        "✅ Addressed in commits 6ae36ec to e0eaca7",
        &at(-300.0),
        None,
    );
    let t = |comments: Vec<Value>| derived(vec![reviewed_status()], comments, 60.0).threads;
    assert_eq!(t(vec![root.clone()]), Some(threads(0, 1)));
    let later = human_reply(2, Some(1), "{me}", "thanks, but also...", &at(-100.0), true);
    assert_eq!(t(vec![root, later]), Some(threads(0, 1)));
}

#[test]
fn others_pending_ignores_author_and_me() {
    let pending =
        |comments: &[Value], me: Option<&str>| cr::facts(comments, me, Some("{a}")).others_pending;
    let r = human_reply(1, None, "{r}", "please rename", &at(0.0), true);
    let me = Some("{me}");
    assert!(pending(std::slice::from_ref(&r), me));
    assert!(pending(
        &[
            r.clone(),
            human_reply(2, Some(1), "{a}", "done", &at(10.0), true)
        ],
        me
    ));
    assert!(!pending(
        &[
            r.clone(),
            human_reply(2, Some(1), "{me}", "done", &at(10.0), true)
        ],
        me
    ));
    assert!(!pending(
        &[human_reply(1, None, "{a}", "note", &at(0.0), true)],
        me
    ));
    assert!(!pending(
        &[human_reply(
            1,
            None,
            "{r}",
            "@coderabbitai review",
            &at(0.0),
            true
        )],
        me
    ));
    assert!(!pending(&[r], None));
    // The CodeRabbit account's own unmarked comments are not a reviewer's.
    let bot = rabbit_comment("summary", &at(-10.0));
    assert!(!pending(
        &[
            bot,
            human_reply(
                1,
                None,
                "{tin}",
                "reply from the bot account",
                &at(0.0),
                true
            )
        ],
        me
    ));
}

#[test]
fn hides_ask_claude_keeps_button_for_others() {
    let none = Some(threads(0, 0));
    assert!(cr::hides_ask_claude(
        Some(&Rabbit::AwaitingApproval),
        none,
        false
    ));
    assert!(!cr::hides_ask_claude(
        Some(&Rabbit::AwaitingApproval),
        none,
        true
    ));
    assert!(cr::hides_ask_claude(
        Some(&Rabbit::ApprovalFailed("x".into())),
        none,
        false
    ));
}

fn replied_threads() -> Vec<Value> {
    vec![
        rabbit_inline(1, "nit", &at(-500.0), None),
        rabbit_inline(2, "nit", &at(-500.0), None),
        human_reply(3, Some(1), "{me}", "fixed", &at(-400.0), true),
        human_reply(4, Some(2), "{me}", "fixed", &at(-400.0), true),
    ]
}

#[test]
fn skipped_after_replied_review_awaits_approval() {
    let skipped = || done("Review skipped", &at(-300.0), None);
    assert_eq!(
        derived(vec![skipped()], replied_threads(), 0.0).rabbit,
        Some(Rabbit::AwaitingApproval)
    );
    let nudged = plus(replied_threads(), vec![approve_command()]);
    assert_eq!(
        derived(vec![skipped()], nudged, 30.0).rabbit,
        Some(Rabbit::Nudged)
    );
    // A thread nobody replied to keeps the skip.
    let open = plus(
        replied_threads(),
        vec![rabbit_inline(5, "nit", &at(-500.0), None)],
    );
    assert_eq!(
        derived(vec![skipped()], open, 0.0).rabbit,
        Some(Rabbit::Skipped)
    );
    // Skipped from the start: no finding to have replied to.
    let skip_note = rabbit_comment("Review skipped", &at(-300.0));
    assert_eq!(
        derived(vec![skipped()], vec![skip_note], 0.0).rabbit,
        Some(Rabbit::Skipped)
    );
}
