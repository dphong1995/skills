//! The state and text assertions of Swift `SnapshotTests.swift`, one test per Swift snapshot test that has one. The rendered
//! images (the other tests of that file) become PNG snapshots in phase 6.

use crate::support::*;
use claude_sessions_core::bitbucket::BitbucketError;
use claude_sessions_core::closed_sessions::{ClosedSession, ClosedSessions};
use claude_sessions_core::code_rabbit::{self, Rabbit};
use claude_sessions_core::feed::FeedMonitor;
use claude_sessions_core::jira_monitor::JiraMonitor;
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::pr_monitor::PRMonitor;
use claude_sessions_core::pull_requests::{PRRef, RabbitThreads};
use claude_sessions_core::quota::Quota;
use claude_sessions_core::store::{
    ItemTone, LinkItem, LinkKind, LinkSection, SendKey, SendKind, SendState, Store,
};
use claude_sessions_core::text::pr_line;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use url::Url;

fn secs(t: SystemTime, s: u64) -> SystemTime {
    t + Duration::from_secs(s)
}

fn status_file(
    dir: &std::path::Path,
    id: &str,
    status: &str,
    waiting_for: Option<&str>,
    name: &str,
    cwd: &str,
    age: f64,
) {
    write_session_with(
        dir,
        id,
        me(),
        status,
        age,
        name,
        cwd,
        SessionOpts {
            waiting_for,
            ..SessionOpts::default()
        },
    );
}

#[test]
fn agents_count_three() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            ..Default::default()
        },
    );
    let agent = |id: &str, tool: &str, d: &str| {
        format!(
            "{}\n",
            tool_use_line(id, tool, json!({"description": d}), false)
        )
    };
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[
            agent("t1", "Agent", "review A"),
            agent("t2", "Agent", "review B"),
            agent("t3", "Task", "review C"),
        ],
    );
    status_file(
        dir.path(),
        "a",
        "busy",
        None,
        "talo6-dct-frontend-d7",
        FRONTEND,
        90.0,
    );
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions.first().map(|s| s.agents.len()), Some(3));
}

#[test]
fn background_agents_two() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            ..Default::default()
        },
    );
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[
            user_line("hi", false, None),
            assistant_line(Some("Đã sửa xong, đang chờ agent nền chạy test."), false),
            turn_duration_line(Some(2), None, false, None),
            String::new(),
        ],
    );
    status_file(
        dir.path(),
        "a",
        "busy",
        None,
        "talo6-dct-frontend-d7",
        FRONTEND,
        90.0,
    );
    store.refresh(SystemTime::now());
    assert_eq!(
        store.sessions.first().and_then(|s| s.background_agents),
        Some(2)
    );
}

#[test]
fn finished_collapsed_not_expanded() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let panel = PanelState::new();
    let t0 = SystemTime::now();
    status_file(
        dir.path(),
        "a",
        "busy",
        None,
        "talo6-dct-frontend-d7",
        FRONTEND,
        30.0,
    );
    status_file(
        dir.path(),
        "b",
        "busy",
        None,
        "talo6-dct-backend-3f",
        BACKEND,
        30.0,
    );
    status_file(
        dir.path(),
        "c",
        "busy",
        None,
        "ocr-service-a1",
        "/Users/phong/ocr-service",
        30.0,
    );
    store.refresh(t0);
    status_file(
        dir.path(),
        "a",
        "idle",
        None,
        "talo6-dct-frontend-d7",
        FRONTEND,
        0.0,
    );
    status_file(
        dir.path(),
        "b",
        "waiting",
        Some("input needed"),
        "talo6-dct-backend-3f",
        BACKEND,
        0.0,
    );
    store.refresh(secs(t0, 1));
    store.refresh(secs(t0, 10));
    assert!(!panel.expanded(store.peeking()));
}

const BACKEND_PR_URL: &str =
    "https://bitbucket.org/t6engineering/talo6-dct-backend/pull-requests/7583";
const C1: &str = "2026-11-03T03:40:00.000000+00:00";
const C2: &str = "2026-11-03T03:41:00.000000+00:00";
const C3: &str = "2026-11-03T03:42:00.000000+00:00";

struct PrRow {
    store: Store,
    _api: Arc<FakeBitbucket>,
    _dir: tempfile::TempDir,
    _projects: tempfile::TempDir,
}

