//! Pointer tracking while the panel ignores the mouse (`main.swift:198-199`).

use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use claude_sessions_core::scene::Point;
use objc2_app_kit::{NSEvent, NSEventMask};
use objc2_foundation::MainThreadMarker;

use crate::observer::EventMonitor;

/// Keeps the global and local monitors alive.
pub struct MouseMonitors {
    _global: Option<EventMonitor>,
    _local: Option<EventMonitor>,
}

/// Calls `moved` on every `mouseMoved` / `leftMouseDragged`, in this app and in others. No timer: the callback is the
/// only wakeup. `None` off the main thread.
pub fn observe_moves(moved: impl Fn() + 'static) -> Option<MouseMonitors> {
    MainThreadMarker::new()?;
    let moved = Rc::new(moved);
    let mask = NSEventMask::MouseMoved | NSEventMask::LeftMouseDragged;
    let global = {
        let moved = moved.clone();
        let block = RcBlock::new(move |_: NonNull<NSEvent>| moved());
        EventMonitor::new(NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            mask, &block,
        ))
    };
    let local = {
        let block = RcBlock::new(move |event: NonNull<NSEvent>| {
            moved();
            event.as_ptr()
        });
        // SAFETY: the block returns the event it was given, which is a valid pointer.
        EventMonitor::new(unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block)
        })
    };
    Some(MouseMonitors {
        _global: global,
        _local: local,
    })
}

/// The pointer in screen points, AppKit's bottom-left origin.
pub fn pointer() -> Point {
    let at = NSEvent::mouseLocation();
    Point { x: at.x, y: at.y }
}
