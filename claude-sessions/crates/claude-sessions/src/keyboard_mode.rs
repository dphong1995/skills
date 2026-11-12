//! Keyboard mode input: `NSEvent` key fields to the core `KeyEvent` (Swift `WidgetView.handleKey`). No AppKit types.

use claude_sessions_core::keyboard::{KeyEvent, KeyOutcome, Mods, handle_key};
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::store::Snapshot;

/// `NSEvent.ModifierFlags` bits the key table reads; shift, caps lock, fn and the device-dependent low bits are dropped.
pub const FLAG_SHIFT: u64 = 1 << 17;
pub const FLAG_CONTROL: u64 = 1 << 18;
pub const FLAG_OPTION: u64 = 1 << 19;
pub const FLAG_COMMAND: u64 = 1 << 20;
/// Shift, control, option and command together: what makes a key press a shortcut rather than typing.
pub const FLAG_MODIFIERS: u64 = FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND;

/// ⌘ alone is held.
pub fn command_only(modifier_flags: u64) -> bool {
    modifier_flags & FLAG_MODIFIERS == FLAG_COMMAND
}

/// No shift, control, option or command is held.
pub fn no_modifiers(modifier_flags: u64) -> bool {
    modifier_flags & FLAG_MODIFIERS == 0
}

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

/// The local key monitor's answer, synchronously: the key table runs on `snapshot` and may change `panel`.
pub fn handle(
    key_code: u16,
    chars: &str,
    modifier_flags: u64,
    snapshot: &Snapshot,
    panel: &mut PanelState,
) -> KeyOutcome {
    handle_key(&key_event(key_code, chars, modifier_flags), snapshot, panel)
}
