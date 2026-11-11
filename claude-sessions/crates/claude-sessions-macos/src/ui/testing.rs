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

/// The native text field with `Field.key` `key`, wherever it sits in `view`.
pub fn field(
    view: &SceneView,
    key: &str,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSTextField>> {
    view.subviews()
        .iter()
        .filter_map(|v| v.downcast::<NodeView>().ok())
        .find_map(|n| n.field(key))
}

/// Test hook: runs a bare, invisible alert (alpha 0, mouse-transparent, offscreen, no app activation) through the
/// dialogs' `run_alert_above_panel` and returns its window level and alpha read inside the modal session, which a
/// main-queue block then aborts (no click needed).
pub fn alert_level_during_modal() -> Option<(isize, f64)> {
    use std::cell::Cell;
    use std::rc::Rc;

    use objc2_app_kit::{NSAlert, NSApplication};
    use objc2_foundation::{MainThreadMarker, NSString};

    let mtm = MainThreadMarker::new()?;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("test"));
    let level = Rc::new(Cell::new(None));
    let (seen, window) = (level.clone(), alert.window());
    window.setAlphaValue(0.0);
    window.setIgnoresMouseEvents(true);
    window.setFrameOrigin(objc2_foundation::NSPoint::new(-30000.0, -30000.0));
    // Queued after `run_alert`'s own in-session block, so it reads the level that block left.
    crate::main_queue::async_main(move || {
        crate::main_queue::async_main(move || {
            seen.set(Some((window.level(), window.alphaValue())));
            NSApplication::sharedApplication(mtm).abortModal();
        });
    });
    crate::dialogs::run_alert_above_panel(mtm, &alert);
    level.get()
}
