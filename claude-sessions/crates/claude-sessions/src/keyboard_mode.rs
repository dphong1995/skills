//! Keyboard mode input: `NSEvent` key fields to the core `KeyEvent` (Swift `WidgetView.handleKey`). No AppKit types.

use claude_sessions_core::keyboard::{KeyEvent, Mods};

/// `NSEvent.ModifierFlags` bits the key table reads; shift, caps lock, fn and the device-dependent low bits are dropped.
pub const FLAG_CONTROL: u64 = 1 << 18;
pub const FLAG_OPTION: u64 = 1 << 19;
pub const FLAG_COMMAND: u64 = 1 << 20;

/// `keyCode`, `charactersIgnoringModifiers` (empty when absent) and `modifierFlags.rawValue` of a key-down event.
pub fn key_event(key_code: u16, chars: &str, modifier_flags: u64) -> KeyEvent {
    KeyEvent {
        key_code,
        chars: chars.to_string(),
        mods: Mods {
            command: modifier_flags & FLAG_COMMAND != 0,
            control: modifier_flags & FLAG_CONTROL != 0,
            option: modifier_flags & FLAG_OPTION != 0,
        },
    }
}
