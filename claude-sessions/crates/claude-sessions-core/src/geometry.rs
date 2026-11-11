//! Points, sizes and rectangles in `f64`. In the layout code they carry AppKit semantics (origin bottom-left, so `max_y` is the
//! top edge); inside a `Scene` the same types are read top-left. `Rect::contains` is half-open like `NSRect.contains`.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl Size {
    pub const fn new(w: f64, h: f64) -> Self {
        Self { w, h }
    }
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self::new(origin.x, origin.y, size.w, size.h)
    }

    pub const fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub const fn size(&self) -> Size {
        Size::new(self.w, self.h)
    }

    pub fn min_x(&self) -> f64 {
        self.x
    }

    pub fn max_x(&self) -> f64 {
        self.x + self.w
    }

    pub fn mid_x(&self) -> f64 {
        self.x + self.w / 2.0
    }

    pub fn min_y(&self) -> f64 {
        self.y
    }

    pub fn max_y(&self) -> f64 {
        self.y + self.h
    }

    pub fn mid_y(&self) -> f64 {
        self.y + self.h / 2.0
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    /// `r` lies inside `self` (edges may touch), as `NSRect.contains(NSRect)`.
    pub fn contains_rect(&self, r: Rect) -> bool {
        r.min_x() >= self.min_x()
            && r.max_x() <= self.max_x()
            && r.min_y() >= self.min_y()
            && r.max_y() <= self.max_y()
    }

    /// The smallest rectangle holding both.
    pub fn union(&self, o: Rect) -> Rect {
        let (x0, y0) = (self.min_x().min(o.min_x()), self.min_y().min(o.min_y()));
        let (x1, y1) = (self.max_x().max(o.max_x()), self.max_y().max(o.max_y()));
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Shrinks every edge by `d` (grows for a negative `d`).
    pub fn inset(&self, d: f64) -> Rect {
        Rect::new(self.x + d, self.y + d, self.w - 2.0 * d, self.h - 2.0 * d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_half_open() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(Point::new(0.0, 0.0)));
        assert!(!r.contains(Point::new(10.0, 5.0)));
        assert!(r.contains_rect(Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(!r.contains_rect(Rect::new(5.0, 5.0, 6.0, 1.0)));
    }

    #[test]
    fn union_and_inset() {
        let u = Rect::new(0.0, 0.0, 2.0, 2.0).union(Rect::new(5.0, 1.0, 1.0, 4.0));
        assert_eq!(u, Rect::new(0.0, 0.0, 6.0, 5.0));
        assert_eq!(u.inset(1.0), Rect::new(1.0, 1.0, 4.0, 3.0));
    }
}
