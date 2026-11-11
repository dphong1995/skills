//! Main-queue turns and the 1 s timer.

use std::cell::Cell;
use std::ptr::NonNull;
use std::time::Duration;

use block2::RcBlock;
use dispatch2::DispatchTime;
use objc2::rc::Retained;
use objc2_core_foundation::{CFRunLoop, kCFRunLoopDefaultMode};
use objc2_foundation::{MainThreadMarker, NSRunLoop, NSRunLoopCommonModes, NSTimer};

/// Moves a main-thread-only closure into the main dispatch queue.
struct MainOnly<F>(F);

// SAFETY: a `MainOnly` is created on the main thread and the main queue runs it on the main thread again.
unsafe impl<F> Send for MainOnly<F> {}

impl<F: FnOnce()> MainOnly<F> {
    fn run(self) {
        (self.0)();
    }
}

/// `work` runs on the main thread in a later run-loop turn, so nothing of the current callback is still borrowed.
/// `work` is not `Send`, so off the main thread it cannot be handed to the main queue soundly: it is dropped (a debug
/// build asserts). Callers that wait for the work (`apple_script::run_later`) check the thread themselves.
pub fn async_main(work: impl FnOnce() + 'static) {
    let on_main = MainThreadMarker::new().is_some();
    debug_assert!(on_main, "async_main called off the main thread");
    if !on_main {
        return;
    }
    let work = MainOnly(work);
    dispatch2::DispatchQueue::main().exec_async(move || work.run());
}

/// Runs `work` on the main thread in a later turn of the main run loop (default mode), outside any dispatch-queue block.
/// Start a nested run loop (a modal dialog) from here, not from `async_main`: a modal entered from a main-queue block
/// does not drain that queue, so every `async_main`, `wake_main` and `after` item would wait until it ends. Main thread only.
pub fn run_loop_later(work: impl FnOnce() + 'static) {
    let on_main = MainThreadMarker::new().is_some();
    debug_assert!(on_main, "run_loop_later called off the main thread");
    if !on_main {
        return;
    }
    let Some(main) = CFRunLoop::main() else {
        return;
    };
    let work = Cell::new(Some(work));
    let block = RcBlock::new(move || {
        if let Some(work) = work.take() {
            work();
        }
    });
    // SAFETY: the block is copied by the run loop and only runs on the main run loop; the mode is a CF constant.
    unsafe {
        let mode = kCFRunLoopDefaultMode.map(|m| m.as_ref());
        main.perform_block(mode, Some(&block));
    }
    main.wake_up();
}

/// Runs `f` on the main thread in a later turn; callable from any thread (a plain `fn` carries no state to share).
pub fn wake_main(f: fn()) {
    dispatch2::DispatchQueue::main().exec_async(f);
}

/// Runs `work` once on the main thread after `seconds`, through the main dispatch queue. A nested run loop (a menu or a
/// modal session) entered from inside a main-queue block does not drain that queue, so `work` waits until the outermost
/// block returns; use a timer on the nested mode for work that must run inside one. Main thread only, like `async_main`.
pub fn after(seconds: f64, work: impl FnOnce() + 'static) {
    let on_main = MainThreadMarker::new().is_some();
    debug_assert!(on_main, "after called off the main thread");
    if !on_main {
        return;
    }
    let delay = Duration::try_from_secs_f64(seconds.max(0.0)).unwrap_or_default();
    let when = DispatchTime::try_from(delay).unwrap_or(DispatchTime::NOW);
    let work = MainOnly(work);
    let _ = dispatch2::DispatchQueue::main().after(when, move || work.run());
}

/// A repeating timer on the main run loop; invalidated when dropped.
pub struct Ticker {
    timer: Retained<NSTimer>,
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.timer.invalidate();
    }
}

/// Calls `tick` every `seconds` on the main thread, also while a modal dialog or a menu runs its own loop (the timer is
/// in the common modes, not only the default one); `None` off the main thread.
pub fn every(seconds: f64, tick: impl Fn() + 'static) -> Option<Ticker> {
    MainThreadMarker::new()?;
    let block = RcBlock::new(move |_: NonNull<NSTimer>| tick());
    // SAFETY: the block is copied by the timer and only runs on the main run loop.
    let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(seconds, true, &block) };
    // SAFETY: a live timer and a run-loop mode constant, on the main thread.
    unsafe { NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
    Some(Ticker { timer })
}
