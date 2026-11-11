//! Native modal dialogs (Swift `showDialog` and `editPrompt`). Token entry is always an `NSAlert` with an
//! `NSSecureTextField`, never a scene field. Both functions spin a nested run loop: call them from
//! `main_queue::run_loop_later` (a modal entered from an `async_main` block starves the main queue), never from inside
//! an AppKit callback and never with the app state borrowed.

use crate::NotificationObserver;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn,
    NSApplication, NSBorderType, NSScrollView, NSSecureTextField, NSStatusWindowLevel, NSTextField,
    NSTextInputClient, NSTextView, NSView, NSWindowDidBecomeKeyNotification,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

/// What the token dialog ended with. The strings are untrimmed and unchecked: the caller trims and checks `is_safe`.
#[derive(Clone, PartialEq, Eq)]
pub enum TokenAnswer {
    Save {
        account: String,
        token: String,
    },
    /// `Tạo token…`: the caller opens the token page and asks again, keeping `account`.
    OpenTokenPage {
        account: String,
    },
    Cancel,
}

// A token must never reach a log through `{:?}`.
impl std::fmt::Debug for TokenAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenAnswer::Save { .. } => f.write_str("Save(..)"),
            TokenAnswer::OpenTokenPage { .. } => f.write_str("OpenTokenPage"),
            TokenAnswer::Cancel => f.write_str("Cancel"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptAnswer {
    /// `Lưu`: the text as typed.
    Save(String),
    /// `Mặc định`.
    Default,
    /// `Huỷ`.
    Cancel,
}

fn text_field(mtm: MainThreadMarker, frame: NSRect, placeholder: &str) -> Retained<NSTextField> {
    let field = NSTextField::initWithFrame(mtm.alloc(), frame);
    field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
    field
}

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

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

/// The email and API token dialog with `Lưu`, `Huỷ` and `Tạo token…`; `notice` (the previous attempt's error) goes above
/// `info`. `Cancel` off the main thread.
pub fn ask_token(title: &str, info: &str, notice: Option<&str>, account: &str) -> TokenAnswer {
    let Some(mtm) = MainThreadMarker::new() else {
        return TokenAnswer::Cancel;
    };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    let body = match notice {
        Some(n) => format!("{n}\n\n{info}"),
        None => info.to_string(),
    };
    alert.setInformativeText(&NSString::from_str(&body));
    let email = text_field(mtm, rect(0.0, 30.0, 320.0, 24.0), "Email Atlassian");
    email.setStringValue(&NSString::from_str(account));
    let token: Retained<NSSecureTextField> =
        NSSecureTextField::initWithFrame(mtm.alloc(), rect(0.0, 0.0, 320.0, 24.0));
    token.setPlaceholderString(Some(&NSString::from_str("API token")));
    // SAFETY: `token` is a live view of the same alert.
    unsafe { email.setNextKeyView(Some(&token)) };
    let holder = NSView::initWithFrame(mtm.alloc(), rect(0.0, 0.0, 320.0, 54.0));
    holder.addSubview(&email);
    holder.addSubview(&token);
    alert.setAccessoryView(Some(&holder));
    for label in ["Lưu", "Huỷ", "Tạo token…"] {
        alert.addButtonWithTitle(&NSString::from_str(label));
    }
    if account.is_empty() {
        alert.window().setInitialFirstResponder(Some(&email));
    } else {
        alert.window().setInitialFirstResponder(Some(&token));
    }
    let response = run_alert(mtm, &alert);
    let account = email.stringValue().to_string();
    if response == NSAlertFirstButtonReturn {
        let field: &NSTextField = &token;
        TokenAnswer::Save {
            account,
            token: field.stringValue().to_string(),
        }
    } else if response == NSAlertThirdButtonReturn {
        TokenAnswer::OpenTokenPage { account }
    } else {
        TokenAnswer::Cancel
    }
}

/// The `Prompt Nhờ Claude` editor with `Lưu`, `Huỷ` and `Mặc định`. `Cancel` off the main thread.
pub fn ask_prompt(current: &str, info: &str) -> PromptAnswer {
    let Some(mtm) = MainThreadMarker::new() else {
        return PromptAnswer::Cancel;
    };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("Prompt Nhờ Claude"));
    alert.setInformativeText(&NSString::from_str(info));
    let scroll = NSScrollView::initWithFrame(mtm.alloc(), rect(0.0, 0.0, 380.0, 120.0));
    scroll.setHasVerticalScroller(true);
    scroll.setBorderType(NSBorderType::BezelBorder);
    let size = scroll.contentSize();
    let text = NSTextView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::new(0.0, 0.0), size));
    text.setRichText(false);
    text.setVerticallyResizable(true);
    text.setHorizontallyResizable(false);
    text.setString(&NSString::from_str(current));
    scroll.setDocumentView(Some(&text));
    alert.setAccessoryView(Some(&scroll));
    for label in ["Lưu", "Huỷ", "Mặc định"] {
        alert.addButtonWithTitle(&NSString::from_str(label));
    }
    alert.window().setInitialFirstResponder(Some(&text));
    let response = run_alert(mtm, &alert);
    if response == NSAlertFirstButtonReturn {
        PromptAnswer::Save(text.string().to_string())
    } else if response == NSAlertThirdButtonReturn {
        PromptAnswer::Default
    } else {
        PromptAnswer::Cancel
    }
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
