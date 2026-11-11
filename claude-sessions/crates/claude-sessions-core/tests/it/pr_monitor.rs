use crate::support::*;
use claude_sessions_core::bitbucket::{BitbucketApi, BitbucketError};
use claude_sessions_core::code_rabbit::{self, Rabbit};
use claude_sessions_core::credentials::ConnectionState;
use claude_sessions_core::pr_monitor::{PRMonitor, PRSnapshot, PRTarget};
use claude_sessions_core::pull_requests::{PREvent, PRRef, RabbitThreads};
use claude_sessions_core::shell::format_iso_date;
use claude_sessions_core::time::add_secs;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BE: &str = "t6engineering/talo6-dct-backend";
const FE: &str = "t6engineering/talo6-dct-frontend";
const OPEN: &str = "/2.0/workspaces/t6engineering/pullrequests/";
const STARTED: &str = "2026-11-03T03:33:15.111541+00:00";
const LATER: &str = "2026-11-03T03:40:00.000000+00:00";
/// After the summary comment (`LATER`): an INPROGRESS status touched again by the approve command.
const BUMPED: &str = "2026-11-03T03:50:00.000000+00:00";
const START: &str = "2026-11-04T10:00:00.000000+00:00";
const SUMMARY_AT: &str = "2026-11-04T10:12:00.000000+00:00";
const C1: &str = "2026-11-04T10:00:00.000000+00:00";
const C2: &str = "2026-11-04T10:01:00.000000+00:00";
const C3: &str = "2026-11-04T10:02:00.000000+00:00";

type Api = Arc<FakeBitbucket>;

fn t0() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

fn at(dt: f64) -> SystemTime {
    add_secs(t0(), dt)
}

fn pr_ref_in(id: i64, repo: &str) -> PRRef {
    let (workspace, name) = repo.split_once('/').unwrap();
    PRRef {
        workspace: workspace.into(),
        repo: name.into(),
        id,
    }
}

fn pr_ref(id: i64) -> PRRef {
    pr_ref_in(id, BE)
}

fn target(link_refs: &[PRRef], branch: Option<&str>) -> Vec<PRTarget> {
    vec![PRTarget {
        session_id: "s".into(),
        link_refs: link_refs.to_vec(),
        cwd: "/x".into(),
        branch: branch.map(String::from),
    }]
}

fn refs(r: &[PRRef]) -> Vec<PRTarget> {
    target(r, None)
}

fn monitor(api: &Api, remote: Option<&str>) -> PRMonitor {
    let remote = remote.map(String::from);
    PRMonitor::with_options(api.clone(), false, Arc::new(move |_| remote.clone()), 45.0)
}

fn spec(id: i64, comments: i64) -> PrSpec {
    PrSpec {
        id,
        comments,
        ..Default::default()
    }
}

fn pr(id: i64) -> Value {
    pr_json(spec(id, 0))
}

fn pr_c(id: i64, comments: i64) -> Value {
    pr_json(spec(id, comments))
}

fn pr_a(id: i64, comments: i64, approved: bool) -> Value {
    pr_json(PrSpec {
        rabbit_approved: Some(approved),
        ..spec(id, comments)
    })
}

fn pr_in(id: i64, repo: &'static str, branch: &'static str, state: &'static str) -> Value {
    pr_json(PrSpec {
        repo,
        branch,
        state,
        ..spec(id, 0)
    })
}

fn with_commit(mut pr: Value, hash: &str) -> Value {
    pr["source"]["commit"] = json!({"hash": hash});
    pr
}

fn set_open(api: &Api, values: Vec<Value>) {
    api.set(OPEN, json!({"values": values}));
}

fn bump(api: &Api, comments: i64) {
    set_open(api, vec![pr_c(7571, comments)]);
}

fn rs(description: &str, created: &str) -> Value {
    rabbit_status("SUCCESSFUL", description, "abc123", created, None)
}

fn rs_state(state: &str, description: &str, created: &str) -> Value {
    rabbit_status(state, description, "abc123", created, None)
}

fn rs_commit(description: &str, commit: &str, created: &str) -> Value {
    rabbit_status("SUCCESSFUL", description, commit, created, None)
}

fn set_statuses(api: &Api, rabbit: Option<Value>) {
    let mut values = vec![json!({"state": "SUCCESSFUL", "commit": {"hash": "abc123"}})];
    values.extend(rabbit);
    api.set("/statuses", json!({"values": values}));
}

fn set_comments(api: &Api, values: Vec<Value>) {
    api.set("/comments", json!({"values": values}));
}

fn prs_of(snap: &PRSnapshot) -> Vec<claude_sessions_core::pull_requests::PRStatus> {
    snap.prs.get("s").cloned().unwrap_or_default()
}

fn first_of(snap: &PRSnapshot) -> Option<claude_sessions_core::pull_requests::PRStatus> {
    prs_of(snap).into_iter().next()
}

fn ids_of(snap: &PRSnapshot) -> Vec<i64> {
    prs_of(snap).iter().map(|p| p.ref_.id).collect()
}

fn rabbit_of(m: &PRMonitor) -> Option<Rabbit> {
    first_of(&m.take_snapshot()).and_then(|p| p.rabbit)
}

fn n(api: &Api, s: &str) -> usize {
    api.requests_containing(s)
}

fn count_of(events: &[PREvent], e: PREvent) -> usize {
    events.iter().filter(|x| **x == e).count()
}

fn in_progress(updated: &str) -> Value {
    rabbit_status(
        "INPROGRESS",
        "Review in progress",
        "abc123",
        START,
        Some(updated),
    )
}

