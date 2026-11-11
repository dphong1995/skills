//! The size model of the redesigned widget. Heights are decided here and nowhere else: the views place every node and block
//! with these helpers and `ExpandedLayout`, they never add heights up themselves. Measurements come from the approved mockups
//! (`plans/2026-11-11-claude-sessions-rust/mockups`): panel 760 pt wide, 10 pt padding, 4 pt between cards.
//! Positions are in scene points (top-left origin), relative to the panel.

use super::{COLLAPSED, EXPANDED_WIDTH, HEADER_HEIGHT, MAX_EXPANDED_HEIGHT, TextMeasure};
use crate::feed::{self, BlockState, FeedQuestion};
use crate::geometry::Size;
use crate::panel_state::{EditKind, PanelState};
use crate::scene::{FontSpec, Weight};
use crate::store::{CommandFit, Section, Session, Snapshot, command_fit};
use crate::text;
use crate::transcript::PendingState;

pub const PANEL_PAD: f64 = 10.0;
pub const CARD_GAP: f64 = 4.0;
pub const CARD_W: f64 = EXPANDED_WIDTH - 2.0 * PANEL_PAD;
pub const CARD_PAD_Y: f64 = 9.0;
/// The status icon sits in this inset; all card text starts after it.
pub const CARD_INSET: f64 = 38.0;
pub const CARD_PAD_RIGHT: f64 = 12.0;
/// Time label, `Trả lời` / `Dừng` button.
pub const RIGHT_COLUMN: f64 = 64.0;
pub const TEXT_W: f64 = CARD_W - CARD_INSET - CARD_PAD_RIGHT;
pub const SECTION_TITLE_H: f64 = 20.0;
pub const SUB_H: f64 = 15.0;
/// One `Todo` / activity line including the 5 pt above it.
pub const LINE_ROW_H: f64 = 20.0;
pub const REPLY_GAP: f64 = 5.0;
pub const CMD_MARGIN: f64 = 7.0;
pub const CMD_PAD: f64 = 7.0;
/// The line pitch of the 12 pt monospaced command text, so the box ends one `CMD_PAD` under its last line.
pub const CMD_LINE_H: f64 = 15.0;
/// Lines of a command the card shows before `Xem đủ` (Swift `lineLimit(4)`).
pub const CMD_COLLAPSED_LINES: usize = 4;
/// The command text sits 10 pt inside its box on both sides.
pub const CMD_TEXT_W: f64 = TEXT_W - 20.0;
pub const CMD_FONT: FontSpec = font(12.0, Weight::Regular, true);
pub const BUTTON_H: f64 = 24.0;
pub const BLOCK_GAP: f64 = 8.0;
pub const FIELD_H: f64 = 28.0;
pub const QUESTION_PROMPT_H: f64 = 22.0;
/// Space above every question after the first.
pub const QUESTION_GAP: f64 = 6.0;
/// Every option is one line: number, label and its description cut with `…`.
pub const OPTION_ROW_H: f64 = 24.0;
pub const OTHER_ROW_H: f64 = 26.0;
pub const NOTE_H: f64 = 24.0;
/// The one-line approve/deny result that replaces the approval block.
pub const SENT_LINE_H: f64 = 24.0;
pub const PR_ROW_H: f64 = 35.0;
pub const PR_ROW_CLOSED_H: f64 = 31.0;
pub const CHIPS_H: f64 = 26.0;
pub const LINK_SECTION_H: f64 = 16.0;
pub const LINK_ROW_H: f64 = 22.0;
/// A card with a right-column button is at least this tall.
pub const CARD_MIN_BUTTON_H: f64 = 54.0;
pub const CLOSED_ROW_H: f64 = 26.0;
pub const CLOSED_ITEM_H: f64 = 24.0;
pub const EMPTY_H: f64 = 40.0;
/// The key hint line at the bottom of the panel in keyboard mode.
pub const KEY_HINT_H: f64 = 26.0;
pub const SPARKLE_W: f64 = 18.0;

const fn font(size: f64, weight: Weight, mono: bool) -> FontSpec {
    FontSpec { size, weight, mono }
}

