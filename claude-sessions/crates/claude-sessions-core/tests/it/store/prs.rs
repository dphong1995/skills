//! Swift `StoreTests.swift` pull-request, link-menu and connection tests (`prsAttachedToSession` through `connectionTexts`).

use super::*;
use claude_sessions_core::bitbucket::BitbucketError;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::jira::JiraError;
use claude_sessions_core::jira_monitor::JiraMonitor;
use claude_sessions_core::pull_requests::{self, PRRef};
use claude_sessions_core::store::{
    ItemTone, LinkKind, PR_LIST_MAX, Session, display_status, link_text, pr_item,
};
use serde_json::{Value, json};

fn pr_ref(id: i64) -> PRRef {
    PRRef {
        workspace: "t6engineering".into(),
        repo: "talo6-dct-backend".into(),
        id,
    }
}

fn at(s: u64) -> SystemTime {
    secs(pr_t0(), s)
}

fn pr_reply_extra(ids: &[i64], extra: &str) -> String {
    let links: Vec<String> = ids
        .iter()
        .map(|i| format!("https://bitbucket.org/{PR_REPO}/pull-requests/{i}"))
        .collect();
    assistant_line(Some(&format!("{}{extra}", links.join(" "))), false)
}

fn plain_pr(id: i64) -> Value {
    pr_json(PrSpec {
        id,
        ..Default::default()
    })
}

fn pr_with_reviewer(id: i64, approved: bool, comments: i64) -> Value {
    let mut o = pr_json(PrSpec {
        id,
        comments,
        ..Default::default()
    });
    o["participants"] = json!([{"role": "REVIEWER", "approved": approved,
        "state": if approved { json!("approved") } else { Value::Null }, "user": {"uuid": "{r}"}}]);
    o
}

fn set_open(api: &FakeBitbucket, prs: Vec<Value>) {
    api.set(
        "/2.0/workspaces/t6engineering/pullrequests/",
        json!({"values": prs}),
    );
}

fn tin_comment(api: &FakeBitbucket, when: &str) {
    api.set(
        "/comments",
        json!({"values": [comment("{tin}", "Tin Tran", when)]}),
    );
}

const COMMENT_AT: &str = "2026-11-04T10:00:00.000000+00:00";

fn unseen_keys(store: &Store) -> HashSet<String> {
    store.unseen.keys().cloned().collect()
}

#[test]
fn prs_attached_to_session() {
    let api = fake([7571, 6874, 1, 2].map(plain_pr).to_vec());
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&[7571, 6874, 1, 2]));
    store.refresh(pr_t0());
    let got: Vec<i64> = store.sessions[0].prs.iter().map(|p| p.ref_.id).collect();
    assert_eq!(got, vec![7571, 6874, 1]);
}

#[test]
fn pr_link_button_dropped() {
    let reply = pr_reply_extra(&[7571], " https://example.com/x");
    let api = fake(vec![plain_pr(7571)]);
    let (mut store, _d, _p) = pr_store(&api, reply.clone());
    store.refresh(pr_t0());
    assert!(store.sessions[0].links.is_empty());

    let failing = fake(vec![]);
    *failing.fail_all.lock().unwrap() = Some(BitbucketError::Transport);
    let (mut other, _d2, _p2) = pr_store(&failing, reply);
    other.refresh(pr_t0());
    assert!(other.sessions[0].prs.is_empty());
    assert_eq!(other.sessions[0].links.len(), 1);
}

#[test]
fn pr_event_chimes_and_peeks() {
    let api = fake(vec![pr_with_reviewer(7571, false, 0)]);
    let (mut store, dir, _p) = pr_store(&api, pr_reply(&[7571]));
    store.refresh(pr_t0());
    assert_eq!(store.chime, None);
    assert!(!store.peeking());

    set_open(&api, vec![pr_with_reviewer(7571, false, 1)]);
    tin_comment(&api, COMMENT_AT);
    store.refresh(at(61));
    assert_eq!(store.chime, Some(Chime::NeedsYou));
    assert!(store.peeking());

    set_open(&api, vec![pr_with_reviewer(7571, true, 1)]);
    store.refresh(at(122));
    assert_eq!(store.chime, Some(Chime::Done));

    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(at(130));
    set_open(&api, vec![pr_with_reviewer(7571, true, 2)]);
    tin_comment(&api, "2026-11-04T11:00:00.000000+00:00");
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    store.refresh(at(200));
    assert_eq!(store.chime, Some(Chime::NeedsYou));
}

