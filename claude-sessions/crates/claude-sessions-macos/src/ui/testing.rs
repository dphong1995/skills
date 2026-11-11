//! Synthetic mouse events dispatched straight to a `SceneView` (never posted to the event queue), so a panel that was
//! never ordered on screen can be clicked in tests.

use claude_sessions_core::scene::Point;
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::NSPoint;

use super::SceneView;
use super::motion;
use super::node_view::NodeView;

fn event(view: &SceneView, kind: NSEventType, at: Point) -> Option<objc2::rc::Retained<NSEvent>> {
    // The view is flipped: scene y runs down, window y runs up.
    let height = view.frame().size.height;
    NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind,
        NSPoint::new(at.x, height - at.y),
        NSEventModifierFlags::empty(),
        0.0,
        0,
        None,
        0,
        1,
        1.0,
    )
}

/// A press and release at `at` (scene points). With `dragged`, a drag event comes between them; the drag distance is
/// read from the real pointer as in the app, so it stays below the drag threshold and the press still clicks.
pub fn click(view: &SceneView, at: Point, dragged: bool) -> bool {
    let (Some(down), Some(up)) = (
        event(view, NSEventType::LeftMouseDown, at),
        event(view, NSEventType::LeftMouseUp, at),
    ) else {
        return false;
    };
    view.mouseDown(&down);
    if let Some(drag) = event(view, NSEventType::LeftMouseDragged, at).filter(|_| dragged) {
        view.mouseDragged(&drag);
    }
    view.mouseUp(&up);
    true
}

/// Moves the first node view of `view` to `frame` through `NodeView::animate_frame` inside an animation group, the path a
/// resize takes. False when `view` has no node view.
pub fn animate_first_node_frame(
    view: &SceneView,
    frame: claude_sessions_core::scene::Rect,
) -> bool {
    let Some(node) = view
        .subviews()
        .iter()
        .find_map(|v| v.downcast::<NodeView>().ok())
    else {
        return false;
    };
    motion::group(
        motion::OPEN_SECS,
        true,
        move || node.animate_frame(frame),
        || {},
    );
    true
}

/// The window server's layer of window `number` (`kCGWindowLayer` of `CGWindowListCopyWindowInfo`), which can differ
/// from `NSWindow.level` when AppKit re-levels the window as it is shown (the real alert sat at 8 while `level()` said 26).
fn server_layer(number: isize) -> Option<isize> {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption};
    use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};

    let list = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionIncludingWindow,
        u32::try_from(number).ok()?,
    )?;
    // SAFETY: a CFArray of CFDictionary is toll-free bridged to NSArray of NSDictionary.
    let list: &NSArray<NSDictionary<NSString, AnyObject>> =
        unsafe { &*std::ptr::from_ref(&*list).cast() };
    let info = list.firstObject()?;
    let layer: Retained<AnyObject> = info.objectForKey(&NSString::from_str("kCGWindowLayer"))?;
    // SAFETY: `kCGWindowLayer` is a CFNumber, toll-free bridged to NSNumber.
    let layer: &NSNumber = unsafe { &*std::ptr::from_ref(&*layer).cast() };
    Some(layer.as_isize())
}

/// What the alert looked like inside its modal session.
#[doc(hidden)]
pub struct AlertInSession {
    pub level: isize,
    /// The window server's layer for the alert window.
    pub server_layer: Option<isize>,
    /// Whether the alert is the key window (the app was activated like the real dialogs do).
    pub key: bool,
    pub alpha: f64,
    /// `app::modal_open()` inside the session.
    pub modal_open: bool,
}

