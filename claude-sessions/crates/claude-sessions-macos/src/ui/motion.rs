//! The motion table (spec `Chuyển động`). Everything here runs in Core Animation, in the render server: nothing wakes the
//! app per frame and there is no app-side timer. Reduce Motion turns all of it off; `render_png` never calls into this.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_app_kit::NSAnimationContext;
use objc2_quartz_core::CAMediaTimingFunction;

use crate::accessibility;

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

/// False while Reduce Motion is on.
pub fn enabled() -> bool {
    !accessibility::reduce_motion()
}

/// A critically damped spring as a timing curve: fast start, settles without overshoot.
fn spring() -> Retained<CAMediaTimingFunction> {
    CAMediaTimingFunction::functionWithControlPoints(0.2, 0.9, 0.25, 1.0)
}

/// Runs `changes` as one animation group of `seconds` (the spring curve when `spring_curve`); `done` runs when it ends.
pub fn group(
    seconds: f64,
    spring_curve: bool,
    changes: impl Fn() + 'static,
    done: impl Fn() + 'static,
) {
    let body = RcBlock::new(move |ctx: NonNull<NSAnimationContext>| {
        // SAFETY: AppKit passes a live context for the length of the call.
        let ctx = unsafe { ctx.as_ref() };
        ctx.setDuration(seconds);
        if spring_curve {
            ctx.setTimingFunction(Some(&spring()));
        }
        changes();
    });
    let end = RcBlock::new(done);
    NSAnimationContext::runAnimationGroup_completionHandler(&body, Some(&end));
}
