//! Swift `StoreTests.swift` from `chimeSilentOnFirstRefresh` through `surfaceMissDroppedWhenPidChanges`.

use super::*;

/// `write_session` named like its id, with an optional `waitingFor`.
fn ws(dir: &std::path::Path, id: &str, status: &str, waiting: Option<&str>, age: f64, cwd: &str) {
    write_session_with(
        dir,
        id,
        me(),
        status,
        age,
        id,
        cwd,
        SessionOpts {
            waiting_for: waiting,
            ..SessionOpts::default()
        },
    );
}

const PERM: Option<&str> = Some("permission prompt");

#[test]
fn chime_silent_on_first_refresh() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    ws(dir.path(), "a", "idle", None, 0.0, FRONTEND);
    ws(dir.path(), "b", "waiting", PERM, 0.0, BACKEND);
    store.refresh(SystemTime::now());
    assert_eq!(store.chime, None);
}

#[test]
fn chime_done_on_busy_to_idle() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    ws(dir.path(), "a", "busy", None, 0.0, FRONTEND);
    store.refresh(t0);
    ws(dir.path(), "a", "idle", None, 0.0, FRONTEND);
    store.refresh(secs(t0, 1));
    assert_eq!(store.chime, Some(Chime::Done));
}

#[test]
fn chime_needs_you_on_busy_to_waiting() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    ws(dir.path(), "a", "busy", None, 0.0, FRONTEND);
    store.refresh(t0);
    ws(dir.path(), "a", "waiting", PERM, 0.0, FRONTEND);
    store.refresh(secs(t0, 1));
    assert_eq!(store.chime, Some(Chime::NeedsYou));
}

#[test]
fn shell_status_shows_waiting_on_background_command() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    ws(dir.path(), "a", "shell", None, 0.0, FRONTEND);
    store.refresh(SystemTime::now());
    let s = &store.sessions[0];
    assert_eq!(claude_sessions_core::text::status_text(s), "chờ lệnh nền");
    assert_eq!(s.color_status(), "busy");
    assert_eq!(s.shown_status(), "idle");
    assert!(s.can_ask_claude());
    assert!(s.is_finished());
    // Deferred to phase 6: `Theme.Symbols.status(s) == "hourglass"` (view-layer symbol table).
}

#[test]
fn shell_announces_once_then_idle_is_quiet() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    ws(dir.path(), "a", "busy", None, 0.0, FRONTEND);
    store.refresh(t0);
    ws(dir.path(), "a", "shell", None, 0.0, FRONTEND);
    store.refresh(secs(t0, 1));
    assert_eq!(store.transitions, tr(&[("a", Chime::Done)]));
    assert_eq!(store.chime, Some(Chime::Done));
    ws(dir.path(), "a", "idle", None, 0.0, FRONTEND);
    store.refresh(secs(t0, 2));
    assert!(store.transitions.is_empty());
    assert_eq!(store.chime, None);
}

#[test]
fn shell_session_has_no_background_count() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    ws(dir.path(), "a", "shell", None, 0.0, FRONTEND);
    store.refresh(SystemTime::now());
    assert_eq!(store.sessions[0].background_agents, None);
}

#[test]
fn chime_needs_you_wins() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    ws(dir.path(), "a", "busy", None, 0.0, FRONTEND);
    ws(dir.path(), "b", "busy", None, 0.0, BACKEND);
    store.refresh(t0);
    ws(dir.path(), "a", "idle", None, 0.0, FRONTEND);
    ws(dir.path(), "b", "waiting", PERM, 0.0, BACKEND);
    store.refresh(secs(t0, 1));
    assert_eq!(store.chime, Some(Chime::NeedsYou));
}

#[test]
fn chime_resets_next_refresh() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    ws(dir.path(), "a", "busy", None, 0.0, FRONTEND);
    store.refresh(t0);
    ws(dir.path(), "a", "idle", None, 0.0, FRONTEND);
    store.refresh(secs(t0, 1));
    assert_eq!(store.chime, Some(Chime::Done));
    store.refresh(secs(t0, 2));
    assert_eq!(store.chime, None);
}

