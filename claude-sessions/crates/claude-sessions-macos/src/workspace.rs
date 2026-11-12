//! `NSWorkspace`: opening links, finding the cmux app by bundle id (never `PATH`), the frontmost app.

use std::path::PathBuf;

use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationOptions, NSPasteboard, NSPasteboardTypeString, NSRunningApplication,
    NSWorkspace,
};
use objc2_foundation::{NSString, NSURL};
use url::Url;

use claude_sessions_core::cmux;

/// Opens `url` with `NSWorkspace` when it is an `https` URL; false (nothing opened) for any other scheme.
pub fn open_https(url: &Url) -> bool {
    open_https_with(url, open_url)
}

/// `open_https` with the opener injected: `opener` runs only for an `https` URL with a host and without userinfo.
pub fn open_https_with(url: &Url, opener: impl FnOnce(&Url) -> bool) -> bool {
    open_checked(url, &["https"], opener)
}

/// Like `open_https` but `http` is allowed too: for links in a transcript's markdown, which may be plain `http`. The
/// token page and PR links stay on `open_https`.
pub fn open_link_allow_http(url: &Url) -> bool {
    open_link_allow_http_with(url, open_url)
}

/// `open_link_allow_http` with the opener injected.
pub fn open_link_allow_http_with(url: &Url, opener: impl FnOnce(&Url) -> bool) -> bool {
    open_checked(url, &["https", "http"], opener)
}

fn open_checked(url: &Url, schemes: &[&str], opener: impl FnOnce(&Url) -> bool) -> bool {
    let plain = schemes.contains(&url.scheme())
        && url.host_str().is_some_and(|h| !h.is_empty())
        && url.username().is_empty()
        && url.password().is_none();
    plain && opener(url)
}

fn open_url(url: &Url) -> bool {
    let Some(ns) = NSURL::URLWithString(&NSString::from_str(url.as_str())) else {
        return false;
    };
    NSWorkspace::sharedWorkspace().openURL(&ns)
}

/// The `cmux` CLI inside the app bundle that `com.cmuxterm.app` resolves to; `None` when cmux is not installed.
pub fn cmux_cli_path() -> Option<PathBuf> {
    let app = NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(cmux::BUNDLE_ID))?;
    let path = app.path()?;
    Some(cmux::cli_path(std::path::Path::new(&path.to_string())))
}

/// The bundle id of the frontmost application.
pub fn frontmost_bundle_id() -> Option<String> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    app.bundleIdentifier().map(|b| b.to_string())
}

/// Replaces the general pasteboard's text with `text`.
pub fn copy_to_pasteboard(text: &str) {
    let board = NSPasteboard::generalPasteboard();
    board.clearContents();
    // SAFETY: `NSPasteboardTypeString` is a constant string of AppKit.
    let kind = unsafe { NSPasteboardTypeString };
    board.setString_forType(&NSString::from_str(text), kind);
}

/// An application that was in front, kept to bring it back later.
pub struct FrontApp(Retained<NSRunningApplication>);

impl FrontApp {
    /// Brings the application to the front. False when it could not be activated (e.g. it quit).
    pub fn activate(&self) -> bool {
        self.0
            .activateWithOptions(NSApplicationActivationOptions::empty())
    }
}

/// The frontmost application unless it is this process.
pub fn frontmost_other_app() -> Option<FrontApp> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    (app.processIdentifier() as u32 != std::process::id()).then_some(FrontApp(app))
}
