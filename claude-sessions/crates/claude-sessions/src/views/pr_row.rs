//! 6b: one PR row (`⑂ #n`, counts, CodeRabbit chip, `Nhờ Claude` / `Nhắc duyệt`). The card reserves
//! `layout::pr_row_height(..)` per PR; nothing is drawn yet.

use super::{Canvas, Ctx};
use claude_sessions_core::scene::Rect;
use claude_sessions_core::store::Session;

/// `block` is the rect placed for `Block::Pr(index)`, relative to the card.
pub fn draw(_ctx: &mut Ctx, _canvas: &mut Canvas, _s: &Session, _index: usize, _block: Rect) {
    // 6b
}