/// INPROGRESS status, then a summary posted after its start and one more comment.
fn overridden_monitor(api: &Api, summary: &str) -> PRMonitor {
    set_statuses(api, Some(in_progress(START)));
    let m = monitor(api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    let _ = m.take_snapshot();
    set_comments(
        api,
        vec![
            rabbit_comment(summary, SUMMARY_AT),
            rabbit_inline(1, "nit", SUMMARY_AT, None),
        ],
    );
    bump(api, 2);
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    m
}

const ONE_FINDING: &str = "**Actionable comments posted: 1**";

/// The #7583 shape: a finding the human answered and CodeRabbit withdrew, then waiting for the approval.
fn answered_thread(api: &Api) {
    set_comments(
        api,
        vec![
            rabbit_comment(ONE_FINDING, C1),
            rabbit_inline(1, "nit", C1, None),
            human_reply(2, Some(1), "{me}", "won't fix", C2, true),
            rabbit_inline(3, "withdrawn", C3, Some(1)),
        ],
    );
}

fn unanswered_thread(api: &Api) {
    set_comments(api, pending_finding(1, C1));
}

/// A monitor on a PR in `chờ duyệt`. Threaded: returns once the first cycle has run on the worker.
fn awaiting_monitor(api: &Api, threaded: bool) -> PRMonitor {
    set_open(api, vec![pr_a(7583, 3, false)]);
    set_statuses(api, Some(rs("Review completed", STARTED)));
    answered_thread(api);
    let m = PRMonitor::with_options(api.clone(), threaded, Arc::new(|_| None), 45.0);
    m.update(&refs(&[pr_ref(7583)]), t0());
    barrier(&m);
    m
}

/// Waits until the worker has run everything submitted so far (a refused nudge for an unknown PR is one more job).
fn barrier(m: &PRMonitor) {
    let (tx, rx) = channel();
    m.nudge(&pr_ref(1), move |ok| tx.send(ok).unwrap());
    assert!(!rx.recv_timeout(Duration::from_secs(5)).unwrap());
}

fn nudge_result(m: &PRMonitor, r: &PRRef) -> bool {
    let (tx, rx) = channel();
    m.nudge(r, move |ok| tx.send(ok).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}

#[test]
fn link_refs_shown_after_first_cycle() {
    let api = fake(vec![pr_json(PrSpec {
        reviewers: 3,
        ..spec(7571, 0)
    })]);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    let snap = m.take_snapshot();
    let prs = prs_of(&snap);
    assert_eq!(ids_of(&snap), [7571]);
    assert_eq!(prs[0].reviewers, 3);
    assert_eq!(prs[0].approved, 1);
    assert_eq!(prs[0].changes_requested, 1);
    assert_eq!(snap.last_success, Some(t0()));
    assert!(snap.events.is_empty());
}

#[test]
fn new_link_ref_triggers_cycle() {
    let api = fake(vec![pr(7571), pr_in(6874, FE, "bugfix/T6IR-14744", "OPEN")]);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(n(&api, "/pullrequests/6874"), 0);
    m.update(&refs(&[pr_ref(7571), pr_ref_in(6874, FE)]), at(5.0));
    assert!(n(&api, "/pullrequests/6874") > 0);
    assert_eq!(ids_of(&m.take_snapshot()), [7571, 6874]);
}

#[test]
fn branch_match_uses_cwd_remote() {
    let branch = "bugfix/T6IR-14745";
    let api = fake(vec![
        pr_in(6874, FE, branch, "OPEN"),
        pr_in(7571, BE, branch, "OPEN"),
    ]);
    let m = monitor(
        &api,
        Some("git@bitbucket.org:t6engineering/talo6-dct-frontend.git"),
    );
    m.update(&target(&[], Some(branch)), t0());
    assert_eq!(ids_of(&m.take_snapshot()), [6874]);
}

#[test]
fn branch_pr_kept_after_branch_change() {
    let branch = "bugfix/T6IR-14745";
    let api = fake(vec![pr_in(6874, FE, branch, "OPEN")]);
    let m = monitor(
        &api,
        Some("git@bitbucket.org:t6engineering/talo6-dct-frontend.git"),
    );
    m.update(&target(&[], Some(branch)), t0());
    assert_eq!(ids_of(&m.take_snapshot()), [6874]);

    set_open(&api, vec![]);
    api.set(
        "/talo6-dct-frontend/pullrequests/6874?",
        pr_in(6874, FE, branch, "MERGED"),
    );
    m.update(&target(&[], Some("master")), at(1.0));
    let snap = m.take_snapshot();
    let states: Vec<String> = prs_of(&snap).iter().map(|p| p.state.clone()).collect();
    assert_eq!(states, ["MERGED"]);
    assert_eq!(snap.events, [PREvent::Merged]);

    let before = n(&api, "pullrequests/6874");
    m.update(&target(&[], Some("master")), at(120.0));
    assert_eq!(n(&api, "pullrequests/6874"), before);
    let states: Vec<String> = prs_of(&m.take_snapshot())
        .iter()
        .map(|p| p.state.clone())
        .collect();
    assert_eq!(states, ["MERGED"]);
}

#[test]
fn foreign_workspace_ref_asked_once() {
    let api = fake(vec![pr(7571)]);
    let m = monitor(&api, None);
    let r = [pr_ref(7571), pr_ref_in(1, "acme/app")];
    m.update(&refs(&r), t0());
    m.update(&refs(&r), at(61.0));
    assert_eq!(n(&api, "repositories/acme/app/pullrequests/1"), 1);
    assert_eq!(ids_of(&m.take_snapshot()), [7571]);
}

#[test]
fn closed_on_first_sight_hidden() {
    let api = fake(vec![]);
    api.set(
        "/talo6-dct-backend/pullrequests/7567?",
        pr_in(7567, BE, "bugfix/T6IR-14744", "MERGED"),
    );
    let m = monitor(&api, None);
    let r = [pr_ref(7567)];
    m.update(&refs(&r), t0());
    assert!(prs_of(&m.take_snapshot()).is_empty());
    m.update(&refs(&r), at(120.0));
    assert_eq!(n(&api, "pullrequests/7567"), 1);
}

#[test]
fn statuses_only_when_needed() {
    let api = fake(vec![pr(7571)]);
    // A finished CodeRabbit review: with none (nil) the statuses are asked again every cycle.
    api.set(
        "/statuses",
        json!({"values": [{"state": "SUCCESSFUL"}, rs("Review completed", RABBIT_CREATED)]}),
    );
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    assert_eq!(n(&api, "/statuses"), 1);
    m.update(&refs(&r), at(61.0));
    assert_eq!(n(&api, "/statuses"), 1);
    set_open(&api, vec![with_commit(pr(7571), "def456")]);
    m.update(&refs(&r), at(122.0));
    assert_eq!(n(&api, "/statuses"), 2);
}

#[test]
fn running_ci_status_does_not_refetch_statuses() {
    let api = fake(vec![pr(7571)]);
    // A still-running CI status next to a finished CodeRabbit review must not keep the statuses polled.
    api.set(
        "/statuses",
        json!({"values": [{"state": "INPROGRESS"}, rs("Review completed", RABBIT_CREATED)]}),
    );
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    assert_eq!(n(&api, "/statuses"), 1);
    m.update(&refs(&r), at(46.0));
    m.update(&refs(&r), at(92.0));
    assert_eq!(n(&api, "/statuses"), 1);
}

#[test]
fn finished_rabbit_without_ci_is_not_refetched_in_window() {
    let api = fake(vec![pr(7571)]);
    // A finished CodeRabbit review and no CI status at all must not keep the statuses polled.
    api.set(
        "/statuses",
        json!({"values": [rs("Review completed", RABBIT_CREATED)]}),
    );
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    assert_eq!(n(&api, "/statuses"), 1);
    m.update(&refs(&r), at(46.0));
    m.update(&refs(&r), at(92.0));
    assert_eq!(n(&api, "/statuses"), 1);
}

#[test]
fn comments_only_when_count_changes() {
    let api = fake(vec![pr_c(7571, 2)]);
    set_comments(
        &api,
        vec![comment(
            "{tin}",
            "Tin Tran",
            "2026-11-04T10:00:00.000000+00:00",
        )],
    );
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    assert_eq!(n(&api, "/comments"), 1);
    assert!(m.take_snapshot().events.is_empty());
    m.update(&refs(&r), at(61.0));
    assert_eq!(n(&api, "/comments"), 1);
    set_open(&api, vec![pr_c(7571, 3)]);
    set_comments(
        &api,
        vec![comment(
            "{tin}",
            "Tin Tran",
            "2026-11-04T11:00:00.000000+00:00",
        )],
    );
    m.update(&refs(&r), at(122.0));
    assert_eq!(n(&api, "/comments"), 2);
    assert_eq!(m.take_snapshot().events, [PREvent::Comment]);
}

#[test]
fn event_refs_match_events() {
    let api = fake(vec![pr_c(7571, 2)]);
    set_comments(
        &api,
        vec![comment(
            "{tin}",
            "Tin Tran",
            "2026-11-04T10:00:00.000000+00:00",
        )],
    );
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    assert!(m.take_snapshot().event_refs.is_empty());
    set_open(&api, vec![pr_c(7571, 3)]);
    set_comments(
        &api,
        vec![comment(
            "{tin}",
            "Tin Tran",
            "2026-11-04T11:00:00.000000+00:00",
        )],
    );
    m.update(&refs(&r), at(61.0));
    assert_eq!(
        m.take_snapshot().event_refs,
        std::collections::HashSet::from([pr_ref(7571)])
    );
    assert!(m.take_snapshot().event_refs.is_empty());
    m.update(&refs(&r), at(122.0));
    assert!(m.take_snapshot().event_refs.is_empty());
}

#[test]
fn failed_cycle_keeps_statuses() {
    let api = fake(vec![pr_json(PrSpec {
        reviewers: 2,
        ..spec(7571, 0)
    })]);
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    let good = m.take_snapshot();
    *api.fail_all.lock().unwrap() = Some(BitbucketError::Http(403));
    m.update(&refs(&r), at(61.0));
    let after = m.take_snapshot();
    assert_eq!(after.prs, good.prs);
    assert_eq!(after.last_success, Some(t0()));
    assert!(after.events.is_empty());
}

#[test]
fn cycle_waits_for_interval() {
    let api = fake(vec![pr(7571), pr(7572)]);
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    let first = api.requests().len();
    assert!(first > 0);
    m.update(&refs(&[pr_ref(7571), pr_ref(7572)]), at(5.0));
    let second = api.requests().len();
    assert!(second > first);
    m.update(&refs(&r), at(5.0 + 44.0));
    assert_eq!(api.requests().len(), second);
    m.update(&refs(&r), at(5.0 + 46.0));
    assert!(api.requests().len() > second);
}

#[test]
fn take_snapshot_drains_events() {
    let api = fake(vec![pr_json(PrSpec {
        reviewers: 1,
        ..spec(7571, 0)
    })]);
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    assert!(m.take_snapshot().events.is_empty());
    set_open(
        &api,
        vec![pr_json(PrSpec {
            reviewers: 2,
            ..spec(7571, 0)
        })],
    );
    m.update(&refs(&r), at(61.0));
    assert_eq!(m.take_snapshot().events, [PREvent::Approved]);
    assert!(m.take_snapshot().events.is_empty());
}

#[test]
fn dropped_session_forgotten() {
    let api = fake(vec![pr(7571)]);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert!(m.take_snapshot().prs.contains_key("s"));
    m.update(&[], at(1.0));
    m.update(&[], at(32.0));
    assert!(!m.take_snapshot().prs.contains_key("s"));
}

#[test]
fn brief_absence_keeps_state() {
    let api = fake(vec![pr(7571)]);
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    set_open(&api, vec![]);
    api.set(
        "/talo6-dct-backend/pullrequests/7571?",
        pr_in(7571, BE, "bugfix/T6IR-14744", "MERGED"),
    );
    m.update(&[], at(1.0));
    m.update(&refs(&r), at(2.0));
    m.update(&refs(&r), at(61.0));
    let states: Vec<String> = prs_of(&m.take_snapshot())
        .iter()
        .map(|p| p.state.clone())
        .collect();
    assert_eq!(states, ["MERGED"]);
}

#[test]
fn foreign_workspace_open_list_failure_skips_only_that_workspace() {
    let api = fake(vec![pr(7571)]);
    let m = monitor(&api, None);
    let r = [pr_ref(7571), pr_ref_in(1, "acme/app")];
    m.update(&refs(&r), t0());
    let snap = m.take_snapshot();
    assert_eq!(ids_of(&snap), [7571]);
    assert_eq!(snap.last_success, Some(t0()));
}

#[test]
fn every_open_list_forbidden_fails_cycle() {
    let api = fake(vec![pr(7571)]);
    let m = monitor(&api, None);
    let r = [pr_ref(7571), pr_ref_in(1, "acme/app")];
    m.update(&refs(&r), t0());
    let good = m.take_snapshot();
    api.set_error(OPEN, BitbucketError::Http(403));
    m.update(&refs(&r), at(61.0));
    let after = m.take_snapshot();
    assert_eq!(after.prs, good.prs);
    assert_eq!(after.last_success, Some(t0()));
}

#[test]
fn failed_statuses_after_push_keeps_old_status_then_retries() {
    let api = fake(vec![pr(7571)]);
    let m = monitor(&api, None);
    let r = [pr_ref(7571)];
    m.update(&refs(&r), t0());
    let good = m.take_snapshot();
    set_open(&api, vec![with_commit(pr(7571), "def456")]);
    api.set_error("/statuses", BitbucketError::Http(500));
    m.update(&refs(&r), at(61.0));
    assert_eq!(m.take_snapshot().prs, good.prs);
    api.set("/statuses", json!({"values": []}));
    let before = n(&api, "/statuses");
    m.update(&refs(&r), at(122.0));
    assert_eq!(n(&api, "/statuses"), before + 1);
    let snap = m.take_snapshot();
    assert_eq!(first_of(&snap).unwrap().commit.as_deref(), Some("def456"));
}

#[test]
fn rabbit_assigned() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_comments(&api, pending_finding(2, LATER));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    let p = first_of(&m.take_snapshot()).unwrap();
    assert_eq!(p.rabbit, Some(Rabbit::Reviewed(Some(2))));
    assert_eq!(p.last_other, None);
}

#[test]
fn statuses_refetched_while_rabbit_pending_or_comments_change() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(
        &api,
        Some(rs_state("INPROGRESS", "Review in progress", STARTED)),
    );
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    let mut count = n(&api, "/statuses");
    m.update(&r, at(61.0));
    assert_eq!(n(&api, "/statuses"), count + 1);

    set_statuses(&api, Some(rs("Review completed", STARTED)));
    m.update(&r, at(122.0));
    count = n(&api, "/statuses");
    assert_eq!(count, 3);
    m.update(&r, at(183.0));
    assert_eq!(n(&api, "/statuses"), count);

    set_open(&api, vec![pr_c(7571, 2)]);
    m.update(&r, at(244.0));
    assert_eq!(n(&api, "/statuses"), count + 1);
}

