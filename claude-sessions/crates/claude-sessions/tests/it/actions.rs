//! Actions with a recording runner: no window, no cmux, no AppleScript.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use claude_sessions::actions::{self, Completion, EffectRunner, Pending, Poster};
use claude_sessions_core::action::{Action, SendKey, SendKind};
use claude_sessions_core::closed_sessions::ClosedSession;
use claude_sessions_core::cmux::Key;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::procs::{GroupKind, ProcGroup, ProcTable, SessionProcs};
use claude_sessions_core::pull_requests::{Chime, PRRef};
use claude_sessions_core::store::{
    LinkKind, NewMark, Paths, SendState, Session, Store, StoreOptions,
};
use claude_sessions_core::transcript::{Pending as ToolPending, PendingState};
use claude_sessions_core::transcript_scanner::TranscriptScanner;
use url::Url;

const SURFACE: &str = "20FAA4D5-E0C5-4F59-9D8E-277C0038558C";
const UUID: &str = "7d0fcbef-0908-41eb-9779-ea86d5bf838a";

#[derive(Debug, PartialEq)]
enum Fx {
    Beep,
    Play(Chime),
    Open(String),
    Copy(String),
    Lookup(String),
    Jump(String),
    Keys(String, Vec<Key>),
    Resume(String),
    Connect(Service),
    Kill(SendKey, Vec<(i32, u64)>),
}

#[derive(Default)]
struct Recorder {
    fx: Vec<Fx>,
    pending: Vec<Pending>,
    posted: Arc<Mutex<Vec<Completion>>>,
}

impl EffectRunner for Recorder {
    fn poster(&self) -> Poster {
        let posted = self.posted.clone();
        Arc::new(move |c| posted.lock().unwrap().push(c))
    }
    fn beep(&mut self) {
        self.fx.push(Fx::Beep);
    }
    fn play(&mut self, chime: Chime) {
        self.fx.push(Fx::Play(chime));
    }
    fn open_link(&mut self, url: &Url, _allow_http: bool) {
        self.fx.push(Fx::Open(url.to_string()));
    }
    fn copy(&mut self, text: &str) {
        self.fx.push(Fx::Copy(text.into()));
    }
    fn lookup_surface(&mut self, session: &str, pending: Pending) {
        self.fx.push(Fx::Lookup(session.into()));
        self.pending.push(pending);
    }
    fn jump(&mut self, surface: &str, _key: Option<SendKey>) {
        self.fx.push(Fx::Jump(surface.into()));
    }
    fn send_keys(&mut self, surface: &str, keys: Vec<Key>, pending: Pending) {
        self.fx.push(Fx::Keys(surface.into(), keys));
        self.pending.push(pending);
    }
    fn resume(&mut self, closed: &ClosedSession, _key: SendKey) {
        self.fx.push(Fx::Resume(closed.id.clone()));
    }
    fn connect(&mut self, service: Service) {
        self.fx.push(Fx::Connect(service));
    }
    fn kill_procs(&mut self, key: SendKey, targets: Vec<(i32, u64)>) {
        self.fx.push(Fx::Kill(key, targets));
    }
}

fn t0() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

/// A store with `sessions`, each with the surface `SURFACE`; its clock is `t0()`.
fn store_with(sessions: Vec<Session>) -> Store {
    let dir = tempfile::Builder::new()
        .prefix("claude-sessions-action-tests-")
        .tempdir()
        .unwrap();
    let mut store = Store::new(
        Paths {
            sessions: dir.path().to_path_buf(),
            projects: dir.path().join("_projects"),
            tasks: dir.path().join("_tasks"),
        },
        StoreOptions {
            scanner: TranscriptScanner::new(false),
            branch: Some(Box::new(|_| None)),
            ..StoreOptions::default()
        },
    );
    store.refresh(t0());
    for s in &sessions {
        store.surfaces.insert(s.id.clone(), SURFACE.into());
    }
    store.sessions = sessions;
    store
}

fn approval(id: &str) -> Session {
    let mut s = Session::new(id, id, "/work/repo", "waiting");
    s.waiting_for = Some("permission prompt".into());
    s.pending = Some(PendingState::One(ToolPending {
        tool_id: "t1".into(),
        tool: "Bash".into(),
        text: "git status".into(),
    }));
    s
}

