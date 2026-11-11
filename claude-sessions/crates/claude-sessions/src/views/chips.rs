//! 6b: ticket chips with the menu count (`Block::Chips`) and the inline link list (`Block::LinkList`). The card reserves
//! `layout::CHIPS_H` and `layout::link_list_height(..)`; nothing is drawn yet.

use super::{Canvas, Ctx};
use claude_sessions_core::scene::Rect;
use claude_sessions_core::store::Session;

pub fn draw_chips(_ctx: &mut Ctx, _canvas: &mut Canvas, _s: &Session, _block: Rect) {
    // 6b
}

pub fn draw_link_list(_ctx: &mut Ctx, _canvas: &mut Canvas, _s: &Session, _block: Rect) {
    // 6b
}
