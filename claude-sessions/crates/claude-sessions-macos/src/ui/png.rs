//! Offscreen rendering: the same draw commands into a bitmap, no window and no display.

use std::ffi::c_void;
use std::path::Path;
use std::ptr::NonNull;

use claude_sessions_core::scene::{Appearance, Scene};
use objc2::AnyThread;
use objc2_app_kit::{NSApplication, NSBitmapImageFileType, NSBitmapImageRep, NSGraphicsContext};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGBitmapContextCreateImage, CGColorSpace, CGContext, kCGColorSpaceSRGB};
use objc2_foundation::{MainThreadMarker, NSData, NSDictionary};

use super::draw::{draw_cmds, register_images};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: &CGColorSpace,
        bitmap_info: u32,
    ) -> Option<NonNull<CGContext>>;
}

/// `kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little`.
const BGRA_PREMULTIPLIED: u32 = 2 | (2 << 12);

/// Draws `scene` at `scale` into a PNG at `path`. Main thread only.
pub fn render_png(
    scene: &Scene,
    scale: f64,
    appearance: Appearance,
    path: &Path,
) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("render_png must run on the main thread")?;
    NSApplication::sharedApplication(mtm);
    let width = (scene.size.w * scale).ceil() as usize;
    let height = (scene.size.h * scale).ceil() as usize;
    let space = // SAFETY: a plain extern constant.
    CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).ok_or("no colour space")?;
    // SAFETY: a null data pointer asks CoreGraphics to own the pixel memory; the colour space outlives the call.
    let ctx = unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width,
            height,
            8,
            0,
            &space,
            BGRA_PREMULTIPLIED,
        )
    }
    .ok_or("cannot create a bitmap context")?;
    // SAFETY: the pointer is a +1 CGContext returned by a Create function.
    let ctx: CFRetained<CGContext> = unsafe { CFRetained::from_raw(ctx) };
    // Flip to a top-left origin and scale to pixels.
    CGContext::translate_ctm(Some(&ctx), 0.0, height as f64);
    CGContext::scale_ctm(Some(&ctx), scale, -scale);

    let gc = NSGraphicsContext::graphicsContextWithCGContext_flipped(&ctx, true);
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&gc));
    register_images(&scene.images);
    for node in &scene.nodes {
        gc.saveGraphicsState();
        CGContext::translate_ctm(Some(&ctx), node.frame.x, node.frame.y);
        CGContext::clip_to_rect(
            Some(&ctx),
            CGRect::new(
                CGPoint::new(0.0, 0.0),
                CGSize::new(node.frame.w, node.frame.h),
            ),
        );
        draw_cmds(&node.cmds, appearance);
        gc.restoreGraphicsState();
    }
    NSGraphicsContext::restoreGraphicsState_class();

    let image = CGBitmapContextCreateImage(Some(&ctx)).ok_or("no image from the bitmap")?;
    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
    // SAFETY: an empty property dictionary is valid for PNG.
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .ok_or("PNG encoding failed")?;
    std::fs::write(path, data.to_vec()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Pixel size of the PNG at `path` and the colour (r, g, b, a in 0..1, not premultiplied) at pixel `(x, y)`.
pub fn read_png_pixel(
    path: &Path,
    x: usize,
    y: usize,
) -> Result<((usize, usize), [f64; 4]), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(&bytes))
        .ok_or("not a readable image")?;
    let size = (rep.pixelsWide() as usize, rep.pixelsHigh() as usize);
    let color = rep
        .colorAtX_y(x as isize, y as isize)
        .ok_or("pixel is outside the image")?;
    Ok((
        size,
        [
            color.redComponent(),
            color.greenComponent(),
            color.blueComponent(),
            color.alphaComponent(),
        ],
    ))
}
