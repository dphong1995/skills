//! Ported from Swift `StoreTests.swift` (tests 1-50), plus the Rust-only section and snapshot tests.

use crate::support::*;
use claude_sessions_core::panel_state::{EditKind, Editing, PanelState};
use claude_sessions_core::store::{Action, Chime, NewMark, Section, Store};
use claude_sessions_core::tasks::Todo;
use claude_sessions_core::transcript::{Pending, PendingState};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

fn secs(t: SystemTime, s: u64) -> SystemTime {
    t + Duration::from_secs(s)
}

fn ids(store: &Store) -> Vec<&str> {
    store.sessions.iter().map(|s| s.id.as_str()).collect()
}

fn set<const N: usize>(v: [&str; N]) -> HashSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn empty_dir_has_no_sessions() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    store.refresh(SystemTime::now());
    assert!(store.sessions.is_empty());
}

#[test]
fn dead_pid_is_filtered_and_first_sight_is_quiet() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    write_session(
        dir.path(),
        "a",
        me(),
        "busy",
        42.0,
        "talo6-dct-frontend-d7",
        FRONTEND,
    );
    write_session(
        dir.path(),
        "b",
        me(),
        "busy",
        300.0,
        "talo6-dct-backend-3f",
        BACKEND,
    );
    write_session(dir.path(), "dead", 999_999, "idle", 10.0, "dead", "/tmp");
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions.len(), 2);
    assert!(!store.peeking());
}

/// `claude -p` runs (plugins' side calls, scripts) are not sessions the user opened: no card, no count and so never a closed row.
#[test]
fn headless_sessions_are_ignored() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let entry = |e| SessionOpts {
        entrypoint: Some(e),
        ..SessionOpts::default()
    };
    write_session_with(
        dir.path(),
        "cli",
        me(),
        "busy",
        5.0,
        "app-1",
        FRONTEND,
        entry("cli"),
    );
    write_session(dir.path(), "old", me(), "busy", 5.0, "app-2", BACKEND);
    write_session_with(
        dir.path(),
        "side",
        me(),
        "busy",
        5.0,
        "te-side-x",
        "/tmp/te-side-x",
        entry("sdk-cli"),
    );
    write_session_with(
        dir.path(),
        "ts",
        me(),
        "busy",
        5.0,
        "sdk",
        "/tmp/sdk",
        entry("sdk-ts"),
    );
    store.refresh(SystemTime::now());
    let got: HashSet<String> = store.sessions.iter().map(|s| s.id.clone()).collect();
    assert_eq!(got, set(["cli", "old"]));
}

#[test]
fn busy_to_idle_peeks() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    write_session(
        dir.path(),
        "a",
        me(),
        "busy",
        42.0,
        "talo6-dct-frontend-d7",
        FRONTEND,
    );
    write_session(
        dir.path(),
        "b",
        me(),
        "busy",
        300.0,
        "talo6-dct-backend-3f",
        BACKEND,
    );
    store.refresh(t0);

    write_session(
        dir.path(),
        "a",
        me(),
        "idle",
        0.0,
        "talo6-dct-frontend-d7",
        FRONTEND,
    );
    store.refresh(secs(t0, 1));
    let (tr, pc) = (store.transitions.clone(), store.pr_chime);
    store.apply_alert(&tr, pc, &HashSet::new(), secs(t0, 1));
    assert!(PanelState::new().expanded(store.peeking()));
    assert_eq!(store.chime, Some(Chime::Done));
    assert_eq!(store.sessions.first().map(|s| s.id.as_str()), Some("a"));

    store.refresh(secs(t0, 7));
    assert!(!store.peeking());
}

#[test]
fn waiting_ordering() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    write_session(
        dir.path(),
        "a",
        me(),
        "busy",
        42.0,
        "talo6-dct-frontend-d7",
        FRONTEND,
    );
    write_session(
        dir.path(),
        "b",
        me(),
        "busy",
        300.0,
        "talo6-dct-backend-3f",
        BACKEND,
    );
    store.refresh(t0);
    write_session(
        dir.path(),
        "a",
        me(),
        "idle",
        0.0,
        "talo6-dct-frontend-d7",
        FRONTEND,
    );
    store.refresh(secs(t0, 1));

    let waiting = |w| SessionOpts {
        waiting_for: Some(w),
        ..SessionOpts::default()
    };
    write_session_with(
        dir.path(),
        "b",
        me(),
        "waiting",
        0.0,
        "talo6-dct-backend-3f",
        BACKEND,
        waiting("permission prompt"),
    );
    write_session_with(
        dir.path(),
        "c",
        me(),
        "waiting",
        75.0,
        "ocr-service-a1",
        "/Users/phong/ocr-service",
        waiting("input needed"),
    );
    write_session(
        dir.path(),
        "d",
        me(),
        "busy",
        3700.0,
        "talo-v2-9c",
        "/Users/phong/talos-v2",
    );
    store.refresh(secs(t0, 8));
    assert_eq!(ids(&store), ["c", "b", "a", "d"]);
    let by = |id: &str| store.sessions.iter().find(|s| s.id == id).unwrap();
    assert!(by("b").is_permission_prompt());
    assert!(!by("c").is_permission_prompt());
}