#[test]
fn refresh_sets_pr_chime_for_pr_events_and_clears_it() {
    let api = fake(vec![pr_with_reviewer(7571, false, 0)]);
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&[7571]));
    store.refresh(pr_t0());
    assert_eq!(store.pr_chime, None);

    set_open(&api, vec![pr_with_reviewer(7571, false, 1)]);
    tin_comment(&api, COMMENT_AT);
    store.refresh(at(61));
    assert_eq!(store.pr_chime, Some(Chime::NeedsYou));

    store.refresh(at(62));
    assert_eq!(store.pr_chime, None);
}

#[test]
fn pr_stale_after_five_minutes() {
    let api = fake(vec![plain_pr(7571)]);
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&[7571]));
    store.refresh(pr_t0());
    assert_eq!(store.pr_last_success, Some(pr_t0()));
    *api.fail_all.lock().unwrap() = Some(BitbucketError::Transport);
    store.refresh(at(61));
    store.refresh(at(299));
    assert!(!store.pr_stale());
    store.refresh(at(301));
    assert!(store.pr_stale());
    assert_eq!(store.pr_last_success, Some(pr_t0()));
}

/// Opens a PR monitor with one reviewer-less PR `7571`, then bumps its comment count so the next refresh at `at(61)` is an event.
fn event_store() -> (Arc<FakeBitbucket>, Store, TempDir, TempDir) {
    let api = fake(vec![pr_with_reviewer(7571, false, 0)]);
    let (store, d, p) = pr_store(&api, pr_reply(&[7571]));
    (api, store, d, p)
}

fn trigger_event(api: &FakeBitbucket) {
    set_open(api, vec![pr_with_reviewer(7571, false, 1)]);
    tin_comment(api, COMMENT_AT);
}

#[test]
fn pr_event_marks_session() {
    let (api, mut store, _d, _p) = event_store();
    store.refresh(pr_t0());
    assert!(store.unseen.is_empty());
    trigger_event(&api);
    store.refresh(at(61));
    assert_eq!(unseen_keys(&store), set(["a"]));
}

#[test]
fn pr_event_marks_every_session_with_the_pr() {
    let (api, mut store, dir, projects) = event_store();
    write_transcript(
        projects.path(),
        BACKEND,
        "b",
        &[pr_reply(&[7571]), String::new()],
    );
    write_session(dir.path(), "b", me(), "idle", 0.0, "b", BACKEND);
    store.refresh(pr_t0());
    assert!(store.unseen.is_empty());
    trigger_event(&api);
    store.refresh(at(61));
    assert_eq!(unseen_keys(&store), set(["a", "b"]));
}

#[test]
fn pr_event_marks_busy_session() {
    let (api, mut store, dir, _p) = event_store();
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(pr_t0());
    assert!(store.unseen.is_empty());
    trigger_event(&api);
    store.refresh(at(61));
    assert_eq!(unseen_keys(&store), set(["a"]));
}

#[test]
fn pr_mark_survives_switch_to_busy() {
    let (api, mut store, dir, _p) = event_store();
    store.refresh(pr_t0());
    store.mark_seen("a");
    trigger_event(&api);
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(at(61));
    assert_eq!(unseen_keys(&store), set(["a"]));
    store.refresh(at(62));
    assert_eq!(unseen_keys(&store), set(["a"]));
}

