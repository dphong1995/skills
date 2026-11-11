//! One panel per display: creation, placement and the pointer test (`syncPanels`, `trackMouse`).

use claude_sessions_core::scene::{Point, Rect, Scene};
use claude_sessions_macos::NotificationObserver;
use claude_sessions_macos::keys::{self, KeyMonitor};
use claude_sessions_macos::mouse;
use claude_sessions_macos::screens::KeyedScreen;
use claude_sessions_macos::ui::{EventHandler, WidgetPanelHandle};

use crate::fake::{self, CANVAS};

pub struct PanelHost {
    pub key: String,
    pub window: WidgetPanelHandle,
    /// Hovered or in keyboard mode: the expanded card is shown.
    pub expanded: bool,
    pub keyboard: bool,
    pub scene: Scene,
    pub key_monitor: Option<KeyMonitor>,
    _resign: NotificationObserver,
}

impl PanelHost {
    /// A hidden panel in the top-right corner of `screen`'s visible area.
    pub fn new(
        screen: &KeyedScreen,
        on_event: EventHandler,
        on_resign_key: impl Fn() + 'static,
    ) -> Option<Self> {
        let v = screen.visible_frame;
        let frame = Rect::new(
            v.x + v.w - CANVAS.w - 16.0,
            v.y + v.h - CANVAS.h - 10.0,
            CANVAS.w,
            CANVAS.h,
        );
        let window = WidgetPanelHandle::new(frame, on_event)?;
        let resign = keys::observe_resign_key(&window, on_resign_key);
        Some(Self {
            key: screen.key.clone(),
            window,
            expanded: false,
            keyboard: false,
            scene: Scene::default(),
            key_monitor: None,
            _resign: resign,
        })
    }

    /// Whether the pointer (screen points, bottom-left origin) is over the card.
    pub fn pointer_inside(&self) -> bool {
        let f = self.window.frame();
        let pointer = mouse::pointer();
        let local = Point {
            x: pointer.x - f.x,
            y: f.y + f.h - pointer.y,
        };
        fake::card_contains(&self.scene, local)
    }
}
