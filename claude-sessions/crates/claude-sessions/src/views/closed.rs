//! The `Vừa đóng · N` row and its open list, one row per closed session (`layout::closed_height`). A row opens the session
//! (`Target::Resume`) and shows `Đang mở…` / `Lỗi — đã copy lệnh` while its send is in flight or failed.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::layout::{CARD_W, CLOSED_ITEM_H, CLOSED_ROW_H};
use claude_sessions_core::scene::{Align, Node, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendKind, SendState};
use claude_sessions_core::text;

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
    if open {
        for (i, item) in snap.closed.iter().enumerate() {
            let y = CLOSED_ROW_H + CLOSED_ITEM_H * i as f64;
            let row = Rect::new(0.0, y, CARD_W, CLOSED_ITEM_H - 2.0);
            c.rrect(row, 6.0, Some(theme::card_fill()), None);
            let title_w = ctx
                .text_w(&item.title, &theme::BUTTON_FONT)
                .min(CARD_W / 2.0);
            c.label(
                Rect::new(12.0, y, title_w, row.h),
                &item.title,
                theme::BUTTON_FONT,
                theme::solid(ThemeColor::Text),
                Align::Left,
            );
            let mut sub = item.folder();
            if let Some(b) = item.branch.as_deref().filter(|b| !b.is_empty()) {
                sub.push_str(" · ");
                sub.push_str(b);
            }
            c.label(
                Rect::new(12.0 + title_w + 10.0, y, CARD_W / 3.0, row.h),
                &sub,
                theme::META_FONT,
                theme::solid(ThemeColor::TextFaint),
                Align::Left,
            );
            let state = snap.send_state(&SendKey::new(&item.id, SendKind::Resume), ctx.view.now);
            let (label, color) = match state {
                Some(SendState::Sending) => ("Đang mở…".to_string(), ThemeColor::TextSecondary),
                Some(SendState::Failed) => ("Lỗi — đã copy lệnh".to_string(), ThemeColor::Red),
                _ => (
                    format!("đóng {}", text::closed_ago(item.closed_at, ctx.view.now)),
                    ThemeColor::TextSecondary,
                ),
            };
            c.label(
                Rect::new(CARD_W - 12.0 - 160.0, y, 160.0, row.h),
                &label,
                theme::META_FONT,
                theme::solid(color),
                Align::Right,
            );
            if state != Some(SendState::Sending) {
                c.hit(row, Target::Resume(item.id.clone()));
            }
        }
    }
    ctx.node("closed", Rect::new(layout_x(), y, CARD_W, h), c)
}

fn layout_x() -> f64 {
    claude_sessions_core::layout::PANEL_PAD
}