#[test]
fn waiting_order() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    let waiting = |w| SessionOpts {
        waiting_for: Some(w),
        ..SessionOpts::default()
    };
    write_session(dir.path(), "idleUnseen", me(), "idle", 10.0, "a", FRONTEND);
    write_session_with(
        dir.path(),
        "waiting",
        me(),
        "waiting",
        20.0,
        "b",
        BACKEND,
        waiting("input needed"),
    );
    write_session_with(
        dir.path(),
        "perm",
        me(),
        "waiting",
        30.0,
        "c",
        BACKEND,
        waiting("permission prompt"),
    );
    write_session(dir.path(), "busy", me(), "busy", 40.0, "d", BACKEND);
    write_session(dir.path(), "idleSeen", me(), "idle", 50.0, "e", BACKEND);
    store.refresh(t0);
    let mark = NewMark {
        claude: true,
        ..NewMark::default()
    };
    store.unseen = ["idleUnseen", "busy", "waiting"]
        .iter()
        .map(|k| (k.to_string(), mark.clone()))
        .collect();
    assert_eq!(store.waiting_order(), ["perm", "waiting", "idleUnseen"]);
}

#[test]
fn hover_expands() {
    let store = make_store(make_temp_dir().path());
    let mut panel = PanelState::new();
    panel.hovering = true;
    assert!(panel.expanded(store.peeking()));
}

#[test]
fn second_turn_finishing_peeks_again() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    let name = "talo6-dct-frontend-d7";
    write_session(dir.path(), "a", me(), "busy", 0.0, name, FRONTEND);
    store.refresh(t0);
    write_session(dir.path(), "a", me(), "idle", 0.0, name, FRONTEND);
    store.refresh(secs(t0, 1));
    store.refresh(secs(t0, 30));
    assert!(!store.peeking());

    write_session(dir.path(), "a", me(), "busy", 0.0, name, FRONTEND);
    store.refresh(secs(t0, 31));
    write_session(dir.path(), "a", me(), "idle", 0.0, name, FRONTEND);
    store.refresh(secs(t0, 32));
    let (tr, pc) = (store.transitions.clone(), store.pr_chime);
    store.apply_alert(&tr, pc, &HashSet::new(), secs(t0, 32));
    assert!(store.peeking());
}

#[test]
fn transitions_recorded() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    write_session(dir.path(), "b", me(), "busy", 0.0, "b", BACKEND);
    store.refresh(t0);
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    write_session(dir.path(), "b", me(), "waiting", 0.0, "b", BACKEND);
    store.refresh(secs(t0, 1));
    let want: HashMap<String, Chime> = HashMap::from([
        ("a".to_string(), Chime::Done),
        ("b".to_string(), Chime::NeedsYou),
    ]);
    assert_eq!(store.transitions, want);
    assert!(!store.peeking());
    assert_eq!(store.chime, Some(Chime::NeedsYou));
    store.refresh(secs(t0, 2));
    assert!(store.transitions.is_empty());
}

fn tr(items: &[(&str, Chime)]) -> HashMap<String, Chime> {
    items.iter().map(|(k, c)| (k.to_string(), *c)).collect()
}

#[test]
fn alert_excludes_focused() {
    let (chime, peek) = Store::alert(&tr(&[("a", Chime::Done)]), None, &set(["a"]));
    assert_eq!(chime, None);
    assert!(!peek);
}

#[test]
fn alert_keeps_other_sessions() {
    let t = tr(&[("a", Chime::NeedsYou), ("b", Chime::Done)]);
    let (chime, peek) = Store::alert(&t, None, &set(["a"]));
    assert_eq!(chime, Some(Chime::Done));
    assert!(peek);
}

#[test]
fn alert_keeps_pr_chime() {
    let (chime, peek) = Store::alert(
        &tr(&[("a", Chime::Done)]),
        Some(Chime::NeedsYou),
        &set(["a"]),
    );
    assert_eq!(chime, Some(Chime::NeedsYou));
    assert!(!peek);
}

#[test]
fn alert_needs_you_wins() {
    let t = tr(&[("a", Chime::Done), ("b", Chime::NeedsYou)]);
    let (chime, peek) = Store::alert(&t, Some(Chime::Done), &HashSet::new());
    assert_eq!(chime, Some(Chime::NeedsYou));
    assert!(peek);
}

#[test]
fn apply_alert_peeks() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(t0);
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    store.refresh(secs(t0, 1));
    let t = secs(t0, 1);
    let (trs, pc) = (store.transitions.clone(), store.pr_chime);
    assert_eq!(store.apply_alert(&trs, pc, &set(["a"]), t), None);
    assert!(!store.peeking());
    assert_eq!(
        store.apply_alert(&trs, pc, &HashSet::new(), t),
        Some(Chime::Done)
    );
    assert!(store.peeking());
}

fn nl(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().map(|l| l + "\n").collect()
}

