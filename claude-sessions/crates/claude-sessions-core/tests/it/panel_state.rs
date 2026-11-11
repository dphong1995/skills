use claude_sessions_core::feed::{FeedItem, FeedOption, FeedQuestion};
use claude_sessions_core::panel_state::{EditKind, Editing, HotkeyToggle, PanelState};
use std::time::{Duration, UNIX_EPOCH};

fn ids(s: &[&str]) -> Vec<String> {
    s.iter().map(|s| s.to_string()).collect()
}

fn two_questions(multi: bool) -> FeedQuestion {
    let opts = || {
        ["x", "y"]
            .iter()
            .map(|l| FeedOption {
                id: l.to_string(),
                label: l.to_string(),
                description: None,
            })
            .collect::<Vec<_>>()
    };
    FeedQuestion {
        request_id: "R1".into(),
        created_at: UNIX_EPOCH,
        session_id: Some("a".into()),
        questions: vec![
            FeedItem {
                prompt: "A".into(),
                header: None,
                multi_select: multi,
                options: opts(),
            },
            FeedItem {
                prompt: "B".into(),
                header: None,
                multi_select: false,
                options: opts(),
            },
        ],
    }
}

#[test]
fn note_row_hover_enter_and_exit() {
    let mut panel = PanelState::new();
    panel.note_row_hover("a", true, UNIX_EPOCH);
    assert_eq!(panel.hovered_row.as_deref(), Some("a"));
    panel.note_row_hover("a", false, UNIX_EPOCH);
    assert_eq!(panel.hovered_row, None);
}

#[test]
fn note_row_hover_enter_overrides_missed_exit() {
    let mut panel = PanelState::new();
    panel.note_row_hover("a", true, UNIX_EPOCH);
    panel.note_row_hover("b", true, UNIX_EPOCH);
    assert_eq!(panel.hovered_row.as_deref(), Some("b"));
}

#[test]
fn note_row_hover_exit_of_other_row_is_ignored() {
    let mut panel = PanelState::new();
    panel.note_row_hover("a", true, UNIX_EPOCH);
    panel.note_row_hover("b", true, UNIX_EPOCH);
    panel.note_row_hover("a", false, UNIX_EPOCH);
    assert_eq!(panel.hovered_row.as_deref(), Some("b"));
    panel.note_row_hover("b", false, UNIX_EPOCH);
    assert_eq!(panel.hovered_row, None);
}

#[test]
fn order_change_keeps_row_that_stayed_in_place() {
    let mut panel = PanelState::new();
    panel.hovered_row = Some("a".into());
    panel.note_order_change(&ids(&["a", "b", "c"]), &ids(&["a", "c", "b"]));
    assert_eq!(panel.hovered_row.as_deref(), Some("a"));
}

#[test]
fn order_change_clears_row_that_moved() {
    let mut panel = PanelState::new();
    panel.hovered_row = Some("b".into());
    panel.note_order_change(&ids(&["a", "b"]), &ids(&["b", "a"]));
    assert_eq!(panel.hovered_row, None);
}

#[test]
fn order_change_clears_row_that_vanished() {
    let mut panel = PanelState::new();
    panel.hovered_row = Some("b".into());
    panel.note_order_change(&ids(&["a", "b"]), &ids(&["a"]));
    assert_eq!(panel.hovered_row, None);
}

#[test]
fn dwell_target_only_for_fresh_row() {
    let mut panel = PanelState::new();
    let fresh = |id: &str| id == "a";
    assert_eq!(panel.dwell_target(fresh), None);
    panel.hovered_row = Some("a".into());
    assert_eq!(panel.dwell_target(fresh).as_deref(), Some("a"));
    panel.hovered_row = Some("b".into());
    assert_eq!(panel.dwell_target(fresh), None);
    panel.hovered_row = Some("a".into());
    assert_eq!(panel.dwell_target(|_| false), None);
}

#[test]
fn single_select_pick_replaces_and_clears_other() {
    let mut panel = PanelState::new();
    let q = two_questions(false);
    panel.pick(&q, 0, "x");
    panel.pick(&q, 0, "y");
    assert_eq!(panel.answers["R1"], vec![vec!["y".to_string()], vec![]]);
    panel.set_other(&q, 0, "free");
    assert!(panel.answers["R1"][0].is_empty());
    assert_eq!(panel.others["R1"][&0], "free");
    panel.pick(&q, 0, "x");
    assert!(!panel.others["R1"].contains_key(&0));
    assert_eq!(panel.answers["R1"][0], vec!["x".to_string()]);
}

#[test]
fn multi_select_pick_toggles() {
    let mut panel = PanelState::new();
    let q = two_questions(true);
    panel.pick(&q, 0, "x");
    panel.pick(&q, 0, "y");
    assert_eq!(panel.answers["R1"][0], ids(&["x", "y"]));
    panel.pick(&q, 0, "x");
    assert_eq!(panel.answers["R1"][0], ids(&["y"]));
    panel.set_other(&q, 0, "more");
    assert_eq!(panel.answers["R1"][0], ids(&["y"]));
    assert_eq!(panel.others["R1"][&0], "more");
}