#[test]
fn comments_refetched_when_rabbit_finishes() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(
        &api,
        Some(rs_state("INPROGRESS", "Review in progress", STARTED)),
    );
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewing));
    let count = n(&api, "/comments");

    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_comments(&api, pending_finding(2, LATER));
    m.update(&r, at(61.0));
    assert_eq!(n(&api, "/comments"), count + 1);
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(2))));
}

#[test]
fn summary_ends_in_progress_review() {
    let m = overridden_monitor(&fake(vec![pr_c(7571, 1)]), ONE_FINDING);
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(1)))
    );
    assert_eq!(
        code_rabbit::label(&Rabbit::Reviewed(Some(1)), None),
        "1 góp ý"
    );
    assert_eq!(snap.events, [PREvent::RabbitNeedsYou]);
}

#[test]
fn override_kept_while_status_stays_in_progress() {
    let api = fake(vec![pr_c(7571, 1)]);
    let m = overridden_monitor(&api, ONE_FINDING);
    let _ = m.take_snapshot();
    bump(&api, 3);
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(1))));
    assert!(m.take_snapshot().events.is_empty());
}

#[test]
fn override_kept_when_only_statuses_are_refetched() {
    let api = fake(vec![pr_c(7571, 1)]);
    // CodeRabbit's status stays INPROGRESS; the finished review is re-polled only by the 5-minute slow poll.
    api.set("/statuses", json!({"values": [in_progress(START)]}));
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    set_comments(&api, pending_finding(1, SUMMARY_AT));
    bump(&api, 2);
    m.update(&r, at(61.0));
    let _ = m.take_snapshot();
    let statuses = n(&api, "/statuses");
    let comments = n(&api, "/comments");
    m.update(&r, at(400.0));
    assert_eq!(n(&api, "/statuses"), statuses + 1);
    assert_eq!(n(&api, "/comments"), comments);
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(1))));
    assert!(m.take_snapshot().events.is_empty());
}

