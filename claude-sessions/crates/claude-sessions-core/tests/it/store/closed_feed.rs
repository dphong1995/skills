//! Swift `StoreTests.swift`: process liveness, the closed list and Feed questions
//! (`exitedButUnreapedPidIsFiltered` through `clearKeepsPidSoNothingIsClosed`).

use super::*;
use claude_sessions_core::closed_sessions::ClosedStore;
use claude_sessions_core::feed::{BlockState, FeedMonitor};
use claude_sessions_core::kv::MemoryStore;
use claude_sessions_core::store::Paths;
use std::process::{Child, Command};
use std::sync::Arc;

/// Swift `exitedButUnreapedPidIsFiltered`, ported against the injectable `StoreOptions.is_running` hook: the core's default check
/// (`kill(pid, 0)`) cannot tell a zombie from a live process, so the app injects the finer one. A hook that says the pid is dead
/// filters the session out; the real zombie check is injected by the macos crate later.
#[test]
fn exited_but_unreaped_pid_is_filtered() {
    let dir = make_temp_dir();
    let live = me();
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            is_running: Some(Box::new(move |pid| pid == live)),
            ..Default::default()
        },
    );
    write_session(dir.path(), "live", live, "idle", 0.0, "live", FRONTEND);
    write_session(
        dir.path(),
        "zombie",
        parent_pid(),
        "idle",
        0.0,
        "zombie",
        FRONTEND,
    );
    store.refresh(SystemTime::now());
    assert_eq!(ids(&store), vec!["live"]);
}

/// A live child process to stand in for Claude; `end_process` kills and reaps it, so its pid is really gone.
fn spawn_sleeper() -> Child {
    Command::new("/bin/sleep").arg("60").spawn().unwrap()
}

fn end_process(mut c: Child) {
    c.kill().unwrap();
    c.wait().unwrap();
}

fn pid_of(c: &Child) -> i32 {
    c.id() as i32
}

#[test]
fn dead_session_goes_to_closed() {
    let dir = make_temp_dir();
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            branch: Some(Box::new(|_| Some("feat/x".into()))),
            ..Default::default()
        },
    );
    write_session(dir.path(), "dead", 999_999, "idle", 10.0, "dead", "/tmp");
    store.refresh(SystemTime::now());
    assert!(store.closed.items().is_empty());
    let child = spawn_sleeper();
    write_session(
        dir.path(),
        "a",
        pid_of(&child),
        "idle",
        10.0,
        "frontend",
        FRONTEND,
    );
    store.refresh(SystemTime::now());
    assert!(store.closed.items().is_empty());
    end_process(child);
    store.refresh(SystemTime::now());
    let c = &store.closed.items()[0];
    assert_eq!(c.id, "a");
    assert_eq!(c.title, "frontend");
    assert_eq!(c.cwd, FRONTEND);
    assert_eq!(c.branch.as_deref(), Some("feat/x"));
    write_session(dir.path(), "a", me(), "idle", 10.0, "frontend", FRONTEND);
    store.refresh(SystemTime::now());
    assert!(store.closed.items().is_empty());
}

fn store_with_closed(dir: &std::path::Path, kv: &MemoryStore) -> Store {
    let (projects, tasks) = (make_temp_dir(), make_temp_dir());
    Store::new(
        Paths {
            sessions: dir.to_path_buf(),
            projects: projects.path().to_path_buf(),
            tasks: tasks.path().to_path_buf(),
        },
        claude_sessions_core::store::StoreOptions {
            scanner: claude_sessions_core::transcript_scanner::TranscriptScanner::new(false),
            branch: Some(Box::new(|_| None)),
            closed_store: Some(ClosedStore::new(Box::new(kv.clone()))),
            ..Default::default()
        },
    )
}

#[test]
fn closed_persists_across_stores() {
    let kv = MemoryStore::default();
    let dir = make_temp_dir();
    let mut store = store_with_closed(dir.path(), &kv);
    let child = spawn_sleeper();
    write_session(
        dir.path(),
        "a",
        pid_of(&child),
        "idle",
        10.0,
        "frontend",
        FRONTEND,
    );
    store.refresh(SystemTime::now());
    end_process(child);
    store.refresh(SystemTime::now());
    let other = make_temp_dir();
    let again = store_with_closed(other.path(), &kv);
    let got: Vec<&str> = again.closed.items().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(got, vec!["a"]);
}

fn feed_store(items: Vec<serde_json::Value>) -> (Store, TempDir, Arc<FakeFeedTransport>) {
    let dir = make_temp_dir();
    let t = FakeFeedTransport::new();
    *t.items.lock().unwrap() = items;
    let store = make_store_with(
        dir.path(),
        StoreSetup {
            feed: Some(FeedMonitor::without_worker(t.clone())),
            ..Default::default()
        },
    );
    (store, dir, t)
}

