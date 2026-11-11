//! The one worker thread of a monitor (Swift's serial `DispatchQueue`): jobs run in submission order, off the UI thread.

use std::sync::mpsc::{Sender, channel};

pub(crate) type Job = Box<dyn FnOnce() + Send>;

/// A serial job queue; without a thread (`inline`) `submit` runs the job on the caller, as Swift's `queue: nil`.
pub(crate) struct Worker(Option<Sender<Job>>);

impl Worker {
    pub(crate) fn spawn(name: &str) -> Self {
        let (tx, rx) = channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                for job in rx {
                    job();
                }
            });
        Worker(spawned.ok().map(|_| tx))
    }

    pub(crate) fn inline() -> Self {
        Worker(None)
    }

    pub(crate) fn is_inline(&self) -> bool {
        self.0.is_none()
    }

    pub(crate) fn submit(&self, job: impl FnOnce() + Send + 'static) {
        match &self.0 {
            Some(tx) => {
                let _ = tx.send(Box::new(job));
            }
            None => job(),
        }
    }
}
