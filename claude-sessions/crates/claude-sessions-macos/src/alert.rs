//! Native dialogs. Token entry is always an `NSAlert` with an `NSSecureTextField`, never a scene field.

use objc2::rc::Retained;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSSecureTextField, NSTextField};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

/// Shows a modal alert with a secure text field and returns the entered text on OK, `None` on Cancel or off the
/// main thread. Call it from `main_queue::async_main`, never from inside an AppKit callback.
pub fn ask_secret(title: &str, message: &str) -> Option<String> {
    let mtm = MainThreadMarker::new()?;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    alert.addButtonWithTitle(&NSString::from_str("OK"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    let field: Retained<NSSecureTextField> = NSSecureTextField::initWithFrame(
        mtm.alloc(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(260.0, 24.0)),
    );
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    if alert.runModal() != NSAlertFirstButtonReturn {
        return None;
    }
    let field: &NSTextField = &field;
    Some(field.stringValue().to_string())
}