#[test]
fn pr_event_marks_only_that_pr() {
    let api = fake(vec![
        pr_with_reviewer(7571, false, 0),
        pr_with_reviewer(7572, false, 0),
    ]);
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&[7571, 7572]));
    store.refresh(pr_t0());
    assert!(store.unseen.is_empty());
    set_open(
        &api,
        vec![
            pr_with_reviewer(7571, false, 0),
            pr_with_reviewer(7572, false, 1),
        ],
    );
    tin_comment(&api, COMMENT_AT);
    store.refresh(at(61));
    assert_eq!(
        store.unseen.get("a"),
        Some(&NewMark {
            claude: false,
            prs: HashSet::from([pr_ref(7572)])
        })
    );
}

#[test]
fn claude_and_pr_mark_together() {
    let (api, mut store, dir, _p) = event_store();
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(pr_t0());
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    trigger_event(&api);
    store.refresh(at(61));
    assert_eq!(
        store.unseen.get("a"),
        Some(&NewMark {
            claude: true,
            prs: HashSet::from([pr_ref(7571)])
        })
    );
}

#[test]
fn claude_only_mark() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    write_session(dir.path(), "a", me(), "busy", 5.0, "a", FRONTEND);
    store.refresh(t0);
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    store.refresh(secs(t0, 1));
    assert_eq!(
        store.unseen.get("a"),
        Some(&NewMark {
            claude: true,
            ..NewMark::default()
        })
    );
}

#[test]
fn pr_mark_dropped_when_pr_gone() {
    let (_api, mut store, _d, _p) = event_store();
    store.refresh(pr_t0());
    store.unseen.insert(
        "a".into(),
        NewMark {
            prs: HashSet::from([pr_ref(9999)]),
            ..NewMark::default()
        },
    );
    store.refresh(at(1));
    assert!(!store.unseen.contains_key("a"));
    assert_eq!(store.new_count(), 0);
}

#[test]
fn pr_mark_on_hidden_fourth_pr() {
    let ids = [7571, 7572, 7573, 7574];
    let api = fake(ids.map(|i| pr_with_reviewer(i, false, 0)).to_vec());
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&ids));
    store.refresh(pr_t0());
    assert!(store.unseen.is_empty());
    set_open(
        &api,
        ids.map(|i| pr_with_reviewer(i, false, i64::from(i == 7574)))
            .to_vec(),
    );
    tin_comment(&api, COMMENT_AT);
    store.refresh(at(61));
    assert_eq!(store.unseen["a"].prs, HashSet::from([pr_ref(7574)]));
    assert!(!store.sessions[0].prs.iter().any(|p| p.ref_ == pr_ref(7574)));
}

#[test]
fn claude_mark_survives_pr_prune() {
    let (_api, mut store, _d, _p) = event_store();
    store.refresh(pr_t0());
    store.unseen.insert(
        "a".into(),
        NewMark {
            claude: true,
            prs: HashSet::from([pr_ref(9999)]),
        },
    );
    store.refresh(at(1));
    assert_eq!(
        store.unseen.get("a"),
        Some(&NewMark {
            claude: true,
            ..NewMark::default()
        })
    );
}

#[test]
fn pr_mark_cleared_on_switch_to_busy() {
    let (_api, mut store, dir, _p) = event_store();
    store.refresh(pr_t0());
    store.unseen.insert(
        "a".into(),
        NewMark {
            prs: HashSet::from([pr_ref(7571)]),
            ..NewMark::default()
        },
    );
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(at(1));
    assert!(!store.unseen.contains_key("a"));
}

// ---- Link menu ----

const MENU_TITLE: &str = "Export CSV thiếu cột site";

fn menu_pr(id: i64, title: &str) -> Value {
    let mut o = plain_pr(id);
    o["title"] = json!(title);
    o
}

struct MenuRig {
    store: Store,
    jira: Arc<FakeJira>,
    _api: Arc<FakeBitbucket>,
    _dir: TempDir,
    _projects: TempDir,
}

/// One idle session `a`: the user mentions T6IR-2 then T6IR-3, Claude T6IR-4 and four PRs.
fn menu_store(extra: &[String], jira: Option<Arc<FakeJira>>, prs: bool) -> MenuRig {
    menu_store_with_prs(&[7571, 6874, 1, 2], extra, jira, prs)
}