#[test]
fn summary_found_on_owed_fetch_at_first_sight_does_not_chime() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(in_progress(START)));
    api.set_error("/comments", BitbucketError::Transport);
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    let _ = m.take_snapshot();
    set_comments(&api, pending_finding(1, SUMMARY_AT));
    m.update(&r, at(61.0));
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(1)))
    );
    assert!(snap.events.is_empty());
}

#[test]
fn override_survives_status_flipping_to_successful() {
    let api = fake(vec![pr_c(7571, 1)]);
    let m = overridden_monitor(&api, ONE_FINDING);
    let _ = m.take_snapshot();
    set_statuses(
        &api,
        Some(rabbit_status(
            "SUCCESSFUL",
            "Review completed",
            "abc123",
            START,
            Some("2026-11-04T10:30:00.000000+00:00"),
        )),
    );
    bump(&api, 3);
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(1))));
    assert!(m.take_snapshot().events.is_empty());
}

#[test]
fn summary_before_in_progress_start_does_not_end_review() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(in_progress(START)));
    set_comments(&api, pending_finding(1, "2026-11-04T09:00:00.000000+00:00"));
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    let _ = m.take_snapshot();
    bump(&api, 2);
    m.update(&r, at(61.0));
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewing));
    assert!(m.take_snapshot().events.is_empty());
}

#[test]
fn newer_in_progress_start_restarts_review() {
    let api = fake(vec![pr_c(7571, 1)]);
    let m = overridden_monitor(&api, ONE_FINDING);
    let _ = m.take_snapshot();
    set_statuses(&api, Some(in_progress("2026-11-04T10:40:00.000000+00:00")));
    bump(&api, 3);
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewing));
}

#[test]
fn no_actionable_summary_ends_in_progress_review() {
    let m = overridden_monitor(
        &fake(vec![pr_c(7571, 1)]),
        "No actionable comments were generated.",
    );
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(0)))
    );
    assert_eq!(snap.events, [PREvent::RabbitDone]);
}

#[test]
fn first_sight_of_reviewed_pr_with_in_progress_status_does_not_chime() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(in_progress(START)));
    set_comments(&api, pending_finding(1, SUMMARY_AT));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(1)))
    );
    assert!(snap.events.is_empty());
}

#[test]
fn rabbit_chimes_once_per_review() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_comments(&api, pending_finding(2, LATER));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert!(m.take_snapshot().events.is_empty());
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    assert!(m.take_snapshot().events.is_empty());

    set_open(&api, vec![with_commit(pr_c(7571, 1), "def456")]);
    api.set(
        "/statuses",
        json!({"values": [rs_commit("Review completed", "def456", "2026-11-03T05:00:00.000000+00:00")]}),
    );
    set_comments(&api, pending_finding(0, "2026-11-03T05:10:00.000000+00:00"));
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    let snap = m.take_snapshot();
    assert_eq!(snap.events, [PREvent::RabbitDone]);
    assert_eq!(
        snap.event_refs,
        std::collections::HashSet::from([pr_ref(7571)])
    );
    m.update(&refs(&[pr_ref(7571)]), at(183.0));
    assert!(m.take_snapshot().events.is_empty());
}

#[test]
fn rabbit_chimes_when_review_ends_between_polls() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_comments(&api, pending_finding(0, LATER));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(0))));

    set_open(&api, vec![with_commit(pr_c(7571, 1), "def456")]);
    api.set(
        "/statuses",
        json!({"values": [rs_commit("Review completed", "def456", "2026-11-03T05:00:00.000000+00:00")]}),
    );
    set_comments(&api, pending_finding(1, "2026-11-03T05:10:00.000000+00:00"));
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    assert_eq!(m.take_snapshot().events, [PREvent::RabbitNeedsYou]);
}

#[test]
fn findings_kept_when_summary_missing_from_comments_page() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_comments(&api, pending_finding(8, LATER));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(8))));

    set_open(&api, vec![pr_c(7571, 2)]);
    set_comments(
        &api,
        vec![rabbit_inline(1, "nit: rename this", LATER, None)],
    );
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(8)))
    );
    assert!(snap.events.is_empty());
}

#[test]
fn first_failed_statuses_fetch_is_not_a_chime_later() {
    let api = fake(vec![pr_c(7571, 1)]);
    api.set_error("/statuses", BitbucketError::Transport);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_comments(&api, pending_finding(2, LATER));
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(2)))
    );
    assert!(snap.events.is_empty());

    set_open(&api, vec![with_commit(pr_c(7571, 1), "def456")]);
    api.set(
        "/statuses",
        json!({"values": [rs_commit("Review completed", "def456", "2026-11-03T05:00:00.000000+00:00")]}),
    );
    set_comments(&api, pending_finding(0, "2026-11-03T05:10:00.000000+00:00"));
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    assert_eq!(m.take_snapshot().events, [PREvent::RabbitDone]);
}

#[test]
fn old_review_after_two_transient_failures_does_not_chime() {
    let api = fake(vec![pr_c(7571, 1)]);
    api.set_error("/statuses", BitbucketError::Transport);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    api.set_error("/comments", BitbucketError::Transport);
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    set_comments(&api, pending_finding(2, LATER));
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(2)))
    );
    assert!(snap.events.is_empty());
}

#[test]
fn reviewed_review_is_slow_polled_until_it_flips_to_approved() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_comments(&api, pending_finding(1, LATER));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(1))));
    let _ = m.take_snapshot();
    let count = n(&api, "/statuses");
    m.update(&refs(&[pr_ref(7571)]), at(299.0));
    assert_eq!(n(&api, "/statuses"), count);
    set_statuses(&api, Some(rs("Review approved", STARTED)));
    set_open(&api, vec![pr_a(7571, 1, true)]);
    m.update(&refs(&[pr_ref(7571)]), at(700.0));
    assert_eq!(n(&api, "/statuses"), count + 1);
    let snap = m.take_snapshot();
    assert_eq!(first_of(&snap).unwrap().rabbit, Some(Rabbit::Approved));
    assert_eq!(code_rabbit::label(&Rabbit::Approved, None), "đã duyệt");
    // The account's approval is also a PR approval event.
    assert_eq!(snap.events, [PREvent::Approved, PREvent::RabbitDone]);
}

