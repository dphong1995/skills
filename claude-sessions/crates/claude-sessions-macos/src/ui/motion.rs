//! The motion table (spec `Chuyển động`). Everything here runs in Core Animation, in the render server: nothing wakes the
//! app per frame and there is no app-side timer. `render_png` never calls into this.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSAnimationContext;
use objc2_core_foundation::CGPoint;
use objc2_foundation::{NSValue, ns_string};
use objc2_quartz_core::{
    CABasicAnimation, CALayer, CAMediaTiming, CAMediaTimingFunction, CATransform3D,
    CATransform3DIdentity, NSValueCATransform3DAdditions, kCAFillModeForwards,
};

/// Collapsed pill to card: the size change, a spring that does not overshoot.
pub const OPEN_SECS: f64 = 0.35;
/// A card sliding to its new place (another group or order).
pub const MOVE_SECS: f64 = 0.25;
/// A card or node fading in or out.
pub const FADE_SECS: f64 = 0.25;
/// A new node starts this far above its place and slides down while it fades in.
pub const APPEAR_SLIDE: f64 = 8.0;
/// Status colours of an icon or text changing.
pub const COLOR_SECS: f64 = 0.2;
/// The one-shot pulse of a new-message sparkle.
pub const SPARKLE_SECS: f64 = 0.5;

/// The panel opening: the zoom from `POP_SCALE` to full size.
pub const POP_OPEN_SECS: f64 = 0.28;
/// The panel opening: the fade-in of everything that appears.
pub const POP_FADE_SECS: f64 = 0.18;
/// The panel closing: the zoom to `POP_SCALE` and the fade-out together.
pub const POP_CLOSE_SECS: f64 = 0.16;
/// The size the panel opens from and closes to, around the grow origin.
pub const POP_SCALE: f64 = 0.96;

/// A critically damped spring as a timing curve: fast start, settles without overshoot.
fn spring() -> Retained<CAMediaTimingFunction> {
    CAMediaTimingFunction::functionWithControlPoints(0.2, 0.9, 0.25, 1.0)
}

/// The curve of the panel opening (a quick start that eases out hard) or, not `opening`, of its closing (ease-out).
fn pop_curve(opening: bool) -> Retained<CAMediaTimingFunction> {
    if opening {
        CAMediaTimingFunction::functionWithControlPoints(0.23, 1.0, 0.32, 1.0)
    } else {
        // SAFETY: a plain extern constant.
        CAMediaTimingFunction::functionWithName(unsafe {
            objc2_quartz_core::kCAMediaTimingFunctionEaseOut
        })
    }
}

/// Runs `changes` as one animation group of `seconds` (the spring curve when `spring_curve`); `done` runs when it ends.
pub fn group(
    seconds: f64,
    spring_curve: bool,
    changes: impl Fn() + 'static,
    done: impl Fn() + 'static,
) {
    run_group(seconds, spring_curve.then(spring), changes, done);
}

/// Like `group`, on the opening curve (`pop_open`), or with `opening` false on ease-out (the close).
pub fn pop_group(
    seconds: f64,
    opening: bool,
    changes: impl Fn() + 'static,
    done: impl Fn() + 'static,
) {
    run_group(seconds, Some(pop_curve(opening)), changes, done);
}

fn run_group(
    seconds: f64,
    curve: Option<Retained<CAMediaTimingFunction>>,
    changes: impl Fn() + 'static,
    done: impl Fn() + 'static,
) {
    let body = RcBlock::new(move |ctx: NonNull<NSAnimationContext>| {
        // SAFETY: AppKit passes a live context for the length of the call.
        let ctx = unsafe { ctx.as_ref() };
        ctx.setDuration(seconds);
        if let Some(curve) = &curve {
            ctx.setTimingFunction(Some(curve));
        }
        changes();
    });
    let end = RcBlock::new(done);
    NSAnimationContext::runAnimationGroup_completionHandler(&body, Some(&end));
}

/// `POP_SCALE` about `fixed`, a point in the coordinates of the layer's superlayer (the scene's). A layer's transform
/// acts about its `position` there, whatever its anchor point or its own flipping, so the translation is `(1 - scale)`
/// times the fixed point's offset from `position`.
fn zoom_transform(layer: &CALayer, fixed: CGPoint) -> CATransform3D {
    let position = layer.position();
    let (dx, dy) = (fixed.x - position.x, fixed.y - position.y);
    // SAFETY: a plain extern constant.
    let mut m = unsafe { CATransform3DIdentity };
    m.m11 = POP_SCALE;
    m.m22 = POP_SCALE;
    m.m41 = (1.0 - POP_SCALE) * dx;
    m.m42 = (1.0 - POP_SCALE) * dy;
    m
}

/// Opening: `layer` grows from `POP_SCALE` to full size about `fixed` (scene coordinates). An explicit animation: the
/// model value stays identity, so AppKit's own layer updates never fight it.
pub fn zoom_in(layer: &CALayer, fixed: CGPoint) {
    // SAFETY: a plain extern constant.
    let rest = unsafe { CATransform3DIdentity };
    let animation = CABasicAnimation::animationWithKeyPath(Some(ns_string!("transform")));
    // SAFETY: from/to are NSValues of CATransform3D, which the transform key path takes.
    unsafe {
        animation.setFromValue(Some(&transform_value(zoom_transform(layer, fixed))));
        animation.setToValue(Some(&transform_value(rest)));
    }
    animation.setDuration(POP_OPEN_SECS);
    animation.setTimingFunction(Some(&pop_curve(true)));
    layer.addAnimation_forKey(&animation, Some(ns_string!("zoom")));
}

/// Closing: `layer` shrinks to `POP_SCALE` about `fixed` (scene coordinates) and stays there (the caller removes the view when done).
pub fn zoom_out(layer: &CALayer, fixed: CGPoint) {
    // SAFETY: a plain extern constant.
    let rest = unsafe { CATransform3DIdentity };
    let animation = CABasicAnimation::animationWithKeyPath(Some(ns_string!("transform")));
    // SAFETY: from/to are NSValues of CATransform3D, which the transform key path takes.
    unsafe {
        animation.setFromValue(Some(&transform_value(rest)));
        animation.setToValue(Some(&transform_value(zoom_transform(layer, fixed))));
        animation.setFillMode(kCAFillModeForwards);
    }
    animation.setRemovedOnCompletion(false);
    animation.setDuration(POP_CLOSE_SECS);
    animation.setTimingFunction(Some(&pop_curve(false)));
    layer.addAnimation_forKey(&animation, Some(ns_string!("zoom")));
}

fn transform_value(m: CATransform3D) -> Retained<AnyObject> {
    // SAFETY: `valueWithCATransform3D:` takes a plain struct.
    let value: Retained<NSValue> = unsafe { NSValue::valueWithCATransform3D(m) };
    value.into()
}
