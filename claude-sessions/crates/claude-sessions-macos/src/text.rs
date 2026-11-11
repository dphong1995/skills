//! Text attributes shared by drawing and measuring, and the `TextMeasure` implementation.

use std::cell::RefCell;

use claude_sessions_core::layout::TextMeasure;
use claude_sessions_core::scene::{Align, FontSpec, Run, Size, Weight};
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSFontDescriptorSymbolicTraits, NSFontWeight,
    NSFontWeightBold, NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold,
    NSForegroundColorAttributeName, NSLineBreakMode, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName, NSStringDrawingOptions, NSStringNSExtendedStringDrawing,
    NSTextAlignment,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString,
};

pub(crate) type Attributes = Retained<NSDictionary<NSString, AnyObject>>;

pub(crate) fn font_weight(weight: Weight) -> NSFontWeight {
    // SAFETY: plain extern constants.
    unsafe {
        match weight {
            Weight::Regular => NSFontWeightRegular,
            Weight::Medium => NSFontWeightMedium,
            Weight::Semibold => NSFontWeightSemibold,
            Weight::Bold => NSFontWeightBold,
        }
    }
}

/// Fonts by spec: every draw and measure asks for the same few; AppKit objects stay on the main thread.
fn ns_font(spec: &FontSpec) -> Retained<NSFont> {
    thread_local! {
        static FONTS: RefCell<Vec<(FontSpec, Retained<NSFont>)>> = const { RefCell::new(Vec::new()) };
    }
    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        if let Some((_, font)) = fonts.iter().find(|(s, _)| s == spec) {
            return font.clone();
        }
        let font = make_font(spec);
        fonts.push((*spec, font.clone()));
        font
    })
}

fn make_font(spec: &FontSpec) -> Retained<NSFont> {
    let weight = font_weight(spec.weight);
    if spec.mono {
        NSFont::monospacedSystemFontOfSize_weight(spec.size, weight)
    } else {
        NSFont::systemFontOfSize_weight(spec.size, weight)
    }
}

/// Height of one line of `spec`.
fn line_height(font: &NSFont) -> f64 {
    (font.ascender() - font.descender() + font.leading()).ceil()
}

pub(crate) fn attributes(spec: &FontSpec, color: [f64; 4], align: Align) -> Attributes {
    attributes_for(&ns_font(spec), color, align)
}

fn attributes_for(font: &NSFont, color: [f64; 4], align: Align) -> Attributes {
    let color = NSColor::colorWithSRGBRed_green_blue_alpha(color[0], color[1], color[2], color[3]);
    let style = NSMutableParagraphStyle::new();
    style.setAlignment(match align {
        Align::Left => NSTextAlignment::Left,
        Align::Center => NSTextAlignment::Center,
        Align::Right => NSTextAlignment::Right,
    });
    // Word wrapping always: `ByTruncatingTail` makes string drawing treat the text as one line. `max_lines` truncates through
    // `draw_options` and the height cap of `text_area`.
    style.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    // SAFETY: the keys are AppKit's own constants and every value is an object of the type the key documents.
    unsafe {
        NSDictionary::from_slices(
            &[
                NSFontAttributeName,
                NSForegroundColorAttributeName,
                NSParagraphStyleAttributeName,
            ],
            &[font as &AnyObject, &*color, &*style],
        )
    }
}

/// The runs of a `Rich` command as one attributed string: each run takes the base `spec` with its own flags.
pub(crate) fn rich_string(
    runs: &[Run],
    spec: &FontSpec,
    color: [f64; 4],
    align: Align,
) -> Retained<NSMutableAttributedString> {
    let out = NSMutableAttributedString::new();
    for run in runs {
        let mut run_spec = *spec;
        run_spec.mono |= run.mono;
        if run.bold {
            run_spec.weight = Weight::Bold;
        }
        let mut font = ns_font(&run_spec);
        if run.italic {
            let descriptor = font
                .fontDescriptor()
                .fontDescriptorWithSymbolicTraits(NSFontDescriptorSymbolicTraits::TraitItalic);
            font = NSFont::fontWithDescriptor_size(&descriptor, run_spec.size).unwrap_or(font);
        }
        let attrs = attributes_for(&font, color, align);
        // SAFETY: the attributes hold AppKit's keys with values of their documented types.
        let piece = unsafe {
            NSAttributedString::initWithString_attributes(
                NSAttributedString::alloc(),
                &NSString::from_str(&run.text),
                Some(&attrs),
            )
        };
        out.appendAttributedString(&piece);
    }
    out
}

