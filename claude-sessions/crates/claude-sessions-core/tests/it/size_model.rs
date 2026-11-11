//! The size model of the redesigned widget (replaces the Swift `HeightTests` and the height tests of `LayoutTests`).

use crate::support::{feed_question, make_store, make_temp_dir};
use claude_sessions_core::feed::BlockState;
use claude_sessions_core::geometry::Size;
use claude_sessions_core::layout::{
    self, CARD_GAP, CLOSED_ITEM_H, CLOSED_ROW_H, EXPANDED_WIDTH, HEADER_HEIGHT, Item,
    MAX_EXPANDED_HEIGHT, PANEL_PAD, SECTION_TITLE_H, TextMeasure, approval_height, card_layout,
    closed_height, collapsed_width, command_lines, expanded_layout, expanded_size, question_height,
};
use claude_sessions_core::panel_state::{EditKind, Editing, PanelState};
use claude_sessions_core::quota::Quota;
use claude_sessions_core::scene::FontSpec;
use claude_sessions_core::store::{Session, Snapshot};
use claude_sessions_core::transcript::{Pending, PendingState};
use std::time::SystemTime;

/// Fixed char width and line height, wrapping at `max_width` and cutting at `max_lines`.
pub struct FakeMeasure;

impl TextMeasure for FakeMeasure {
    fn measure(&self, text: &str, _font: &FontSpec, max_width: f64, max_lines: u32) -> Size {
        let per_line = ((max_width / 7.0).floor() as usize).max(1);
        let mut lines = 0usize;
        let mut widest = 0usize;
        for line in text.split('\n') {
            let n = line.chars().count();
            lines += n.div_ceil(per_line).max(1);
            widest = widest.max(n.min(per_line));
        }
        if max_lines > 0 {
            lines = lines.min(max_lines as usize);
        }
        Size::new(widest as f64 * 7.0, lines as f64 * 16.0)
    }
}

fn snapshot(sessions: Vec<Session>) -> Snapshot {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    store.sessions = sessions;
    store.snapshot()
}

fn idle(id: &str) -> Session {
    Session::new(id, id, "/x", "idle")
}

fn approval(id: &str, command: &str) -> Session {
    let mut s = Session::new(id, id, "/x", "waiting");
    s.waiting_for = Some("permission prompt".into());
    s.pending = Some(PendingState::One(Pending {
        tool_id: "t1".into(),
        tool: "Bash".into(),
        text: command.into(),
    }));
    s
}

fn panel() -> PanelState {
    PanelState::new()
}

fn card_h(s: Session) -> f64 {
    let snap = snapshot(vec![s.clone()]);
    card_layout(&s, &snap, &panel(), &FakeMeasure).height
}

fn quota(percent: i64) -> Quota {
    Quota {
        five_hour: percent,
        five_hour_reset: SystemTime::now(),
        weekly: 10,
        weekly_reset: SystemTime::now(),
        forecast: None,
    }
}

#[test]
fn collapsed_width_grows_with_the_quota_ring_and_per_group() {
    let one = collapsed_width(&snapshot(vec![idle("a")]));
    let two = collapsed_width(&snapshot(vec![
        idle("a"),
        Session::new("b", "b", "/x", "busy"),
    ]));
    assert!(two > one);
    let mut snap = snapshot(vec![idle("a")]);
    snap.quota = Some(quota(79));
    assert_eq!(collapsed_width(&snap), one, "below 80% shows no ring");
    snap.quota = Some(quota(86));
    assert!(collapsed_width(&snap) > one);
    assert_eq!(collapsed_width(&snapshot(vec![])), layout::COLLAPSED.w);
}

#[test]
fn expanded_height_is_header_titles_cards_gaps_and_closed_row() {
    let mut busy = Session::new("b", "b", "/x", "busy");
    busy.activity = None;
    let (a, b) = (idle("a"), busy);
    let snap = snapshot(vec![a.clone(), b.clone()]);
    let p = panel();
    let ha = card_layout(&a, &snap, &p, &FakeMeasure).height;
    let hb = card_layout(&b, &snap, &p, &FakeMeasure).height;
    let want = PANEL_PAD * 2.0 + HEADER_HEIGHT + 2.0 * SECTION_TITLE_H + ha + hb + 2.0 * CARD_GAP;
    assert_eq!(
        expanded_size(&snap, &p, &FakeMeasure),
        Size::new(EXPANDED_WIDTH, want)
    );
}