/// One idle session `b` with PR 7583. `reviewed`: CodeRabbit finished its review; `comments`: the PR's comments, `None` when that fetch fails.
fn pr_row_store(reviewed: bool, comments: Option<Vec<Value>>) -> PrRow {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let open = if reviewed {
        pr_json(PrSpec {
            id: 7583,
            comments: 3,
            rabbit_approved: Some(false),
            ..Default::default()
        })
    } else {
        pr_json(PrSpec {
            id: 7583,
            ..Default::default()
        })
    };
    let api = fake(vec![open]);
    let statuses = if reviewed {
        vec![rabbit_status(
            "SUCCESSFUL",
            "Review completed",
            "abc123",
            "2026-11-03T03:33:15.111541+00:00",
            None,
        )]
    } else {
        vec![]
    };
    api.set("/statuses", json!({ "values": statuses }));
    match comments {
        Some(c) => api.set("/comments", json!({ "values": c })),
        None => api.set_error("/comments", BitbucketError::Transport),
    }
    write_transcript(
        projects.path(),
        BACKEND,
        "b",
        &[
            user_line(&format!("Xem {BACKEND_PR_URL}"), false, None),
            assistant_line(Some(&format!("Đã push, xem {BACKEND_PR_URL}")), false),
            String::new(),
        ],
    );
    status_file(
        dir.path(),
        "b",
        "idle",
        None,
        "talo6-dct-backend-3f",
        BACKEND,
        30.0,
    );
    let store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            prs: Some(PRMonitor::with_options(
                api.clone(),
                false,
                Arc::new(|_| None),
                45.0,
            )),
            ..Default::default()
        },
    );
    PrRow {
        store,
        _api: api,
        _dir: dir,
        _projects: projects,
    }
}

fn pr_of_b(store: &Store) -> claude_sessions_core::pull_requests::PRStatus {
    store.sessions.iter().find(|s| s.id == "b").unwrap().prs[0].clone()
}

#[test]
fn pr_lines_rabbit_threads_2_of_2() {
    let mut r = pr_row_store(
        true,
        Some(vec![
            rabbit_comment("**Actionable comments posted: 1**", C1),
            rabbit_inline(1, "nit", C1, None),
            rabbit_inline(4, "nit 2", C2, None),
        ]),
    );
    r.store.refresh(SystemTime::now());
    let pr0 = pr_of_b(&r.store);
    assert_eq!(
        pr0.rabbit_threads,
        Some(RabbitThreads {
            pending: 2,
            total: 2
        })
    );
    assert!(pr_line(&pr0, SystemTime::now(), None).contains("còn 2/2"));
}

#[test]
fn ask_claude_shown_when_counts_unknown() {
    let mut r = pr_row_store(true, None);
    r.store.refresh(SystemTime::now());
    let pr0 = pr_of_b(&r.store);
    assert_eq!(pr0.rabbit_threads, None);
    assert_eq!(pr0.rabbit, Some(Rabbit::Reviewed(None)));
    assert!(!code_rabbit::hides_ask_claude(
        pr0.rabbit.as_ref(),
        pr0.rabbit_threads,
        pr0.rabbit_others_pending
    ));
}

#[test]
fn code_rabbit_hides_ask_claude() {
    let mut r = pr_row_store(
        true,
        Some(vec![
            rabbit_comment("**Actionable comments posted: 1**", C1),
            rabbit_inline(1, "nit", C1, None),
            human_reply(2, Some(1), "{me}", "won't fix", C2, true),
            rabbit_inline(3, "withdrawn", C3, Some(1)),
        ]),
    );
    r.store.refresh(SystemTime::now());
    let pr0 = pr_of_b(&r.store);
    assert_eq!(
        pr0.rabbit_threads,
        Some(RabbitThreads {
            pending: 0,
            total: 1
        })
    );
    assert!(code_rabbit::hides_ask_claude(
        pr0.rabbit.as_ref(),
        pr0.rabbit_threads,
        pr0.rabbit_others_pending
    ));
}

fn icon(name: &str) -> Url {
    Url::parse(&format!("https://talo6.atlassian.net/icon/{name}")).unwrap()
}

struct LinkRig {
    store: Store,
    _dir: tempfile::TempDir,
    _projects: tempfile::TempDir,
}

