//! The `Vừa đóng N` disclosure row and its open list: one group with a row per closed session (`layout::closed_height`) and a
//! separator between rows. A row opens the session (`Target::Resume`) and shows `Đang mở…` / `Lỗi — đã copy lệnh` while its send
//! is in flight or failed.

use super::{Canvas, Ctx, section};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::layout::{CARD_W, CLOSED_ITEM_H, CLOSED_ROW_H, KEY_HINT_H, PANEL_PAD};
use claude_sessions_core::scene::{Align, Node, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendKind, SendState};
use claude_sessions_core::text;

/// The label column of the panel: the group titles, the disclosure row and the hint start here.
const TEXT_X: f64 = 8.0;
const CHEVRON_W: f64 = 9.0;

/// The key hint line under the list while keyboard mode is on.
pub fn key_hint_node(ctx: &mut Ctx, y: f64) -> Node {
    let mut c = Canvas::default();
    c.label(
        Rect::new(TEXT_X, 0.0, CARD_W - 2.0 * TEXT_X, KEY_HINT_H),
        "↑↓ chọn thẻ · ↩ nhảy tới pane · R trả lời · Esc đóng",
        theme::HINT_FONT,
        theme::solid(ThemeColor::TextFaint),
        Align::Left,
    );
    ctx.node("key-hint", Rect::new(PANEL_PAD, y, CARD_W, KEY_HINT_H), c)
}

pub fn node(ctx: &mut Ctx, y: f64, h: f64) -> Node {
    let snap = ctx.view.snap;
    let open = ctx.view.panel.closed_open;
    let mut c = Canvas::default();
    // Same baseline as the group titles.
    let top = CLOSED_ROW_H - 18.0;
    c.symbol(
        if open {
            "chevron.down"
        } else {
            "chevron.right"
        },
        Rect::new(TEXT_X, top + 1.0, CHEVRON_W, 12.0),
        theme::solid(ThemeColor::TextSecondary),
        Weight::Bold,
    );
    section::draw_title(
        ctx,
        &mut c,
        TEXT_X + CHEVRON_W + 5.0,
        CLOSED_ROW_H,
        "Vừa đóng",
        snap.closed.len(),
    );
    c.hit(
        Rect::new(0.0, 0.0, CARD_W, CLOSED_ROW_H),
        Target::ToggleClosed,
    );
    if open {
        c.rrect(
            Rect::new(
                0.0,
                CLOSED_ROW_H,
                CARD_W,
                CLOSED_ITEM_H * snap.closed.len() as f64,
            ),
            theme::GROUP_RADIUS,
            Some(theme::solid(ThemeColor::Group)),
            None,
        );
        for (i, item) in snap.closed.iter().enumerate() {
            let y = CLOSED_ROW_H + CLOSED_ITEM_H * i as f64;
            let row = Rect::new(0.0, y, CARD_W, CLOSED_ITEM_H);
            if i > 0 {
                c.hairline(12.0, y, CARD_W - 12.0);
            }
            let title_w = ctx.text_w(&item.title, &theme::ROW_FONT).min(CARD_W / 2.0);
            c.label(
                Rect::new(12.0, y, title_w, row.h),
                &item.title,
                theme::ROW_FONT,
                theme::solid(ThemeColor::Text),
                Align::Left,
            );
            let mut sub = item.folder();
            if let Some(b) = item.branch.as_deref().filter(|b| !b.is_empty()) {
                sub.push_str(" · ");
                sub.push_str(b);
            }
            c.label(
                Rect::new(12.0 + title_w + 8.0, y, CARD_W / 3.0, row.h),
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
                    ThemeColor::TextFaint,
                ),
            };
            c.label(
                Rect::new(CARD_W - 12.0 - 160.0, y, 160.0, row.h),
                &label,
                theme::TIME_FONT,
                theme::solid(color),
                Align::Right,
            );
            if state != Some(SendState::Sending) {
                c.hit(row, Target::Resume(item.id.clone()));
            }
        }
    }
    ctx.node("closed", Rect::new(PANEL_PAD, y, CARD_W, h), c)
}
