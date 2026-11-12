//! The modal `NSAlert` helpers `ui::testing` runs its alerts with: forced activation, a window level above the panel, `Esc`
//! as the second button.

use crate::NotificationObserver;
use objc2_app_kit::{
    NSAlert, NSAlertSecondButtonReturn, NSApplication, NSStatusWindowLevel, NSTextInputClient,
    NSTextView, NSWindowDidBecomeKeyNotification,
};
use objc2_foundation::MainThreadMarker;

/// Runs `alert` modally the way Swift does: forced activation (`NSApp.activate(ignoringOtherApps: true)`) so the alert is
/// key, and a window level above the panel's `NSStatusWindowLevel` so the panel cannot cover it. AppKit puts the alert
/// back to the modal level (8) once it is key, so the level is also re-applied inside the session (`keep_above_panel`).
pub(crate) fn run_alert(mtm: MainThreadMarker, alert: &NSAlert) -> isize {
    // `activateIgnoringOtherApps:` is deprecated since macOS 14 but is what Swift calls and still forces activation.
    #[allow(deprecated)]
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    run_alert_above_panel(alert)
}

/// The level and `Esc` handling of `run_alert` without the activation.
pub(crate) fn run_alert_above_panel(alert: &NSAlert) -> isize {
    alert.window().setLevel(NSStatusWindowLevel + 1);
    let _keeper = keep_above_panel(alert);
    let _escape = escape_closes(alert);
    alert.runModal()
}

/// AppKit puts the alert back to `NSModalPanelWindowLevel` (8) when it becomes key, below the panel's 25; the level set
/// before `runModal` only holds until then. Each time the alert becomes key (after the session started, and again when the
/// user comes back to the app) the level is put back. The observer block is registered on the main queue and the
/// notification is posted from the main thread, so it runs synchronously inside the post; that is still late enough
/// because AppKit re-levels to 8 before it posts `didBecomeKey`.
fn keep_above_panel(alert: &NSAlert) -> NotificationObserver {
    let window = alert.window();
    // SAFETY: a plain notification-name constant.
    let became_key = unsafe { NSWindowDidBecomeKeyNotification };
    let target = window.clone();
    NotificationObserver::new(became_key, Some(&window), move || {
        if target.level() != NSStatusWindowLevel + 1 {
            target.setLevel(NSStatusWindowLevel + 1);
        }
    })
}

/// `Esc` ends the session with the second button's code (`Huỷ`) while the alert is key. AppKit binds `Esc` only to a
/// button titled "Cancel", and giving `Huỷ` that key equivalent makes AppKit move it to the end of the button column.
/// Only Shift, Control, Option and Command count as modifiers (Caps Lock, Numeric Pad and Function do not), and `Esc`
/// passes through while an input method composes text, so it cancels the composition and not the dialog.
fn escape_closes(alert: &NSAlert) -> Option<crate::keys::KeyMonitor> {
    let mtm = MainThreadMarker::new()?;
    let window = alert.window();
    crate::keys::KeyMonitor::new(move |key| {
        let plain = key.flags & 0x1e_0000 == 0;
        let composing = window
            .firstResponder()
            .and_then(|r| r.downcast::<NSTextView>().ok())
            .is_some_and(|text| text.hasMarkedText());
        if key.key_code == crate::keys::KEY_CODE_ESCAPE
            && plain
            && !composing
            && window.isKeyWindow()
        {
            NSApplication::sharedApplication(mtm).stopModalWithCode(NSAlertSecondButtonReturn);
            true
        } else {
            false
        }
    })
}
