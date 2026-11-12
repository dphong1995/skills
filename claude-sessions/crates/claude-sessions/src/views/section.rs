//! Group titles (`Cần bạn 2`, sentence case, the count in the faint colour) and the empty note.

use super::{Canvas, Ctx};
use crate::theme;
use claude_sessions_core::layout::{CARD_W, EMPTY_H, PANEL_PAD, SECTION_TITLE_H};
use claude_sessions_core::scene::{Align, Node, Rect, ThemeColor};
use claude_sessions_core::store::Section;

pub fn key(section: Section) -> &'static str {
    match section {
        Section::NeedsYou => "section-needs",
        Section::Done => "section-done",
        Section::Running => "section-running",
    }
}

pub fn title(section: Section) -> &'static str {
    match section {
        Section::NeedsYou => "Cần bạn",
        Section::Done => "Xong",
        Section::Running => "Đang chạy",
    }
}

/// Where the label of a group starts, and the space between it and its count.
const LABEL_X: f64 = 8.0;
const LABEL_GAP: f64 = 6.0;

pub fn node(ctx: &mut Ctx, section: Section, count: usize, y: f64) -> Node {
    let mut c = Canvas::default();
    let label = title(section);
    let w = ctx.text_w(label, &theme::SECTION_FONT);
    // The label sits low in its row, 4 pt above the cards.
    let top = SECTION_TITLE_H - 18.0;
    c.text(
        Rect::new(LABEL_X, top, w + 2.0, 14.0),
        label,
        theme::SECTION_FONT,
        theme::solid(ThemeColor::TextSecondary),
        1,
        Align::Left,
    );
    c.text(
        Rect::new(LABEL_X + w + LABEL_GAP, top, 40.0, 14.0),
        &count.to_string(),
        theme::SECTION_FONT,
        theme::solid(ThemeColor::TextFaint),
        1,
        Align::Left,
    );
    ctx.node(
        key(section),
        Rect::new(PANEL_PAD, y, CARD_W, SECTION_TITLE_H),
        c,
    )
}

pub fn empty_node(ctx: &mut Ctx, y: f64) -> Node {
    let mut c = Canvas::default();
    c.text(
        Rect::new(LABEL_X, 12.0, CARD_W - 2.0 * LABEL_X, 16.0),
        "Không có session",
        theme::SECTION_FONT,
        theme::solid(ThemeColor::TextSecondary),
        1,
        Align::Left,
    );
    ctx.node("empty", Rect::new(PANEL_PAD, y, CARD_W, EMPTY_H), c)
}