/// One session `b` mentioning eight tickets and PRs; icons are put straight into the store.
fn link_store(connected: bool) -> LinkRig {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let urls = (101..=108)
        .map(|n| format!("https://talo6.atlassian.net/browse/T6IR-{n}"))
        .collect::<Vec<_>>()
        .join(" ");
    let prs = "https://github.com/a/b/pull/1 https://github.com/a/b/pull/2";
    write_transcript(
        projects.path(),
        BACKEND,
        "b",
        &[
            user_line(&urls, false, None),
            assistant_line(Some(prs), false),
            String::new(),
        ],
    );
    status_file(
        dir.path(),
        "b",
        "idle",
        None,
        "talo6-dct-backend-3f",
        BACKEND,
        30.0,
    );
    let api = Arc::new(FakeJira::default());
    let long = "Very long ticket title that overflows the row ".repeat(3);
    type Spec<'a> = (
        &'a str,
        String,
        Option<&'a str>,
        Option<&'a str>,
        Option<(&'a str, &'a str)>,
    );
    let spec: Vec<Spec> = vec![
        (
            "T6IR-101",
            "Add retry to the sync worker".into(),
            Some("To Do"),
            Some("new"),
            Some(("Bug", "bug")),
        ),
        (
            "T6IR-102",
            "Fix site scope filter on the dashboard".into(),
            Some("In Progress"),
            Some("indeterminate"),
            None,
        ),
        (
            "T6IR-103",
            "Remove the legacy export endpoint".into(),
            Some("Done"),
            Some("done"),
            Some(("Task", "task")),
        ),
        (
            "T6IR-104",
            "Cache the permission lookups".into(),
            Some("Ready for Grooming with Engineering"),
            Some("new"),
            Some(("Bug", "bug")),
        ),
        (
            "T6IR-105",
            "Odd workflow status".into(),
            Some("Blocked"),
            Some("weird"),
            Some(("Story", "story")),
        ),
        (
            "T6IR-106",
            "Ticket with a status and no category".into(),
            Some("Review"),
            None,
            Some(("Task", "task")),
        ),
        (
            "T6IR-107",
            "Ticket whose icon never arrived".into(),
            Some("To Do"),
            Some("new"),
            None,
        ),
        (
            "T6IR-108",
            long,
            Some("To Do"),
            Some("new"),
            Some(("Task", "task")),
        ),
    ];
    for (k, title, status, cat, ty) in spec {
        api.issues
            .lock()
            .unwrap()
            .insert(k.into(), (title, status.map(String::from)));
        if let Some(c) = cat {
            api.categories.lock().unwrap().insert(k.into(), c.into());
        }
        if let Some((name, i)) = ty {
            api.types
                .lock()
                .unwrap()
                .insert(k.into(), (name.into(), icon(i).to_string()));
        }
    }
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            jira: connected.then(|| JiraMonitor::new(api.clone(), false)),
            ..Default::default()
        },
    );
    store
        .jira_projects
        .insert("T6IR".into(), "talo6.atlassian.net".into());
    store.refresh(SystemTime::now());
    if connected {
        // Deferred to phase 6: the icon images (`issue_icons` is private in Rust; no rule here reads them).
        // Bitbucket is not faked here: swap the untracked PR links for one PR in each state.
        let pr = |id: i64, status: &str, tone: ItemTone| LinkItem {
            text: format!("PR #{id}"),
            url: Url::parse(&format!("https://github.com/a/b/pull/{id}")).unwrap(),
            kind: LinkKind::Pr,
            code: format!("#{id}"),
            title: Some(format!("Rework the {status} pull request")),
            status: Some(status.into()),
            tone: Some(tone),
            icon_url: None,
            help: None,
        };
        let i = store.sessions[0]
            .menu
            .iter()
            .position(|m| m.title == "PR")
            .unwrap();
        store.sessions[0].menu[i] = LinkSection {
            title: "PR".into(),
            items: vec![
                pr(41, "mở", ItemTone::Open),
                pr(42, "đã merge", ItemTone::Merged),
                pr(43, "đã đóng", ItemTone::Closed),
            ],
        };
    }
    LinkRig {
        store,
        _dir: dir,
        _projects: projects,
    }
}

#[test]
fn link_menu_row_menu_count_11() {
    let r = link_store(true);
    assert_eq!(r.store.sessions.first().map(|s| s.menu_count()), Some(11));
}

#[test]
fn link_list_open_menu_titles() {
    let r = link_store(true);
    let s = &r.store.sessions[0];
    let mut panel = PanelState::new();
    panel.open_menus.insert(s.id.clone());
    assert_eq!(s.menu_count(), 11);
    let titles: Vec<&str> = s.menu.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(titles, vec!["Bạn nhắc", "PR"]);
    assert_eq!(
        s.menu.iter().map(|m| m.items.len()).sum::<usize>(),
        s.menu_count()
    );
}

#[test]
fn link_list_connect_needs_jira() {
    let r = link_store(false);
    assert!(r.store.sessions[0].menu_needs_jira);
}

