//! Keyboard mode: the non-activating panel becomes key on request, and hands the keys back (`main.swift:77-108`).

use std::ptr::NonNull;

use block2::RcBlock;
use objc2_app_kit::{NSEvent, NSEventMask, NSWindowDidResignKeyNotification};
use objc2_foundation::MainThreadMarker;

use crate::observer::{EventMonitor, NotificationObserver};
use crate::ui::WidgetPanelHandle;

pub const KEY_CODE_ESCAPE: u16 = 53;

/// A local `keyDown` monitor; dropped to stop.
pub struct KeyMonitor {
    _monitor: EventMonitor,
}

impl KeyMonitor {
    /// `handler` gets the key code and returns true to consume the key; false lets it reach the focused field.
    pub fn new(handler: impl Fn(u16) -> bool + 'static) -> Option<Self> {
        MainThreadMarker::new()?;
        let block = RcBlock::new(move |event: NonNull<NSEvent>| {
            // SAFETY: AppKit passes a live event for the length of the call.
            let code = unsafe { event.as_ref() }.keyCode();
            if handler(code) {
                std::ptr::null_mut()
            } else {
                event.as_ptr()
            }
        });
        // SAFETY: the block returns either null (consumed) or the event it was given.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
        };
        EventMonitor::new(monitor).map(|_monitor| Self { _monitor })
    }
}

/// Makes the panel key and puts the cursor in its first text field. Call from a later main-queue turn.
pub fn enter_keyboard(panel: &WidgetPanelHandle) {
    panel.panel.set_wants_key(true);
    panel.panel.makeKeyWindow();
    if let Some(field) = panel.view.first_field() {
        panel.panel.makeFirstResponder(Some(&field));
    }
}

/// `resignKey` is only a callback; reordering the panel is what hands key status back to the app that was in front.
/// Call from a later main-queue turn.
pub fn hand_back(panel: &WidgetPanelHandle) {
    panel.panel.set_wants_key(false);
    panel.panel.orderOut(None);
    panel.panel.orderFrontRegardless();
}

/// Calls `callback` when the panel stops being the key window.
pub fn observe_resign_key(
    panel: &WidgetPanelHandle,
    callback: impl Fn() + 'static,
) -> NotificationObserver {
    // SAFETY: a plain extern constant.
    let name = unsafe { NSWindowDidResignKeyNotification };
    NotificationObserver::new(name, Some(&panel.panel), callback)
}
