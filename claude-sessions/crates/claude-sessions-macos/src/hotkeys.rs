//! System-wide hotkeys through Carbon `RegisterEventHotKey`, which needs no Accessibility permission (`HotKeys.swift`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;

use objc2_foundation::MainThreadMarker;

pub const CONTROL: u32 = 0x1000;
pub const OPTION: u32 = 0x0800;
pub const COMMAND: u32 = 0x0100;
pub const KEY_SPACE: u32 = 49;

const SIGNATURE: u32 = 0x4353_4B59; // "CSKY"
const EVENT_CLASS_KEYBOARD: u32 = 0x6B65_7962; // 'keyb'
const EVENT_HOT_KEY_PRESSED: u32 = 5;
const EVENT_PARAM_DIRECT_OBJECT: u32 = 0x2D2D_2D2D; // '----'
const TYPE_EVENT_HOT_KEY_ID: u32 = 0x686B_6964; // 'hkid'
const EVENT_NOT_HANDLED: i32 = -9874;

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct EventHotKeyId {
    signature: u32,
    id: u32,
}

type Handler = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: Handler,
        count: u32,
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        out_ref: *mut *mut c_void,
    ) -> i32;
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyId,
        target: *mut c_void,
        options: u32,
        out_ref: *mut *mut c_void,
    ) -> i32;
    fn UnregisterEventHotKey(hot_key: *mut c_void) -> i32;
    fn GetEventParameter(
        event: *mut c_void,
        name: u32,
        wanted_type: u32,
        actual_type: *mut u32,
        buffer_size: usize,
        actual_size: *mut usize,
        data: *mut c_void,
    ) -> i32;
}

struct Registered {
    hot_key: *mut c_void,
    handler: Rc<dyn Fn()>,
}

thread_local! {
    static REGISTERED: RefCell<HashMap<u32, Registered>> = RefCell::new(HashMap::new());
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "C" fn on_hot_key(_next: *mut c_void, event: *mut c_void, _user: *mut c_void) -> i32 {
    let mut id = EventHotKeyId::default();
    // SAFETY: `id` is a valid buffer of the size passed; `event` is the live event Carbon handed us.
    let status = unsafe {
        GetEventParameter(
            event,
            EVENT_PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOT_KEY_ID,
            ptr::null_mut(),
            size_of::<EventHotKeyId>(),
            ptr::null_mut(),
            (&mut id as *mut EventHotKeyId).cast(),
        )
    };
    if status != 0 {
        return EVENT_NOT_HANDLED;
    }
    // Cloned out so the handler may register or unregister without a borrow held.
    let handler = REGISTERED.with(|r| r.borrow().get(&id.id).map(|r| r.handler.clone()));
    match handler {
        Some(handler) => {
            handler();
            0
        }
        None => EVENT_NOT_HANDLED,
    }
}

fn install_handler_once() -> bool {
    if INSTALLED.get() {
        return true;
    }
    let spec = EventTypeSpec {
        event_class: EVENT_CLASS_KEYBOARD,
        event_kind: EVENT_HOT_KEY_PRESSED,
    };
    // SAFETY: the target is the application target, `spec` outlives the call, the handler is a plain function.
    let status = unsafe {
        InstallEventHandler(
            GetApplicationEventTarget(),
            on_hot_key,
            1,
            &spec,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    INSTALLED.set(status == 0);
    status == 0
}

/// Registers `key_code` + `modifiers` under `id`, replacing the previous one. False when the system refuses (another
/// app owns the combo) or off the main thread. `handler` runs on the main thread inside Carbon's event dispatch: do
/// real work on a later turn.
pub fn register(id: u32, key_code: u32, modifiers: u32, handler: impl Fn() + 'static) -> bool {
    if MainThreadMarker::new().is_none() || !install_handler_once() {
        return false;
    }
    unregister(id);
    let mut hot_key: *mut c_void = ptr::null_mut();
    // SAFETY: valid out pointer; the target is the application target.
    let status = unsafe {
        RegisterEventHotKey(
            key_code,
            modifiers,
            EventHotKeyId {
                signature: SIGNATURE,
                id,
            },
            GetApplicationEventTarget(),
            0,
            &mut hot_key,
        )
    };
    if status != 0 || hot_key.is_null() {
        return false;
    }
    REGISTERED.with(|r| {
        r.borrow_mut().insert(
            id,
            Registered {
                hot_key,
                handler: Rc::new(handler),
            },
        )
    });
    true
}

pub fn unregister(id: u32) {
    if let Some(old) = REGISTERED.with(|r| r.borrow_mut().remove(&id)) {
        // SAFETY: `hot_key` came from `RegisterEventHotKey` and is unregistered once.
        unsafe { UnregisterEventHotKey(old.hot_key) };
    }
}
