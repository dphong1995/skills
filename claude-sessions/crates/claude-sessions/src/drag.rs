//! Dropping a dragged panel (Swift `settleAfterDrop`): the pill stays where it was shown, `place` re-anchors it and the new
//! offset is saved to `pillOrigins`. The clamp while the pointer moves is `layout::clamp_drag`, called from the panel's hook.

use claude_sessions_core::kv::KeyValueStore;
use claude_sessions_core::layout::{self, Anchor};
use claude_sessions_core::scene::{Point, Rect, Size};

use crate::placement::{self, Placed};

/// The placement after a drop. `start`, `raw` and `clamped` are the window origins of the drag; `window` is the panel size.
pub fn settle(
    kv: &dyn KeyValueStore,
    display_key: &str,
    (start, raw, clamped): (Point, Point, Point),
    window: Size,
    anchor: Anchor,
    screen: Rect,
) -> Placed {
    let pill = layout::settled_pill_origin(start, raw, clamped, window, anchor, screen);
    let placed = placement::panel_frame(pill, screen);
    placement::save_origin(kv, display_key, placed.offset);
    placed
}
