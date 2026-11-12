//! Process-level AppKit state.

use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;

/// `setActivationPolicy(.accessory)`: no Dock icon, no menu bar of its own (as `main.swift:649` does).
/// Returns false when not called on the main thread.
pub fn set_accessory_policy() -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory)
}

/// `NSApplication.run()`; returns when the app stops. Does nothing off the main thread.
pub fn run() {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).run();
    }
}

/// `NSApplication.terminate(nil)`.
pub fn quit() {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}

/// Whether a modal session (a dialog's `runModal`) is running: hotkeys and key-taking wait until it ends. False off the
/// main thread.
pub fn modal_open() -> bool {
    MainThreadMarker::new().is_some_and(|mtm| {
        NSApplication::sharedApplication(mtm)
            .modalWindow()
            .is_some()
    })
}

/// Whether this app is the active (frontmost) application. False off the main thread.
pub fn is_active() -> bool {
    MainThreadMarker::new().is_some_and(|mtm| NSApplication::sharedApplication(mtm).isActive())
}