#[test]
fn settled_rabbit_kinds_are_not_slow_polled() {
    let cases = [
        ("SUCCESSFUL", "Review approved"),
        ("SUCCESSFUL", "Review skipped"),
        ("FAILED", "Review failed"),
    ];
    for (state, description) in cases {
        let api = fake(vec![pr_c(7571, 1)]);
        set_statuses(&api, Some(rs_state(state, description, STARTED)));
        let m = monitor(&api, None);
        m.update(&refs(&[pr_ref(7571)]), t0());
        let count = n(&api, "/statuses");
        m.update(&refs(&[pr_ref(7571)]), at(700.0));
        m.update(&refs(&[pr_ref(7571)]), at(1400.0));
        assert_eq!(n(&api, "/statuses"), count, "{description}");
    }
}

#[test]
fn slow_poll_after_override_with_status_still_in_progress_changes_nothing() {
    let api = fake(vec![pr_c(7571, 1)]);
    let m = overridden_monitor(&api, ONE_FINDING);
    let _ = m.take_snapshot();
    let statuses = n(&api, "/statuses");
    let comments = n(&api, "/comments");
    m.update(&refs(&[pr_ref(7571)]), at(700.0));
    assert_eq!(n(&api, "/statuses"), statuses + 1);
    assert_eq!(n(&api, "/comments"), comments);
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(1))));
    assert!(m.take_snapshot().events.is_empty());
}

#[test]
fn closed_pr_is_never_slow_polled() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    set_open(&api, vec![]);
    api.set(
        "/talo6-dct-backend/pullrequests/7571?",
        pr_in(7571, BE, "bugfix/T6IR-14744", "MERGED"),
    );
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    let count = n(&api, "/statuses");
    m.update(&refs(&[pr_ref(7571)]), at(700.0));
    assert_eq!(n(&api, "/statuses"), count);
}

#[test]
fn status_appearing_after_window_is_picked_up_by_the_slow_poll() {
    let api = fake(vec![pr_c(7571, 1)]);
    api.set("/statuses", json!({"values": []}));
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    m.update(&r, at(599.0));
    let count = n(&api, "/statuses");
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    m.update(&r, at(660.0));
    m.update(&r, at(800.0));
    assert_eq!(n(&api, "/statuses"), count);
    m.update(&r, at(900.0));
    assert_eq!(n(&api, "/statuses"), count + 1);
    assert!(rabbit_of(&m).is_some());
}

#[test]
fn statuses_failing_through_the_window_are_fetched_after_it() {
    let api = fake(vec![pr_c(7571, 1)]);
    api.set_error("/statuses", BitbucketError::Transport);
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    m.update(&r, at(599.0));
    set_statuses(&api, None);
    let count = n(&api, "/statuses");
    m.update(&r, at(660.0));
    assert_eq!(n(&api, "/statuses"), count + 1);
    m.update(&r, at(721.0));
    assert_eq!(n(&api, "/statuses"), count + 1);
}

#[test]
fn statuses_not_polled_forever_without_any_statuses() {
    let api = fake(vec![pr_c(7571, 1)]);
    api.set("/statuses", json!({"values": []}));
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    m.update(&r, at(61.0));
    let count = n(&api, "/statuses");
    m.update(&r, at(660.0 + 61.0));
    assert_eq!(n(&api, "/statuses"), count + 1);
    m.update(&r, at(660.0 + 122.0));
    m.update(&r, at(660.0 + 183.0));
    assert_eq!(n(&api, "/statuses"), count + 1);

    set_open(&api, vec![with_commit(pr_c(7571, 1), "def456")]);
    m.update(&r, at(660.0 + 244.0));
    assert_eq!(n(&api, "/statuses"), count + 2);
}

#[test]
fn new_final_kind_on_same_commit_chimes_after_comments_owed() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs_state("FAILED", "Review failed", STARTED)));
    api.set_error("/comments", BitbucketError::Transport);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Failed));
    let _ = m.take_snapshot();

    set_open(&api, vec![pr_c(7571, 2)]);
    set_statuses(&api, Some(rs("Review completed", LATER)));
    set_comments(&api, pending_finding(2, LATER));
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    assert_eq!(m.take_snapshot().events, [PREvent::RabbitNeedsYou]);
}

#[test]
fn new_review_on_same_commit_after_unsaved_reviewing_cycle_still_chimes() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    api.set_error("/comments", BitbucketError::Transport);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    let _ = m.take_snapshot();

    // A re-review starts and finishes between two saved cycles: the in-progress cycle fails on comments and is not saved.
    set_open(&api, vec![pr_c(7571, 2)]);
    set_statuses(
        &api,
        Some(rs_state("INPROGRESS", "Review in progress", LATER)),
    );
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    set_statuses(
        &api,
        Some(rs("Review completed", "2026-11-03T04:00:00.000000+00:00")),
    );
    set_comments(&api, pending_finding(2, "2026-11-03T04:05:00.000000+00:00"));
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    assert_eq!(m.take_snapshot().events, [PREvent::RabbitNeedsYou]);
}

#[test]
fn new_review_on_same_commit_chimes_again_after_awaiting() {
    let api = fake(vec![pr_a(7583, 1, false)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    unanswered_thread(&api);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7583)]), t0());
    let _ = m.take_snapshot();

    set_open(&api, vec![pr_a(7583, 3, false)]);
    answered_thread(&api);
    m.update(&refs(&[pr_ref(7583)]), at(61.0));
    assert_eq!(rabbit_of(&m), Some(Rabbit::AwaitingApproval));

    set_open(&api, vec![pr_a(7583, 4, false)]);
    set_statuses(
        &api,
        Some(rs_state(
            "INPROGRESS",
            "Review in progress",
            "2026-11-05T10:00:00.000000+00:00",
        )),
    );
    m.update(&refs(&[pr_ref(7583)]), at(122.0));
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewing));

    set_open(&api, vec![pr_a(7583, 5, false)]);
    set_statuses(
        &api,
        Some(rs("Review completed", "2026-11-05T10:00:00.000000+00:00")),
    );
    set_comments(&api, pending_finding(1, "2026-11-05T10:05:00.000000+00:00"));
    m.update(&refs(&[pr_ref(7583)]), at(183.0));
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(1)))
    );
    assert_eq!(snap.events, [PREvent::RabbitNeedsYou]);
}

#[test]
fn failed_first_comments_fetch_is_retried() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    api.set_error("/comments", BitbucketError::Transport);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(None)));
    let count = n(&api, "/comments");

    set_comments(&api, pending_finding(2, LATER));
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    assert_eq!(n(&api, "/comments"), count + 1);
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(2)))
    );
    assert!(snap.events.is_empty());
}

