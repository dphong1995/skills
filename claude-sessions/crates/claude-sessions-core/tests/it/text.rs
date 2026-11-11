//! The text tests of Swift `LayoutTests.swift` (`prLineText`, `prLineShowsRabbit`, `agentCountMergesForegroundAndBackground`).

use claude_sessions_core::code_rabbit::Rabbit;
use claude_sessions_core::pull_requests::{PRComment, PRRef, PRStatus};
use claude_sessions_core::store::Session;
use claude_sessions_core::text::{agent_count, pr_line};
use std::time::{Duration, UNIX_EPOCH};
use url::Url;

fn pr_status(
    id: i64,
    state: &str,
    reviewers: i64,
    changes: i64,
    comments: i64,
    last_other: Option<PRComment>,
) -> PRStatus {
    PRStatus {
        ref_: PRRef {
            workspace: "w".into(),
            repo: "r".into(),
            id,
        },
        url: Url::parse(&format!("https://bitbucket.org/w/r/pull-requests/{id}")).unwrap(),
        state: state.into(),
        reviewers,
        approved: 0,
        changes_requested: changes,
        comments,
        commit: None,
        source_repo: None,
        branch: None,
        title: None,
        last_other,
        approvers: Default::default(),
        author: None,
        rabbit: None,
        rabbit_status: None,
        rabbit_since: None,
        rabbit_threads: None,
        rabbit_others_pending: false,
    }
}

#[test]
fn pr_line_text() {
    let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let pr = pr_status(
        7571,
        "OPEN",
        5,
        1,
        23,
        Some(PRComment {
            name: "Tin Tran".into(),
            at: now - Duration::from_secs(180),
        }),
    );
    let stale = Some(now - Duration::from_secs(420));
    assert_eq!(
        pr_line(&pr, now, stale),
        "PR #7571 · 1 yêu cầu sửa · 0/5 · 23 · Tin Tran 3m · cũ 7m"
    );
    assert_eq!(
        pr_line(&pr, now, None),
        "PR #7571 · 1 yêu cầu sửa · 0/5 · 23 · Tin Tran 3m"
    );
    assert_eq!(
        pr_line(&pr_status(9, "OPEN", 0, 0, 0, None), now, None),
        "PR #9"
    );
    assert_eq!(
        pr_line(&pr_status(7567, "MERGED", 2, 0, 0, None), now, None),
        "PR #7567 · đã merge"
    );
    assert_eq!(
        pr_line(&pr_status(7567, "DECLINED", 0, 0, 0, None), now, None),
        "PR #7567 · đã đóng"
    );
}

#[test]
fn pr_line_shows_rabbit() {
    let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let mut pr = pr_status(9, "OPEN", 0, 0, 0, None);
    pr.rabbit = Some(Rabbit::Reviewed(Some(2)));
    assert_eq!(pr_line(&pr, now, None), "PR #9 · 2 góp ý");
    let mut closed = pr_status(9, "MERGED", 0, 0, 0, None);
    closed.rabbit = Some(Rabbit::Reviewed(Some(2)));
    assert!(!pr_line(&closed, now, None).contains("góp ý"));
}

#[test]
fn agent_count_merges_foreground_and_background() {
    let mut s = Session::new("a", "a", "/x", "idle");
    assert_eq!(agent_count(&s), 0);
    s.background_agents = Some(2);
    assert_eq!(agent_count(&s), 2);
    s.background_agents = None;
    s.agents = vec!["x".into(), "y".into(), "z".into()];
    assert_eq!(agent_count(&s), 3);
}