#[test]
fn closed_row_adds_its_height_and_grows_with_the_open_list() {
    assert_eq!(closed_height(0, true), 0.0);
    assert_eq!(closed_height(3, false), CLOSED_ROW_H);
    assert_eq!(closed_height(3, true), CLOSED_ROW_H + 3.0 * CLOSED_ITEM_H);
    let mut snap = snapshot(vec![idle("a")]);
    let none = expanded_size(&snap, &panel(), &FakeMeasure).h;
    snap.closed = vec![claude_sessions_core::closed_sessions::ClosedSession {
        id: "c".into(),
        title: "t".into(),
        cwd: "/x".into(),
        branch: None,
        closed_at: SystemTime::now(),
    }];
    let shut = expanded_size(&snap, &panel(), &FakeMeasure).h;
    assert_eq!(shut - none, CLOSED_ROW_H);
    let mut open = panel();
    open.closed_open = true;
    assert_eq!(
        expanded_size(&snap, &open, &FakeMeasure).h - shut,
        CLOSED_ITEM_H
    );
}

#[test]
fn approval_card_grows_per_command_line() {
    let one = card_h(approval("a", "ls"));
    let three = card_h(approval("a", "ls\nls\nls"));
    assert!(three > one);
    assert_eq!(three - one, 2.0 * layout::CMD_LINE_H);
    assert_eq!(command_lines("a\nb\nc"), 3);
    assert_eq!(command_lines(&"x\n".repeat(50)), layout::CMD_MAX_LINES);
    assert!(approval_height(1, true) > approval_height(1, false));
}

#[test]
fn question_card_grows_per_option_row() {
    let mut q = feed_question("R1", SystemTime::now());
    let base = question_height(&q, BlockState::Form, None);
    let mut extra = q.questions[0].options[0].clone();
    extra.description = None;
    q.questions[0].options.push(extra);
    assert_eq!(
        question_height(&q, BlockState::Form, None) - base,
        layout::OPTION_H
    );
    assert!(question_height(&q, BlockState::Form, Some(0)) > base);
    assert!(question_height(&q, BlockState::Sent, None) < base);
    let mut s = Session::new("a", "a", "/x", "waiting");
    s.question = Some(q);
    s.question_live = true;
    s.block = Some(BlockState::Form);
    assert!(card_h(s) > card_h(approval("a", "ls")));
}

#[test]
fn two_line_title_is_taller_than_one_line() {
    let short = card_h(Session::new("a", "short", "/x", "waiting"));
    let long = card_h(Session::new("a", &"w".repeat(200), "/x", "waiting"));
    assert_eq!(long - short, 16.0);
}

#[test]
fn reply_open_menu_pr_lines_and_editor_add_height() {
    // A waiting card has no right-column button, so its height is its content.
    let plain = Session::new("a", "a", "/x", "waiting");
    let base = card_h(plain.clone());
    let mut s = plain;
    s.last_reply = Some("done".into());
    assert_eq!(card_h(s.clone()) - base, layout::REPLY_GAP + 16.0);
    s.todo = Some(claude_sessions_core::tasks::Todo {
        done: 1,
        total: 2,
        current: None,
    });
    s.activity = Some("Bash ls".into());
    let with_lines = card_h(s.clone());
    let snap = snapshot(vec![s.clone()]);
    let mut p = panel();
    p.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    let editing = card_layout(&s, &snap, &p, &FakeMeasure).height;
    assert_eq!(editing - with_lines, layout::editor_height());
}

#[test]
fn quick_button_gives_the_card_a_minimum_height() {
    assert!(card_h(idle("a")) >= layout::CARD_MIN_BUTTON_H);
    assert!(card_h(approval("a", "ls")) > layout::CARD_MIN_BUTTON_H);
}

#[test]
fn layout_items_follow_the_sections_and_height_is_capped() {
    let sessions: Vec<Session> = (0..30).map(|i| idle(&format!("s{i}"))).collect();
    let snap = snapshot(sessions);
    let l = expanded_layout(&snap, &panel(), &FakeMeasure);
    assert_eq!(l.size.h, MAX_EXPANDED_HEIGHT);
    assert!(l.content_h > MAX_EXPANDED_HEIGHT);
    let ids: Vec<&str> = l
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Card { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 30);
    assert_eq!(ids[0], "s0");
}

#[test]
fn a_failed_send_shows_the_short_fallback_note_so_the_panel_shrinks() {
    // Re-homes the phase 5 `Layout.expanded(.., blocks:)` assertion of `failed_send_falls_back_to_the_fallback_note`.
    let mut s = Session::new("a", "a", "/x", "waiting");
    s.question = Some(feed_question("R1", SystemTime::now()));
    s.question_live = true;
    s.block = Some(BlockState::Form);
    let form = expanded_size(&snapshot(vec![s.clone()]), &panel(), &FakeMeasure).h;
    s.block = Some(BlockState::Fallback);
    let fallback = expanded_size(&snapshot(vec![s.clone()]), &panel(), &FakeMeasure).h;
    assert!(fallback < form);
    s.block = Some(BlockState::Sent);
    assert_eq!(
        expanded_size(&snapshot(vec![s]), &panel(), &FakeMeasure).h,
        fallback,
        "sent and fallback are both one note line"
    );
}
