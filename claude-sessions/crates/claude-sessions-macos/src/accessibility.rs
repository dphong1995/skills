//! The system's Reduce Motion setting.

use crate::NotificationObserver;
use objc2_app_kit::{NSWorkspace, NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification};

/// `NSWorkspace.accessibilityDisplayShouldReduceMotion`: when true, nothing may animate.
pub fn reduce_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// Calls `on_change` (main thread) whenever an accessibility display option changes, until the observer is dropped.
pub fn observe_reduce_motion(on_change: impl Fn() + 'static) -> NotificationObserver {
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    // SAFETY: a plain notification-name constant of AppKit.
    let name = unsafe { NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification };
    NotificationObserver::on(&center, name, None, on_change)
}
