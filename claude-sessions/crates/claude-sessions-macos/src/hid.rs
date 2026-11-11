//! Real HID events (`kCGHIDEventTap`) for `cargo xtask hid`: the cursor really moves and keys really type, unlike AX
//! actions. Needs Accessibility for the posting process. Coordinates are points from the top-left of the main display.

use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CgPoint {
    x: f64,
    y: f64,
}

type CgEventRef = *mut c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateMouseEvent(
        source: *const c_void,
        kind: u32,
        at: CgPoint,
        button: u32,
    ) -> CgEventRef;
    fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, down: bool) -> CgEventRef;
    fn CGEventSetFlags(event: CgEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CgEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

const HID_TAP: u32 = 0;
const MOVED: u32 = 5;
const LEFT_DOWN: u32 = 1;
const LEFT_UP: u32 = 2;
const RIGHT_DOWN: u32 = 3;
const RIGHT_UP: u32 = 4;
const LEFT_DRAGGED: u32 = 6;

const FLAG_SHIFT: u64 = 0x20000;
const FLAG_CONTROL: u64 = 0x40000;
const FLAG_ALT: u64 = 0x80000;
const FLAG_COMMAND: u64 = 0x100000;

/// Whether this process may post events (Accessibility granted to the terminal that runs it).
pub fn trusted() -> bool {
    // SAFETY: no arguments.
    unsafe { AXIsProcessTrusted() }
}

fn mouse(kind: u32, x: f64, y: f64, button: u32) {
    // SAFETY: the event is created, posted and released here; a null source is allowed.
    unsafe {
        let event = CGEventCreateMouseEvent(std::ptr::null(), kind, CgPoint { x, y }, button);
        if event.is_null() {
            return;
        }
        CGEventPost(HID_TAP, event);
        CFRelease(event);
    }
}

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(60));
}

pub fn move_to(x: f64, y: f64) {
    mouse(MOVED, x, y, 0);
}

/// Moves in small steps so tracking areas and hover monitors see a real pointer path.
pub fn glide_to(x: f64, y: f64) {
    mouse(MOVED, x - 6.0, y - 6.0, 0);
    pause();
    mouse(MOVED, x, y, 0);
    pause();
}

pub fn click(x: f64, y: f64) {
    glide_to(x, y);
    mouse(LEFT_DOWN, x, y, 0);
    pause();
    mouse(LEFT_UP, x, y, 0);
    pause();
}

pub fn right_click(x: f64, y: f64) {
    glide_to(x, y);
    mouse(RIGHT_DOWN, x, y, 1);
    pause();
    mouse(RIGHT_UP, x, y, 1);
    pause();
}

pub fn drag(x1: f64, y1: f64, x2: f64, y2: f64) {
    glide_to(x1, y1);
    mouse(LEFT_DOWN, x1, y1, 0);
    pause();
    for i in 1..=20 {
        let t = f64::from(i) / 20.0;
        mouse(LEFT_DRAGGED, x1 + (x2 - x1) * t, y1 + (y2 - y1) * t, 0);
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    pause();
    mouse(LEFT_UP, x2, y2, 0);
    pause();
}

fn key_code(name: &str) -> Option<u16> {
    Some(match name {
        "a" => 0,
        "s" => 1,
        "d" => 2,
        "f" => 3,
        "h" => 4,
        "g" => 5,
        "z" => 6,
        "x" => 7,
        "c" => 8,
        "v" => 9,
        "b" => 11,
        "q" => 12,
        "w" => 13,
        "e" => 14,
        "r" => 15,
        "y" => 16,
        "t" => 17,
        "o" => 31,
        "u" => 32,
        "i" => 34,
        "p" => 35,
        "l" => 37,
        "j" => 38,
        "k" => 40,
        "n" => 45,
        "m" => 46,
        "return" | "enter" => 36,
        "tab" => 48,
        "space" => 49,
        "delete" => 51,
        "escape" | "esc" => 53,
        "grave" => 50,
        "left" => 123,
        "right" => 124,
        "down" => 125,
        "up" => 126,
        _ => return None,
    })
}

/// Presses a combo such as `a`, `escape`, `cmd+v` or `ctrl+alt+space`.
pub fn keys(combo: &str) -> Result<(), String> {
    let mut flags = 0u64;
    let mut key = None;
    for part in combo.split('+') {
        match part {
            "ctrl" | "control" => flags |= FLAG_CONTROL,
            "alt" | "option" => flags |= FLAG_ALT,
            "shift" => flags |= FLAG_SHIFT,
            "cmd" | "command" => flags |= FLAG_COMMAND,
            other => key = Some(key_code(other).ok_or_else(|| format!("unknown key {other:?}"))?),
        }
    }
    let key = key.ok_or("no key in combo")?;
    for down in [true, false] {
        // SAFETY: the event is created, posted and released here; a null source is allowed.
        unsafe {
            let event = CGEventCreateKeyboardEvent(std::ptr::null(), key, down);
            if event.is_null() {
                return Err("could not create the key event".into());
            }
            CGEventSetFlags(event, flags);
            CGEventPost(HID_TAP, event);
            CFRelease(event);
        }
        pause();
    }
    Ok(())
}
