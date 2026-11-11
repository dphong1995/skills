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
