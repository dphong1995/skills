//! Displays: one stable key per display and the change notification (`syncPanels`, `main.swift:117-140`).

use std::ptr::NonNull;

use claude_sessions_core::scene::Rect;
use objc2_app_kit::{NSApplicationDidChangeScreenParametersNotification, NSScreen};
use objc2_core_foundation::{CFRetained, CFUUID};
use objc2_foundation::{MainThreadMarker, NSNumber, NSRect, ns_string};

use crate::observer::NotificationObserver;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> Option<NonNull<CFUUID>>;
}

/// A display with its UUID string as key (`"main"` when no UUID is available). Frames are in screen points with
/// AppKit's bottom-left origin.
pub struct KeyedScreen {
    pub key: String,
    pub frame: Rect,
    pub visible_frame: Rect,
}

fn rect(r: NSRect) -> Rect {
    Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height)
}

fn display_uuid(screen: &NSScreen) -> Option<String> {
    let number = screen
        .deviceDescription()
        .objectForKey(ns_string!("NSScreenNumber"))?;
    let id = number.downcast_ref::<NSNumber>()?.unsignedIntValue();
    // SAFETY: a Create function: the result is +1 and wrapped into an owning `CFRetained`.
    let uuid = unsafe { CFRetained::from_raw(CGDisplayCreateUUIDFromDisplayID(id)?) };
    CFUUID::new_string(None, Some(&uuid)).map(|s| s.to_string())
}

/// Connected displays, the main one first; `None` off the main thread.
pub fn keyed_screens() -> Option<Vec<KeyedScreen>> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    let keyed: Vec<KeyedScreen> = screens
        .iter()
        .filter_map(|s| {
            Some(KeyedScreen {
                key: display_uuid(&s)?,
                frame: rect(s.frame()),
                visible_frame: rect(s.visibleFrame()),
            })
        })
        .collect();
    if keyed.is_empty() {
        let first = screens.firstObject()?;
        return Some(vec![KeyedScreen {
            key: "main".into(),
            frame: rect(first.frame()),
            visible_frame: rect(first.visibleFrame()),
        }]);
    }
    Some(keyed)
}

/// Calls `callback` on the main thread whenever `NSApplicationDidChangeScreenParametersNotification` fires.
pub fn observe_changes(callback: impl Fn() + 'static) -> Option<NotificationObserver> {
    MainThreadMarker::new()?;
    // SAFETY: a plain extern constant.
    let name = unsafe { NSApplicationDidChangeScreenParametersNotification };
    Some(NotificationObserver::new(name, None, callback))
}
