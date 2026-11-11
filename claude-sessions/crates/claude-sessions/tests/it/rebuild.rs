//! The idle rule: how many scene rebuilds 1 s ticks cause, with no window and a simulated clock.

use crate::support::{FakeMeasure, snapshot};
use claude_sessions::rebuild::{RebuildGate, next_rebuild};
use claude_sessions::views::{View, build};
use claude_sessions_core::feed::{FeedItem, FeedOption, FeedQuestion};
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::scene::changed_nodes;
use claude_sessions_core::store::Session;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn t0() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

/// An idle and a busy session whose labels are in minutes, started `age` before `t0()`.
fn minute_sessions(age: Duration) -> Vec<Session> {
    let mut a = Session::new("a", "alpha", "/work/a", "idle");
    a.since = Some(t0() - age);
    let mut b = Session::new("b", "beta", "/work/b", "busy");
    b.since = Some(t0() - age);
    vec![a, b]
}

/// Rebuilds over `ticks` 1 s ticks starting at `t0()`; the first tick builds the first scene.
fn rebuilds(
    snap: &claude_sessions_core::store::Snapshot,
    panel: &PanelState,
    expanded: bool,
    ticks: u64,
) -> Vec<u64> {
    let mut gate = RebuildGate::default();
    let mut at = Vec::new();
    for i in 0..ticks {
        let now = t0() + Duration::from_secs(i);
        if gate.needs_rebuild(snap, expanded, now) {
            at.push(i);
            gate.built(snap, panel, expanded, now);
        }
    }
    at
}

#[test]
fn collapsed_and_idle_never_rebuilds_after_the_first_scene() {
    let snap = snapshot(minute_sessions(Duration::from_secs(5 * 60)));
    let panel = PanelState::new();
    assert_eq!(next_rebuild(&snap, &panel, false, t0()), None);
    assert_eq!(rebuilds(&snap, &panel, false, 60), vec![0]);
}

#[test]
fn expanded_with_minute_labels_rebuilds_once_per_minute_boundary() {
    // Both sessions are 5 min 30 s old at t0, so the labels change at +30 s and +90 s.
    let snap = snapshot(minute_sessions(Duration::from_secs(5 * 60 + 30)));
    let panel = PanelState::new();
    assert_eq!(rebuilds(&snap, &panel, true, 60), vec![0, 30]);
    assert_eq!(rebuilds(&snap, &panel, true, 180), vec![0, 30, 90, 150]);
}

#[test]
fn expanded_with_second_labels_rebuilds_every_tick() {
    let snap = snapshot(minute_sessions(Duration::from_secs(5)));
    let all = rebuilds(&snap, &PanelState::new(), true, 10);
    assert_eq!(all.len(), 10);
}

#[test]
fn a_visible_countdown_rebuilds_every_second() {
    let mut s = Session::new("q", "ask", "/work/q", "waiting");
    s.question = Some(FeedQuestion {
        request_id: "R1".into(),
        created_at: t0(),
        session_id: Some("q".into()),
        questions: vec![FeedItem {
            prompt: "Cache?".into(),
            header: None,
            multi_select: false,
            options: vec![FeedOption {
                id: "o1".into(),
                label: "Redis".into(),
                description: None,
            }],
        }],
    });
    s.question_live = true;
    let snap = snapshot(vec![s]);
    let panel = PanelState::new();
    assert_eq!(rebuilds(&snap, &panel, true, 20).len(), 20);
    // Collapsed, the same countdown is not visible.
    assert_eq!(rebuilds(&snap, &panel, false, 20), vec![0]);
}

#[test]
fn the_stop_arm_window_rebuilds_at_its_end() {
    let snap = snapshot(minute_sessions(Duration::from_secs(5 * 60 + 5)));
    let mut panel = PanelState::new();
    panel.stop_armed.insert("b".into(), t0());
    // Armed `b` waits for the 3 s window; `a` has its own label change at +55 s.
    assert_eq!(
        next_rebuild(&snap, &panel, true, t0()),
        Some(t0() + Duration::from_secs(3))
    );
    assert_eq!(rebuilds(&snap, &panel, true, 60), vec![0, 3, 55]);
}

#[test]
fn a_changed_snapshot_or_expanded_state_rebuilds() {
    let snap = snapshot(minute_sessions(Duration::from_secs(5 * 60 + 30)));
    let panel = PanelState::new();
    let mut gate = RebuildGate::default();
    let now = t0();
    assert!(gate.needs_rebuild(&snap, false, now));
    gate.built(&snap, &panel, false, now);
    assert!(!gate.needs_rebuild(&snap, false, now + Duration::from_secs(1)));
    assert!(gate.needs_rebuild(&snap, true, now + Duration::from_secs(1)));
    let mut other = snap.clone();
    other.sessions[0].status = "busy".into();
    assert!(gate.needs_rebuild(&other, false, now + Duration::from_secs(1)));
}

#[test]
fn a_minute_boundary_changes_only_the_label_node() {
    let snap = snapshot(minute_sessions(Duration::from_secs(5 * 60 + 30)));
    let mut panel = PanelState::new();
    panel.hovering = true;
    let scene = |now: SystemTime| {
        build(&View {
            snap: &snap,
            panel: &panel,
            measure: &FakeMeasure,
            now,
            clock: &|_| "09:41".to_string(),
        })
        .scene
    };
    let (before, after) = (scene(t0()), scene(t0() + Duration::from_secs(30)));
    let diff = changed_nodes(&before, &after);
    assert!(diff.added.is_empty() && diff.removed.is_empty());
    let keys: Vec<&str> = diff.updated.iter().map(|u| u.node.key.0.as_str()).collect();
    assert_eq!(keys.len(), 2, "one card node per session: {keys:?}");
    assert!(keys.iter().all(|k| k.contains("card")), "{keys:?}");
    assert!(
        diff.updated
            .iter()
            .all(|u| u.cmds_changed && !u.frame_changed)
    );
    // Inside the same minute nothing differs.
    let same = scene(t0() + Duration::from_secs(10));
    let none = changed_nodes(&before, &same);
    assert!(none.updated.is_empty() && none.added.is_empty() && none.removed.is_empty());
}
