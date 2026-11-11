//! In-process AppleScript (`NSAppleScript`), never `osascript`. It spins a nested run loop while it runs (and shows the
//! Automation prompt the first time), so it is only started on a fresh main-queue turn.

use objc2::AnyThread;
use objc2_foundation::{MainThreadMarker, NSAppleScript, NSString};

use crate::main_queue;

/// Runs `body` under `with timeout of <timeout_s> seconds` on a later main-queue turn, so no borrow of the caller is
/// held while the script runs; `done(true)` when it ran without an AppleScript error, on the main thread right after.
/// Does nothing off the main thread.
pub fn run_later(body: String, timeout_s: u32, done: impl FnOnce(bool) + 'static) {
    main_queue::async_main(move || done(run(&body, timeout_s)));
}

fn run(body: &str, timeout_s: u32) -> bool {
    if MainThreadMarker::new().is_none() {
        return false;
    }
    let source = format!("with timeout of {timeout_s} seconds\n{body}\nend timeout");
    let Some(script) =
        NSAppleScript::initWithSource(NSAppleScript::alloc(), &NSString::from_str(&source))
    else {
        return false;
    };
    let mut error = None;
    // SAFETY: `error` is a valid out-pointer for the call; the result descriptor is dropped unused.
    let _ = unsafe { script.executeAndReturnError(Some(&mut error)) };
    error.is_none()
}