pub const TITLE_FONT: FontSpec = font(13.5, Weight::Semibold, false);
pub const STATE_FONT: FontSpec = font(11.0, Weight::Semibold, false);
pub const SUB_FONT: FontSpec = font(11.5, Weight::Regular, false);
pub const REPLY_FONT: FontSpec = font(12.5, Weight::Regular, false);

/// `chờ duyệt · Bash`, `chờ trả lời · 2 câu hỏi`, else the plain state text. Empty when the text only repeats the
/// section (plain `xong` in Done, plain `đang chạy` in Running).
pub fn state_label(s: &Session) -> String {
    let base = text::status_text(s);
    if s.question.is_none() && (base == "xong" || base == "đang chạy") {
        return String::new();
    }
    if let Some(q) = &s.question {
        return format!("{base} · {} câu hỏi", q.questions.len());
    }
    match (&s.pending, s.is_permission_prompt()) {
        (Some(PendingState::One(p)), true) => format!("{base} · {}", p.tool),
        _ => base,
    }
}

/// What the question block of `s` shows: the stored state, else the form while the question is live and supported.
pub fn question_state(s: &Session, q: &FeedQuestion) -> BlockState {
    s.block.unwrap_or(if s.question_live && feed::supports(q) {
        BlockState::Form
    } else {
        BlockState::Fallback
    })
}

/// The question of `s` whose `Khác…` field is open, from the panel's `other_key` (`"<session id>#<index>"`).
pub fn other_open(s: &Session, panel: &PanelState) -> Option<usize> {
    panel
        .other_key
        .as_deref()
        .and_then(|k| k.strip_prefix(&format!("{}#", s.id)))
        .and_then(|i| i.parse().ok())
}

/// What the row under the command box is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalRow {
    /// `Cho phép` / `Từ chối…`: the whole command is shown.
    Buttons,
    /// `Xem đủ`: a long command is cut to `CMD_COLLAPSED_LINES` lines.
    ShowFull,
    /// `Lệnh dài, bấm để duyệt trong cmux`: too long for the widget to approve.
    JumpTooLong,
}

/// How the approval block of a card looks; the view draws exactly this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalLook {
    /// `Cần duyệt trong cmux`: no single command to show.
    Unclear,
    Command {
        /// The wrapped lines the command box shows.
        lines: usize,
        row: ApprovalRow,
    },
}

impl ApprovalLook {
    pub fn height(self, deny_editing: bool) -> f64 {
        match self {
            ApprovalLook::Unclear => BLOCK_GAP + NOTE_H,
            ApprovalLook::Command {
                lines,
                row: ApprovalRow::JumpTooLong,
            } => CMD_MARGIN + command_box_height(lines) + BLOCK_GAP + NOTE_H,
            ApprovalLook::Command { lines, .. } => approval_height(lines, deny_editing),
        }
    }
}

