//! The size model of the redesigned widget (replaces the Swift `HeightTests` and the height tests of `LayoutTests`).

use crate::support::{feed_question, make_store, make_temp_dir};
use claude_sessions_core::feed::BlockState;
use claude_sessions_core::geometry::Size;
use claude_sessions_core::layout::{
    self, CARD_GAP, CLOSED_ITEM_H, CLOSED_ROW_H, EXPANDED_WIDTH, HEADER_HEIGHT, Item,
    MAX_EXPANDED_HEIGHT, PANEL_PAD, SECTION_TITLE_H, TextMeasure, approval_height, card_layout,
    closed_height, collapsed_width, expanded_layout, expanded_size, question_height,
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
        layout::OPTION_ROW_H
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
    // One line is held at the 20 pt title line; two measured lines (16 pt each here) are taller than that.
    assert_eq!(long - short, 2.0 * 16.0 - layout::TITLE_LINE_H);
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
fn a_row_is_at_least_the_glyph_with_its_padding_and_an_approval_is_taller() {
    let idle_h = card_h(idle("a"));
    assert!(idle_h >= layout::CARD_PAD_Y * 2.0 + 20.0);
    assert!(card_h(approval("a", "ls")) > idle_h);
}

#[test]
fn the_pr_list_is_the_gap_and_one_fixed_item_per_pr() {
    assert_eq!(layout::PR_ITEM_H, 42.0);
    assert_eq!(layout::PR_LIST_RADIUS, 10.0);
    assert_eq!(layout::PR_BUTTON_H, 22.0);
    assert_eq!(layout::prs_height(0), layout::BLOCK_GAP);
    assert_eq!(layout::prs_height(1), layout::BLOCK_GAP + 42.0);
    assert_eq!(layout::prs_height(3), layout::BLOCK_GAP + 3.0 * 42.0);
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

#[test]
fn reply_height_is_measured_on_the_text_that_is_drawn() {
    // 99 chars with markers, 91 without: one line at 98 chars per line.
    let reply = format!("**{}** **{}**", "a".repeat(45), "a".repeat(45));
    assert_eq!(reply.chars().count(), 99);
    let mut s = idle("a");
    s.last_reply = Some(reply);
    let marked = card_h(s.clone());
    s.last_reply = Some(format!("{} {}", "a".repeat(45), "a".repeat(45)));
    assert_eq!(marked, card_h(s), "markers do not add a line");
}

#[test]
fn an_approval_result_line_adds_its_own_height_in_place_of_the_approval_block() {
    use claude_sessions_core::layout::{Block, SENT_LINE_H, sent_line_height};
    use claude_sessions_core::store::{SendKey, SendKind, SendState};
    assert_eq!(sent_line_height(), SENT_LINE_H);
    let s = Session::new("a", "a", "/x", "waiting");
    let mut snap = snapshot(vec![s.clone()]);
    let base = card_layout(&s, &snap, &panel(), &FakeMeasure);
    assert!(base.find(Block::SentLine).is_none());
    snap.sends.insert(
        SendKey::new("a", SendKind::Deny),
        (SendState::Sent, SystemTime::now()),
    );
    let with = card_layout(&s, &snap, &panel(), &FakeMeasure);
    assert_eq!(with.height - base.height, SENT_LINE_H);
    assert!(with.find(Block::SentLine).is_some());
}

#[test]
fn the_command_box_ends_one_padding_under_its_last_line() {
    let box_h =
        approval_height(1, false) - layout::CMD_MARGIN - layout::BLOCK_GAP - layout::BUTTON_H;
    assert_eq!(
        box_h,
        layout::CMD_PAD + layout::CMD_LINE_H + layout::CMD_PAD
    );
    // 11.5 pt monospaced text is 14 pt per line.
    assert_eq!(layout::CMD_LINE_H, 14.0);
}

#[test]
fn a_generic_state_label_is_not_reserved_room_in_the_title() {
    let w =
        |s: Session| card_layout(&s, &snapshot(vec![s.clone()]), &panel(), &FakeMeasure).title_w;
    let idle = Session::new("a", "a", "/w", "idle");
    let mut bg = Session::new("b", "b", "/w", "busy");
    bg.background_agents = Some(2);
    assert_eq!(layout::state_label(&idle), "");
    assert_eq!(
        layout::state_label(&Session::new("c", "c", "/w", "busy")),
        ""
    );
    assert_eq!(layout::state_label(&bg), "chờ 2 agent nền");
    // The state word is its own text in the subtitle now: it takes no room from the title either way.
    assert_eq!(w(idle), w(bg));
}

#[test]
fn the_title_is_one_line_high_at_least() {
    // A measure that finds no height at all still leaves the title room for one line.
    struct Flat;
    impl TextMeasure for Flat {
        fn measure(&self, _: &str, _: &FontSpec, _: f64, _: u32) -> Size {
            Size::new(0.0, 0.0)
        }
    }
    let s = Session::new("a", "a", "/w", "idle");
    let snap = snapshot(vec![s.clone()]);
    let flat = card_layout(&s, &snap, &panel(), &Flat);
    assert_eq!(
        (flat.title_text_h, flat.title_h),
        (0.0, layout::TITLE_LINE_H)
    );
    let l = card_layout(&s, &snap, &panel(), &FakeMeasure);
    assert!(l.title_h >= layout::TITLE_LINE_H);
    assert_eq!(l.title_h, l.title_text_h.max(layout::TITLE_LINE_H));
}

#[test]
fn keyboard_mode_adds_the_hint_line_to_the_panel() {
    let snap = snapshot(vec![approval("a", "ls")]);
    let mut keys = panel();
    keys.keyboard = true;
    assert_eq!(
        expanded_size(&snap, &keys, &FakeMeasure).h
            - expanded_size(&snap, &panel(), &FakeMeasure).h,
        layout::KEY_HINT_H
    );
}

use claude_sessions_core::layout::{ApprovalLook, ApprovalRow, Block};

fn look(snap: &Snapshot, s: &Session) -> ApprovalLook {
    card_layout(s, snap, &panel(), &FakeMeasure)
        .approval
        .expect("approval block")
}

#[test]
fn command_fit_thresholds() {
    use claude_sessions_core::store::{CommandFit, command_fit};
    assert_eq!(command_fit("npm test"), CommandFit::Short);
    // 4 lines, 240 characters including the 3 newlines
    let four = [59, 59, 59, 60]
        .iter()
        .map(|n| "a".repeat(*n))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(four.chars().count(), 240);
    assert_eq!(command_fit(&four), CommandFit::Short);
    assert_eq!(command_fit(&["a"; 5].join("\n")), CommandFit::Long);
    assert_eq!(command_fit(&"a".repeat(241)), CommandFit::Long);
    assert_eq!(command_fit(&["a"; 13].join("\n")), CommandFit::TooLong);
    assert_eq!(command_fit(&"a".repeat(1201)), CommandFit::TooLong);
    // Swift counts graphemes: 200 `e` + combining acute are 400 chars but 200 characters.
    assert_eq!(command_fit(&"e\u{301}".repeat(200)), CommandFit::Short);
}

/// The lines `chars` characters of one line take as `FakeMeasure` wraps them in the command box (7 pt per character); the
/// model counts them in the pitch the measure gives one line (16 pt here) and the box grows `CMD_LINE_H` per line.
fn command_lines(chars: usize) -> usize {
    let per_line = (layout::CMD_TEXT_W / 7.0).floor() as usize;
    chars.div_ceil(per_line)
}

#[test]
fn a_long_one_line_command_is_cut_until_xem_du_and_then_grows() {
    // 400 characters wrap to several lines: more than the cut.
    assert!(command_lines(400) > layout::CMD_COLLAPSED_LINES);
    let s = approval("a", &"x".repeat(400));
    let mut snap = snapshot(vec![s.clone()]);
    assert_eq!(
        look(&snap, &s),
        ApprovalLook::Command {
            lines: layout::CMD_COLLAPSED_LINES,
            row: ApprovalRow::ShowFull
        }
    );
    let cut = card_layout(&s, &snap, &panel(), &FakeMeasure);
    assert_eq!(
        cut.find(Block::Approval).unwrap().h,
        approval_height(layout::CMD_COLLAPSED_LINES, false)
    );
    snap.shown_commands.insert("t1".into());
    assert_eq!(
        look(&snap, &s),
        ApprovalLook::Command {
            lines: command_lines(400),
            row: ApprovalRow::Buttons
        }
    );
    let full = card_layout(&s, &snap, &panel(), &FakeMeasure);
    assert_eq!(
        full.height - cut.height,
        layout::CMD_LINE_H * (command_lines(400) - layout::CMD_COLLAPSED_LINES) as f64
    );
}

#[test]
fn a_short_command_shows_its_buttons_at_once_and_a_200_char_line_wraps() {
    let s = approval("a", &"x".repeat(200));
    let snap = snapshot(vec![s.clone()]);
    assert_eq!(
        look(&snap, &s),
        ApprovalLook::Command {
            lines: command_lines(200),
            row: ApprovalRow::Buttons
        }
    );
}

#[test]
fn a_too_long_command_gets_the_jump_line_and_no_buttons() {
    let s = approval("a", &"x".repeat(1201));
    let mut snap = snapshot(vec![s.clone()]);
    // Showing it does not help: it is too long for the widget.
    snap.shown_commands.insert("t1".into());
    let want = ApprovalLook::Command {
        lines: layout::CMD_COLLAPSED_LINES,
        row: ApprovalRow::JumpTooLong,
    };
    assert_eq!(look(&snap, &s), want);
    assert_eq!(
        card_layout(&s, &snap, &panel(), &FakeMeasure)
            .find(Block::Approval)
            .unwrap()
            .h,
        layout::CMD_MARGIN
            + layout::command_box_height(layout::CMD_COLLAPSED_LINES)
            + layout::BLOCK_GAP
            + layout::NOTE_H
    );
}

#[test]
fn an_unclear_pending_gets_the_jump_line() {
    let mut s = approval("a", "ls");
    s.pending = Some(PendingState::Unclear { count: 2 });
    let snap = snapshot(vec![s.clone()]);
    assert_eq!(look(&snap, &s), ApprovalLook::Unclear);
    let l = card_layout(&s, &snap, &panel(), &FakeMeasure);
    assert_eq!(
        l.find(Block::Approval).unwrap().h,
        layout::BODY_GAP + layout::NOTE_H
    );
    assert!(l.find(Block::Reply).is_none());
}