#[test]
fn last_reply_on_idle() {
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
        &[assistant_line(Some("all done"), false)],
    );
    put(dir.path(), "a", "busy", None, 5.0);
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].last_reply, None);
    put(dir.path(), "a", "idle", None, 0.0);
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].last_reply.as_deref(), Some("all done"));
}

fn with_projects(dir: &std::path::Path, projects: &std::path::Path) -> Store {
    make_store_with(
        dir,
        StoreSetup {
            projects: Some(projects.into()),
            ..Default::default()
        },
    )
}

#[test]
fn no_reread_when_unchanged() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let f = write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[assistant_line(Some("aaaa"), false)],
    );
    put(dir.path(), "a", "idle", None, 0.0);
    store.refresh(SystemTime::now());
    std::fs::write(&f, assistant_line(Some("bbbb"), false)).unwrap();
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].last_reply.as_deref(), Some("aaaa"));
}

#[test]
fn rereads_when_transcript_grows() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let f = write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[assistant_line(Some("first"), false)],
    );
    put(dir.path(), "a", "idle", None, 0.0);
    store.refresh(SystemTime::now());
    let grown = [
        assistant_line(Some("first"), false),
        assistant_line(Some("a longer final text"), false),
    ]
    .join("\n");
    std::fs::write(&f, grown).unwrap();
    store.refresh(SystemTime::now());
    assert_eq!(
        store.sessions[0].last_reply.as_deref(),
        Some("a longer final text")
    );
}

fn bash(id: &str, cmd: &str) -> String {
    tool_use_line(id, "Bash", serde_json::json!({"command": cmd}), false)
}

#[test]
fn pending_only_for_permission_prompt() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    write_transcript(projects.path(), FRONTEND, "a", &[bash("t1", "npm test")]);
    put(dir.path(), "a", "waiting", Some("permission prompt"), 0.0);
    store.refresh(SystemTime::now());
    let want = PendingState::One(Pending {
        tool_id: "t1".into(),
        tool: "Bash".into(),
        text: "npm test".into(),
    });
    assert_eq!(store.sessions[0].pending, Some(want));
    put(dir.path(), "a", "waiting", Some("input needed"), 0.0);
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].pending, None);
}

fn approve(id: &str, tool: &str) -> Action {
    Action::Approve {
        session: id.into(),
        tool_id: tool.into(),
    }
}

#[test]
fn still_valid_rejects_changed_state() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let f = write_transcript(projects.path(), FRONTEND, "a", &[bash("t1", "npm test")]);
    put(dir.path(), "a", "waiting", Some("permission prompt"), 0.0);
    store.refresh(SystemTime::now());
    assert!(store.still_valid(&approve("a", "t1")));
    assert!(!store.still_valid(&approve("a", "t2")));

    // Same state, a different tool open: only the tool id differs.
    let lines = [
        bash("t1", "npm test"),
        tool_result_line("t1"),
        bash("t2", "ls"),
    ];
    std::fs::write(&f, lines.join("\n")).unwrap();
    put(dir.path(), "a", "waiting", Some("permission prompt"), 1.0);
    store.refresh(SystemTime::now());
    assert!(!store.still_valid(&approve("a", "t1")));
    assert!(store.still_valid(&approve("a", "t2")));

    std::fs::write(
        &f,
        [bash("t1", "npm test"), tool_result_line("t1")].join("\n"),
    )
    .unwrap();
    put(dir.path(), "a", "busy", None, 0.0);
    store.refresh(SystemTime::now());
    assert!(!store.still_valid(&approve("a", "t1")));
    put(dir.path(), "a", "idle", None, 0.0);
    store.refresh(SystemTime::now());
    assert!(!store.still_valid(&approve("a", "t1")));
    assert!(store.still_valid(&Action::Jump("zzz".into())));
}

const NOT_IDLE: [(&str, Option<&str>, bool); 4] = [
    ("idle", None, true),
    ("busy", None, false),
    ("waiting", Some("input needed"), false),
    ("waiting", Some("permission prompt"), false),
];

fn ask(id: &str) -> Action {
    Action::AskClaude {
        session: id.into(),
        prompt: "x".into(),
        pr: test_pr(),
    }
}

#[test]
fn ask_claude_valid_only_when_idle() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    for (status, waiting_for, ok) in NOT_IDLE {
        put(dir.path(), "a", status, waiting_for, 0.0);
        store.refresh(SystemTime::now());
        assert_eq!(store.still_valid(&ask("a")), ok, "{status} {waiting_for:?}");
    }
    assert!(!store.still_valid(&ask("zzz")));
}

#[test]
fn reply_needs_idle() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let reply = |id: &str| Action::Reply {
        session: id.into(),
        text: "hi".into(),
    };
    for (status, waiting_for, ok) in NOT_IDLE {
        put(dir.path(), "a", status, waiting_for, 0.0);
        store.refresh(SystemTime::now());
        assert_eq!(
            store.still_valid(&reply("a")),
            ok,
            "{status} {waiting_for:?}"
        );
    }
    assert!(!store.still_valid(&reply("zzz")));
}