/// The look of the approval block for a pending state. The command is measured on the width the view draws it at: a
/// command not shown whole is cut to `CMD_COLLAPSED_LINES` lines, a shown one is not cut.
pub fn approval_look(pending: &PendingState, snap: &Snapshot, m: &dyn TextMeasure) -> ApprovalLook {
    let PendingState::One(p) = pending else {
        return ApprovalLook::Unclear;
    };
    let row = match command_fit(&p.text) {
        CommandFit::TooLong => ApprovalRow::JumpTooLong,
        _ if snap.is_command_shown(p) => ApprovalRow::Buttons,
        _ => ApprovalRow::ShowFull,
    };
    let limit = if row == ApprovalRow::Buttons {
        0
    } else {
        CMD_COLLAPSED_LINES as u32
    };
    // A command not shown whole shows its first lines only: measure that prefix, not a command of up to megabytes.
    let prefix;
    let text = if row == ApprovalRow::Buttons {
        p.text.as_str()
    } else {
        // Each line is capped at a length that wraps past the limit on any glyph at least 1 pt wide.
        let cap = CMD_COLLAPSED_LINES * CMD_TEXT_W as usize;
        prefix = p
            .text
            .split('\n')
            .take(CMD_COLLAPSED_LINES)
            .map(|l| l.chars().take(cap).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        prefix.as_str()
    };
    let h = m.measure(text, &CMD_FONT, CMD_TEXT_W, limit).h;
    ApprovalLook::Command {
        lines: ((h / CMD_LINE_H).round() as usize).max(1),
        row,
    }
}

pub fn header_height() -> f64 {
    HEADER_HEIGHT
}

pub fn section_title_height() -> f64 {
    SECTION_TITLE_H
}

/// The head of a card: title (at most 2 lines), then the folder line. `title_h` is the measured title height.
pub fn head_height(title_h: f64) -> f64 {
    CARD_PAD_Y + title_h + SUB_H
}

pub fn command_box_height(lines: usize) -> f64 {
    2.0 * CMD_PAD + CMD_LINE_H * lines.max(1) as f64
}

/// Command box (`cmd_lines` lines) and the `Cho phép` / `Từ chối` row; a deny reason field adds its own row.
pub fn approval_height(cmd_lines: usize, deny_editing: bool) -> f64 {
    CMD_MARGIN
        + command_box_height(cmd_lines)
        + BLOCK_GAP
        + BUTTON_H
        + if deny_editing {
            BLOCK_GAP + FIELD_H
        } else {
            0.0
        }
}

/// A question block: per question the prompt, one row per option and the `Khác…` row (plus its field for the question
/// `other_open`), then the `Gửi` row; `Sent` and `Fallback` are one note line.
pub fn question_height(q: &FeedQuestion, state: BlockState, other_open: Option<usize>) -> f64 {
    if state != BlockState::Form {
        return BLOCK_GAP + NOTE_H;
    }
    let questions: f64 = q
        .questions
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let options = OPTION_ROW_H * item.options.len() as f64;
            let other = if other_open == Some(i) { FIELD_H } else { 0.0 };
            QUESTION_PROMPT_H + options + OTHER_ROW_H + other
        })
        .sum();
    let gaps = QUESTION_GAP * q.questions.len().saturating_sub(1) as f64;
    BLOCK_GAP + questions + gaps + BLOCK_GAP + BUTTON_H
}

/// The reply, at most 2 lines, with the gap above it.
pub fn reply_height(reply_h: f64) -> f64 {
    REPLY_GAP + reply_h
}

/// The approve/deny result line shown where the approval block was.
pub fn sent_line_height() -> f64 {
    SENT_LINE_H
}

pub fn pr_row_height(open: bool) -> f64 {
    if open { PR_ROW_H } else { PR_ROW_CLOSED_H }
}

/// The inline ticket / PR list opened by the menu count.
pub fn link_list_height(s: &Session) -> f64 {
    s.menu
        .iter()
        .map(|sec| LINK_SECTION_H + LINK_ROW_H * sec.items.len() as f64)
        .sum::<f64>()
        + if s.menu_needs_jira { LINK_ROW_H } else { 0.0 }
}

pub fn editor_height() -> f64 {
    BLOCK_GAP + FIELD_H
}