fn approve(id: &str, tool_id: &str) -> Action {
    Action::Approve {
        session: id.into(),
        tool_id: tool_id.into(),
    }
}

fn key(id: &str, kind: SendKind) -> SendKey {
    SendKey::new(id, kind)
}

fn run(action: &Action, store: &mut Store, rec: &mut Recorder) -> Option<String> {
    actions::handle(action, store, rec, t0())
}

#[test]
fn approve_types_one_into_the_session_surface() {
    let mut store = store_with(vec![approval("a")]);
    store.unseen.insert("a".into(), NewMark::default());
    let mut rec = Recorder::default();
    let a = approve("a", "t1");
    run(&a, &mut store, &mut rec);
    assert_eq!(
        rec.fx,
        vec![Fx::Keys(SURFACE.into(), vec![Key::Raw("1".into())])]
    );
    let k = a.send_key().unwrap();
    assert_eq!(store.send_state(&k), Some(SendState::Sending));
    // The mark clears when the keys went through, not before.
    assert!(store.unseen.contains_key("a"));
    let pending = rec.pending.remove(0);
    actions::complete(Completion::Sent(pending, true), &mut store, &mut rec, t0());
    assert_eq!(store.send_state(&k), Some(SendState::Sent));
    assert!(!store.unseen.contains_key("a"));
}

#[test]
fn sent_and_failed_show_for_three_seconds() {
    let mut store = store_with(vec![approval("a")]);
    let mut rec = Recorder::default();
    let a = approve("a", "t1");
    let k = a.send_key().unwrap();
    run(&a, &mut store, &mut rec);
    let pending = rec.pending.remove(0);
    actions::complete(Completion::Sent(pending, false), &mut store, &mut rec, t0());
    assert_eq!(store.send_state(&k), Some(SendState::Failed));
    assert_eq!(rec.fx.last(), Some(&Fx::Beep));
    store.refresh(t0() + Duration::from_secs(2));
    assert_eq!(store.send_state(&k), Some(SendState::Failed));
    store.refresh(t0() + Duration::from_secs(4));
    assert_eq!(store.send_state(&k), None);
}

#[test]
fn a_second_send_to_the_same_session_beeps() {
    let mut store = store_with(vec![approval("a")]);
    let mut rec = Recorder::default();
    run(&approve("a", "t1"), &mut store, &mut rec);
    rec.fx.clear();
    run(&approve("a", "t1"), &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Beep]);
}

#[test]
fn an_action_that_is_no_longer_valid_beeps_and_types_nothing() {
    let mut store = store_with(vec![approval("a")]);
    let mut rec = Recorder::default();
    let a = approve("a", "gone");
    run(&a, &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Beep]);
    assert_eq!(
        store.send_state(&a.send_key().unwrap()),
        Some(SendState::Failed)
    );
    // A session that left the list behaves the same.
    let mut rec = Recorder::default();
    store.surfaces.insert("nobody".into(), SURFACE.into());
    let stop = Action::Stop("nobody".into());
    run(&stop, &mut store, &mut rec);
    assert!(!rec.fx.iter().any(|f| matches!(f, Fx::Keys(..))));
    assert_eq!(
        store.send_state(&stop.send_key().unwrap()),
        Some(SendState::Failed)
    );
}

#[test]
fn deny_with_a_reason_closes_the_editor_and_clears_its_draft_when_sent() {
    let mut store = store_with(vec![approval("a")]);
    let draft = Store::draft_key("a", claude_sessions_core::panel_state::EditKind::Deny);
    store.drafts.insert(draft.clone(), "no thanks".into());
    let mut rec = Recorder::default();
    let a = Action::Deny {
        session: "a".into(),
        tool_id: "t1".into(),
        reason: "no thanks".into(),
    };
    assert_eq!(run(&a, &mut store, &mut rec), Some("a".into()));
    let Some(Fx::Keys(_, keys)) = rec.fx.first() else {
        panic!("no keys: {:?}", rec.fx);
    };
    assert_eq!(keys[0], Key::Raw("\\x1b".into()));
    assert_eq!(keys.last(), Some(&Key::Raw("\\r".into())));
    assert!(
        store.drafts.contains_key(&draft),
        "kept until the send succeeds"
    );
    let pending = rec.pending.remove(0);
    actions::complete(Completion::Sent(pending, true), &mut store, &mut rec, t0());
    assert!(!store.drafts.contains_key(&draft));
}

