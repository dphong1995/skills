//! Native modal dialogs (Swift `showDialog` and `editPrompt`). Token entry is always an `NSAlert` with an `NSSecureTextField`,
//! never a scene field. Both functions spin a nested run loop: call them from `main_queue::async_main`, never from inside an
//! AppKit callback and never with the app state borrowed.

use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertThirdButtonReturn, NSApplication, NSBorderType,
    NSScrollView, NSSecureTextField, NSTextField, NSTextView, NSView,
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
    NSApplication::sharedApplication(mtm).activate();
    let response = alert.runModal();
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
    NSApplication::sharedApplication(mtm).activate();
    let response = alert.runModal();
    if response == NSAlertFirstButtonReturn {
        PromptAnswer::Save(text.string().to_string())
    } else if response == NSAlertThirdButtonReturn {
        PromptAnswer::Default
    } else {
        PromptAnswer::Cancel
    }
}