#[test]
fn rabbit_threads_filled() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    api.set_error("/comments", BitbucketError::Transport);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(first_of(&m.take_snapshot()).unwrap().rabbit_threads, None);

    set_comments(&api, pending_finding(2, LATER));
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    assert_eq!(
        first_of(&m.take_snapshot()).unwrap().rabbit_threads,
        Some(RabbitThreads {
            pending: 1,
            total: 1
        })
    );
}

#[test]
fn statuses_not_polled_forever_without_rabbit() {
    let api = fake(vec![pr_c(7571, 1)]);
    let m = monitor(&api, None);
    let r = refs(&[pr_ref(7571)]);
    m.update(&r, t0());
    m.update(&r, at(61.0));
    assert_eq!(n(&api, "/statuses"), 2);
    m.update(&r, at(660.0 + 61.0));
    assert_eq!(n(&api, "/statuses"), 3);
    m.update(&r, at(660.0 + 122.0));
    m.update(&r, at(660.0 + 183.0));
    assert_eq!(n(&api, "/statuses"), 3);
}

#[test]
fn rabbit_comment_does_not_chime() {
    let api = fake(vec![pr_c(7571, 0)]);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    let _ = m.take_snapshot();
    set_open(&api, vec![pr_c(7571, 1)]);
    set_comments(&api, pending_finding(1, LATER));
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    let snap = m.take_snapshot();
    assert!(snap.events.is_empty());
    assert_eq!(first_of(&snap).unwrap().last_other, None);
}

#[test]
fn nil_remote_retried_after_five_minutes() {
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    let m = PRMonitor::with_options(
        fake(vec![]),
        false,
        Arc::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            None
        }),
        45.0,
    );
    m.update(&target(&[], None), t0());
    m.update(&target(&[], None), at(100.0));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    m.update(&target(&[], None), at(301.0));
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn connection_ok_after_cycle() {
    let m = monitor(&fake(vec![]), None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(m.take_snapshot().connection, ConnectionState::Ok);
}

#[test]
fn connection_rejected_on_401() {
    let api = fake(vec![]);
    *api.fail_all.lock().unwrap() = Some(BitbucketError::Http(401));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(m.take_snapshot().connection, ConnectionState::Rejected);
}

#[test]
fn connection_ignores_per_pr_errors() {
    let api = fake(vec![pr_json(PrSpec {
        reviewers: 1,
        ..spec(7571, 0)
    })]);
    api.set_error(
        "/2.0/workspaces/t6engineering/pullrequests/9999",
        BitbucketError::Http(404),
    );
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571), pr_ref(9999)]), t0());
    assert_eq!(m.take_snapshot().connection, ConnectionState::Ok);
}

#[test]
fn approved_from_rabbit_account() {
    let api = fake(vec![pr_a(7571, 1, true)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_comments(
        &api,
        vec![rabbit_comment("**Actionable comments posted: 0**", LATER)],
    );
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Approved));
}

/// The #6884 shape: the approve command bumped the INPROGRESS status's updated_on past the summary comment.
#[test]
fn approved_while_status_in_progress() {
    let api = fake(vec![pr_a(6884, 1, true)]);
    set_statuses(
        &api,
        Some(rabbit_status(
            "INPROGRESS",
            "Review queued",
            "abc123",
            STARTED,
            Some(BUMPED),
        )),
    );
    set_comments(&api, vec![rabbit_comment(ONE_FINDING, LATER)]);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(6884)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Approved));
}

/// INPROGRESS then SUCCESSFUL after the approval moves `since`: the approval must not chime a second time.
#[test]
fn approved_while_in_progress_chimes_once() {
    let api = fake(vec![pr_a(6884, 1, false)]);
    set_statuses(
        &api,
        Some(rabbit_status(
            "INPROGRESS",
            "Review queued",
            "abc123",
            STARTED,
            Some(BUMPED),
        )),
    );
    set_comments(&api, vec![rabbit_comment(ONE_FINDING, LATER)]);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(6884)]), t0());
    assert!(m.take_snapshot().events.is_empty());

    set_open(&api, vec![pr_a(6884, 2, true)]);
    m.update(&refs(&[pr_ref(6884)]), at(61.0));
    let c2 = m.take_snapshot();
    assert_eq!(first_of(&c2).unwrap().rabbit, Some(Rabbit::Approved));
    assert_eq!(c2.events, [PREvent::Approved, PREvent::RabbitDone]);

    set_statuses(&api, Some(rs("Review completed", STARTED)));
    set_open(&api, vec![pr_a(6884, 3, true)]);
    m.update(&refs(&[pr_ref(6884)]), at(122.0));
    let c3 = m.take_snapshot();
    assert!(c3.events.is_empty());
    assert_eq!(first_of(&c3).unwrap().rabbit, Some(Rabbit::Approved));
    set_open(&api, vec![pr_a(6884, 4, true)]);
    m.update(&refs(&[pr_ref(6884)]), at(183.0));
    let c4 = m.take_snapshot();
    assert!(c4.events.is_empty());
    assert_eq!(first_of(&c4).unwrap().rabbit, Some(Rabbit::Approved));
}

#[test]
fn awaiting_approval_chimes_once() {
    let api = fake(vec![pr_a(7583, 1, false)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    unanswered_thread(&api);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7583)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(1))));

    set_open(&api, vec![pr_a(7583, 3, false)]);
    answered_thread(&api);
    m.update(&refs(&[pr_ref(7583)]), at(61.0));
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::AwaitingApproval)
    );
    assert_eq!(count_of(&snap.events, PREvent::RabbitDone), 1);

    m.update(&refs(&[pr_ref(7583)]), at(122.0));
    let later = m.take_snapshot();
    assert_eq!(
        first_of(&later).unwrap().rabbit,
        Some(Rabbit::AwaitingApproval)
    );
    assert!(!later.events.contains(&PREvent::RabbitDone));
    assert!(!later.events.contains(&PREvent::RabbitNeedsYou));
}

#[test]
fn awaiting_survives_cycle_without_statuses() {
    let api = fake(vec![pr_a(7583, 3, false)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    answered_thread(&api);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7583)]), t0());
    let first = first_of(&m.take_snapshot()).unwrap();
    assert_eq!(first.rabbit, Some(Rabbit::AwaitingApproval));
    let count = n(&api, "/statuses");
    m.update(&refs(&[pr_ref(7583)]), at(61.0));
    assert_eq!(n(&api, "/statuses"), count);
    let again = first_of(&m.take_snapshot()).unwrap();
    assert_eq!(again.rabbit, Some(Rabbit::AwaitingApproval));
    assert_eq!(again.rabbit_status, first.rabbit_status);
}

#[test]
fn comments_follow_next() {
    let api = fake(vec![pr_a(7583, 3, false)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    let next = "https://api.bitbucket.org/2.0/repositories/t6engineering/talo6-dct-backend/pullrequests/7583/comments?page=2";
    api.set(
        "/comments",
        json!({"values": [rabbit_inline(1, "nit", C1, None), human_reply(2, Some(1), "{me}", "won't fix", C2, true)], "next": next}),
    );
    api.set(
        "/comments?page=2",
        json!({"values": [rabbit_inline(3, "withdrawn", C3, Some(1))]}),
    );
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7583)]), t0());
    assert_eq!(n(&api, "page=2"), 1);
    assert_eq!(rabbit_of(&m), Some(Rabbit::AwaitingApproval));
}