#[test]
fn stop_goes_to_a_busy_session_only() {
    let mut store = store_with(vec![Session::new("b", "b", "/w", "busy")]);
    let mut rec = Recorder::default();
    run(&Action::Stop("b".into()), &mut store, &mut rec);
    assert_eq!(
        rec.fx,
        vec![Fx::Keys(SURFACE.into(), vec![Key::Raw("\\x1b".into())])]
    );
}

#[test]
fn reply_waits_for_an_idle_session() {
    let mut store = store_with(vec![Session::new("i", "i", "/w", "idle")]);
    let mut rec = Recorder::default();
    let a = Action::Reply {
        session: "i".into(),
        text: "hi".into(),
    };
    assert_eq!(run(&a, &mut store, &mut rec), Some("i".into()));
    assert!(matches!(rec.fx.as_slice(), [Fx::Keys(_, k)] if k.contains(&Key::Paste("hi".into()))));
    let mut store = store_with(vec![Session::new("i", "i", "/w", "busy")]);
    let mut rec = Recorder::default();
    run(&a, &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Beep]);
}

#[test]
fn jump_marks_seen_and_goes_to_the_cached_surface() {
    let mut store = store_with(vec![approval("a")]);
    store.unseen.insert("a".into(), NewMark::default());
    let mut rec = Recorder::default();
    run(&Action::Jump("a".into()), &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Jump(SURFACE.into())]);
    assert!(!store.unseen.contains_key("a"));
    actions::complete(Completion::Jumped(None, false), &mut store, &mut rec, t0());
    assert_eq!(rec.fx.last(), Some(&Fx::Beep));
}

#[test]
fn a_jump_claims_its_button_and_the_result_releases_it() {
    let key = SendKey::new("a", SendKind::Jump);
    let mut store = store_with(vec![approval("a")]);
    let mut rec = Recorder::default();
    run(&Action::Jump("a".into()), &mut store, &mut rec);
    assert_eq!(store.send_state(&key), Some(SendState::Sending));
    actions::complete(
        Completion::Jumped(Some(key.clone()), true),
        &mut store,
        &mut rec,
        t0(),
    );
    assert_eq!(store.send_state(&key), Some(SendState::Sent));

    // No surface: the button fails and beeps.
    let mut store = store_with(vec![approval("a")]);
    store.surfaces.clear();
    store.surface_misses.insert("a".into());
    let mut rec = Recorder::default();
    run(&Action::Jump("a".into()), &mut store, &mut rec);
    let pending = rec.pending.remove(0);
    actions::complete(
        Completion::Surface(pending, None),
        &mut store,
        &mut rec,
        t0(),
    );
    assert_eq!(store.send_state(&key), Some(SendState::Failed));
    assert_eq!(rec.fx.last(), Some(&Fx::Beep));
}

#[test]
fn jump_without_a_surface_looks_it_up_first() {
    let mut store = store_with(vec![approval("a")]);
    store.surfaces.clear();
    let mut rec = Recorder::default();
    run(&Action::Jump("a".into()), &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Lookup("a".into())]);
    let pending = rec.pending.remove(0);
    actions::complete(
        Completion::Surface(pending.clone(), None),
        &mut store,
        &mut rec,
        t0(),
    );
    assert_eq!(rec.fx.last(), Some(&Fx::Beep));
    assert!(store.surface_misses.contains("a"));
    actions::complete(
        Completion::Surface(pending, Some(SURFACE.into())),
        &mut store,
        &mut rec,
        t0(),
    );
    assert_eq!(rec.fx.last(), Some(&Fx::Jump(SURFACE.into())));
    assert_eq!(store.surfaces.get("a").map(String::as_str), Some(SURFACE));
    assert!(!store.surface_misses.contains("a"));
}

