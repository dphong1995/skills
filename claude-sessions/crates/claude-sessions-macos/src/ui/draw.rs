//! Draws `DrawCmd`s into the current `NSGraphicsContext`, which must be flipped (top-left origin).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use claude_sessions_core::scene::{Appearance, Color, DrawCmd, Rect};
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAttributedStringNSExtendedStringDrawing, NSBezierPath, NSColor, NSCompositingOperation,
    NSImage, NSImageSymbolConfiguration, NSStringNSExtendedStringDrawing,
};
use objc2_foundation::{NSData, NSPoint, NSRect, NSSize, NSString};

use crate::text;

thread_local! {
    /// Decoded pictures by `DrawCmd::Image` key; a key that failed to decode stays `None` so it is not retried.
    static IMAGES: RefCell<HashMap<String, Option<Retained<NSImage>>>> = RefCell::new(HashMap::new());
}

/// Decodes the pictures of a scene that are not cached yet (once per key).
pub(crate) fn register_images(images: &HashMap<String, Arc<Vec<u8>>>) {
    IMAGES.with_borrow_mut(|cache| {
        for (key, bytes) in images {
            cache.entry(key.clone()).or_insert_with(|| {
                NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(bytes))
            });
        }
    });
}

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
                let attrs = text::attributes(font, color.resolve(appearance), *align);
                let area = text::text_rect(ns_rect(*rect), font, *max_lines);
                // SAFETY: plain draw call with attributes built by `text::attributes`.
                unsafe {
                    NSString::from_str(text).drawWithRect_options_attributes_context(
                        area,
                        text::draw_options(*max_lines),
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
                let string = text::rich_string(runs, font, color.resolve(appearance), *align);
                let area = text::text_rect(ns_rect(*rect), font, *max_lines);
                string.drawWithRect_options_context(area, text::draw_options(*max_lines), None);
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
            DrawCmd::Arc {
                center,
                radius,
                width,
                start,
                end,
                color,
            } => {
                // Scene angles run clockwise from 12 o'clock; in the flipped context that is increasing angle from -90.
                let path = NSBezierPath::bezierPath();
                path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
                    NSPoint::new(center.x, center.y),
                    *radius,
                    start - 90.0,
                    end - 90.0,
                    false,
                );
                path.setLineWidth(*width);
                path.setLineCapStyle(objc2_app_kit::NSLineCapStyle::Round);
                ns_color(*color, appearance).setStroke();
                path.stroke();
            }
            DrawCmd::Image { rect, key } => {
                if let Some(image) = IMAGES.with_borrow(|c| c.get(key).cloned().flatten()) {
                    // SAFETY: no hints are passed.
                    unsafe {
                        image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                            ns_rect(*rect),
                            NSRect::ZERO,
                            NSCompositingOperation::SourceOver,
                            1.0,
                            true,
                            None,
                        );
                    }
                }
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