#[test]
fn bg_opacity_default() {
    let dir = make_temp_dir();
    let store = make_store(dir.path());
    assert_eq!(store.bg_opacity, 0.25);
    // Deferred to phase 6: `Layout.bgOpacities == [0, 0.25, 0.5, 0.75, 0.92]` (view-layer constant).
}

// ---- Unseen marks ----

fn marked_store() -> (Store, TempDir, SystemTime) {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    ws(dir.path(), "a", "busy", None, 5.0, FRONTEND);
    store.refresh(t0);
    ws(dir.path(), "a", "idle", None, 0.0, FRONTEND);
    store.refresh(secs(t0, 1));
    (store, dir, t0)
}

fn unseen_keys(store: &Store) -> HashSet<String> {
    store.unseen.keys().cloned().collect()
}

#[test]
fn unseen_on_busy_to_idle_and_waiting() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    ws(dir.path(), "a", "busy", None, 5.0, FRONTEND);
    ws(dir.path(), "b", "busy", None, 5.0, BACKEND);
    ws(dir.path(), "c", "idle", None, 5.0, BACKEND);
    store.refresh(t0);
    assert!(store.unseen.is_empty());
    ws(dir.path(), "a", "idle", None, 0.0, FRONTEND);
    ws(dir.path(), "b", "waiting", PERM, 0.0, BACKEND);
    store.refresh(secs(t0, 1));
    assert_eq!(unseen_keys(&store), set(["a", "b"]));
}

#[test]
fn unseen_kept_across_refreshes() {
    let (mut store, _d, t0) = marked_store();
    for i in 1..=3 {
        store.refresh(secs(t0, 1 + i * 30));
    }
    assert_eq!(unseen_keys(&store), set(["a"]));
}

#[test]
fn unseen_cleared_on_switch_to_busy() {
    let (mut store, dir, t0) = marked_store();
    ws(dir.path(), "a", "busy", None, 0.0, FRONTEND);
    store.refresh(secs(t0, 2));
    assert!(store.unseen.is_empty());
}

#[test]
fn unseen_dropped_when_session_dies() {
    let (mut store, dir, t0) = marked_store();
    write_session(dir.path(), "a", 999_999, "idle", 0.0, "a", FRONTEND);
    store.refresh(secs(t0, 2));
    assert!(!store.unseen.contains_key("a"));
}

#[test]
fn mark_seen_clears() {
    let (mut store, _d, _t) = marked_store();
    store.mark_seen("zzz");
    assert_eq!(unseen_keys(&store), set(["a"]));
    store.mark_seen("a");
    assert!(store.unseen.is_empty());
}

#[test]
fn new_count_counts_live_only() {
    let (mut store, _d, _t) = marked_store();
    store.unseen.insert(
        "dead".into(),
        NewMark {
            claude: true,
            ..NewMark::default()
        },
    );
    assert_eq!(store.new_count(), 1);
}

#[test]
fn note_focused_needs_two_consecutive() {
    let (mut store, _d, _t) = marked_store();
    store.surfaces.insert("a".into(), "S1".into());
    store.note_focused(Some("S1"));
    assert_eq!(unseen_keys(&store), set(["a"]));
    store.note_focused(Some("s1"));
    assert!(store.unseen.is_empty());
}

#[test]
fn note_focused_nil_resets() {
    let (mut store, _d, _t) = marked_store();
    store.surfaces.insert("a".into(), "S1".into());
    store.note_focused(Some("S1"));
    store.note_focused(None);
    store.note_focused(Some("S1"));
    assert_eq!(unseen_keys(&store), set(["a"]));
}

#[test]
fn surface_miss_dropped_when_pid_changes() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    ws(dir.path(), "a", "idle", None, 0.0, FRONTEND);
    store.refresh(SystemTime::now());
    store.surface_misses.insert("a".into());
    store.refresh(SystemTime::now());
    assert_eq!(store.surface_misses, set(["a"]));
    write_session(dir.path(), "a", parent_pid(), "idle", 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    assert!(store.surface_misses.is_empty());
    store.surface_misses.insert("a".into());
    write_session(dir.path(), "a", 999_999, "idle", 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    assert!(store.surface_misses.is_empty());
}
