//! Text attributes shared by drawing and measuring, and the `TextMeasure` implementation.

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
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSRect, NSSize, NSString,
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

fn ns_font(spec: &FontSpec) -> Retained<NSFont> {
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

/// `max_lines` 0 wraps without limit; otherwise the last allowed line is cut with an ellipsis.
pub(crate) fn attributes(
    spec: &FontSpec,
    color: [f64; 4],
    align: Align,
    max_lines: u32,
) -> Attributes {
    attributes_for(&ns_font(spec), color, align, max_lines)
}

fn attributes_for(font: &NSFont, color: [f64; 4], align: Align, max_lines: u32) -> Attributes {
    let color = NSColor::colorWithSRGBRed_green_blue_alpha(color[0], color[1], color[2], color[3]);
    let style = NSMutableParagraphStyle::new();
    style.setAlignment(match align {
        Align::Left => NSTextAlignment::Left,
        Align::Center => NSTextAlignment::Center,
        Align::Right => NSTextAlignment::Right,
    });
    style.setLineBreakMode(if max_lines == 0 {
        NSLineBreakMode::ByWordWrapping
    } else {
        NSLineBreakMode::ByTruncatingTail
    });
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
    max_lines: u32,
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
        let attrs = attributes_for(&font, color, align, max_lines);
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

pub(crate) fn draw_options() -> NSStringDrawingOptions {
    NSStringDrawingOptions::UsesLineFragmentOrigin
}

/// The rect text is laid out in: `max_lines` caps the height so truncation happens at the right line.
pub(crate) fn text_rect(rect: NSRect, spec: &FontSpec, max_lines: u32) -> NSRect {
    if max_lines == 0 {
        return rect;
    }
    let height = line_height(&ns_font(spec)) * f64::from(max_lines);
    NSRect::new(
        rect.origin,
        NSSize::new(rect.size.width, rect.size.height.min(height)),
    )
}

/// `TextMeasure` over the AppKit text system (CoreText underneath).
pub struct CoreTextMeasure;

impl TextMeasure for CoreTextMeasure {
    fn measure(&self, text: &str, font: &FontSpec, max_width: f64, max_lines: u32) -> Size {
        let attrs = attributes(font, [0.0; 4], Align::Left, max_lines);
        let unlimited = NSSize::new(max_width, 1.0e6);
        // SAFETY: plain call on an owned string with attributes built above.
        let bounds = unsafe {
            NSString::from_str(text).boundingRectWithSize_options_attributes_context(
                unlimited,
                draw_options(),
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