#[test]
fn stop_needs_busy() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let stop = Action::Stop("a".into());
    put(dir.path(), "a", "busy", None, 0.0);
    store.refresh(SystemTime::now());
    assert!(store.still_valid(&stop));
    put(dir.path(), "a", "idle", None, 0.0);
    store.refresh(SystemTime::now());
    assert!(!store.still_valid(&stop));
    let lines = nl(vec![
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_duration_line(Some(1), None, false, None),
    ]);
    write_transcript(projects.path(), FRONTEND, "a", &lines);
    put(dir.path(), "a", "busy", None, 0.0);
    store.refresh(SystemTime::now());
    assert!(!store.still_valid(&stop));
    assert!(!store.still_valid(&Action::Stop("zzz".into())));
}

#[test]
fn ask_claude_invalid_when_waiting_after_turn_end() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let lines = [
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_duration_line(Some(1), None, false, None),
    ];
    write_transcript(projects.path(), FRONTEND, "a", &lines);
    put(dir.path(), "a", "waiting", Some("input needed"), 0.0);
    store.refresh(SystemTime::now());
    assert!(!store.still_valid(&ask("a")));
    put(dir.path(), "a", "idle", None, 0.0);
    store.refresh(SystemTime::now());
    assert!(store.still_valid(&ask("a")));
}

#[test]
fn ask_claude_invalid_when_busy_ended_then_waiting() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let lines = nl(vec![
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_duration_line(Some(1), None, false, None),
    ]);
    write_transcript(projects.path(), FRONTEND, "a", &lines);
    put(dir.path(), "a", "busy", None, 0.0);
    store.refresh(SystemTime::now());
    assert!(store.still_valid(&ask("a")));
    put(dir.path(), "a", "waiting", Some("input needed"), 0.0);
    store.refresh(SystemTime::now());
    assert!(!store.still_valid(&ask("a")));
}

#[test]
fn allows_false_when_session_gone() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    put(dir.path(), "a", "idle", None, 0.0);
    store.refresh(SystemTime::now());
    let e = Editing {
        id: "a".into(),
        kind: EditKind::Deny,
        tool_id: Some("t1".into()),
    };
    std::fs::remove_file(dir.path().join("a.json")).unwrap();
    store.refresh(SystemTime::now());
    assert!(!store.allows(&e));
}

#[test]
fn editing_keeps_expanded() {
    let store = make_store(make_temp_dir().path());
    let mut panel = PanelState::new();
    panel.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Deny,
        tool_id: Some("t1".into()),
    });
    assert!(panel.expanded(store.peeking()));
}

#[test]
fn hidden_after_30s_empty() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    put(dir.path(), "a", "idle", None, 5.0);
    store.refresh(t0);
    assert!(!store.hidden());
    let _ = std::fs::remove_file(dir.path().join("a.json"));
    store.refresh(secs(t0, 1));
    assert!(!store.hidden());
    store.refresh(secs(t0, 30));
    assert!(!store.hidden());
    store.refresh(secs(t0, 31));
    assert!(store.hidden());
}

type TempDir = tempfile::TempDir;

fn approvable_store() -> (Store, TempDir, TempDir, std::path::PathBuf) {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let store = with_projects(dir.path(), projects.path());
    let f = write_transcript(projects.path(), FRONTEND, "a", &[bash("t1", "npm test")]);
    (store, dir, projects, f)
}

#[test]
fn shown_when_session_appears() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    store.refresh(t0 - Duration::from_secs(60));
    store.refresh(t0);
    assert!(store.hidden());
    put(dir.path(), "a", "idle", None, 5.0);
    store.refresh(secs(t0, 1));
    assert!(!store.hidden() && store.empty_since.is_none());
}

#[test]
fn start_hidden_hides_immediately() {
    let empty = make_temp_dir();
    let mut store = make_store_with(
        empty.path(),
        StoreSetup {
            start_hidden: true,
            ..Default::default()
        },
    );
    store.refresh(SystemTime::now());
    assert!(store.hidden());
    let dir = make_temp_dir();
    put(dir.path(), "a", "idle", None, 5.0);
    let mut live = make_store_with(
        dir.path(),
        StoreSetup {
            start_hidden: true,
            ..Default::default()
        },
    );
    live.refresh(SystemTime::now());
    assert!(!live.hidden());
}

#[test]
fn default_starts_visible() {
    let mut store = make_store(make_temp_dir().path());
    store.refresh(SystemTime::now());
    assert!(!store.hidden());
}

fn parent_pid() -> i32 {
    rustix::process::getppid().unwrap().as_raw_nonzero().get()
}

#[test]
fn surface_dropped_when_pid_changes() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    put(dir.path(), "a", "idle", None, 0.0);
    store.refresh(SystemTime::now());
    store.surfaces.insert("a".into(), "S".into());
    store.refresh(SystemTime::now());
    assert_eq!(store.surfaces.get("a").map(String::as_str), Some("S"));
    assert_eq!(ids(&store), ["a"]);
    write_session(dir.path(), "a", parent_pid(), "idle", 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    assert_eq!(ids(&store), ["a"]);
    assert_eq!(store.surfaces.get("a"), None);
}

