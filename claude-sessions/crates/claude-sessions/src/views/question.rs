//! 6b: the question form (prompts, numbered option rows, `Khác…`, `Gửi`). The card reserves
//! `layout::question_height(..)` for it; nothing is drawn yet.

use super::{Canvas, Ctx};
use claude_sessions_core::scene::Rect;
use claude_sessions_core::store::Session;

/// `block` is the rect the card layout placed for `Block::Question`, relative to the card.
pub fn draw(_ctx: &mut Ctx, _canvas: &mut Canvas, _s: &Session, _block: Rect) {
    // 6b
}
