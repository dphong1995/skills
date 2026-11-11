//! Text measurement, implemented over CoreText by the macOS crate and by a fixed-width fake in tests.

use crate::scene::{FontSpec, Size};

pub trait TextMeasure {
    /// The size `text` takes in `font` when wrapped at `max_width` and cut at `max_lines` (0 = unlimited).
    fn measure(&self, text: &str, font: &FontSpec, max_width: f64, max_lines: u32) -> Size;
}
