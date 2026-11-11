//! Synthetic mouse events dispatched straight to a `SceneView` (never posted to the event queue), so a panel that was
//! never ordered on screen can be clicked in tests.

use claude_sessions_core::scene::Point;
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::NSPoint;

use super::SceneView;
use super::motion;
use super::node_view::NodeView;

fn event(view: &SceneView, kind: NSEventType, at: Point) -> Option<objc2::rc::Retained<NSEvent>> {
    // The view is flipped: scene y runs down, window y runs up.
    let height = view.frame().size.height;
    NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind,
        NSPoint::new(at.x, height - at.y),
        NSEventModifierFlags::empty(),
        0.0,
        0,
        None,
        0,
        1,
        1.0,
    )
}

/// A press and release at `at` (scene points). With `dragged`, a drag event comes between them; the drag distance is
/// read from the real pointer as in the app, so it stays below the drag threshold and the press still clicks.
pub fn click(view: &SceneView, at: Point, dragged: bool) -> bool {
    let (Some(down), Some(up)) = (
        event(view, NSEventType::LeftMouseDown, at),
        event(view, NSEventType::LeftMouseUp, at),
    ) else {
        return false;
    };
    view.mouseDown(&down);
    if let Some(drag) = event(view, NSEventType::LeftMouseDragged, at).filter(|_| dragged) {
        view.mouseDragged(&drag);
    }
    view.mouseUp(&up);
    true
}

/// Moves the first node view of `view` to `frame` through `NodeView::animate_frame` inside an animation group, the path a
/// resize takes when motion is on (`set_scene` skips it under Reduce Motion). False when `view` has no node view.
pub fn animate_first_node_frame(
    view: &SceneView,
    frame: claude_sessions_core::scene::Rect,
) -> bool {
    let Some(node) = view
        .subviews()
        .iter()
        .find_map(|v| v.downcast::<NodeView>().ok())
    else {
        return false;
    };
    motion::group(
        motion::OPEN_SECS,
        true,
        move || node.animate_frame(frame),
        || {},
    );
    true
}

/// Test hook: runs a bare, invisible alert (alpha 0, mouse-transparent, offscreen, no app activation) the way the real
/// dialogs run theirs, from inside a main-queue block, through `dialogs::run_alert_above_panel`. A timer serviced by the
/// modal run loop (a main-queue block would not run inside a modal entered from a main-queue block) reads the window
/// level and alpha inside the session and aborts it. A watchdog thread ends the process if the session never ends.
/// Not part of the app's API.
#[doc(hidden)]
pub fn alert_level_during_modal() -> Result<(isize, f64), String> {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use block2::RcBlock;
    use objc2_app_kit::{NSAlert, NSApplication, NSModalPanelRunLoopMode};
    use objc2_foundation::{
        MainThreadMarker, NSDate, NSDefaultRunLoopMode, NSRunLoop, NSString, NSTimer,
    };

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let finished = Arc::new(AtomicBool::new(false));
    let guard = finished.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        if !guard.load(Ordering::SeqCst) {
            eprintln!("alert_level_during_modal: the modal session never ended (watchdog)");
            std::process::exit(1);
        }
    });
    let level = Rc::new(Cell::new(None));
    let ended = Rc::new(Cell::new(false));
    let (seen, over) = (level.clone(), ended.clone());
    crate::main_queue::async_main(move || {
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str("test"));
        let window = alert.window();
        window.setAlphaValue(0.0);
        window.setIgnoresMouseEvents(true);
        window.setFrameOrigin(objc2_foundation::NSPoint::new(-30000.0, -30000.0));
        let read = RcBlock::new(move |_: std::ptr::NonNull<NSTimer>| {
            seen.set(Some((window.level(), window.alphaValue())));
            NSApplication::sharedApplication(mtm).abortModal();
        });
        // SAFETY: the block is copied by the timer and only runs on the main run loop.
        let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(0.1, false, &read) };
        // SAFETY: a live timer and a run-loop mode constant, on the main thread.
        unsafe {
            NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSModalPanelRunLoopMode);
        }
        crate::dialogs::run_alert_above_panel(&alert);
        over.set(true);
    });
    while !ended.get() {
        // SAFETY: a plain run-loop mode constant.
        let mode = unsafe { NSDefaultRunLoopMode };
        NSRunLoop::currentRunLoop()
            .runMode_beforeDate(mode, &NSDate::dateWithTimeIntervalSinceNow(0.05));
    }
    finished.store(true, Ordering::SeqCst);
    level
        .get()
        .ok_or_else(|| "the modal session ended without the level read".to_string())
}
