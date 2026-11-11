//! Dropping a dragged panel: the settled pill origin is saved to `pillOrigins` per display.

use claude_sessions::drag::settle;
use claude_sessions::placement::load_origins;
use claude_sessions_core::kv::MemoryStore;
use claude_sessions_core::layout::{Anchor, CANVAS, COLLAPSED, HAnchor, MARGIN, canvas_frame};
use claude_sessions_core::scene::{Point, Rect};

const SCREEN: Rect = Rect::new(0.0, 0.0, 1440.0, 900.0);

#[test]
fn drop_saves_the_pill_where_it_was_shown() {
    let kv = MemoryStore::default();
    let anchor = Anchor {
        h: HAnchor::Trailing,
        up: false,
    };
    let pill = Point::new(1000.0, 800.0);
    let start = canvas_frame(pill, anchor).origin();
    let moved = Point::new(start.x - 300.0, start.y - 400.0);
    let placed = settle(&kv, "UUID-A", (start, moved, moved), CANVAS, anchor, SCREEN);
    assert_eq!(placed.origin, Point::new(700.0, 400.0));
    assert_eq!(placed.offset, [700.0, 400.0]);
    assert_eq!(load_origins(&kv)["UUID-A"], vec![700.0, 400.0]);
    // Lower half of the screen: the card grows up, the pill sits MARGIN above the canvas bottom.
    assert!(placed.anchor.up);
    assert_eq!(placed.frame.y, 400.0 - MARGIN);
}

#[test]
fn drop_against_the_edge_goes_to_the_edge() {
    let kv = MemoryStore::default();
    let anchor = Anchor::default();
    let start = canvas_frame(Point::new(1000.0, 800.0), anchor).origin();
    // The pointer asked for 500 pt past the right edge; the clamp stopped the window earlier.
    let raw = Point::new(start.x + 500.0, start.y);
    let clamped = Point::new(start.x + 100.0, start.y);
    let placed = settle(&kv, "UUID-A", (start, raw, clamped), CANVAS, anchor, SCREEN);
    assert_eq!(placed.origin.x, SCREEN.max_x() - COLLAPSED.w);
}