fn waiting_input(dir: &std::path::Path, id: &str, cwd: &str) {
    write_session_with(
        dir,
        id,
        me(),
        "waiting",
        0.0,
        id,
        cwd,
        SessionOpts {
            waiting_for: Some("input needed"),
            ..SessionOpts::default()
        },
    );
}

fn idle_now(dir: &std::path::Path, status: &str) {
    write_session(dir, "a", me(), status, 0.0, "a", FRONTEND);
}

#[test]
fn question_attaches_to_the_session_of_its_id() {
    let (mut store, dir, _t) = feed_store(vec![feed_item("R1", "b", SystemTime::now())]);
    waiting_input(dir.path(), "a", FRONTEND);
    waiting_input(dir.path(), "b", BACKEND);
    store.refresh(SystemTime::now());
    let q = |s: &Store, id: &str| {
        s.sessions
            .iter()
            .find(|x| x.id == id)
            .unwrap()
            .question
            .as_ref()
            .map(|q| q.request_id.clone())
    };
    assert_eq!(q(&store, "b").as_deref(), Some("R1"));
    assert_eq!(q(&store, "a"), None);
    // An unknown session id attaches to nobody, even when the pid is shared.
    let (mut s2, d2, _t2) = feed_store(vec![feed_item("R2", "other-id", SystemTime::now())]);
    waiting_input(d2.path(), "a", FRONTEND);
    s2.refresh(SystemTime::now());
    assert!(s2.sessions.iter().all(|s| s.question.is_none()));
}

#[test]
fn question_polls_only_for_busy_or_asking_sessions() {
    let (mut store, dir, t) = feed_store(vec![feed_item("R1", "a", SystemTime::now())]);
    idle_now(dir.path(), "idle");
    store.refresh(SystemTime::now());
    assert!(t.calls.lock().unwrap().is_empty());
    write_session_with(
        dir.path(),
        "a",
        me(),
        "waiting",
        0.0,
        "a",
        FRONTEND,
        SessionOpts {
            waiting_for: Some("permission prompt"),
            ..SessionOpts::default()
        },
    );
    store.refresh(SystemTime::now());
    assert!(t.calls.lock().unwrap().is_empty());
    idle_now(dir.path(), "busy");
    store.refresh(SystemTime::now());
    assert_eq!(t.calls.lock().unwrap().len(), 1);
}

#[test]
fn question_counts_as_waiting() {
    let (mut store, dir, t) = feed_store(vec![]);
    idle_now(dir.path(), "busy");
    let t0 = SystemTime::now();
    store.refresh(t0);
    assert!(!store.unseen.contains_key("a") && store.transitions.is_empty());
    // The question shows up while the session still reads `busy`.
    *t.items.lock().unwrap() = vec![feed_item("R1", "a", t0)];
    store.refresh(secs(t0, 1));
    assert!(store.unseen["a"].claude);
    assert_eq!(store.transitions.get("a"), Some(&Chime::NeedsYou));
    assert_eq!(store.chime, Some(Chime::NeedsYou));
    let (tr, pr) = (store.transitions.clone(), store.pr_chime);
    let chime = store.apply_alert(&tr, pr, &HashSet::new(), secs(t0, 1));
    assert_eq!(chime, Some(Chime::NeedsYou));
    assert!(store.peeking());
    // The same question is announced once; the later busy -> waiting flip stays quiet.
    store.refresh(secs(t0, 2));
    assert!(store.transitions.is_empty());
    waiting_input(dir.path(), "a", FRONTEND);
    store.refresh(secs(t0, 3));
    assert!(store.transitions.is_empty() && store.chime.is_none());
    assert!(store.unseen["a"].claude);
}

#[test]
fn question_not_announced_when_already_there_at_first_sight() {
    let (mut store, dir, _t) = feed_store(vec![feed_item("R1", "a", SystemTime::now())]);
    waiting_input(dir.path(), "a", FRONTEND);
    store.refresh(SystemTime::now());
    assert!(!store.unseen.contains_key("a") && store.transitions.is_empty());
}