// ---- nudge ----

#[test]
fn own_reply_does_not_rechime() {
    let api = fake(vec![pr_a(7583, 1, false)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    unanswered_thread(&api);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7583)]), t0());
    let _ = m.take_snapshot();

    set_open(&api, vec![pr_a(7583, 3, false)]);
    answered_thread(&api);
    m.update(&refs(&[pr_ref(7583)]), at(61.0));
    let awaiting = m.take_snapshot();
    assert_eq!(
        first_of(&awaiting).unwrap().rabbit,
        Some(Rabbit::AwaitingApproval)
    );
    assert_eq!(count_of(&awaiting.events, PREvent::RabbitDone), 1);

    // The human replies on a CodeRabbit thread again: CodeRabbit's answer is no longer the newest.
    set_open(&api, vec![pr_a(7583, 4, false)]);
    set_comments(
        &api,
        vec![
            rabbit_comment(ONE_FINDING, C1),
            rabbit_inline(1, "nit", C1, None),
            human_reply(2, Some(1), "{me}", "won't fix", C2, true),
            rabbit_inline(3, "withdrawn", C3, Some(1)),
            human_reply(
                4,
                Some(1),
                "{me}",
                "one more thing",
                "2026-11-04T10:03:00.000000+00:00",
                true,
            ),
        ],
    );
    m.update(&refs(&[pr_ref(7583)]), at(122.0));
    let replied = m.take_snapshot();
    assert_eq!(
        first_of(&replied).unwrap().rabbit,
        Some(Rabbit::Reviewed(Some(1)))
    );
    assert!(!replied.events.contains(&PREvent::RabbitDone));
    assert!(!replied.events.contains(&PREvent::RabbitNeedsYou));

    set_open(&api, vec![pr_a(7583, 5, false)]);
    answered_thread(&api);
    m.update(&refs(&[pr_ref(7583)]), at(183.0));
    let answered = m.take_snapshot();
    assert_eq!(
        first_of(&answered).unwrap().rabbit,
        Some(Rabbit::AwaitingApproval)
    );
    assert!(!answered.events.contains(&PREvent::RabbitDone));
    assert!(!answered.events.contains(&PREvent::RabbitNeedsYou));
}

#[test]
fn nudge_posts_approve_command() {
    let api = fake(vec![]);
    let m = awaiting_monitor(&api, false);
    assert_eq!(rabbit_of(&m), Some(Rabbit::AwaitingApproval));
    assert!(nudge_result(&m, &pr_ref(7583)));
    let posts = api.posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(
        posts[0].0,
        "/2.0/repositories/t6engineering/talo6-dct-backend/pullrequests/7583/comments"
    );
    assert_eq!(
        posts[0].1,
        json!({"content": {"raw": "@coderabbitai approve"}})
    );
    assert_eq!(rabbit_of(&m), Some(Rabbit::Nudged));

    // The next cycle reads the comments again and still sees the command as the newest one.
    let count = n(&api, "/comments");
    let users = n(&api, "/2.0/user");
    set_open(&api, vec![pr_a(7583, 4, false)]);
    set_comments(
        &api,
        vec![
            human_reply(
                5,
                None,
                "{me}",
                "@coderabbitai approve",
                "2026-11-04T10:10:00.000000+00:00",
                false,
            ),
            rabbit_comment(ONE_FINDING, C1),
            rabbit_inline(1, "nit", C1, None),
            human_reply(2, Some(1), "{me}", "won't fix", C2, true),
            rabbit_inline(3, "withdrawn", C3, Some(1)),
        ],
    );
    m.update(&refs(&[pr_ref(7583)]), at(1.0));
    assert_eq!(n(&api, "/comments"), count + 1);
    assert_eq!(n(&api, "/2.0/user"), users);
    assert_eq!(rabbit_of(&m), Some(Rabbit::Nudged));
}

#[test]
fn nudge_failure_keeps_state() {
    let api = fake(vec![]);
    let m = awaiting_monitor(&api, false);
    *api.post_result.lock().unwrap() = Some(Err(BitbucketError::Http(403)));
    assert!(!nudge_result(&m, &pr_ref(7583)));
    assert_eq!(rabbit_of(&m), Some(Rabbit::AwaitingApproval));
}

#[test]
fn nudge_refused_when_not_awaiting() {
    let api = fake(vec![pr_a(7583, 1, false)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    unanswered_thread(&api);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7583)]), t0());
    assert_eq!(rabbit_of(&m), Some(Rabbit::Reviewed(Some(1))));
    assert!(!nudge_result(&m, &pr_ref(7583)));
    assert!(api.posts().is_empty());
}

/// Two clicks while the first post is held: the second is refused at once, the first finishes as its result.
fn two_nudges_while_gated(api: &Api) -> (usize, Vec<bool>) {
    let m = awaiting_monitor(api, true);
    let (gate_tx, gate_rx) = sync_channel::<()>(1);
    *api.post_gate.lock().unwrap() = Some(gate_rx);
    let results = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = channel();
    for _ in 0..2 {
        let (results, tx) = (results.clone(), tx.clone());
        m.nudge(&pr_ref(7583), move |ok| {
            results.lock().unwrap().push(ok);
            tx.send(()).unwrap();
        });
    }
    gate_tx.send(()).unwrap();
    for _ in 0..2 {
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    barrier(&m);
    let out = results.lock().unwrap().clone();
    (api.posts().len(), out)
}

#[test]
fn nudge_twice_sends_once() {
    let (posts, mut results) = two_nudges_while_gated(&fake(vec![]));
    assert_eq!(posts, 1);
    results.sort();
    assert_eq!(results, [false, true]);
}

#[test]
fn nudge_twice_after_failure_sends_once() {
    let api = fake(vec![]);
    *api.post_result.lock().unwrap() = Some(Err(BitbucketError::Transport));
    let (posts, results) = two_nudges_while_gated(&api);
    assert_eq!(posts, 1);
    assert_eq!(results, [false, false]);
}

#[test]
fn approval_failed_chimes_once() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    let _ = m.take_snapshot();
    set_open(&api, vec![pr_c(7571, 2)]);
    set_statuses(
        &api,
        Some(rs(
            "Approve command performed: Approval skipped: durable Cloud recovery is unavailable",
            LATER,
        )),
    );
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    let snap = m.take_snapshot();
    assert_eq!(
        first_of(&snap).unwrap().rabbit,
        Some(Rabbit::ApprovalFailed(
            "durable Cloud recovery is unavailable".into()
        ))
    );
    assert_eq!(snap.events, [PREvent::RabbitNeedsYou]);
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    assert!(m.take_snapshot().events.is_empty());
}