pub(crate) fn draw_options(max_lines: u32) -> NSStringDrawingOptions {
    if max_lines == 0 {
        NSStringDrawingOptions::UsesLineFragmentOrigin
    } else {
        NSStringDrawingOptions::UsesLineFragmentOrigin
            | NSStringDrawingOptions::TruncatesLastVisibleLine
    }
}

/// The rect text is drawn in: `max_lines` caps the height so truncation happens at the right line, and when `rect` is taller
/// than the text the text is moved down to the middle of it. The text's laid-out height is `laid_out(width)`; a rect no
/// taller than one line, or text capped to one line (always one line tall), skips the layout.
pub(crate) fn text_area(
    rect: NSRect,
    spec: &FontSpec,
    max_lines: u32,
    laid_out: impl FnOnce(f64) -> f64,
) -> NSRect {
    let line = line_height(&ns_font(spec));
    let area = if max_lines == 0 {
        rect
    } else {
        NSRect::new(
            rect.origin,
            NSSize::new(
                rect.size.width,
                rect.size.height.min(line * f64::from(max_lines)),
            ),
        )
    };
    if rect.size.height <= line {
        return area;
    }
    let h = if max_lines == 1 {
        line
    } else {
        let h = laid_out(area.size.width).ceil();
        if max_lines > 0 {
            h.min(line * f64::from(max_lines))
        } else {
            h
        }
    };
    if rect.size.height <= h {
        return area;
    }
    NSRect::new(
        NSPoint::new(area.origin.x, area.origin.y + (rect.size.height - h) / 2.0),
        NSSize::new(area.size.width, h),
    )
}

/// `TextMeasure` over the AppKit text system (CoreText underneath).
pub struct CoreTextMeasure;

impl TextMeasure for CoreTextMeasure {
    fn measure(&self, text: &str, font: &FontSpec, max_width: f64, max_lines: u32) -> Size {
        let attrs = attributes(font, [0.0; 4], Align::Left);
        let unlimited = NSSize::new(max_width, 1.0e6);
        // SAFETY: plain call on an owned string with attributes built above.
        let bounds = unsafe {
            NSString::from_str(text).boundingRectWithSize_options_attributes_context(
                unlimited,
                draw_options(max_lines),
                Some(&attrs),
                None,
            )
        };
        let mut h = bounds.size.height.ceil();
        if max_lines > 0 {
            h = h.min(line_height(&ns_font(font)) * f64::from(max_lines));
        }
        Size {
            w: bounds.size.width.ceil(),
            h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONG: &str = "A long card title that has to wrap over several lines when the column is only a hundred points wide";

    fn font() -> FontSpec {
        FontSpec {
            size: 13.0,
            weight: Weight::Regular,
            mono: false,
        }
    }

    /// One line's measured height.
    fn one_line() -> f64 {
        CoreTextMeasure.measure("A", &font(), 100.0, 0).h
    }

    #[test]
    fn unlimited_text_wraps_over_many_lines() {
        let h = CoreTextMeasure.measure(LONG, &font(), 100.0, 0).h;
        assert!(h >= 4.0 * one_line() - 4.0, "wrapped height {h}");
    }

    #[test]
    fn max_lines_caps_the_wrapped_height() {
        let line = line_height(&ns_font(&font()));
        let two = CoreTextMeasure.measure(LONG, &font(), 100.0, 2).h;
        let one = CoreTextMeasure.measure(LONG, &font(), 100.0, 1).h;
        assert!((two - 2.0 * line).abs() < 1.0, "two lines: {two} vs {line}");
        assert!((one - line).abs() < 1.0, "one line: {one} vs {line}");
        assert!(
            two > one + 5.0,
            "max_lines 2 must be taller than max_lines 1"
        );
    }

    #[test]
    fn the_layout_height_wraps_before_the_cap() {
        // The bounding rect itself (before the cap) must be taller than one line for max_lines 2: it truncates, not clips.
        let attrs = attributes(&font(), [0.0; 4], Align::Left);
        // SAFETY: plain call on an owned string with attributes built above.
        let rect = unsafe {
            NSString::from_str(LONG).boundingRectWithSize_options_attributes_context(
                NSSize::new(100.0, 1.0e6),
                draw_options(2),
                Some(&attrs),
                None,
            )
        };
        assert!(rect.size.height > one_line() * 1.5, "{:?}", rect.size);
    }
}