#[test]
fn status_flip_with_unchanged_file_size_is_seen() {
    let (mut store, dir, _p, _f) = approvable_store();
    let t = SystemTime::now();
    let at = |w| SessionOpts {
        waiting_for: w,
        updated_at: Some(t),
        ..SessionOpts::default()
    };
    write_session_with(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND, at(None));
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].pending, None);
    write_session_with(
        dir.path(),
        "a",
        me(),
        "waiting",
        0.0,
        "a",
        FRONTEND,
        at(Some("permission prompt")),
    );
    store.refresh(SystemTime::now());
    let want = PendingState::One(Pending {
        tool_id: "t1".into(),
        tool: "Bash".into(),
        text: "npm test".into(),
    });
    assert_eq!(store.sessions[0].pending, Some(want));
}

#[test]
fn allows_false_when_tool_changes() {
    let (mut store, dir, _p, f) = approvable_store();
    put(dir.path(), "a", "waiting", Some("permission prompt"), 0.0);
    store.refresh(SystemTime::now());
    let e = Editing {
        id: "a".into(),
        kind: EditKind::Deny,
        tool_id: Some("t1".into()),
    };
    store.refresh(SystemTime::now());
    assert!(store.allows(&e));
    let lines = [
        bash("t1", "npm test"),
        tool_result_line("t1"),
        bash("t2", "ls"),
    ];
    std::fs::write(&f, lines.join("\n")).unwrap();
    put(dir.path(), "a", "waiting", Some("permission prompt"), 1.0);
    store.refresh(SystemTime::now());
    assert!(!store.allows(&e));
}

// One conversation open in two processes (resumed in a second pane): one row, the newest process, and its cached surface stays.
#[test]
fn same_session_in_two_processes_shows_newest_once() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    write_session(dir.path(), "a", me(), "busy", 0.0, "new", FRONTEND);
    let old = serde_json::json!({"pid": parent_pid(), "sessionId": "a", "name": "old", "cwd": FRONTEND, "status": "idle",
        "statusUpdatedAt": (claude_sessions_core::time::unix_secs(SystemTime::now()) - 600.0) * 1000.0});
    std::fs::write(
        dir.path().join("old.json"),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();
    store.refresh(SystemTime::now());
    let names: Vec<&str> = store.sessions.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["new"]);
    store.surfaces.insert("a".into(), "S".into());
    store.refresh(SystemTime::now());
    assert_eq!(store.surfaces.get("a").map(String::as_str), Some("S"));
}

#[test]
fn title_order_user_beats_ai_title_beats_prompt() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let t0 = SystemTime::now();
    let upd = |t, src| SessionOpts {
        updated_at: Some(t),
        name_source: src,
        ..SessionOpts::default()
    };
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[user_line("sửa widget", false, None)],
    );
    write_session_with(
        dir.path(),
        "a",
        me(),
        "idle",
        0.0,
        "front-d7",
        FRONTEND,
        upd(t0, None),
    );
    store.refresh(t0);
    assert_eq!(store.sessions[0].title(), "sửa widget");

    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[
            user_line("sửa widget", false, None),
            ai_title_line("widget-fix"),
        ],
    );
    write_session_with(
        dir.path(),
        "a",
        me(),
        "idle",
        0.0,
        "front-d7",
        FRONTEND,
        upd(secs(t0, 1), None),
    );
    store.refresh(secs(t0, 1));
    assert_eq!(store.sessions[0].title(), "widget-fix");

    write_session_with(
        dir.path(),
        "a",
        me(),
        "idle",
        0.0,
        "tên tôi",
        FRONTEND,
        upd(secs(t0, 1), Some("user")),
    );
    store.refresh(secs(t0, 2));
    assert_eq!(store.sessions[0].title(), "tên tôi");
}

#[test]
fn ai_title_stays_after_leaving_tail() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let t0 = SystemTime::now();
    let upd = |t| SessionOpts {
        updated_at: Some(t),
        ..SessionOpts::default()
    };
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[ai_title_line("t1"), assistant_line(Some("x"), false)],
    );
    write_session_with(
        dir.path(),
        "a",
        me(),
        "idle",
        0.0,
        "front-d7",
        FRONTEND,
        upd(t0),
    );
    store.refresh(t0);
    assert_eq!(store.sessions[0].title(), "t1");

    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[
            assistant_line(Some("x"), false),
            assistant_line(Some("a longer reply"), false),
        ],
    );
    write_session_with(
        dir.path(),
        "a",
        me(),
        "idle",
        0.0,
        "front-d7",
        FRONTEND,
        upd(secs(t0, 1)),
    );
    store.refresh(secs(t0, 1));
    assert_eq!(store.sessions[0].title(), "t1");
}