#[test]
fn a_send_without_a_surface_fails() {
    let mut store = store_with(vec![approval("a")]);
    store.surfaces.clear();
    let mut rec = Recorder::default();
    let a = approve("a", "t1");
    run(&a, &mut store, &mut rec);
    let pending = rec.pending.remove(0);
    actions::complete(
        Completion::Surface(pending, None),
        &mut store,
        &mut rec,
        t0(),
    );
    assert_eq!(
        store.send_state(&a.send_key().unwrap()),
        Some(SendState::Failed)
    );
    assert_eq!(rec.fx.last(), Some(&Fx::Beep));
}

fn closed(id: &str) -> ClosedSession {
    ClosedSession {
        id: id.into(),
        title: "t".into(),
        cwd: "/work/it's".into(),
        branch: None,
        closed_at: t0(),
    }
}

#[test]
fn resume_runs_for_a_uuid_and_never_for_another_id() {
    let mut store = store_with(vec![]);
    let mut rec = Recorder::default();
    run(&Action::Resume(closed(UUID)), &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Resume(UUID.into())]);

    let mut rec = Recorder::default();
    let bad = Action::Resume(closed("x'; touch /tmp/pwned; '"));
    run(&bad, &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Beep]);
    assert_eq!(
        store.send_state(&bad.send_key().unwrap()),
        Some(SendState::Failed)
    );
}

#[test]
fn a_failed_resume_copies_the_shell_command() {
    let mut store = store_with(vec![]);
    let mut rec = Recorder::default();
    let c = closed(UUID);
    let k = key(UUID, SendKind::Resume);
    actions::complete(
        Completion::Resumed(c.clone(), k.clone(), false),
        &mut store,
        &mut rec,
        t0(),
    );
    assert_eq!(rec.fx, vec![Fx::Copy(c.shell_command().unwrap()), Fx::Beep]);
    assert_eq!(store.send_state(&k), Some(SendState::Failed));
    let mut rec = Recorder::default();
    actions::complete(
        Completion::Resumed(c, k.clone(), true),
        &mut store,
        &mut rec,
        t0(),
    );
    assert!(rec.fx.is_empty());
    assert_eq!(store.send_state(&k), Some(SendState::Sent));
}

#[test]
fn open_link_opens_https_only() {
    let mut store = store_with(vec![approval("a")]);
    store.unseen.insert("a".into(), NewMark::default());
    for bad in [
        "file:///tmp/x",
        "vscode://x",
        "http://example.com/",
        "https://u:p@example.com/",
    ] {
        let mut rec = Recorder::default();
        let a = Action::OpenLink("a".into(), Url::parse(bad).unwrap());
        run(&a, &mut store, &mut rec);
        assert_eq!(rec.fx, vec![Fx::Beep], "{bad}");
    }
    let mut rec = Recorder::default();
    let a = Action::OpenLink("a".into(), Url::parse("https://example.com/x").unwrap());
    run(&a, &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Open("https://example.com/x".into())]);
    assert!(!store.unseen.contains_key("a"));
}

#[test]
fn only_a_transcript_link_may_be_plain_http() {
    use claude_sessions_core::store::{LinkItem, LinkSection};
    let item = |url: &str, kind| LinkItem {
        text: "x".into(),
        url: Url::parse(url).unwrap(),
        kind,
        code: String::new(),
        title: None,
        status: None,
        tone: None,
        icon_url: None,
        help: None,
    };
    let mut s = approval("a");
    s.menu = vec![LinkSection {
        title: "Links".into(),
        items: vec![
            item("http://docs.example.com/a", LinkKind::Link),
            item("http://pr.example.com/1", LinkKind::Pr),
        ],
    }];
    let mut store = store_with(vec![s]);
    for (url, opens) in [
        ("http://docs.example.com/a", true),
        ("http://pr.example.com/1", false),
    ] {
        let mut rec = Recorder::default();
        run(
            &Action::OpenLink("a".into(), Url::parse(url).unwrap()),
            &mut store,
            &mut rec,
        );
        let want = if opens {
            Fx::Open(url.into())
        } else {
            Fx::Beep
        };
        assert_eq!(rec.fx, vec![want], "{url}");
    }
}

