//! Ported from Swift `KeyboardTests.swift`. `press` runs `KeyCommand::resolve`; the `handle_key_*` tests cover what the Rust
//! `handle_key` adds (it applies `Close`/`Move`/`Edit`/`Pick` to the panel).

use crate::support::*;
use claude_sessions_core::action::Action;
use claude_sessions_core::feed::{BlockState, FeedQuestion};
use claude_sessions_core::keyboard::{
    KeyCommand, KeyEvent, KeyOutcome, Mods, handle_key, key_code,
};
use claude_sessions_core::panel_state::{EditKind, Editing, HotkeyToggle, PanelState};
use claude_sessions_core::store::{Session, Snapshot};
use claude_sessions_core::transcript::{Pending, PendingState};
use std::time::SystemTime;

fn question(counts: &[usize]) -> FeedQuestion {
    use claude_sessions_core::feed::{FeedItem, FeedOption};
    FeedQuestion {
        request_id: "r".into(),
        created_at: SystemTime::now(),
        session_id: Some("s".into()),
        questions: counts
            .iter()
            .enumerate()
            .map(|(i, &n)| FeedItem {
                prompt: format!("q{i}"),
                header: None,
                multi_select: false,
                options: (0..n)
                    .map(|j| FeedOption {
                        id: format!("{i}-{j}"),
                        label: format!("o{i}-{j}"),
                        description: None,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn ids(s: &[&str]) -> Vec<String> {
    s.iter().map(|s| s.to_string()).collect()
}

const NONE: Mods = Mods {
    command: false,
    control: false,
    option: false,
};
const COMMAND: Mods = Mods {
    command: true,
    control: false,
    option: false,
};
const OPTION: Mods = Mods {
    command: false,
    control: false,
    option: true,
};

/// A keyboard-mode panel with the first session selected, and the snapshot of `sessions`.
fn panel_with(f: impl FnOnce(&mut PanelState)) -> PanelState {
    let mut p = PanelState::new();
    f(&mut p);
    p
}

fn keyboard_setup(sessions: Vec<Session>) -> (Snapshot, PanelState) {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    store.sessions = sessions;
    let panel = panel_with(|p| {
        p.keyboard = true;
        p.selected = store.sessions.first().map(|s| s.id.clone());
    });
    (store.snapshot(), panel)
}

fn idle(id: &str) -> Session {
    Session::new(id, id, "/x", "idle")
}

fn waiting(command: &str) -> Session {
    let mut s = Session::new("a", "a", "/x", "waiting");
    s.waiting_for = Some("permission prompt".into());
    s.pending = Some(PendingState::One(Pending {
        tool_id: "t1".into(),
        tool: "Bash".into(),
        text: command.into(),
    }));
    s
}

fn asking() -> Session {
    let mut s = Session::new("a", "a", "/x", "waiting");
    s.question = Some(feed_question("R1", SystemTime::now()));
    s.question_live = true;
    s.block = Some(BlockState::Form);
    s
}

fn ev(key: &str, code: u16, mods: Mods) -> KeyEvent {
    KeyEvent {
        key_code: code,
        chars: key.into(),
        mods,
    }
}

fn press(key: &str, snap: &Snapshot, panel: &PanelState) -> KeyCommand {
    press_with(key, snap, panel, 0, NONE)
}

fn press_with(key: &str, snap: &Snapshot, panel: &PanelState, code: u16, mods: Mods) -> KeyCommand {
    KeyCommand::resolve(&ev(key, code, mods), snap, panel)
}

fn deny(tool: &str) -> KeyCommand {
    KeyCommand::Edit(Editing {
        id: "a".into(),
        kind: EditKind::Deny,
        tool_id: Some(tool.into()),
    })
}

fn approve() -> KeyCommand {
    KeyCommand::Action(Action::Approve {
        session: "a".into(),
        tool_id: "t1".into(),
    })
}

fn long_command() -> String {
    (1..=6)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn keyboard_keeps_pinned_card_open_on_leave() {
    let mut panel = PanelState::new();
    panel.pin_with_keyboard();
    assert!(panel.pointer_wants(true));
    assert!(panel.pointer_wants(false));
    assert!(panel.pinned_open && panel.pinned_entered);
    panel.keyboard = false;
    assert!(!panel.pointer_wants(false));
    assert!(!panel.pinned_open && !panel.pinned_entered);
}

#[test]
fn pin_never_entered_stays_open() {
    let mut panel = PanelState::new();
    panel.pinned_open = true;
    assert!(panel.pointer_wants(false));
    assert!(panel.pinned_open);
    assert!(!PanelState::new().pointer_wants(false));
}

#[test]
fn move_selection() {
    let mut panel = PanelState::new();
    let ids = ids(&["a", "b", "c"]);
    panel.move_selection(1, &ids);
    assert_eq!(panel.selected.as_deref(), Some("a"));
    panel.move_selection(1, &ids);
    panel.move_selection(1, &ids);
    panel.move_selection(1, &ids);
    assert_eq!(panel.selected.as_deref(), Some("c"));
    panel.move_selection(-1, &ids);
    panel.move_selection(-1, &ids);
    panel.move_selection(-1, &ids);
    assert_eq!(panel.selected.as_deref(), Some("a"));
}

#[test]
fn selection_survives_removal() {
    let mut panel = PanelState::new();
    panel.selected = Some("b".into());
    panel.reconcile_selection(&ids(&["a", "b", "c"]), &ids(&["a", "c"]));
    assert_eq!(panel.selected.as_deref(), Some("c"));
    panel.selected = Some("c".into());
    panel.reconcile_selection(&ids(&["a", "c"]), &ids(&["a"]));
    assert_eq!(panel.selected.as_deref(), Some("a"));
    panel.reconcile_selection(&ids(&["a"]), &[]);
    assert_eq!(panel.selected, None);
}

#[test]
fn digit_maps_across_questions() {
    let q = question(&[3, 3]);
    assert_eq!(PanelState::option_at(&q, 4), Some((1, "o1-0".to_string())));
    assert_eq!(PanelState::option_at(&q, 7), None);
}

#[test]
fn arrows_move_selection() {
    let (snap, panel) = keyboard_setup(vec![idle("a")]);
    assert_eq!(
        press_with("", &snap, &panel, key_code::UP, NONE),
        KeyCommand::Move(-1)
    );
    assert_eq!(
        press_with("", &snap, &panel, key_code::DOWN, NONE),
        KeyCommand::Move(1)
    );
    assert_eq!(
        press_with("", &snap, &panel, key_code::DOWN, COMMAND),
        KeyCommand::Pass
    );
}

#[test]
fn return_jumps() {
    let (snap, panel) = keyboard_setup(vec![idle("a")]);
    let jump = KeyCommand::Action(Action::Jump("a".into()));
    assert_eq!(press_with("", &snap, &panel, key_code::RETURN, NONE), jump);
    assert_eq!(
        press_with("", &snap, &panel, key_code::KEYPAD_ENTER, NONE),
        jump
    );
    assert_eq!(
        press_with("", &snap, &panel, key_code::RETURN, OPTION),
        KeyCommand::Pass
    );
}

#[test]
fn approve_needs_approvable_and_full_command() {
    let (snap, panel) = keyboard_setup(vec![waiting("ls")]);
    assert_eq!(press("a", &snap, &panel), approve());
    assert_eq!(press("A", &snap, &panel), approve());
    let (snap, panel) = keyboard_setup(vec![idle("a")]);
    assert_eq!(press("a", &snap, &panel), KeyCommand::Beep);
    let (mut snap, panel) = keyboard_setup(vec![waiting(&long_command())]);
    assert_eq!(press("a", &snap, &panel), KeyCommand::Beep);
    snap.shown_commands.insert("t1".into());
    assert_eq!(press("a", &snap, &panel), approve());
    let (mut snap, panel) = keyboard_setup(vec![waiting(&"x\n".repeat(20))]);
    snap.shown_commands.insert("t1".into());
    assert_eq!(press("a", &snap, &panel), KeyCommand::Beep);
}

#[test]
fn deny_follows_the_deny_button() {
    let (snap, panel) = keyboard_setup(vec![waiting("ls")]);
    assert_eq!(press("d", &snap, &panel), deny("t1"));
    let (snap, panel) = keyboard_setup(vec![idle("a")]);
    assert_eq!(press("d", &snap, &panel), KeyCommand::Beep);
    // Only `duyệt trong cmux` is offered for a long command that is not expanded, or too long: D beeps too.
    let (mut snap, panel) = keyboard_setup(vec![waiting(&long_command())]);
    assert_eq!(press("d", &snap, &panel), KeyCommand::Beep);
    snap.shown_commands.insert("t1".into());
    assert_eq!(press("d", &snap, &panel), deny("t1"));
    let (mut snap, panel) = keyboard_setup(vec![waiting(&"x\n".repeat(20))]);
    snap.shown_commands.insert("t1".into());
    assert_eq!(press("d", &snap, &panel), KeyCommand::Beep);
}

#[test]
fn reply_needs_idle_session_in_cmux() {
    let (mut snap, panel) = keyboard_setup(vec![idle("a")]);
    let reply = KeyCommand::Edit(Editing {
        id: "a".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    assert_eq!(press("r", &snap, &panel), reply);
    snap.surface_misses.insert("a".into());
    assert_eq!(press("r", &snap, &panel), KeyCommand::Beep);
    let (snap, panel) = keyboard_setup(vec![Session::new("a", "a", "/x", "busy")]);
    assert_eq!(press("r", &snap, &panel), KeyCommand::Beep);
}

#[test]
fn digits_pick_options_and_beep_beyond_count() {
    let (snap, panel) = keyboard_setup(vec![asking()]);
    let pick = |label: &str| KeyCommand::Pick {
        question: 0,
        label: label.into(),
    };
    assert_eq!(press("1", &snap, &panel), pick("Red"));
    assert_eq!(press("2", &snap, &panel), pick("Blue"));
    assert_eq!(press("3", &snap, &panel), KeyCommand::Beep);
    assert_eq!(press("9", &snap, &panel), KeyCommand::Beep);
    assert_eq!(press("0", &snap, &panel), KeyCommand::Pass);
    let (plain, plain_panel) = keyboard_setup(vec![idle("a")]);
    assert_eq!(press("1", &plain, &plain_panel), KeyCommand::Beep);
}

#[test]
fn command_return_submits_only_a_complete_form() {
    let (snap, mut panel) = keyboard_setup(vec![asking()]);
    assert_eq!(
        press_with("", &snap, &panel, key_code::RETURN, COMMAND),
        KeyCommand::Beep
    );
    panel.pick(&feed_question("R1", SystemTime::now()), 0, "Blue");
    assert_eq!(
        press_with("", &snap, &panel, key_code::RETURN, COMMAND),
        KeyCommand::Action(Action::Answer {
            session: "a".into(),
            request_id: "R1".into(),
            answers: vec![vec!["Blue".into()]],
            other: Default::default(),
        })
    );
    let (plain, plain_panel) = keyboard_setup(vec![idle("a")]);
    assert_eq!(
        press_with("", &plain, &plain_panel, key_code::RETURN, COMMAND),
        KeyCommand::Beep
    );
}

#[test]
fn escape_closes() {
    let (snap, mut panel) = keyboard_setup(vec![idle("a")]);
    assert_eq!(
        press_with("", &snap, &panel, key_code::ESCAPE, NONE),
        KeyCommand::Close
    );
    panel.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    assert_eq!(
        press_with("", &snap, &panel, key_code::ESCAPE, NONE),
        KeyCommand::Pass
    );
}

#[test]
fn any_key_passes_while_editing() {
    let (snap, mut panel) = keyboard_setup(vec![waiting("ls")]);
    panel.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Deny,
        tool_id: Some("t1".into()),
    });
    for key in ["a", "d", "r", "1", "x"] {
        assert_eq!(press(key, &snap, &panel), KeyCommand::Pass);
    }
    assert_eq!(
        press_with("", &snap, &panel, key_code::RETURN, NONE),
        KeyCommand::Pass
    );
    assert_eq!(
        press_with("", &snap, &panel, key_code::UP, NONE),
        KeyCommand::Pass
    );
}

#[test]
fn unbound_keys_pass_without_selection() {
    let (snap, panel) = keyboard_setup(vec![]);
    assert_eq!(press("x", &snap, &panel), KeyCommand::Pass);
    assert_eq!(press("0", &snap, &panel), KeyCommand::Pass);
    assert_eq!(press_with("a", &snap, &panel, 0, COMMAND), KeyCommand::Pass);
    assert_eq!(press("ab", &snap, &panel), KeyCommand::Pass);
    for key in ["a", "d", "r", "1"] {
        assert_eq!(press(key, &snap, &panel), KeyCommand::Beep);
    }
    assert_eq!(
        press_with("", &snap, &panel, key_code::RETURN, NONE),
        KeyCommand::Beep
    );
    let (full, full_panel) = keyboard_setup(vec![idle("a")]);
    assert_eq!(press("x", &full, &full_panel), KeyCommand::Pass);
}

#[test]
fn not_in_keyboard_mode_passes() {
    let (snap, mut panel) = keyboard_setup(vec![idle("a")]);
    panel.keyboard = false;
    assert_eq!(press("a", &snap, &panel), KeyCommand::Pass);
    assert_eq!(
        press_with("", &snap, &panel, key_code::ESCAPE, NONE),
        KeyCommand::Pass
    );
}

#[test]
fn hotkey_opens_when_nothing_is_open() {
    assert_eq!(PanelState::hotkey_toggle(&[]), HotkeyToggle::Open);
}

#[test]
fn hotkey_closes_only_a_keyboard_card() {
    let pinned = panel_with(|p| {
        p.hovering = true;
        p.pinned_open = true;
    });
    assert_eq!(
        PanelState::hotkey_toggle(&[&pinned]),
        HotkeyToggle::Keyboard
    );
    let typing = panel_with(|p| {
        p.hovering = true;
        p.keyboard = true;
    });
    assert_eq!(PanelState::hotkey_toggle(&[&typing]), HotkeyToggle::Close);
}

#[test]
fn hotkey_on_a_hover_card_takes_the_keyboard_and_pins() {
    let mut hover = panel_with(|p| {
        p.hovering = true;
    });
    assert_eq!(PanelState::hotkey_toggle(&[&hover]), HotkeyToggle::Keyboard);
    hover.pin_with_keyboard();
    assert!(hover.keyboard && hover.pinned_open && !hover.pinned_entered);
}

#[test]
fn close_clears_everything() {
    let mut panel = panel_with(|p| {
        p.hovering = true;
        p.pinned_open = true;
        p.pinned_entered = true;
        p.keyboard = true;
        p.selected = Some("a".into());
        p.editing = Some(Editing {
            id: "a".into(),
            kind: EditKind::Reply,
            tool_id: None,
        });
    });
    panel.close();
    assert!(
        !panel.hovering
            && !panel.pinned_open
            && !panel.pinned_entered
            && !panel.keyboard
            && panel.editing.is_none()
            && panel.selected.is_none()
    );
    assert!(!panel.wants_key());
}

#[test]
fn editor_close_keeps_keyboard() {
    let mut panel = panel_with(|p| {
        p.keyboard = true;
    });
    panel.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    panel.editing = None;
    assert!(panel.wants_key());
    assert!(!panel.should_hand_back_key(true, false));
}

#[test]
fn key_is_handed_back_only_when_nothing_wants_it() {
    let mut panel = PanelState::new();
    panel.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    assert!(panel.wants_key());
    panel.editing = None;
    assert!(panel.should_hand_back_key(true, false));
    assert!(!panel.should_hand_back_key(false, false));
    assert!(!panel.should_hand_back_key(true, true));
}

// Rust-only: `handle_key` applies the mutating commands to the panel.

#[test]
fn handle_key_applies_move_close_edit_and_pick() {
    let (snap, mut panel) = keyboard_setup(vec![idle("a"), idle("b")]);
    assert_eq!(
        handle_key(&ev("", key_code::DOWN, NONE), &snap, &mut panel),
        KeyOutcome::Consumed(None)
    );
    assert_eq!(panel.selected.as_deref(), Some("b"));
    assert_eq!(
        handle_key(&ev("r", 0, NONE), &snap, &mut panel),
        KeyOutcome::Consumed(None)
    );
    assert_eq!(
        panel.editing.as_ref().map(|e| e.kind),
        Some(EditKind::Reply)
    );
    assert_eq!(
        handle_key(&ev("x", 0, NONE), &snap, &mut panel),
        KeyOutcome::PassThrough
    );
    panel.editing = None;
    assert_eq!(
        handle_key(&ev("", key_code::RETURN, NONE), &snap, &mut panel),
        KeyOutcome::Consumed(Some(Action::Jump("b".into())))
    );
    assert_eq!(
        handle_key(&ev("", key_code::ESCAPE, NONE), &snap, &mut panel),
        KeyOutcome::Consumed(None)
    );
    assert!(!panel.keyboard && panel.selected.is_none());

    let (snap, mut panel) = keyboard_setup(vec![asking()]);
    assert_eq!(
        handle_key(&ev("2", 0, NONE), &snap, &mut panel),
        KeyOutcome::Consumed(None)
    );
    assert_eq!(panel.answers["R1"], vec![vec!["Blue".to_string()]]);
    assert_eq!(
        handle_key(&ev("9", 0, NONE), &snap, &mut panel),
        KeyOutcome::Beep
    );
}
