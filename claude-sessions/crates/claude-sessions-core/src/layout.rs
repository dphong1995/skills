//! Text measurement (implemented over CoreText by the macOS crate and by a fixed-width fake in tests) and the panel geometry of
//! Swift's `Layout`/`Anchor`. All rectangles are in AppKit coordinates (origin bottom-left, same as the `NSPanel` frames).

use crate::geometry::{Point, Rect, Size};
use crate::scene::FontSpec;

mod blocks;
pub use blocks::*;

pub trait TextMeasure {
    /// The size `text` takes in `font` when wrapped at `max_width` and cut at `max_lines` (0 = unlimited).
    fn measure(&self, text: &str, font: &FontSpec, max_width: f64, max_lines: u32) -> Size;
}

/// Where the card grows from relative to the pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAnchor {
    Leading,
    Center,
    Trailing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub h: HAnchor,
    /// The card grows upward (the pill is in the lower half of the screen).
    pub up: bool,
}

impl Default for Anchor {
    fn default() -> Self {
        Anchor {
            h: HAnchor::Trailing,
            up: false,
        }
    }
}

pub const MARGIN: f64 = 14.0;
pub const EXPANDED_WIDTH: f64 = 560.0;
pub const CANVAS: Size = Size::new(EXPANDED_WIDTH + MARGIN * 2.0, 640.0);
/// The collapsed capsule.
pub const PILL_HEIGHT: f64 = 30.0;
/// The scene key of the pill node (the open/close detection of the renderer reads it too).
pub const PILL_KEY: &str = "pill";
/// The header row of the expanded panel.
pub const HEADER_HEIGHT: f64 = 38.0;
pub const COLLAPSED: Size = Size::new(126.0, PILL_HEIGHT);
pub const MAX_EXPANDED_HEIGHT: f64 = CANVAS.h - 2.0 * MARGIN;
/// How long the mouse must rest on a fresh row before its mark clears.
pub const SEEN_DWELL_SECS: f64 = 1.0;
pub const DRAG_THRESHOLD: f64 = 4.0;

/// The anchor for a pill centred at `p` on screen `s`: outer thirds anchor to that edge, the half it is in decides up/down.
pub fn near(p: Point, s: Rect) -> Anchor {
    let third = s.w / 3.0;
    let h = if p.x < s.min_x() + third {
        HAnchor::Leading
    } else if p.x > s.max_x() - third {
        HAnchor::Trailing
    } else {
        HAnchor::Center
    };
    Anchor {
        h,
        up: p.y < s.mid_y(),
    }
}

/// Shifts a saved pill x when the collapsed width changed, keeping the anchored edge (or centre) in place.
pub fn migrate(pill_x: f64, old: f64, new: f64, anchor: Anchor) -> f64 {
    match anchor.h {
        HAnchor::Trailing => pill_x + (old - new),
        HAnchor::Center => pill_x + (old - new) / 2.0,
        HAnchor::Leading => pill_x,
    }
}

pub fn drag_origin(start: Point, mouse_down: Point, mouse: Point) -> Point {
    Point::new(
        start.x + mouse.x - mouse_down.x,
        start.y + mouse.y - mouse_down.y,
    )
}

/// Pill origin after a drop: the pill stays where it was shown (in the window at `clamped`, the exact origin given
/// to the window; its frame is rounded to whole points, so it must not be read back), except that on an axis where
/// the drag moved toward a screen edge and pushed the card against it (`clamped` short of `raw`) it goes to that edge.
/// `place` then re-anchors it.
pub fn settled_pill_origin(
    start: Point,
    raw: Point,
    clamped: Point,
    size: Size,
    anchor: Anchor,
    screen: Rect,
) -> Point {
    let mut o = pill(Rect::from_origin_size(clamped, size), COLLAPSED, anchor).origin();
    if raw.x < clamped.x && raw.x < start.x {
        o.x = screen.min_x();
    } else if raw.x > clamped.x && raw.x > start.x {
        o.x = screen.max_x() - COLLAPSED.w;
    }
    if raw.y < clamped.y && raw.y < start.y {
        o.y = screen.min_y();
    } else if raw.y > clamped.y && raw.y > start.y {
        o.y = screen.max_y() - COLLAPSED.h;
    }
    o
}

pub fn is_drag(from: Point, to: Point) -> bool {
    (to.x - from.x).hypot(to.y - from.y) > DRAG_THRESHOLD
}