#[test]
fn comments_slow_polled_while_pending() {
    let run = |comments: Vec<Value>| {
        let api = fake(vec![pr_c(7571, 1)]);
        set_statuses(&api, Some(rs("Review completed", STARTED)));
        set_comments(&api, comments);
        let m = monitor(&api, None);
        m.update(&refs(&[pr_ref(7571)]), t0());
        (api, m)
    };
    let early = run(pending_finding(1, LATER));
    let count = n(&early.0, "/comments");
    early.1.update(&refs(&[pr_ref(7571)]), at(299.0));
    assert_eq!(n(&early.0, "/comments"), count);

    let late = run(pending_finding(1, LATER));
    late.1.update(&refs(&[pr_ref(7571)]), at(301.0));
    assert_eq!(n(&late.0, "/comments"), count + 1);

    let done = run(vec![rabbit_comment(
        "**Actionable comments posted: 0**",
        LATER,
    )]);
    let k = n(&done.0, "/comments");
    done.1.update(&refs(&[pr_ref(7571)]), at(301.0));
    assert_eq!(n(&done.0, "/comments"), k);
}

#[test]
fn old_unanswered_command_is_awaiting_without_chime() {
    let api = fake(vec![pr_a(7583, 4, false)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    answered_thread(&api);
    let body: Value = serde_json::from_slice(&api.get("/comments").unwrap()).unwrap();
    let mut values = body["values"].as_array().unwrap().clone();
    values.insert(
        0,
        human_reply(
            5,
            None,
            "{me}",
            "@coderabbitai approve",
            &format_iso_date(t0() - Duration::from_secs(7200)),
            false,
        ),
    );
    set_comments(&api, values);
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7583)]), t0());
    let first = m.take_snapshot();
    assert_eq!(
        first_of(&first).unwrap().rabbit,
        Some(Rabbit::AwaitingApproval)
    );
    m.update(&refs(&[pr_ref(7583)]), at(601.0));
    let later = m.take_snapshot();
    assert_eq!(
        first_of(&later).unwrap().rabbit,
        Some(Rabbit::AwaitingApproval)
    );
    assert!(!later.events.contains(&PREvent::RabbitDone));
}

#[test]
fn nudge_times_out_from_its_own_post() {
    let api = fake(vec![]);
    let m = awaiting_monitor(&api, false);
    nudge_result(&m, &pr_ref(7583));
    set_open(&api, vec![pr_a(7583, 4, false)]);
    set_comments(
        &api,
        vec![
            human_reply(
                5,
                None,
                "{me}",
                "@coderabbitai approve",
                &format_iso_date(t0()),
                false,
            ),
            rabbit_comment(ONE_FINDING, C1),
            rabbit_inline(1, "nit", C1, None),
            human_reply(2, Some(1), "{me}", "won't fix", C2, true),
            rabbit_inline(3, "withdrawn", C3, Some(1)),
        ],
    );
    m.update(&refs(&[pr_ref(7583)]), at(599.0));
    assert_eq!(rabbit_of(&m), Some(Rabbit::Nudged));
    // 601 s after the post, and past the 45 s cycle interval since the +599 s cycle.
    m.update(&refs(&[pr_ref(7583)]), at(646.0));
    assert_eq!(rabbit_of(&m), Some(Rabbit::AwaitingApproval));
    assert!(nudge_result(&m, &pr_ref(7583)));
    assert_eq!(api.posts().len(), 2);
}

#[test]
fn superseded_status_failure_does_not_refetch_comments() {
    let api = fake(vec![pr_a(7583, 4, false)]);
    set_statuses(
        &api,
        Some(rs(
            "Approve command performed: Approval skipped: durable Cloud recovery is unavailable",
            LATER,
        )),
    );
    set_comments(
        &api,
        vec![
            human_reply(
                5,
                None,
                "{me}",
                "@coderabbitai approve",
                "2026-11-04T10:10:00.000000+00:00",
                false,
            ),
            rabbit_comment(ONE_FINDING, C1),
            rabbit_inline(1, "nit", C1, None),
            human_reply(2, Some(1), "{me}", "won't fix", C2, true),
            rabbit_inline(3, "withdrawn", C3, Some(1)),
        ],
    );
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7583)]), t0());
    let comments = n(&api, "/comments");
    let statuses = n(&api, "/statuses");
    m.update(&refs(&[pr_ref(7583)]), at(301.0));
    assert_eq!(n(&api, "/statuses"), statuses + 1);
    assert_eq!(n(&api, "/comments"), comments);
}

#[test]
fn comment_failure_chimes_once_and_reason_change_stays_silent() {
    let api = fake(vec![pr_c(7571, 1)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    let m = monitor(&api, None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    let _ = m.take_snapshot();
    let failure = |text: &str, count: i64| {
        set_open(&api, vec![pr_c(7571, count)]);
        set_comments(
            &api,
            vec![
                rabbit_comment(text, C2),
                human_reply(5, None, "{me}", "@coderabbitai approve", C1, false),
            ],
        );
    };
    failure("Comments resolved, but approval failed.", 2);
    m.update(&refs(&[pr_ref(7571)]), at(61.0));
    let first = m.take_snapshot();
    assert_eq!(
        first_of(&first).unwrap().rabbit,
        Some(Rabbit::ApprovalFailed(
            "Comments resolved, but approval failed.".into()
        ))
    );
    assert_eq!(first.events, [PREvent::RabbitNeedsYou]);
    failure("Approval failed: CodeRabbit is unavailable.", 3);
    m.update(&refs(&[pr_ref(7571)]), at(122.0));
    let second = m.take_snapshot();
    assert_eq!(
        first_of(&second).unwrap().rabbit,
        Some(Rabbit::ApprovalFailed(
            "Approval failed: CodeRabbit is unavailable.".into()
        ))
    );
    assert!(second.events.is_empty());
}

#[test]
fn author_parsed() {
    let mut p = pr(7571);
    p["author"] = json!({"uuid": "{author}"});
    let m = monitor(&fake(vec![p]), None);
    m.update(&refs(&[pr_ref(7571)]), t0());
    assert_eq!(
        first_of(&m.take_snapshot()).unwrap().author.as_deref(),
        Some("{author}")
    );
}

/// Wraps the fake API; the first `post` panics.
struct PanicOnFirstPost {
    inner: Api,
    armed: AtomicUsize,
}

impl BitbucketApi for PanicOnFirstPost {
    fn get(&self, path: &str) -> Result<Vec<u8>, BitbucketError> {
        self.inner.get(path)
    }

    fn post(&self, path: &str, json: &Value) -> Result<Vec<u8>, BitbucketError> {
        if self.armed.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("injected panic in a worker job");
        }
        self.inner.post(path, json)
    }
}

#[test]
fn panicking_job_does_not_stop_later_jobs() {
    let api = fake(vec![]);
    set_open(&api, vec![pr_a(7583, 3, false)]);
    set_statuses(&api, Some(rs("Review completed", STARTED)));
    answered_thread(&api);
    let wrapped = Arc::new(PanicOnFirstPost {
        inner: api.clone(),
        armed: AtomicUsize::new(0),
    });
    let m = PRMonitor::with_options(wrapped, true, Arc::new(|_| None), 45.0);
    m.update(&refs(&[pr_ref(7583)]), t0());
    barrier(&m);
    // The first post panics inside the job: `done(false)` still runs and the PR is not stuck as "in flight".
    assert!(!nudge_result(&m, &pr_ref(7583)));
    // The worker survived: the next job runs and the same PR can be nudged again.
    assert!(nudge_result(&m, &pr_ref(7583)));
    assert_eq!(api.posts().len(), 1);
}
