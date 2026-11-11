//! Group titles (`CẦN BẠN · 2`) and the empty note.

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
        Section::NeedsYou => "CẦN BẠN",
        Section::Done => "XONG",
        Section::Running => "ĐANG CHẠY",
    }
}

fn token(section: Section) -> ThemeColor {
    match section {
        Section::NeedsYou => ThemeColor::Red,
        Section::Done => ThemeColor::Orange,
        Section::Running => ThemeColor::Blue,
    }
}

pub fn node(ctx: &mut Ctx, section: Section, count: usize, y: f64) -> Node {
    let mut c = Canvas::default();
    c.rrect(
        Rect::new(12.0, 14.0, 7.0, 7.0),
        3.5,
        Some(theme::solid(token(section))),
        None,
    );
    c.text(
        Rect::new(28.0, 10.0, CARD_W - 40.0, 14.0),
        &format!("{} · {count}", title(section)),
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
        Rect::new(12.0, 12.0, CARD_W - 24.0, 16.0),
        "Không có session",
        theme::META_FONT,
        theme::solid(ThemeColor::TextFaint),
        1,
        Align::Center,
    );
    ctx.node("empty", Rect::new(PANEL_PAD, y, CARD_W, EMPTY_H), c)
}