#[test]
fn busy_session_has_prompt_title_and_branch() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let branch: claude_sessions_core::store::BranchFn =
        Box::new(|c| (c == FRONTEND).then(|| "main".to_string()));
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            branch: Some(branch),
            ..Default::default()
        },
    );
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[user_line("chạy test", false, None)],
    );
    write_session(dir.path(), "a", me(), "busy", 5.0, "front-d7", FRONTEND);
    store.refresh(SystemTime::now());
    let s = &store.sessions[0];
    assert_eq!(s.title(), "chạy test");
    assert_eq!(s.folder(), "talo6-dct-frontend");
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!(s.last_reply, None);
}

#[test]
fn title_appears_after_transcript_created() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let t0 = SystemTime::now();
    let upd = |t| SessionOpts {
        updated_at: Some(t),
        ..SessionOpts::default()
    };
    write_session_with(
        dir.path(),
        "a",
        me(),
        "busy",
        0.0,
        "front-d7",
        FRONTEND,
        upd(t0),
    );
    store.refresh(t0);
    assert_eq!(store.sessions[0].title(), "front-d7");

    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[user_line("chạy test", false, None)],
    );
    store.refresh(t0);
    assert_eq!(store.sessions[0].title(), "front-d7");

    write_session_with(
        dir.path(),
        "a",
        me(),
        "busy",
        0.0,
        "front-d7",
        FRONTEND,
        upd(secs(t0, 1)),
    );
    store.refresh(secs(t0, 1));
    assert_eq!(store.sessions[0].title(), "chạy test");
}

#[test]
fn sorts_by_title() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[user_line("zzz", false, None)],
    );
    write_transcript(
        projects.path(),
        BACKEND,
        "b",
        &[user_line("aaa", false, None)],
    );
    write_session(dir.path(), "a", me(), "idle", 5.0, "a", FRONTEND);
    write_session(dir.path(), "b", me(), "idle", 5.0, "b", BACKEND);
    store.refresh(SystemTime::now());
    assert_eq!(ids(&store), ["b", "a"]);
}

fn with_tasks(dir: &std::path::Path, tasks: &std::path::Path) -> Store {
    make_store_with(
        dir,
        StoreSetup {
            tasks: Some(tasks.into()),
            ..Default::default()
        },
    )
}

#[test]
fn todo_for_busy_session() {
    let (dir, tasks) = (make_temp_dir(), make_temp_dir());
    let mut store = with_tasks(dir.path(), tasks.path());
    write_session(dir.path(), "a", me(), "busy", 5.0, "a", FRONTEND);
    write_task(
        tasks.path(),
        Some("a"),
        1,
        "in_progress",
        "Phase 1",
        Some("Doing phase 1"),
    );
    store.refresh(SystemTime::now());
    let want = Todo {
        done: 0,
        total: 1,
        current: Some("Doing phase 1".into()),
    };
    assert_eq!(store.sessions[0].todo, Some(want));
}

#[test]
fn todo_updates_when_task_changes() {
    let (dir, tasks) = (make_temp_dir(), make_temp_dir());
    let mut store = with_tasks(dir.path(), tasks.path());
    write_session(dir.path(), "a", me(), "busy", 5.0, "a", FRONTEND);
    write_task(tasks.path(), Some("a"), 1, "in_progress", "Phase 1", None);
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].todo.as_ref().map(|t| t.done), Some(0));
    write_task(tasks.path(), Some("a"), 1, "completed", "Phase 1", None);
    write_task(tasks.path(), Some("a"), 2, "pending", "Phase 2", None);
    store.refresh(SystemTime::now());
    let want = Todo {
        done: 1,
        total: 2,
        current: Some("Phase 2".into()),
    };
    assert_eq!(store.sessions[0].todo, Some(want));
}

#[test]
fn todo_gone_when_files_deleted() {
    let (dir, tasks) = (make_temp_dir(), make_temp_dir());
    let mut store = with_tasks(dir.path(), tasks.path());
    write_session(dir.path(), "a", me(), "busy", 5.0, "a", FRONTEND);
    write_task(tasks.path(), Some("a"), 1, "in_progress", "Phase 1", None);
    store.refresh(SystemTime::now());
    assert!(store.sessions[0].todo.is_some());
    std::fs::remove_file(tasks.path().join("a/1.json")).unwrap();
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].todo, None);
}

#[test]
fn session_json_same_size_rewrite_is_seen() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t = SystemTime::now();
    let opts = SessionOpts {
        updated_at: Some(t),
        ..SessionOpts::default()
    };
    write_session_with(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND, opts);
    store.refresh(t);
    assert_eq!(store.sessions[0].status, "idle");
    // Same byte size ("idle" -> "busy" padded), written to a temp file and renamed like Claude Code does.
    let file = dir.path().join("a.json");
    let old = std::fs::read_to_string(&file).unwrap();
    let mut text = old.replace("\"idle\"", "\"busy\"");
    text += &" ".repeat(old.len() - text.len());
    assert_eq!(text.len(), old.len());
    let tmp = dir.path().join("a.tmp");
    std::fs::write(&tmp, text).unwrap();
    std::fs::rename(&tmp, &file).unwrap();
    store.refresh(secs(t, 1));
    assert_eq!(store.sessions[0].status, "busy");
}