/// Runs a bare, invisible alert (alpha 0, mouse-transparent) the way the real dialogs run theirs, through
/// `main_queue::run_loop_later`: with `activate` through `dialogs::run_alert` (the app is activated), otherwise
/// `dialogs::run_alert_above_panel`. `setup` runs once the alert exists and returns a guard that lives for the session.
/// A timer serviced by the modal run loop calls `read` after `delay` seconds and aborts the session. A watchdog thread
/// ends the process if the session never ends.
fn run_invisible_modal<G: 'static>(
    name: &'static str,
    activate: bool,
    delay: f64,
    setup: impl FnOnce(&objc2_app_kit::NSAlert) -> G + 'static,
    read: impl Fn(&objc2_app_kit::NSWindow) + 'static,
) -> Result<isize, String> {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use block2::RcBlock;
    use objc2_app_kit::{NSAlert, NSApplication, NSModalPanelRunLoopMode};
    use objc2_foundation::{
        MainThreadMarker, NSDate, NSDefaultRunLoopMode, NSRunLoop, NSString, NSTimer,
    };

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let finished = Arc::new(AtomicBool::new(false));
    let guard = finished.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        if !guard.load(Ordering::SeqCst) {
            eprintln!("{name}: the modal session never ended (watchdog)");
            std::process::exit(1);
        }
    });
    let ended = Rc::new(Cell::new(false));
    let over = ended.clone();
    let response = Rc::new(Cell::new(0isize));
    let answer = response.clone();
    crate::main_queue::run_loop_later(move || {
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str("test"));
        let window = alert.window();
        window.setAlphaValue(0.0);
        window.setIgnoresMouseEvents(true);
        let _session_guard = setup(&alert);
        let read = RcBlock::new(move |_: std::ptr::NonNull<NSTimer>| {
            read(&window);
            NSApplication::sharedApplication(mtm).abortModal();
        });
        // SAFETY: the block is copied by the timer and only runs on the main run loop.
        let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(delay, false, &read) };
        // SAFETY: a live timer and a run-loop mode constant, on the main thread.
        unsafe {
            NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSModalPanelRunLoopMode);
        }
        let code = if activate {
            crate::dialogs::run_alert(mtm, &alert)
        } else {
            crate::dialogs::run_alert_above_panel(&alert)
        };
        // The session may have ended by itself (Esc): the abort must not fire outside it.
        timer.invalidate();
        answer.set(code);
        over.set(true);
    });
    while !ended.get() {
        // SAFETY: a plain run-loop mode constant.
        let mode = unsafe { NSDefaultRunLoopMode };
        NSRunLoop::currentRunLoop()
            .runMode_beforeDate(mode, &NSDate::dateWithTimeIntervalSinceNow(0.05));
    }
    finished.store(true, Ordering::SeqCst);
    Ok(response.get())
}

/// Hands focus back to the app that was frontmost before a test activated this one.
fn give_focus_back(front: Option<objc2::rc::Retained<objc2_app_kit::NSRunningApplication>>) {
    if let Some(front) = front {
        front.activateWithOptions(objc2_app_kit::NSApplicationActivationOptions::empty());
    }
}

/// Test hook: starts an invisible, activated alert (see `run_invisible_modal`) and posts a synthetic `Esc` key-down
/// (with the Caps Lock flag when `caps_lock`) 0.4 s into the session; returns the response the session ended with
/// (`Esc` must end it with `NSAlertSecondButtonReturn`; otherwise an abort at 3 s ends it). Focus goes back to the
/// app that was frontmost. Not part of the app's API.
#[doc(hidden)]
pub fn escape_response_during_modal(caps_lock: bool) -> Result<isize, String> {
    use block2::RcBlock;
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventModifierFlags, NSEventType,
        NSModalPanelRunLoopMode, NSWorkspace,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRunLoop, NSString, NSTimer};

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let policy = app.activationPolicy();
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let front = NSWorkspace::sharedWorkspace().frontmostApplication();
    let flags = if caps_lock {
        NSEventModifierFlags::CapsLock
    } else {
        NSEventModifierFlags::empty()
    };
    let result = run_invisible_modal(
        "escape_response_during_modal",
        true,
        3.0,
        move |alert| {
            let window = alert.window();
            window.setFrameOrigin(NSPoint::new(-30000.0, -30000.0));
            let number = window.windowNumber();
            let post = RcBlock::new(move |_: std::ptr::NonNull<NSTimer>| {
                let esc = NSString::from_str("\u{1b}");
                if let Some(event) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                    NSEventType::KeyDown,
                    NSPoint::new(0.0, 0.0),
                    flags,
                    0.0,
                    number,
                    None,
                    &esc,
                    &esc,
                    false,
                    crate::keys::KEY_CODE_ESCAPE,
                ) {
                    NSApplication::sharedApplication(mtm).postEvent_atStart(&event, true);
                }
            });
            // SAFETY: the block is copied by the timer and only runs on the main run loop.
            let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(0.4, false, &post) };
            // SAFETY: a live timer and a run-loop mode constant, on the main thread.
            unsafe {
                NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSModalPanelRunLoopMode);
            }
            timer
        },
        |_| (),
    );
    app.setActivationPolicy(policy);
    give_focus_back(front);
    result
}