#[test]
fn sent_mark_shown_until_session_runs_again() {
    let (mut store, dir, _t) = feed_store(vec![feed_item("R1", "a", SystemTime::now())]);
    waiting_input(dir.path(), "a", FRONTEND);
    store.refresh(SystemTime::now());
    store.note_sent("a", "R1");
    assert_eq!(store.sent.get("a").map(String::as_str), Some("R1"));
    store.refresh(SystemTime::now());
    assert_eq!(store.sent.get("a").map(String::as_str), Some("R1"));
    // It stays while the session reads `busy` (the reply is still being taken up) and goes once it is idle.
    idle_now(dir.path(), "busy");
    store.refresh(SystemTime::now());
    assert_eq!(store.sent.get("a").map(String::as_str), Some("R1"));
    idle_now(dir.path(), "idle");
    store.refresh(SystemTime::now());
    assert!(!store.sent.contains_key("a"));
}

#[test]
fn failed_send_falls_back_and_is_kept_like_sent() {
    let (mut store, dir, _t) = feed_store(vec![feed_item("R1", "a", SystemTime::now())]);
    waiting_input(dir.path(), "a", FRONTEND);
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].block, Some(BlockState::Form));
    store.note_sent("a", "R1");
    assert_eq!(store.sessions[0].block, Some(BlockState::Sent));
    store.note_send_failed("a", "R1");
    assert!(!store.sent.contains_key("a"));
    assert_eq!(store.failed.get("a").map(String::as_str), Some("R1"));
    assert_eq!(store.sessions[0].block, Some(BlockState::Fallback));
    // Deferred to phase 6: `Layout.expanded(store.sessions, blocks:).height < Layout.expanded(store.sessions).height` (view-layer layout).
    store.refresh(SystemTime::now());
    assert_eq!(store.failed.get("a").map(String::as_str), Some("R1"));
    idle_now(dir.path(), "idle");
    store.refresh(SystemTime::now());
    assert!(!store.failed.contains_key("a"));
}

#[test]
fn close_is_found_when_the_file_goes_before_the_process() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let child = spawn_sleeper();
    write_session(dir.path(), "a", pid_of(&child), "idle", 10.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    // Claude deletes the file while the process is still exiting.
    std::fs::remove_file(dir.path().join("a.json")).unwrap();
    store.refresh(SystemTime::now());
    assert!(store.sessions.is_empty());
    assert!(store.closed.items().is_empty());
    end_process(child);
    store.refresh(SystemTime::now());
    let got: Vec<&str> = store.closed.items().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(got, vec!["a"]);
}

#[test]
fn clear_keeps_pid_so_nothing_is_closed() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    write_session(dir.path(), "a", me(), "idle", 10.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    // `/clear`: the same process, a new session id.
    std::fs::remove_file(dir.path().join("a.json")).unwrap();
    write_session(dir.path(), "b", me(), "idle", 10.0, "b", FRONTEND);
    store.refresh(SystemTime::now());
    assert_eq!(ids(&store), vec!["b"]);
    assert!(store.closed.items().is_empty());
}

#[test]
fn reply_and_deny_drafts_are_kept_apart() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    write_session(dir.path(), "a", me(), "idle", 10.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    assert_ne!(
        Store::draft_key("a", EditKind::Reply),
        Store::draft_key("a", EditKind::Deny)
    );
    store.drafts.insert(
        Store::draft_key("a", EditKind::Reply),
        "half a reply".into(),
    );
    assert!(
        !store
            .drafts
            .contains_key(&Store::draft_key("a", EditKind::Deny))
    );
    // Both survive a refresh while the session lives, and go with it.
    store.refresh(SystemTime::now());
    assert_eq!(
        store
            .drafts
            .get(&Store::draft_key("a", EditKind::Reply))
            .map(String::as_str),
        Some("half a reply")
    );
    std::fs::remove_file(dir.path().join("a.json")).unwrap();
    store.refresh(SystemTime::now());
    assert!(store.drafts.is_empty());
}

#[test]
fn answer_valid_only_while_its_question_is_live() {
    let (mut store, dir, t) = feed_store(vec![feed_item("R1", "a", SystemTime::now())]);
    waiting_input(dir.path(), "a", FRONTEND);
    let t0 = SystemTime::now();
    store.refresh(t0);
    let answer = |session: &str, request_id: &str| Action::Answer {
        session: session.into(),
        request_id: request_id.into(),
        answers: vec![vec!["Red".into()]],
        other: HashMap::new(),
    };
    let editing = Editing {
        id: "a".into(),
        kind: EditKind::Answer,
        tool_id: None,
    };
    let ok = answer("a", "R1");
    assert!(store.still_valid(&ok));
    assert!(!store.still_valid(&answer("a", "R2")));
    assert!(!store.still_valid(&answer("zzz", "R1")));
    assert!(store.allows(&editing));
    *t.items.lock().unwrap() = vec![];
    store.refresh(secs(t0, 1));
    assert!(!store.still_valid(&ok));
    assert!(!store.allows(&editing));
}
