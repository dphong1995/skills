//! The AppKit renderer for a `Scene`: panel handle, scene view, events and the offscreen PNG.

pub(crate) mod draw;
pub mod motion;
mod node_view;
mod png;
mod scene_view;
pub mod testing;

use std::rc::Rc;

pub use claude_sessions_core::scene::Appearance;
use claude_sessions_core::scene::{HitId, Point, Rect, Scene};
use objc2::rc::Retained;
use objc2::{ClassType, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSTextView};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol};

use self::draw::ns_rect;

use crate::panel::{self, PanelProps, WidgetPanel};
pub use png::{PngDiff, compare_png, read_png_pixel, render_png};
pub use scene_view::SceneView;

/// What the user did in the scene; `at` is in scene points (top-left origin).
#[derive(Clone, PartialEq)]
pub enum UiEvent {
    Click(HitId),
    RightClick {
        hit: Option<HitId>,
        at: Point,
    },
    /// The panel was dragged and dropped; its frame already moved. Window origins in screen points: where the drag
    /// started, where the pointer asked for, and where the clamp hook left the window.
    Dragged {
        start: Point,
        raw: Point,
        clamped: Point,
    },
    /// The user typed in the native text field with this `Field.key`; `text` is its whole content now.
    FieldChanged {
        key: String,
        text: String,
    },
    /// Return was pressed in the text field with this `Field.key` (the key press is consumed).
    FieldSubmit {
        key: String,
    },
    /// Esc was pressed in the text field with this `Field.key` (the key press is consumed).
    FieldCancel {
        key: String,
    },
    /// The edit of the single-line or secure field with this `Field.key` ended (Return, Tab or a click elsewhere). A
    /// multi-line field sends `FieldSubmit` for this.
    FieldEnded {
        key: String,
    },
}

/// Field text can be a token, so `Debug` prints only its length.
impl std::fmt::Debug for UiEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Click(id) => f.debug_tuple("Click").field(id).finish(),
            Self::RightClick { hit, at } => f
                .debug_struct("RightClick")
                .field("hit", hit)
                .field("at", at)
                .finish(),
            Self::Dragged {
                start,
                raw,
                clamped,
            } => f
                .debug_struct("Dragged")
                .field("start", start)
                .field("raw", raw)
                .field("clamped", clamped)
                .finish(),
            Self::FieldChanged { key, text } => write!(
                f,
                "FieldChanged {{ key: {key:?}, text: <{} chars> }}",
                text.chars().count()
            ),
            Self::FieldSubmit { key } => f.debug_struct("FieldSubmit").field("key", key).finish(),
            Self::FieldCancel { key } => f.debug_struct("FieldCancel").field("key", key).finish(),
            Self::FieldEnded { key } => f.debug_struct("FieldEnded").field("key", key).finish(),
        }
    }
}

/// Receives what the user does in a scene.
pub type EventHandler = Rc<dyn Fn(UiEvent)>;

/// Maps the window origin a drag asks for to the origin the window takes.
pub type ClampHandler = Rc<dyn Fn(Point) -> Point>;

/// A panel with its scene view. Frames are in screen points with AppKit's bottom-left origin.
#[derive(Clone)]
pub struct WidgetPanelHandle {
    pub(crate) panel: Retained<WidgetPanel>,
    pub(crate) view: Retained<SceneView>,
}

impl WidgetPanelHandle {
    /// `None` off the main thread. The panel stays hidden until `show`.
    pub fn new(frame: Rect, on_event: EventHandler) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let frame = ns_rect(frame);
        let panel = panel::new_widget_panel(mtm, frame);
        let view = SceneView::new(mtm, frame.size);
        view.set_handler(on_event);
        panel.setContentView(Some(&view));
        Some(Self { panel, view })
    }

    /// Like `new`, but the panel can become key, takes mouse events and sits one level above the widget's panels.
    pub fn new_key(frame: Rect, on_event: EventHandler) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let frame = ns_rect(frame);
        let panel = panel::new_key_panel(mtm, frame);
        let view = SceneView::new(mtm, frame.size);
        view.set_handler(on_event);
        panel.setContentView(Some(&view));
        Some(Self { panel, view })
    }

    pub fn view(&self) -> Retained<SceneView> {
        self.view.clone()
    }

    pub fn set_scene(&self, scene: &Scene) {
        self.view.set_scene(scene);
    }

    pub fn frame(&self) -> Rect {
        let f = self.panel.frame();
        Rect::new(f.origin.x, f.origin.y, f.size.width, f.size.height)
    }

    pub fn show(&self) {
        self.panel.orderFrontRegardless();
        self.view.set_motion_live(true);
    }

    /// Activates the app and makes the panel key, like a normal window.
    pub fn show_key(&self) {
        // `activateIgnoringOtherApps:` is deprecated since macOS 14 but still forces activation, as `run_alert` does.
        #[allow(deprecated)]
        NSApplication::sharedApplication(self.panel.mtm()).activateIgnoringOtherApps(true);
        self.panel.makeKeyAndOrderFront(None);
        self.view.set_motion_live(true);
    }

    /// Whether the window's first responder is a text view: a field editor or a multi-line field.
    pub fn is_editing(&self) -> bool {
        self.panel
            .firstResponder()
            .is_some_and(|r| r.isKindOfClass(NSTextView::class()))
    }

    /// Ends the edit in progress (a multi-line field sends its `FieldSubmit`).
    pub fn end_editing(&self) {
        self.panel.makeFirstResponder(None);
    }

    /// Makes the native field with that `Field.key` first responder and scrolls it into view.
    pub fn focus_field(&self, key: &str) -> bool {
        self.view.focus_field(key)
    }

    /// Hidden panels keep no looping animation.
    pub fn hide(&self) {
        self.view.set_motion_live(false);
        self.panel.orderOut(None);
    }

    pub fn set_clamp(&self, clamp: ClampHandler) {
        self.view.set_clamp(clamp);
    }

    /// Moves the panel without animating; `size` is the frame size, `origin` the bottom-left corner.
    pub fn set_frame(&self, frame: Rect) {
        self.panel.setFrame_display(ns_rect(frame), false);
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// Whether the panel is on the active Space (a `canJoinAllSpaces` panel always should be).
    pub fn is_on_active_space(&self) -> bool {
        self.panel.isOnActiveSpace()
    }

    pub fn close(&self) {
        self.panel.close();
    }

    pub fn ignores_mouse(&self) -> bool {
        self.panel.ignoresMouseEvents()
    }

    pub fn set_ignores_mouse(&self, ignores: bool) {
        self.panel.setIgnoresMouseEvents(ignores);
    }

    pub fn props(&self) -> PanelProps {
        panel::props(&self.panel)
    }

    pub fn is_key(&self) -> bool {
        self.panel.isKeyWindow()
    }
}

#[cfg(test)]
mod tests {
    use super::UiEvent;

    #[test]
    fn field_changed_debug_hides_the_text() {
        let shown = format!(
            "{:?}",
            UiEvent::FieldChanged {
                key: "k".into(),
                text: "s3cr3t-token".into()
            }
        );
        assert!(!shown.contains("s3cr3t"), "{shown}");
        assert!(shown.contains("12 chars"), "{shown}");
    }

    #[test]
    fn field_ended_debug_names_only_the_key() {
        let shown = format!("{:?}", UiEvent::FieldEnded { key: "k".into() });
        assert_eq!(shown, "FieldEnded { key: \"k\" }");
    }
}
