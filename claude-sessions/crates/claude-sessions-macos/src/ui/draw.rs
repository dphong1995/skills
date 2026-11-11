//! Draws `DrawCmd`s into the current `NSGraphicsContext`, which must be flipped (top-left origin).

use claude_sessions_core::scene::{Appearance, Color, DrawCmd, Rect};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAttributedStringNSExtendedStringDrawing, NSBezierPath, NSColor, NSCompositingOperation,
    NSImage, NSImageSymbolConfiguration, NSStringNSExtendedStringDrawing,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::text;

pub(crate) fn ns_rect(r: Rect) -> NSRect {
    NSRect::new(NSPoint::new(r.x, r.y), NSSize::new(r.w, r.h))
}

pub(crate) fn ns_color(color: Color, appearance: Appearance) -> Retained<NSColor> {
    let [r, g, b, a] = color.resolve(appearance);
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a)
}

pub(crate) fn draw_cmds(cmds: &[DrawCmd], appearance: Appearance) {
    for cmd in cmds {
        match cmd {
            DrawCmd::RoundedRect {
                rect,
                radius,
                fill,
                stroke,
            } => {
                let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    ns_rect(*rect),
                    *radius,
                    *radius,
                );
                if let Some(fill) = fill {
                    ns_color(*fill, appearance).setFill();
                    path.fill();
                }
                if let Some(stroke) = stroke {
                    ns_color(*stroke, appearance).setStroke();
                    path.setLineWidth(1.0);
                    path.stroke();
                }
            }
            DrawCmd::Text {
                rect,
                text,
                font,
                color,
                max_lines,
                align,
            } => {
                let attrs = text::attributes(font, color.resolve(appearance), *align, *max_lines);
                let area = text::text_rect(ns_rect(*rect), font, *max_lines);
                // SAFETY: plain draw call with attributes built by `text::attributes`.
                unsafe {
                    NSString::from_str(text).drawWithRect_options_attributes_context(
                        area,
                        text::draw_options(),
                        Some(&attrs),
                        None,
                    );
                }
            }
            DrawCmd::Rich {
                rect,
                runs,
                font,
                color,
                max_lines,
                align,
            } => {
                let string =
                    text::rich_string(runs, font, color.resolve(appearance), *align, *max_lines);
                let area = text::text_rect(ns_rect(*rect), font, *max_lines);
                string.drawWithRect_options_context(area, text::draw_options(), None);
            }
            DrawCmd::Symbol {
                name,
                rect,
                color,
                weight,
            } => draw_symbol(name, *rect, *color, *weight, appearance),
            DrawCmd::Line {
                from,
                to,
                color,
                width,
            } => {
                let path = NSBezierPath::bezierPath();
                path.moveToPoint(NSPoint::new(from.x, from.y));
                path.lineToPoint(NSPoint::new(to.x, to.y));
                path.setLineWidth(*width);
                ns_color(*color, appearance).setStroke();
                path.stroke();
            }
        }
    }
}

/// An SF Symbol tinted with `color`, sized to the rect's height and centred in it. An unknown name draws nothing.
fn draw_symbol(
    name: &str,
    rect: Rect,
    color: Color,
    weight: claude_sessions_core::scene::Weight,
    appearance: Appearance,
) {
    let Some(image) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    ) else {
        return;
    };
    let size = NSImageSymbolConfiguration::configurationWithPointSize_weight(
        rect.h * 0.8,
        text::font_weight(weight),
    );
    let tint = NSImageSymbolConfiguration::configurationWithHierarchicalColor(&ns_color(
        color, appearance,
    ));
    let Some(image) =
        image.imageWithSymbolConfiguration(&size.configurationByApplyingConfiguration(&tint))
    else {
        return;
    };
    let natural = image.size();
    let target = NSRect::new(
        NSPoint::new(
            rect.x + (rect.w - natural.width) / 2.0,
            rect.y + (rect.h - natural.height) / 2.0,
        ),
        natural,
    );
    // SAFETY: no hints are passed.
    unsafe {
        image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
            target,
            NSRect::ZERO,
            NSCompositingOperation::SourceOver,
            1.0,
            true,
            None,
        );
    }
}