#[test]
fn task_rewrite_in_place_is_seen() {
    let (dir, tasks) = (make_temp_dir(), make_temp_dir());
    let mut store = with_tasks(dir.path(), tasks.path());
    write_session(dir.path(), "a", me(), "busy", 5.0, "a", FRONTEND);
    write_task(tasks.path(), Some("a"), 1, "pending", "Phase 1", None);
    store.refresh(SystemTime::now());
    let todo = |s: &str| {
        Some(Todo {
            done: 0,
            total: 1,
            current: Some(s.into()),
        })
    };
    assert_eq!(store.sessions[0].todo, todo("Phase 1"));
    // Same file name, same size, rewritten in place: the directory does not change.
    write_task(tasks.path(), Some("a"), 1, "pending", "Phase 2", None);
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].todo, todo("Phase 2"));
}

#[test]
fn branch_follows_checkout_while_busy() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let heads = std::rc::Rc::new(std::cell::RefCell::new(HashMap::from([(
        FRONTEND.to_string(),
        "main".to_string(),
    )])));
    let reader = heads.clone();
    let branch: claude_sessions_core::store::BranchFn =
        Box::new(move |c| reader.borrow().get(c).cloned());
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            branch: Some(branch),
            ..Default::default()
        },
    );
    let t0 = SystemTime::now();
    let upd = |t| SessionOpts {
        updated_at: Some(t),
        ..SessionOpts::default()
    };
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[
            user_line("x", false, None),
            assistant_line(Some("y"), false),
        ],
    );
    write_session_with(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND, upd(t0));
    store.refresh(t0);
    assert_eq!(store.sessions[0].folder(), "talo6-dct-frontend");

    heads
        .borrow_mut()
        .insert(FRONTEND.to_string(), "feat".to_string());
    write_session_with(
        dir.path(),
        "a",
        me(),
        "busy",
        0.0,
        "a",
        FRONTEND,
        upd(secs(t0, 1)),
    );
    store.refresh(secs(t0, 1));
    assert_eq!(store.sessions[0].folder(), "talo6-dct-frontend");
    assert_eq!(store.sessions[0].branch.as_deref(), Some("feat"));
}

const JIRA_HOST: &str = "talosix.atlassian.net";

/// One session "a" whose transcript holds `lines`.
fn scan_store(lines: &[String], status: &str) -> (Store, TempDir, TempDir) {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let mut all = lines.to_vec();
    all.push(String::new());
    write_transcript(projects.path(), FRONTEND, "a", &all);
    write_session(dir.path(), "a", me(), status, 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    (store, dir, projects)
}

fn labels(store: &Store) -> Option<Vec<String>> {
    store
        .sessions
        .first()
        .map(|s| s.links.iter().map(|l| l.label.clone()).collect())
}

#[test]
fn branch_comes_from_cwd_not_transcript() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let branch: claude_sessions_core::store::BranchFn =
        Box::new(|c| (c == FRONTEND).then(|| "master".to_string()));
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            branch: Some(branch),
            ..Default::default()
        },
    );
    store.jira_projects.insert("T6IR".into(), JIRA_HOST.into());
    let line = json_line(
        serde_json::json!({"type": "user", "gitBranch": "bugfix/T6IR-30", "message": {"content": "hello"}}),
    );
    write_transcript(projects.path(), FRONTEND, "a", &[line, String::new()]);
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].folder(), "talo6-dct-frontend");
    assert_eq!(labels(&store), Some(vec![]));
}

#[test]
fn branch_ticket_not_a_button() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let branch: claude_sessions_core::store::BranchFn =
        Box::new(|_| Some("bugfix/T6IR-30".to_string()));
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            branch: Some(branch),
            ..Default::default()
        },
    );
    let line = user_line(&format!("https://{JIRA_HOST}/browse/T6IR-10"), false, None);
    write_transcript(projects.path(), FRONTEND, "a", &[line, String::new()]);
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    store.refresh(SystemTime::now());
    assert_eq!(labels(&store), Some(vec!["T6IR-10".to_string()]));
    assert!(!store.sessions[0].menu.iter().any(|s| s.title == "Nhánh"));
}

#[test]
fn title_cut_at_200() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[user_line(&"t".repeat(260), false, None)],
    );
    write_session(dir.path(), "a", me(), "busy", 5.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].title(), format!("{}…", "t".repeat(200)));
}

#[test]
fn reply_cut_at_600_with_labels() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let text = format!("PR https://github.com/a/b/pull/9 {}", "x".repeat(1000));
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[assistant_line(Some(&text), false)],
    );
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    let reply = store.sessions[0].last_reply.clone().unwrap();
    assert!(reply.starts_with("PR PR #9 "));
    assert!(reply.ends_with('…'));
    assert_eq!(reply.chars().count(), 601);
}