/// `menu_store` with the PR ids Claude links (and Bitbucket answers for) chosen by the test.
fn menu_store_with_prs(
    ids: &[i64],
    extra: &[String],
    jira: Option<Arc<FakeJira>>,
    prs: bool,
) -> MenuRig {
    let fake_jira = jira.unwrap_or_else(|| {
        let j = Arc::new(FakeJira::default());
        j.issues
            .lock()
            .unwrap()
            .insert("T6IR-2".into(), (MENU_TITLE.into(), Some("To Do".into())));
        j
    });
    let api = fake(
        ids.iter()
            .map(|&id| menu_pr(id, &format!("PR {id}")))
            .collect(),
    );
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut lines = vec![
        user_line("T6IR-2", false, None),
        user_line("T6IR-3", false, None),
        assistant_line(Some("T6IR-4"), false),
        pr_reply(ids),
    ];
    lines.extend(extra.iter().cloned());
    lines.push(String::new());
    write_transcript(projects.path(), FRONTEND, "a", &lines);
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            prs: prs.then(|| PRMonitor::with_options(api.clone(), false, Arc::new(|_| None), 45.0)),
            jira: Some(JiraMonitor::new(fake_jira.clone(), false)),
            ..Default::default()
        },
    );
    store.jira_projects.insert("T6IR".into(), JIRA_HOST.into());
    store.refresh(pr_t0());
    MenuRig {
        store,
        jira: fake_jira,
        _api: api,
        _dir: dir,
        _projects: projects,
    }
}

fn section(s: &Session, title: &str) -> Option<Vec<String>> {
    s.menu
        .iter()
        .find(|m| m.title == title)
        .map(|m| m.items.iter().map(|i| i.text.clone()).collect())
}

fn failing_jira(e: JiraError) -> Arc<FakeJira> {
    let j = Arc::new(FakeJira::default());
    *j.fail.lock().unwrap() = Some(e);
    j
}

#[test]
fn menu_sections_in_order() {
    let r = menu_store(&[], None, true);
    let s = &r.store.sessions[0];
    let titles: Vec<&str> = s.menu.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(titles, vec!["Bạn nhắc", "Claude nhắc", "PR"]);
    let last = |m: &claude_sessions_core::store::LinkSection| -> Vec<String> {
        m.items
            .iter()
            .map(|i| {
                i.url
                    .path_segments()
                    .unwrap()
                    .next_back()
                    .unwrap()
                    .to_string()
            })
            .collect()
    };
    assert_eq!(last(&s.menu[0]), vec!["T6IR-2", "T6IR-3"]);
    assert_eq!(last(&s.menu[1]), vec!["T6IR-4"]);
    assert_eq!(
        section(s, "Bạn nhắc"),
        Some(vec![
            format!("T6IR-2  {MENU_TITLE}  ·  To Do"),
            "T6IR-3".to_string()
        ])
    );
}

fn bare_status(state: &str, title: Option<&str>) -> pull_requests::PRStatus {
    let mut o = plain_pr(5);
    o["state"] = json!(state);
    match title {
        Some(t) => o["title"] = json!(t),
        None => {
            o.as_object_mut().unwrap().remove("title");
        }
    }
    pull_requests::status_from(&o).unwrap()
}

#[test]
fn menu_lists_the_prs_past_the_first_three() {
    let r = menu_store(&[], None, true);
    let s = &r.store.sessions[0];
    assert_eq!(s.prs.len(), 3);
    let prs = section(s, "PR").unwrap();
    assert_eq!(prs, vec!["PR #2  PR 2  ·  mở"]);
    assert_eq!(
        link_text(&bare_status("MERGED", Some("Done"))),
        "PR #5  Done  ·  đã merge"
    );
    assert_eq!(
        link_text(&bare_status("DECLINED", None)),
        "PR #5  ·  đã đóng"
    );
}

