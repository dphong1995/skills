//! The focused-pane chime exclusion and the 2 s mark clearing, with the cmux queries injected.

use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use claude_sessions::focus::{self, FocusPlan, Located};
use claude_sessions_core::pull_requests::Chime;
use claude_sessions_core::store::{NewMark, Paths, Store, StoreOptions};

fn store(dir: &std::path::Path) -> Store {
    Store::new(
        Paths {
            sessions: dir.join("sessions"),
            projects: dir.join("projects"),
            tasks: dir.join("tasks"),
        },
        StoreOptions::default(),
    )
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_focused_sessions_alert_does_not_chime() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    let transitions: HashMap<String, Chime> =
        [("a".to_string(), Chime::NeedsYou)].into_iter().collect();
    let surfaces: HashMap<String, String> = [("a".to_string(), "AB-CD".to_string())]
        .into_iter()
        .collect();
    // The same surface in another case is still the focused one.
    let silent = focus::silent_ids(transitions.keys(), &surfaces, Some("ab-cd"));
    assert_eq!(silent, HashSet::from(["a".to_string()]));
    let chime = s.apply_alert(&transitions, None, &silent, SystemTime::now());
    assert_eq!(chime, None);
    assert!(s.peek_until.is_none(), "no peek either");
    // Another pane focused: it chimes.
    let silent = focus::silent_ids(transitions.keys(), &surfaces, Some("other"));
    assert!(silent.is_empty());
    assert_eq!(
        s.apply_alert(&transitions, None, &silent, SystemTime::now()),
        Some(Chime::NeedsYou)
    );
}

#[test]
fn a_cmux_failure_silences_nothing() {
    let surfaces: HashMap<String, String> = [("a".into(), "S".into())].into_iter().collect();
    let all = ids(&["a"]);
    assert!(focus::silent_ids(&all, &surfaces, None).is_empty());
}

#[test]
fn gather_looks_up_then_asks_for_the_focus() {
    let targets = vec![("a".to_string(), Some(1)), ("b".to_string(), None)];
    let (located, focused) = focus::gather(
        &targets,
        false,
        |id| (id == "a").then(|| "SURF".to_string()),
        || Some("SURF".to_string()),
    );
    assert_eq!(
        located,
        vec![
            Located {
                id: "a".into(),
                pid: Some(1),
                surface: Some("SURF".into())
            },
            Located {
                id: "b".into(),
                pid: None,
                surface: None
            },
        ]
    );
    assert_eq!(focused.as_deref(), Some("SURF"));
}

#[test]
fn gather_does_not_query_cmux_when_no_surface_is_known() {
    let targets = vec![("a".to_string(), None)];
    let (_, focused) = focus::gather(&targets, false, |_| None, || panic!("no surface known"));
    assert_eq!(focused, None);
    // A surface known from the cache is enough.
    let (_, focused) = focus::gather(&targets, true, |_| None, || Some("X".into()));
    assert_eq!(focused.as_deref(), Some("X"));
}

#[test]
fn located_results_fill_the_cache_and_misses() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    focus::apply_located(
        &mut s,
        &[
            Located {
                id: "a".into(),
                pid: None,
                surface: Some("S".into()),
            },
            Located {
                id: "b".into(),
                pid: None,
                surface: None,
            },
            // The process changed while the lookup ran.
            Located {
                id: "c".into(),
                pid: Some(9),
                surface: Some("T".into()),
            },
        ],
    );
    assert_eq!(s.surfaces.get("a").map(String::as_str), Some("S"));
    assert!(s.surface_misses.contains("b"));
    assert!(!s.surfaces.contains_key("c"));
}

#[test]
fn the_two_second_check_plans_by_marks_surfaces_and_frontmost() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    let busy = HashSet::new();
    assert_eq!(focus::plan(&s, true, &busy), FocusPlan::Clear, "no mark");
    s.unseen.insert(
        "a".into(),
        NewMark {
            claude: true,
            prs: HashSet::new(),
        },
    );
    assert_eq!(
        focus::plan(&s, false, &busy),
        FocusPlan::Clear,
        "cmux not frontmost"
    );
    assert_eq!(
        focus::plan(&s, true, &busy),
        FocusPlan::Clear,
        "no surface known yet"
    );
    s.surfaces.insert("a".into(), "S".into());
    s.unseen.insert(
        "b".into(),
        NewMark {
            claude: true,
            prs: HashSet::new(),
        },
    );
    // `b` still needs a lookup; `a` is known.
    assert_eq!(
        focus::plan(&s, true, &busy),
        FocusPlan::Check {
            lookups: vec![("b".into(), None)],
            known_surface: true
        }
    );
    let looking = HashSet::from(["b".to_string()]);
    assert_eq!(
        focus::plan(&s, true, &looking),
        FocusPlan::Check {
            lookups: vec![],
            known_surface: true
        }
    );
}

#[test]
fn a_focused_session_loses_its_mark_after_two_checks() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    s.unseen.insert(
        "a".into(),
        NewMark {
            claude: true,
            prs: HashSet::new(),
        },
    );
    s.surfaces.insert("a".into(), "S".into());
    s.note_focused(Some("s"));
    assert!(s.unseen.contains_key("a"));
    s.note_focused(Some("S"));
    assert!(!s.unseen.contains_key("a"));
}
