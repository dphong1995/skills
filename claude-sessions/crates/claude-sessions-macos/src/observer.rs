//! Registrations that undo themselves when dropped.

use block2::RcBlock;
use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::NSEvent;
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNotificationName, NSObjectProtocol,
};
use std::ptr::NonNull;

/// An `NSEvent` local or global monitor.
pub(crate) struct EventMonitor(Retained<AnyObject>);

impl EventMonitor {
    pub(crate) fn new(monitor: Option<Retained<AnyObject>>) -> Option<Self> {
        monitor.map(Self)
    }
}

impl Drop for EventMonitor {
    fn drop(&mut self) {
        // SAFETY: the object came from `addGlobal/LocalMonitor...` and is removed once.
        unsafe { NSEvent::removeMonitor(&self.0) };
    }
}

/// A notification-center observer; main thread only.
pub struct NotificationObserver {
    center: Retained<NSNotificationCenter>,
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl NotificationObserver {
    /// Calls `callback` on the posting thread (the main thread for AppKit notifications) while the observer lives.
    pub(crate) fn new(
        name: &NSNotificationName,
        object: Option<&AnyObject>,
        callback: impl Fn() + 'static,
    ) -> Self {
        Self::on(
            &NSNotificationCenter::defaultCenter(),
            name,
            object,
            callback,
        )
    }

    /// Like `new`, on `center` (the workspace has its own).
    pub(crate) fn on(
        center: &NSNotificationCenter,
        name: &NSNotificationName,
        object: Option<&AnyObject>,
        callback: impl Fn() + 'static,
    ) -> Self {
        let block = RcBlock::new(move |_: NonNull<NSNotification>| callback());
        // SAFETY: the block is copied by the center; the callback is only run on the posting (main) thread.
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(Some(name), object, None, &block)
        };
        Self {
            center: center.retain(),
            token,
        }
    }
}

impl Drop for NotificationObserver {
    fn drop(&mut self) {
        // SAFETY: `token` came from `addObserverForName...` on the default center.
        unsafe { self.center.removeObserver(self.token.as_ref()) };
    }
}