#[test]
fn menu_lists_only_prs_past_the_pr_list() {
    let r = menu_store_with_prs(
        &[7571, 6874, 1, 2, 3],
        &[assistant_line(
            Some("https://github.com/a/b/pull/12"),
            false,
        )],
        None,
        true,
    );
    let s = &r.store.sessions[0];
    assert_eq!(PR_LIST_MAX, 3);
    assert_eq!(s.prs.len(), PR_LIST_MAX);
    assert!(section(s, "Bạn nhắc").is_some());
    // PRs 4 and 5 stay, the untracked link too; the three above are gone.
    assert_eq!(
        section(s, "PR"),
        Some(vec![
            "PR #2  PR 2  ·  mở".to_string(),
            "PR #3  PR 3  ·  mở".to_string(),
            "PR #12".to_string(),
        ])
    );
    // 2 tickets by the user, 1 by Claude, 3 PR rows.
    assert_eq!(s.menu_count(), 6);
}

#[test]
fn prs_shown_above_leave_no_menu() {
    let api = fake([7571, 6874].map(plain_pr).to_vec());
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&[7571, 6874]));
    store.refresh(pr_t0());
    let s = &store.sessions[0];
    assert_eq!(s.prs.len(), 2);
    assert!(s.menu.is_empty());
    assert_eq!(s.menu_count(), 0);
}

#[test]
fn display_status_sentence_cases_all_caps() {
    assert_eq!(display_status("CONFIRMING"), "Confirming");
    assert_eq!(display_status("IN PROGRESS"), "In progress");
    assert_eq!(display_status("QA Verified"), "QA Verified");
    assert_eq!(display_status("Done"), "Done");
    assert_eq!(display_status(""), "");
    assert_eq!(display_status("123"), "123");
    assert_eq!(display_status("(QA) DONE"), "(Qa) done");
}

#[test]
fn display_status_handles_non_ascii_capitals() {
    assert_eq!(display_status("ĐANG LÀM"), "Đang làm");
    assert_eq!(display_status("Đang làm"), "Đang làm");
    assert_eq!(display_status("ĐANG LÀM 2"), "Đang làm 2");
    assert_eq!(display_status("IN-PROGRESS (QA)"), "In-progress (qa)");
}

#[test]
fn ticket_item_status_is_sentence_cased_but_help_keeps_jira_text() {
    let r = menu_store(&[], Some(jira_with("Title", Some("IN PROGRESS"))), true);
    let item = &r.store.sessions[0].menu[0].items[0];
    assert_eq!(item.status.as_deref(), Some("In progress"));
    assert_eq!(item.help.as_deref(), Some("T6IR-2 · Title · IN PROGRESS"));
    assert_eq!(item.text, "T6IR-2  Title  ·  IN PROGRESS");
}

#[test]
fn menu_untracked_pr_link() {
    let r = menu_store(
        &[assistant_line(
            Some("https://github.com/a/b/pull/12"),
            false,
        )],
        None,
        true,
    );
    assert_eq!(
        section(&r.store.sessions[0], "PR")
            .unwrap()
            .last()
            .map(String::as_str),
        Some("PR #12")
    );
}

fn jira_with(title: &str, status: Option<&str>) -> Arc<FakeJira> {
    let j = Arc::new(FakeJira::default());
    j.issues
        .lock()
        .unwrap()
        .insert("T6IR-2".into(), (title.into(), status.map(String::from)));
    j
}

#[test]
fn menu_title_cut_at_60() {
    let r = menu_store(&[], Some(jira_with(&"x".repeat(70), Some("Done"))), true);
    assert_eq!(
        section(&r.store.sessions[0], "Bạn nhắc").unwrap()[0],
        format!("T6IR-2  {}…  ·  Done", "x".repeat(60))
    );
}

#[test]
fn menu_title_without_status() {
    let r = menu_store(&[], Some(jira_with("Only title", None)), true);
    assert_eq!(
        section(&r.store.sessions[0], "Bạn nhắc").unwrap()[0],
        "T6IR-2  Only title"
    );
}

fn icon_url() -> url::Url {
    url::Url::parse("https://talosix.atlassian.net/avatar/1?size=medium").unwrap()
}