#[test]
fn ticket_button_order() {
    let lines = [
        user_line(&format!("https://{JIRA_HOST}/browse/T6IR-10"), false, None),
        user_line("T6IR-11", false, None),
        assistant_line(Some("T6IR-20"), false),
        assistant_line(Some("T6IR-21"), false),
    ];
    let (store, _d, _p) = scan_store(&lines, "idle");
    assert_eq!(
        labels(&store),
        Some(
            ["T6IR-10", "T6IR-11", "T6IR-21", "T6IR-20"]
                .map(String::from)
                .to_vec()
        )
    );
}

// ---- Rust-only and deferred tests ----

use claude_sessions_core::bitbucket::BitbucketError;
use claude_sessions_core::pr_monitor::PRMonitor;
use claude_sessions_core::text::pr_line;
use std::sync::Arc;

const PR_REPO: &str = "t6engineering/talo6-dct-backend";

fn pr_t0() -> SystemTime {
    std::time::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

fn pr_reply(ids: &[i64]) -> String {
    let links: Vec<String> = ids
        .iter()
        .map(|i| format!("https://bitbucket.org/{PR_REPO}/pull-requests/{i}"))
        .collect();
    assistant_line(Some(&links.join(" ")), false)
}

/// One idle session `a` whose last reply is `reply`, with a PR monitor on the fake.
fn pr_store(api: &Arc<FakeBitbucket>, reply: String) -> (Store, TempDir, TempDir) {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    write_transcript(projects.path(), FRONTEND, "a", &[reply, String::new()]);
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    let monitor = PRMonitor::with_options(api.clone(), false, Arc::new(|_| None), 45.0);
    let store = make_store_with(
        dir.path(),
        StoreSetup {
            projects: Some(projects.path().into()),
            prs: Some(monitor),
            ..Default::default()
        },
    );
    (store, dir, projects)
}

/// Swift `prStaleAfterFiveMinutes` plus recovery: the rows read `cũ <thời gian>` while `pr_stale`, and lose it on a success.
#[test]
fn stale_rows_after_300s_and_recover() {
    let api = fake(vec![pr_json(PrSpec {
        id: 7571,
        ..Default::default()
    })]);
    let (mut store, _d, _p) = pr_store(&api, pr_reply(&[7571]));
    let at = |s: u64| secs(pr_t0(), s);
    store.refresh(pr_t0());
    assert_eq!(store.pr_last_success, Some(pr_t0()));
    *api.fail_all.lock().unwrap() = Some(BitbucketError::Transport);
    store.refresh(at(61));
    store.refresh(at(299));
    assert!(!store.pr_stale());
    store.refresh(at(301));
    assert!(store.pr_stale());
    assert_eq!(store.pr_last_success, Some(pr_t0()));
    let line = |store: &Store, now| {
        let stale = store.pr_stale().then_some(store.pr_last_success).flatten();
        pr_line(&store.sessions[0].prs[0], now, stale)
    };
    assert!(
        line(&store, at(301)).ends_with(" · cũ 5m"),
        "{}",
        line(&store, at(301))
    );

    *api.fail_all.lock().unwrap() = None;
    store.refresh(at(400));
    assert!(!store.pr_stale());
    assert_eq!(store.pr_last_success, Some(at(400)));
    assert!(!line(&store, at(400)).contains("cũ"));
}

#[test]
fn sections_group_by_need() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let waiting = |w| SessionOpts {
        waiting_for: Some(w),
        ..SessionOpts::default()
    };
    write_session_with(
        dir.path(),
        "perm",
        me(),
        "waiting",
        5.0,
        "a-perm",
        FRONTEND,
        waiting("permission prompt"),
    );
    write_session_with(
        dir.path(),
        "question",
        me(),
        "waiting",
        5.0,
        "b-question",
        FRONTEND,
        waiting("input needed"),
    );
    write_session(dir.path(), "idle", me(), "idle", 5.0, "c-idle", FRONTEND);
    write_session(dir.path(), "busy", me(), "busy", 5.0, "d-busy", FRONTEND);
    let ended = nl(vec![
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_duration_line(Some(2), None, false, None),
    ]);
    write_transcript(projects.path(), FRONTEND, "agents", &ended);
    write_session(
        dir.path(),
        "agents",
        me(),
        "busy",
        5.0,
        "e-agents",
        FRONTEND,
    );
    store.refresh(SystemTime::now());
    assert_eq!(
        store
            .sessions
            .iter()
            .find(|s| s.id == "agents")
            .unwrap()
            .background_agents,
        Some(2)
    );
    let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        store.snapshot().sections(),
        vec![
            (Section::NeedsYou, ids(&["perm", "question"])),
            (Section::Done, ids(&["idle"])),
            (Section::Running, ids(&["busy", "agents"])),
        ]
    );
}

#[test]
fn snapshot_stable_without_change() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[assistant_line(Some("all done"), false)],
    );
    put(dir.path(), "a", "idle", None, 5.0);
    put(dir.path(), "b", "busy", None, 9.0);
    let t0 = SystemTime::now();
    store.refresh(t0);
    let first = store.snapshot();
    store.refresh(secs(t0, 1));
    assert_eq!(store.snapshot(), first);
}
