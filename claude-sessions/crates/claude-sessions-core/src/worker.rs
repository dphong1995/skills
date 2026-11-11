//! The one worker thread of a monitor (Swift's serial `DispatchQueue`): jobs run in submission order, off the UI thread.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Sender, channel};

pub(crate) type Job = Box<dyn FnOnce() + Send>;

/// A serial job queue; without a thread (`inline`) `submit` runs the job on the caller, as Swift's `queue: nil`.
pub(crate) struct Worker(Option<Sender<Job>>);

impl Worker {
    pub(crate) fn spawn(name: &str) -> Self {
        let (tx, rx) = channel::<Job>();
        // A thread that cannot start is fatal: falling back to inline would run blocking HTTP on the UI thread.
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                for job in rx {
                    // A panicking job must not end the loop: later jobs would be dropped and their callbacks lost.
                    let _ = catch_unwind(AssertUnwindSafe(job));
                }
            })
            .expect("cannot start a monitor worker thread");
        Worker(Some(tx))
    }

    pub(crate) fn inline() -> Self {
        Worker(None)
    }

    pub(crate) fn is_inline(&self) -> bool {
        self.0.is_none()
    }

    /// Queues `job`; `dead` runs on the caller instead when the worker thread is gone (the job is dropped).
    pub(crate) fn submit_or_else(&self, job: impl FnOnce() + Send + 'static, dead: impl FnOnce()) {
        match &self.0 {
            Some(tx) => {
                if tx.send(Box::new(job)).is_err() {
                    dead();
                }
            }
            None => job(),
        }
    }
}