#[test]
fn only_actions_that_type_need_a_fresh_store() {
    assert!(actions::sends_keys(&Action::Stop("a".into())));
    assert!(actions::sends_keys(&Action::Approve {
        session: "a".into(),
        tool_id: "t".into()
    }));
    assert!(!actions::sends_keys(&Action::Jump("a".into())));
    assert!(!actions::sends_keys(&Action::MarkSeen("a".into())));
}

#[test]
fn actions_that_need_a_monitor_fail_without_one() {
    let mut store = store_with(vec![]);
    let pr = PRRef {
        workspace: "w".into(),
        repo: "r".into(),
        id: 1,
    };
    let nudge = Action::NudgeRabbit("a".into(), pr);
    let mut rec = Recorder::default();
    run(&nudge, &mut store, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Beep]);
    assert_eq!(
        store.send_state(&nudge.send_key().unwrap()),
        Some(SendState::Failed)
    );
}

#[test]
fn connect_opens_settings_even_without_a_jira_site() {
    let mut store = store_with(vec![]);
    let mut rec = Recorder::default();
    run(&Action::Connect(Service::Bitbucket), &mut store, &mut rec);
    run(&Action::Connect(Service::Jira), &mut store, &mut rec);
    assert_eq!(
        rec.fx,
        vec![Fx::Connect(Service::Bitbucket), Fx::Connect(Service::Jira)]
    );
}

#[test]
fn chimes_play_unless_muted() {
    let mut rec = Recorder::default();
    actions::play_chime(Some(Chime::Done), false, &mut rec);
    actions::play_chime(Some(Chime::NeedsYou), true, &mut rec);
    actions::play_chime(None, false, &mut rec);
    assert_eq!(rec.fx, vec![Fx::Play(Chime::Done)]);
}

#[test]
fn mark_seen_only_touches_the_store() {
    let mut store = store_with(vec![approval("a")]);
    store.unseen.insert("a".into(), NewMark::default());
    let mut rec = Recorder::default();
    run(&Action::MarkSeen("a".into()), &mut store, &mut rec);
    assert!(rec.fx.is_empty());
    assert!(!store.unseen.contains_key("a"));
}

#[test]
fn kill_action_calls_runner_and_finishes_send() {
    let action = Action::KillProcs {
        session: "a".into(),
        key: "g".into(),
        targets: vec![(10, 1)],
    };
    let key = action.send_key().unwrap();
    for (ok, want) in [(true, SendState::Sent), (false, SendState::Failed)] {
        let mut store = store_with(vec![approval("a")]);
        store.set_procs(procs_of("a", "g", (10, 1)));
        let mut rec = Recorder::default();
        run(&action, &mut store, &mut rec);
        assert_eq!(rec.fx, [Fx::Kill(key.clone(), vec![(10, 1)])]);
        assert_eq!(store.send_state(&key), Some(SendState::Sending));
        actions::complete(
            Completion::Killed(key.clone(), ok),
            &mut store,
            &mut rec,
            t0(),
        );
        assert_eq!(store.send_state(&key), Some(want));
    }
}

#[test]
fn kill_of_processes_that_are_gone_fails_without_the_runner() {
    let action = Action::KillProcs {
        session: "a".into(),
        key: "g".into(),
        targets: vec![(10, 1)],
    };
    let mut store = store_with(vec![approval("a")]);
    let mut rec = Recorder::default();
    run(&action, &mut store, &mut rec);
    assert_eq!(rec.fx, [Fx::Beep]);
    assert_eq!(
        store.send_state(&action.send_key().unwrap()),
        Some(SendState::Failed)
    );
}

/// A table whose live session `session` has one command group `key` with one member.
fn procs_of(session: &str, key: &str, member: (i32, u64)) -> ProcTable {
    let group = ProcGroup {
        key: key.into(),
        kind: GroupKind::Command,
        name: key.into(),
        command: key.into(),
        ports: vec![],
        rss: 0,
        cpu_percent: None,
        started_us: 0,
        members: vec![member],
    };
    let procs = SessionProcs {
        session: session.into(),
        commands: vec![group],
        claude_and_mcp: None,
        pwd: None,
        started_us: 0,
    };
    ProcTable {
        live: [(session.to_string(), procs)].into(),
        closed: vec![],
    }
}