#[test]
fn pr_row_shows_sent_state() {
    let mut r = pr_row_store(false, Some(vec![]));
    let t0 = SystemTime::now();
    r.store.refresh(t0);
    let ask = SendKey {
        pr: Some(PRRef {
            workspace: "t6engineering".into(),
            repo: "talo6-dct-backend".into(),
            id: 7583,
        }),
        ..SendKey::new("b", SendKind::AskClaude)
    };
    assert!(r.store.begin_send(ask.clone(), t0));
    r.store.refresh(secs(t0, 1));
    assert_eq!(r.store.send_state(&ask), Some(SendState::Sending));
    r.store.finish_send(&ask, true, secs(t0, 1));
    r.store.refresh(secs(t0, 2));
    assert_eq!(r.store.send_state(&ask), Some(SendState::Sent));
    r.store.finish_send(&ask, false, secs(t0, 2));
    assert_eq!(r.store.send_state(&ask), Some(SendState::Failed));
}

#[test]
fn approval_replaced_by_sent_line() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    status_file(
        dir.path(),
        "a",
        "busy",
        None,
        "talo6-dct-frontend-d7",
        FRONTEND,
        5.0,
    );
    let t0 = SystemTime::now();
    store.refresh(t0);
    let k = SendKey::new("a", SendKind::Approve);
    store.finish_send(&k, true, t0);
    assert_eq!(store.send_state(&k), Some(SendState::Sent));
}

fn closed_two(now: SystemTime) -> ClosedSessions {
    ClosedSessions::new(vec![
        ClosedSession {
            id: "x".into(),
            title: "Sửa lỗi đăng nhập".into(),
            cwd: BACKEND.into(),
            branch: Some("feat/login".into()),
            closed_at: now - Duration::from_secs(12 * 60),
        },
        ClosedSession {
            id: "y".into(),
            title: "ocr-service-a1".into(),
            cwd: "/Users/phong/ocr-service".into(),
            branch: None,
            closed_at: now - Duration::from_secs(3 * 3600),
        },
    ])
}

#[test]
fn closed_row_shows_opening() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let now = SystemTime::now();
    store.refresh(now);
    store.closed = closed_two(now);
    assert!(store.begin_send(SendKey::new("x", SendKind::Resume), now));
    store.finish_send(&SendKey::new("y", SendKind::Resume), false, now);
    assert_eq!(
        store.send_state(&SendKey::new("x", SendKind::Resume)),
        Some(SendState::Sending)
    );
    assert_eq!(
        store.send_state(&SendKey::new("y", SendKind::Resume)),
        Some(SendState::Failed)
    );
}

#[test]
fn quota_pill_percent_85() {
    let now = SystemTime::now();
    let quota = |five, weekly| Quota {
        five_hour: five,
        five_hour_reset: secs(now, 2 * 3600),
        weekly,
        weekly_reset: secs(now, 5 * 86400),
        forecast: None,
    };
    assert_eq!(quota(85, 10).pill_percent(), Some(85));
    assert_eq!(quota(30, 10).pill_percent(), None);
    // Deferred to phase 6: the collapsed width (`Layout.size`) grows by `quotaPillExtra` while the pill shows a percent.
}

#[test]
fn question_not_live_after_item_leaves() {
    let dir = make_temp_dir();
    let t = FakeFeedTransport::new();
    *t.items.lock().unwrap() = vec![feed_item("R1", "a", SystemTime::now())];
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            feed: Some(FeedMonitor::without_worker(t.clone())),
            ..Default::default()
        },
    );
    let now = SystemTime::now();
    status_file(
        dir.path(),
        "a",
        "waiting",
        Some("input needed"),
        "talo6-dct-backend-3f",
        BACKEND,
        5.0,
    );
    status_file(
        dir.path(),
        "b",
        "busy",
        None,
        "talo6-dct-frontend-d7",
        FRONTEND,
        42.0,
    );
    store.refresh(now);
    let session = |s: &Store| s.sessions.iter().find(|x| x.id == "a").unwrap().clone();
    assert!(session(&store).question_live);
    *t.items.lock().unwrap() = vec![feed_item("R2", "a", now)];
    store.refresh(secs(now, 1));
    store.note_sent("a", "R2");
    t.items.lock().unwrap().clear();
    store.sent.clear();
    store.refresh(secs(now, 100));
    assert!(!session(&store).question_live);
}

#[test]
fn open_menus_cleared_on_collapse() {
    let r = link_store(true);
    let mut panel = PanelState::new();
    panel.open_menus.insert(r.store.sessions[0].id.clone());
    panel.hovering = false;
    panel.on_collapse();
    assert!(panel.open_menus.is_empty());
    // Deferred to phase 6: the WidgetView re-layout that calls `on_collapse` when hovering ends.
}