fn typed_jira() -> Arc<FakeJira> {
    let j = jira_with(MENU_TITLE, Some("Done"));
    j.types
        .lock()
        .unwrap()
        .insert("T6IR-2".into(), ("Bug".into(), icon_url().to_string()));
    j.categories
        .lock()
        .unwrap()
        .insert("T6IR-2".into(), "done".into());
    j
}

#[test]
fn menu_items_carry_ticket_fields() {
    let r = menu_store(&[], Some(typed_jira()), true);
    let item = &r.store.sessions[0]
        .menu
        .iter()
        .find(|m| m.title == "Bạn nhắc")
        .unwrap()
        .items[0];
    assert_eq!(item.kind, LinkKind::Ticket);
    assert_eq!(item.code, "T6IR-2");
    assert_eq!(item.title.as_deref(), Some(MENU_TITLE));
    assert_eq!(item.status.as_deref(), Some("Done"));
    assert_eq!(item.tone, Some(ItemTone::Done));
    assert_eq!(item.icon_url, Some(icon_url()));
    assert_eq!(
        item.help.as_deref(),
        Some(format!("T6IR-2 · {MENU_TITLE} · Done").as_str())
    );
}

#[test]
fn menu_items_carry_pr_fields() {
    let r = menu_store(
        &[assistant_line(
            Some("https://github.com/a/b/pull/12"),
            false,
        )],
        None,
        true,
    );
    let items = &r.store.sessions[0]
        .menu
        .iter()
        .find(|m| m.title == "PR")
        .unwrap()
        .items;
    assert_eq!(items[0].kind, LinkKind::Pr);
    assert_eq!(items[0].code, "#2");
    assert_eq!(items[0].status.as_deref(), Some("mở"));
    assert_eq!(items[0].tone, Some(ItemTone::Open));
    assert_eq!(items[0].help.as_deref(), Some("PR 2"));
    let last = items.last().unwrap();
    assert_eq!(last.kind, LinkKind::Link);
    assert_eq!(last.code, "PR #12");
    assert_eq!(last.tone, None);
    let merged = pr_item(&bare_status("MERGED", Some("T")));
    assert_eq!(merged.code, "#5");
    assert_eq!(merged.status.as_deref(), Some("đã merge"));
    assert_eq!(merged.tone, Some(ItemTone::Merged));
}

#[test]
fn session_link_carries_icon_and_tone() {
    let r = menu_store(&[], Some(typed_jira()), true);
    let link = r.store.sessions[0]
        .links
        .iter()
        .find(|l| l.label == "T6IR-2")
        .unwrap();
    assert_eq!(link.icon_url, Some(icon_url()));
    assert_eq!(link.tone, Some(ItemTone::Done));

    let bare = menu_store(&[], Some(failing_jira(JiraError::Transport)), true);
    let none = bare.store.sessions[0]
        .links
        .iter()
        .find(|l| l.label == "T6IR-2")
        .unwrap();
    assert!(none.icon_url.is_none() && none.tone.is_none() && none.help.is_none());
}

/// Swift `storeDecodesIssueIcons`: the core keeps raw icon bytes (the macos crate decodes them), once per URL.
#[test]
fn store_keeps_issue_icon_bytes_once_per_url() {
    let jira = typed_jira();
    jira.icon_data
        .lock()
        .unwrap()
        .insert(icon_url(), vec![1, 2, 3]);
    let mut r = menu_store(&[], Some(jira.clone()), true);
    assert_eq!(
        r.store.snapshot().issue_icons.get(&icon_url()),
        Some(&vec![1, 2, 3])
    );

    // A later different image for the same URL is ignored.
    jira.icon_data
        .lock()
        .unwrap()
        .insert(icon_url(), vec![9, 9]);
    r.store.refresh(at(1000));
    assert_eq!(
        r.store.snapshot().issue_icons.get(&icon_url()),
        Some(&vec![1, 2, 3])
    );

    // A failed icon fetch leaves nothing behind.
    let none = menu_store(&[], Some(typed_jira()), true);
    assert!(!none.store.snapshot().issue_icons.contains_key(&icon_url()));
}

