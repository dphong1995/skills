//! Main-queue turns and the 1 s timer.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_foundation::{MainThreadMarker, NSTimer};

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
/// Does nothing off the main thread.
pub fn async_main(work: impl FnOnce() + 'static) {
    if MainThreadMarker::new().is_none() {
        return;
    }
    let work = MainOnly(work);
    dispatch2::DispatchQueue::main().exec_async(move || work.run());
}

/// Runs `f` on the main thread in a later turn; callable from any thread (a plain `fn` carries no state to share).
pub fn wake_main(f: fn()) {
    dispatch2::DispatchQueue::main().exec_async(f);
}

/// Runs `work` once on the main thread after `seconds`.
pub fn after(seconds: f64, work: impl FnOnce() + 'static) {
    let work = std::cell::Cell::new(Some(work));
    let block = RcBlock::new(move |_: NonNull<NSTimer>| {
        if let Some(work) = work.take() {
            work();
        }
    });
    // SAFETY: the block is copied by the timer and only runs on the main run loop.
    unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(seconds, false, &block) };
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

/// Calls `tick` every `seconds` on the main thread; `None` off the main thread.
pub fn every(seconds: f64, tick: impl Fn() + 'static) -> Option<Ticker> {
    MainThreadMarker::new()?;
    let block = RcBlock::new(move |_: NonNull<NSTimer>| tick());
    // SAFETY: the block is copied by the timer and only runs on the main run loop.
    let timer =
        unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(seconds, true, &block) };
    Some(Ticker { timer })
}