/// Test hook: runs an invisible alert (see `run_invisible_modal`) with the app activated (as an accessory app, like the
/// widget) so the alert becomes key: AppKit re-levels the alert to 8 only then, so a run without activation cannot show
/// the bug. Reads the window level, the window server's layer, key status, alpha and `modal_open` inside the session
/// 0.5 s in, after AppKit has shown the window; the activation policy is restored. Not part of the app's API.
#[doc(hidden)]
pub fn alert_level_during_modal() -> Result<AlertInSession, String> {
    use std::cell::RefCell;
    use std::rc::Rc;

    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let policy = app.activationPolicy();
    app.setActivationPolicy(objc2_app_kit::NSApplicationActivationPolicy::Accessory);
    let front = objc2_app_kit::NSWorkspace::sharedWorkspace().frontmostApplication();
    let seen = Rc::new(RefCell::new(None));
    let slot = seen.clone();
    let result = run_invisible_modal(
        "alert_level_during_modal",
        true,
        0.5,
        |alert| {
            alert
                .window()
                .setFrameOrigin(objc2_foundation::NSPoint::new(-30000.0, -30000.0));
        },
        move |window| {
            *slot.borrow_mut() = Some(AlertInSession {
                level: window.level(),
                server_layer: server_layer(window.windowNumber()),
                key: window.isKeyWindow(),
                alpha: window.alphaValue(),
                modal_open: crate::app::modal_open(),
            });
        },
    );
    app.setActivationPolicy(policy);
    give_focus_back(front);
    result?;
    seen.borrow_mut()
        .take()
        .ok_or_else(|| "the modal session ended without the level read".to_string())
}

/// Test hook: starts an invisible alert the way the app starts its dialogs (`main_queue::run_loop_later`), queues a
/// `main_queue::async_main` block while the modal session runs, and reports whether that block ran before the session
/// ended, and a `main_queue::every` ticker (the 1 s refresh) fired during it (a timer serviced by the modal run loop
/// checks, then aborts it). Not part of the app's API.
#[doc(hidden)]
pub fn main_queue_runs_during_modal() -> Result<bool, String> {
    use std::cell::Cell;
    use std::rc::Rc;

    let (ran, ticked, seen) = (
        Rc::new(Cell::new(false)),
        Rc::new(Cell::new(false)),
        Rc::new(Cell::new(false)),
    );
    let (flag, tick_flag, out) = (ran.clone(), ticked.clone(), seen.clone());
    run_invisible_modal(
        "main_queue_runs_during_modal",
        false,
        0.3,
        move |alert| {
            alert
                .window()
                .setFrameOrigin(objc2_foundation::NSPoint::new(-30000.0, -30000.0));
            crate::main_queue::async_main(move || flag.set(true));
            crate::main_queue::every(0.05, move || tick_flag.set(true))
        },
        move |_| out.set(ran.get() && ticked.get()),
    )?;
    Ok(seen.get())
}
