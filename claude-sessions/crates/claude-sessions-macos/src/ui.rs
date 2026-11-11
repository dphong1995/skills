//! The AppKit renderer for a `Scene`: panel handle, scene view, events and the offscreen PNG.

mod draw;
pub mod motion;
mod node_view;
mod png;
mod scene_view;

use std::rc::Rc;

pub use claude_sessions_core::scene::Appearance;
use claude_sessions_core::scene::{HitId, Point, Rect, Scene};
use objc2::rc::Retained;
use objc2_foundation::MainThreadMarker;

use self::draw::ns_rect;

use crate::panel::{self, PanelProps, WidgetPanel};
pub use png::{PngDiff, compare_png, read_png_pixel, render_png};
pub use scene_view::SceneView;

/// What the user did in the scene; `at` is in scene points (top-left origin).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiEvent {
    Click(HitId),
    RightClick {
        hit: Option<HitId>,
        at: Point,
    },
    /// The panel was dragged and dropped; its frame already moved.
    Dragged,
}

/// Receives what the user does in a scene.
pub type EventHandler = Rc<dyn Fn(UiEvent)>;

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
        SceneView::observe_reduce_motion(&view);
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

    /// Hidden panels keep no looping animation.
    pub fn hide(&self) {
        self.view.set_motion_live(false);
        self.panel.orderOut(None);
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
