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