#[test]
fn menu_needs_jira_when_not_ok() {
    let r = menu_store(&[], None, true);
    assert!(!r.store.sessions[0].menu_needs_jira);

    let rejected = menu_store(&[], Some(failing_jira(JiraError::Http(401))), true);
    assert!(rejected.store.sessions[0].menu_needs_jira);

    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[user_line("T6IR-2", false, None), String::new()],
    );
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    let mut bare = with_projects(dir.path(), projects.path());
    bare.jira_projects.insert("T6IR".into(), JIRA_HOST.into());
    bare.refresh(pr_t0());
    assert!(bare.jira_monitor.is_none());
    assert!(bare.sessions[0].menu_needs_jira);

    let (mut no_tickets, _d, _p) = pr_store(&fake(vec![plain_pr(7571)]), pr_reply(&[7571]));
    no_tickets.refresh(pr_t0());
    assert!(!no_tickets.sessions[0].menu_needs_jira);
}

#[test]
fn menu_count_excludes_footer() {
    let r = menu_store(&[], Some(failing_jira(JiraError::Transport)), true);
    let s = &r.store.sessions[0];
    assert!(s.menu_needs_jira);
    // Two user tickets, one by Claude, and the fourth PR (the first three are in the PR list).
    assert_eq!(s.menu_count(), 2 + 1 + 1);
}

#[test]
fn menu_row_for_pr_only_session() {
    let ids = [7571, 6874, 1, 2];
    let (mut store, _d, _p) = pr_store(&fake(ids.map(plain_pr).to_vec()), pr_reply(&ids));
    store.refresh(pr_t0());
    let s = &store.sessions[0];
    // Only the fourth PR's button is left; the first three are in the PR list.
    assert_eq!(s.links.len(), 1);
    assert_eq!(s.prs.len(), 3);
    let titles: Vec<&str> = s.menu.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(titles, vec!["PR"]);
    assert_eq!(s.menu_count(), 1);
}

#[test]
fn jira_targets_only_known_hosts() {
    let r = menu_store(&[user_line("X9-1 please", false, None)], None, true);
    let calls = r.jira.calls.lock().unwrap();
    assert!(!calls.is_empty());
    assert!(calls.iter().all(|(host, _)| host == JIRA_HOST));
    assert!(
        calls
            .iter()
            .flat_map(|(_, k)| k)
            .all(|k| !k.starts_with("X9"))
    );
}

#[test]
fn jira_targets_skip_foreign_hosts() {
    let jira = Arc::new(FakeJira::default());
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[user_line("T6IR-2", false, None), String::new()],
    );
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            jira: Some(JiraMonitor::new(jira.clone(), false)),
            ..Default::default()
        },
    );
    store
        .jira_projects
        .insert("T6IR".into(), "evil.example".into());
    store
        .accounts
        .insert(Service::Jira, "pluu@talosix.com".into());
    store.refresh(pr_t0());
    assert!(jira.calls.lock().unwrap().is_empty());
    assert_eq!(
        store.connection_text(Service::Jira),
        "Jira: ✗ chưa biết site Jira"
    );
}

#[test]
fn learned_atlassian_host_replaces_foreign_one() {
    let learn = |stored: &str| -> Option<String> {
        let (dir, projects) = (make_temp_dir(), make_temp_dir());
        write_transcript(
            projects.path(),
            FRONTEND,
            "a",
            &[
                user_line("https://talosix.atlassian.net/browse/T6IR-2", false, None),
                String::new(),
            ],
        );
        write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
        let mut store = with_projects(dir.path(), projects.path());
        store.jira_projects.insert("T6IR".into(), stored.into());
        store.refresh(pr_t0());
        store.jira_projects.get("T6IR").cloned()
    };
    assert_eq!(
        learn("evil.example").as_deref(),
        Some("talosix.atlassian.net")
    );
    assert_eq!(
        learn("other.atlassian.net").as_deref(),
        Some("other.atlassian.net")
    );
}