/// The rectangle of a `size` pill or card placed in the canvas `f` according to `anchor`.
pub fn pill(f: Rect, size: Size, anchor: Anchor) -> Rect {
    let x = match anchor.h {
        HAnchor::Leading => f.min_x() + MARGIN,
        HAnchor::Center => f.mid_x() - size.w / 2.0,
        HAnchor::Trailing => f.max_x() - MARGIN - size.w,
    };
    let y = if anchor.up {
        f.min_y() + MARGIN
    } else {
        f.max_y() - MARGIN - size.h
    };
    Rect::new(x, y, size.w, size.h)
}

pub fn canvas_frame(pill_origin: Point, anchor: Anchor) -> Rect {
    let c = COLLAPSED;
    let x = match anchor.h {
        HAnchor::Leading => pill_origin.x - MARGIN,
        HAnchor::Center => pill_origin.x + c.w / 2.0 - CANVAS.w / 2.0,
        HAnchor::Trailing => pill_origin.x + c.w + MARGIN - CANVAS.w,
    };
    let y = if anchor.up {
        pill_origin.y - MARGIN
    } else {
        pill_origin.y + c.h + MARGIN - CANVAS.h
    };
    Rect::new(x, y, CANVAS.w, CANVAS.h)
}

pub fn clamp(pill_origin: Point, to: Rect) -> Point {
    Point::new(
        pill_origin.x.max(to.min_x()).min(to.max_x() - COLLAPSED.w),
        pill_origin.y.max(to.min_y()).min(to.max_y() - COLLAPSED.h),
    )
}

/// Window origin shifted by the smallest dx, dy that keeps the visible pill or card inside `screen`.
pub fn clamp_drag(
    window_origin: Point,
    window_size: Size,
    visible: Size,
    anchor: Anchor,
    screen: Rect,
) -> Point {
    let r = pill(
        Rect::from_origin_size(window_origin, window_size),
        visible,
        anchor,
    );
    let dx = if r.min_x() < screen.min_x() {
        screen.min_x() - r.min_x()
    } else if r.max_x() > screen.max_x() {
        screen.max_x() - r.max_x()
    } else {
        0.0
    };
    let dy = if r.min_y() < screen.min_y() {
        screen.min_y() - r.min_y()
    } else if r.max_y() > screen.max_y() {
        screen.max_y() - r.max_y()
    } else {
        0.0
    };
    Point::new(window_origin.x + dx, window_origin.y + dy)
}

/// A collapsed pill origin on screen `a` mapped to screen `b`, keeping the distance to the anchored edges.
pub fn relative_origin(o: Point, a: Rect, b: Rect, anchor: Anchor) -> Point {
    let x = match anchor.h {
        HAnchor::Trailing => b.max_x() - (a.max_x() - o.x),
        HAnchor::Leading => b.min_x() + (o.x - a.min_x()),
        HAnchor::Center => b.mid_x() + (o.x - a.mid_x()),
    };
    let y = if anchor.up {
        b.min_y() + (o.y - a.min_y())
    } else {
        b.max_y() - (a.max_y() - o.y)
    };
    clamp(Point::new(x, y), b)
}

pub fn pill_centre(o: Point) -> Point {
    Point::new(o.x + COLLAPSED.w / 2.0, o.y + COLLAPSED.h / 2.0)
}

/// A pill origin saved by the old single-pill keys, as the screen (index into `screens`) holding it and its offset there.
/// A pill saved at another width is shifted to the current one; a pill on no screen goes to the first.
pub fn legacy_offset(x: f64, y: f64, width: f64, screens: &[Rect]) -> Option<(usize, [f64; 2])> {
    if screens.is_empty() {
        return None;
    }
    let centre = Point::new(x + width / 2.0, y + COLLAPSED.h / 2.0);
    let index = screens.iter().position(|s| s.contains(centre)).unwrap_or(0);
    let f = screens[index];
    let mut x = x;
    if width != COLLAPSED.w {
        x = migrate(x, width, COLLAPSED.w, near(centre, f));
    }
    Some((index, offset(clamp(Point::new(x, y), f), f)))
}

pub fn offset(o: Point, screen: Rect) -> [f64; 2] {
    [o.x - screen.min_x(), o.y - screen.min_y()]
}

pub fn origin(offset: &[f64], screen: Rect) -> Option<Point> {
    (offset.len() == 2).then(|| Point::new(screen.min_x() + offset[0], screen.min_y() + offset[1]))
}