#[test]
fn dwell_marks_seen_after_one_second() {
    let mut panel = PanelState::new();
    let t = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    panel.note_row_hover("a", true, t);
    let fresh = |id: &str| id == "a";
    assert_eq!(
        panel.due_mark_seen(t + Duration::from_millis(900), fresh),
        None
    );
    assert_eq!(
        panel
            .due_mark_seen(t + Duration::from_secs(1), fresh)
            .as_deref(),
        Some("a")
    );
}

#[test]
fn dwell_ignores_stale_row_and_starts_when_it_turns_fresh() {
    let mut panel = PanelState::new();
    let t = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    panel.note_row_hover("a", true, t);
    assert_eq!(
        panel.due_mark_seen(t + Duration::from_secs(5), |_| false),
        None
    );
    // The mark appears while hovered: the second counts from then, as SwiftUI's `.task(id:)` restarts.
    let fresh = |_: &str| true;
    assert_eq!(panel.due_mark_seen(t + Duration::from_secs(6), fresh), None);
    assert_eq!(
        panel
            .due_mark_seen(t + Duration::from_secs(7), fresh)
            .as_deref(),
        Some("a")
    );
}

#[test]
fn dwell_cancelled_by_exit() {
    let mut panel = PanelState::new();
    let t = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    panel.note_row_hover("a", true, t);
    panel.note_row_hover("a", false, t + Duration::from_millis(500));
    assert_eq!(
        panel.due_mark_seen(t + Duration::from_secs(2), |_| true),
        None
    );
}

#[test]
fn collapse_clears_open_menus() {
    let mut panel = PanelState::new();
    panel.open_menus.insert("a".into());
    panel.closed_open = true;
    panel.on_collapse();
    assert!(panel.open_menus.is_empty());
    assert!(!panel.closed_open);
}

#[test]
fn on_order_change_reconciles_selection_and_hover() {
    let mut panel = PanelState::new();
    panel.selected = Some("b".into());
    panel.hovered_row = Some("c".into());
    panel.on_order_change(&ids(&["a", "b", "c"]), &ids(&["a", "c"]));
    assert_eq!(panel.selected.as_deref(), Some("c"));
    // "c" moved from index 2 to 1.
    assert_eq!(panel.hovered_row, None);
    panel.hovered_row = Some("a".into());
    panel.selected = Some("c".into());
    panel.on_order_change(&ids(&["a", "c"]), &ids(&["a"]));
    assert_eq!(panel.selected.as_deref(), Some("a"));
    assert_eq!(panel.hovered_row.as_deref(), Some("a"));
}

#[test]
fn move_selection_clamps_and_picks_first() {
    let mut panel = PanelState::new();
    let list = ids(&["a", "b", "c"]);
    panel.move_selection(1, &list);
    assert_eq!(panel.selected.as_deref(), Some("a"));
    panel.move_selection(5, &list);
    assert_eq!(panel.selected.as_deref(), Some("c"));
    panel.move_selection(-9, &list);
    assert_eq!(panel.selected.as_deref(), Some("a"));
    panel.selected = Some("gone".into());
    panel.move_selection(1, &list);
    assert_eq!(panel.selected.as_deref(), Some("a"));
}

#[test]
fn pointer_pin_and_hotkey_toggle() {
    let mut p = PanelState::new();
    assert!(p.pointer_wants(true));
    assert!(!p.pointer_wants(false));
    p.pin_with_keyboard();
    assert!(p.pointer_wants(false));
    assert!(p.pointer_wants(true));
    // Keyboard mode keeps a pinned card open after the pointer left.
    assert!(p.pointer_wants(false));
    p.keyboard = false;
    assert!(!p.pointer_wants(false));
    assert!(!p.pinned_open);
    assert_eq!(PanelState::hotkey_toggle(&[]), HotkeyToggle::Open);
    let mut k = PanelState::new();
    k.keyboard = true;
    let plain = PanelState::new();
    assert_eq!(PanelState::hotkey_toggle(&[&plain]), HotkeyToggle::Keyboard);
    assert_eq!(
        PanelState::hotkey_toggle(&[&plain, &k]),
        HotkeyToggle::Close
    );
}

#[test]
fn wants_key_and_close() {
    let mut p = PanelState::new();
    assert!(!p.wants_key());
    p.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    assert!(p.wants_key());
    assert!(!p.should_hand_back_key(true, false));
    p.close();
    assert!(!p.wants_key() && !p.hovering);
    assert!(p.should_hand_back_key(true, false));
    assert!(!p.should_hand_back_key(true, true));
}
