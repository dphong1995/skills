//! 6b: the `Vừa đóng · N` row and its open list (`layout::closed_height`). Only the closed row is drawn so far.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::layout::{CARD_W, CLOSED_ROW_H};
use claude_sessions_core::scene::{Align, Node, Rect, ThemeColor, Weight};

pub fn node(ctx: &mut Ctx, y: f64, h: f64) -> Node {
    let snap = ctx.view.snap;
    let open = ctx.view.panel.closed_open;
    let mut c = Canvas::default();
    c.text(
        Rect::new(12.0, 8.0, 300.0, 16.0),
        &format!("Vừa đóng · {}", snap.closed.len()),
        theme::META_FONT,
        theme::solid(ThemeColor::TextFaint),
        1,
        Align::Left,
    );
    c.symbol(
        if open {
            "chevron.down"
        } else {
            "chevron.right"
        },
        Rect::new(CARD_W - 24.0, 9.0, 10.0, 12.0),
        theme::solid(ThemeColor::TextFaint),
        Weight::Bold,
    );
    c.hit(
        Rect::new(0.0, 0.0, CARD_W, CLOSED_ROW_H),
        Target::ToggleClosed,
    );
    // 6b: the open list, one row per closed session with `Target::Resume`.
    ctx.node("closed", Rect::new(layout_x(), y, CARD_W, h), c)
}

fn layout_x() -> f64 {
    claude_sessions_core::layout::PANEL_PAD
}