#[test]
fn ticket_button_help() {
    let r = menu_store(&[], None, true);
    let links = &r.store.sessions[0].links;
    let help = |label: &str| {
        links
            .iter()
            .find(|l| l.label == label)
            .unwrap()
            .help
            .clone()
    };
    assert_eq!(
        help("T6IR-2").as_deref(),
        Some(format!("T6IR-2 · {MENU_TITLE} · To Do").as_str())
    );
    assert_eq!(help("T6IR-3"), None);
}

#[test]
fn connection_texts() {
    let email = "pluu@talosix.com";
    let bare = menu_store(&[], None, false);
    assert_eq!(
        bare.store.connection_text(Service::Bitbucket),
        "Bitbucket: ✗ chưa có token"
    );

    let api = fake(vec![plain_pr(7571)]);
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&[7571]));
    store.accounts.insert(Service::Bitbucket, email.into());
    // No refresh yet.
    assert_eq!(
        store.connection_text(Service::Bitbucket),
        "Bitbucket: ✗ chưa có token"
    );
    store.refresh(pr_t0());
    assert_eq!(
        store.connection_text(Service::Bitbucket),
        format!("Bitbucket: ✓ {email}")
    );

    let text = |e: BitbucketError| -> String {
        let f = fake(vec![plain_pr(7571)]);
        *f.fail_all.lock().unwrap() = Some(e);
        let (mut s, _d, _p) = pr_store(&f, pr_reply(&[7571]));
        s.accounts.insert(Service::Bitbucket, email.into());
        s.refresh(pr_t0());
        s.connection_text(Service::Bitbucket)
    };
    assert_eq!(
        text(BitbucketError::Http(401)),
        "Bitbucket: ✗ token bị từ chối (401)"
    );
    assert_eq!(
        text(BitbucketError::Transport),
        "Bitbucket: ✗ không kết nối được"
    );

    // Idle monitor that never called: still checking.
    let (mut idle, _d, _p) = pr_store(&fake(vec![]), assistant_line(Some("hi"), false));
    idle.accounts.insert(Service::Bitbucket, email.into());
    idle.refresh(pr_t0());
    assert_eq!(
        idle.connection_text(Service::Bitbucket),
        format!("Bitbucket: {email} (đang kiểm tra)")
    );

    let dir = make_temp_dir();
    let mut no_site = make_store(dir.path());
    no_site.accounts.insert(Service::Jira, email.into());
    no_site.refresh(pr_t0());
    assert_eq!(
        no_site.connection_text(Service::Jira),
        "Jira: ✗ chưa biết site Jira"
    );

    let mut with_jira = menu_store(&[], None, true);
    with_jira.store.accounts.insert(Service::Jira, email.into());
    with_jira.store.refresh(at(1));
    assert_eq!(
        with_jira.store.connection_text(Service::Jira),
        format!("Jira: ✓ {email}")
    );
    let mut no_token = menu_store(&[], None, false);
    no_token.store.jira_monitor = None;
    no_token.store.refresh(at(1));
    assert_eq!(
        no_token.store.connection_text(Service::Jira),
        "Jira: ✗ chưa có token"
    );
}

/// A successful poll that changes nothing visible leaves the snapshot alone; `pr_stale_since` shows only while stale.
#[test]
fn snapshot_ignores_a_fresh_pr_success() {
    let api = fake(vec![plain_pr(7571)]);
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&[7571]));
    store.refresh(pr_t0());
    let first = store.snapshot();
    assert_eq!(first.pr_stale_since, None);
    store.refresh(at(61));
    assert_eq!(store.pr_last_success, Some(at(61)));
    assert_eq!(store.snapshot(), first);

    *api.fail_all.lock().unwrap() = Some(BitbucketError::Transport);
    store.refresh(at(122));
    store.refresh(at(362));
    assert!(store.snapshot().pr_stale);
    assert_eq!(store.snapshot().pr_stale_since, Some(at(61)));
}