/// The `Vừa đóng · N` row, and one row per closed session while it is open.
pub fn closed_height(count: usize, open: bool) -> f64 {
    if count == 0 {
        return 0.0;
    }
    CLOSED_ROW_H
        + if open {
            CLOSED_ITEM_H * count as f64
        } else {
            0.0
        }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Head,
    Approval,
    /// The result of an approve/deny while the pending command is already gone.
    SentLine,
    Question,
    Reply,
    Todo,
    Activity,
    /// Index into `Session::prs`.
    Pr(usize),
    /// The ticket buttons and the menu count.
    Chips,
    LinkList,
    Editor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub block: Block,
    /// Relative to the card's top.
    pub y: f64,
    pub h: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CardLayout {
    pub height: f64,
    /// The measured title height, for the head block.
    pub title_h: f64,
    /// Where the title may wrap: the text column less the right column, the state text and the new-mark sparkle.
    pub title_w: f64,
    /// What `Block::Approval` shows, when the card has one.
    pub approval: Option<ApprovalLook>,
    pub blocks: Vec<Placed>,
}

impl CardLayout {
    pub fn find(&self, block: Block) -> Option<Placed> {
        self.blocks.iter().copied().find(|p| p.block == block)
    }
}

/// Whether the right column shows a button (`Trả lời…` on a finished session, `Dừng` on a busy one).
pub fn has_quick_button(s: &Session) -> bool {
    s.can_ask_claude() || s.shown_status() == "busy"
}

pub fn card_layout(
    s: &Session,
    snap: &Snapshot,
    panel: &PanelState,
    m: &dyn TextMeasure,
) -> CardLayout {
    let new = snap.unseen.get(&s.id).is_some_and(|n| n.claude);
    let state_w = m.measure(&state_label(s), &STATE_FONT, f64::MAX / 4.0, 1).w;
    let state_gap = if state_w > 0.0 { state_w + 6.0 } else { 0.0 };
    let title_w = TEXT_W - RIGHT_COLUMN - state_gap - if new { SPARKLE_W + 4.0 } else { 0.0 };
    let title_h = m.measure(s.title(), &TITLE_FONT, title_w.max(40.0), 2).h;

    let mut blocks = Vec::new();
    let mut approval = None;
    let mut y = 0.0;
    let mut place = |block: Block, h: f64| {
        blocks.push(Placed { block, y, h });
        y += h;
    };
    place(Block::Head, head_height(title_h));
    let editing = panel.editing.as_ref().filter(|e| e.id == s.id);
    if let Some(q) = &s.question {
        place(
            Block::Question,
            question_height(q, question_state(s, q), other_open(s, panel)),
        );
    } else if let Some(look) = s
        .pending
        .as_ref()
        .filter(|_| s.is_permission_prompt())
        .map(|p| approval_look(p, snap, m))
    {
        let deny = editing.is_some_and(|e| e.kind == EditKind::Deny)
            && matches!(
                look,
                ApprovalLook::Command {
                    row: ApprovalRow::Buttons,
                    ..
                }
            );
        place(Block::Approval, look.height(deny));
        approval = Some(look);
    } else if s.pending.is_none() && snap.approval_send(&s.id).is_some() {
        place(Block::SentLine, sent_line_height());
    } else if let Some(reply) = s.last_reply.as_deref().filter(|_| s.is_finished()) {
        // Measured on what the view draws: markdown markers are gone.
        let shown: String = text::markdown_runs(reply)
            .into_iter()
            .map(|r| r.text)
            .collect();
        place(
            Block::Reply,
            reply_height(m.measure(&shown, &REPLY_FONT, TEXT_W, 2).h),
        );
    }
    if s.todo.is_some() {
        place(Block::Todo, LINE_ROW_H);
    }
    if s.activity.is_some() {
        place(Block::Activity, LINE_ROW_H);
    }
    for (i, pr) in s.prs.iter().enumerate() {
        place(Block::Pr(i), pr_row_height(pr.is_open()));
    }
    if !s.menu.is_empty() {
        place(Block::Chips, CHIPS_H);
        if panel.open_menus.contains(&s.id) {
            place(Block::LinkList, link_list_height(s));
        }
    }
    if editing.is_some_and(|e| e.kind == EditKind::Reply) {
        place(Block::Editor, editor_height());
    }
    let mut height = y + CARD_PAD_Y;
    if has_quick_button(s) {
        height = height.max(CARD_MIN_BUTTON_H);
    }
    CardLayout {
        height,
        title_h,
        title_w,
        approval,
        blocks,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Header {
        y: f64,
    },
    Section {
        section: Section,
        count: usize,
        y: f64,
    },
    Card {
        id: String,
        y: f64,
        layout: CardLayout,
    },
    Empty {
        y: f64,
    },
    Closed {
        y: f64,
        h: f64,
    },
    /// Keyboard mode only.
    KeyHint {
        y: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExpandedLayout {
    /// Capped at `MAX_EXPANDED_HEIGHT`.
    pub size: Size,
    /// Everything laid out, uncapped.
    pub content_h: f64,
    pub items: Vec<Item>,
}

/// Every item of the expanded card with its position: header, per non-empty section its title and cards (4 pt after each card),
/// the empty note when there is no session, the `Vừa đóng` row.
pub fn expanded_layout(snap: &Snapshot, panel: &PanelState, m: &dyn TextMeasure) -> ExpandedLayout {
    let mut items = vec![Item::Header { y: PANEL_PAD }];
    let mut y = PANEL_PAD + header_height();
    if snap.sessions.is_empty() {
        items.push(Item::Empty { y });
        y += EMPTY_H;
    }
    for (section, ids) in snap.sections() {
        if ids.is_empty() {
            continue;
        }
        items.push(Item::Section {
            section,
            count: ids.len(),
            y,
        });
        y += section_title_height();
        for id in ids {
            let Some(s) = snap.sessions.iter().find(|s| s.id == id) else {
                continue;
            };
            let layout = card_layout(s, snap, panel, m);
            let h = layout.height;
            items.push(Item::Card { id, y, layout });
            y += h + CARD_GAP;
        }
    }
    let closed = closed_height(snap.closed.len(), panel.closed_open);
    if closed > 0.0 {
        items.push(Item::Closed { y, h: closed });
        y += closed;
    }
    if panel.keyboard {
        items.push(Item::KeyHint { y });
        y += KEY_HINT_H;
    }
    let content_h = y + PANEL_PAD;
    ExpandedLayout {
        size: Size::new(EXPANDED_WIDTH, content_h.min(MAX_EXPANDED_HEIGHT)),
        content_h,
        items,
    }
}

pub fn expanded_size(snap: &Snapshot, panel: &PanelState, m: &dyn TextMeasure) -> Size {
    expanded_layout(snap, panel, m).size
}

/// One group of the pill: the sparkle count, the red `Cần bạn` badge, the orange `Xong` and blue `Đang chạy` counts.
pub fn pill_groups(snap: &Snapshot) -> [usize; 4] {
    let sections = snap.sections();
    let n = |i: usize| sections[i].1.len();
    let new = snap
        .sessions
        .iter()
        .filter(|s| {
            snap.unseen
                .get(&s.id)
                .is_some_and(|m| m.claude || !m.prs.is_empty())
        })
        .count();
    [new, n(0), n(1), n(2)]
}

pub const PILL_PAD: f64 = 14.0;
pub const PILL_GAP: f64 = 10.0;
/// The quota ring plus `86%`.
pub const PILL_QUOTA_W: f64 = 58.0;

/// Width of one digit of a pill count.
pub const PILL_DIGIT_W: f64 = 7.5;

/// Width of the pill group `index` (see `pill_groups`) as drawn: the sparkle, the `Cần bạn` badge or the dot, then the
/// digits of its count. It is the group's visible width, so the gap between groups is `PILL_GAP` everywhere.
pub fn pill_group_width(index: usize, count: usize) -> f64 {
    let digits = count.to_string().len() as f64;
    match index {
        0 => 18.0 + PILL_DIGIT_W * digits,
        1 if count < 10 => 18.0,
        1 => 18.0 + PILL_DIGIT_W * digits,
        _ => 7.0 + 5.0 + PILL_DIGIT_W * digits,
    }
}

/// Total width of the count groups of `snap` with `PILL_GAP` between them (0 without any).
pub fn groups_width(snap: &Snapshot) -> f64 {
    let widths: Vec<f64> = pill_groups(snap)
        .into_iter()
        .enumerate()
        .filter(|(_, n)| *n > 0)
        .map(|(i, n)| pill_group_width(i, n))
        .collect();
    match widths.len() {
        0 => 0.0,
        n => widths.iter().sum::<f64>() + PILL_GAP * (n - 1) as f64,
    }
}

/// The collapsed pill: padding, one group per non-zero count, the quota ring from 80%. Without any session the pill keeps
/// `COLLAPSED.w` for its idle text.
pub fn collapsed_width(snap: &Snapshot) -> f64 {
    let groups = groups_width(snap);
    let quota = snap.quota.as_ref().and_then(|q| q.pill_percent()).is_some();
    match (groups > 0.0, quota) {
        (false, false) => COLLAPSED.w,
        (true, false) => 2.0 * PILL_PAD + groups,
        (false, true) => 2.0 * PILL_PAD + PILL_QUOTA_W,
        (true, true) => 2.0 * PILL_PAD + groups + PILL_GAP + PILL_QUOTA_W,
    }
}

/// The size the panel shows: the pill, or the expanded card while `panel.expanded`.
pub fn panel_size(snap: &Snapshot, panel: &PanelState, m: &dyn TextMeasure) -> Size {
    if panel.expanded(snap.peeking) {
        expanded_size(snap, panel, m)
    } else {
        Size::new(collapsed_width(snap), HEADER_HEIGHT)
    }
}
