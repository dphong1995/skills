//! `key_event` builds the `KeyEvent`s the core `KeyboardTests` use: key code, characters and the command/control/option set.

use claude_sessions::keyboard_mode::{FLAG_COMMAND, FLAG_CONTROL, FLAG_OPTION, key_event};
use claude_sessions_core::keyboard::{KeyEvent, Mods, key_code};

const NONE: Mods = Mods {
    command: false,
    control: false,
    option: false,
};
const COMMAND: Mods = Mods {
    command: true,
    control: false,
    option: false,
};
const OPTION: Mods = Mods {
    command: false,
    control: false,
    option: true,
};

fn ev(chars: &str, code: u16, mods: Mods) -> KeyEvent {
    KeyEvent {
        key_code: code,
        chars: chars.into(),
        mods,
    }
}

#[test]
fn letters_and_digits_keep_their_characters() {
    // `A` (code 0) and `D` (code 2), as typed with or without shift.
    assert_eq!(key_event(0, "a", 0), ev("a", 0, NONE));
    assert_eq!(key_event(0, "A", 1 << 17), ev("A", 0, NONE));
    assert_eq!(key_event(2, "d", 0), ev("d", 2, NONE));
    for (code, digit) in [(18u16, "1"), (19, "2"), (20, "3"), (25, "9"), (29, "0")] {
        assert_eq!(key_event(code, digit, 0), ev(digit, code, NONE));
    }
}

#[test]
fn return_escape_and_arrows_use_the_key_codes() {
    assert_eq!(
        key_event(key_code::RETURN, "\r", 0),
        ev("\r", key_code::RETURN, NONE)
    );
    assert_eq!(
        key_event(key_code::KEYPAD_ENTER, "\u{3}", 0),
        ev("\u{3}", key_code::KEYPAD_ENTER, NONE)
    );
    assert_eq!(
        key_event(key_code::ESCAPE, "\u{1b}", 0),
        ev("\u{1b}", key_code::ESCAPE, NONE)
    );
    assert_eq!(key_event(key_code::UP, "", 0), ev("", key_code::UP, NONE));
    assert_eq!(
        key_event(key_code::DOWN, "", 0),
        ev("", key_code::DOWN, NONE)
    );
}

#[test]
fn modifier_flags_map_to_command_control_option_only() {
    assert_eq!(
        key_event(key_code::RETURN, "", FLAG_COMMAND),
        ev("", key_code::RETURN, COMMAND)
    );
    assert_eq!(
        key_event(key_code::RETURN, "", FLAG_OPTION),
        ev("", key_code::RETURN, OPTION)
    );
    let all = key_event(0, "", FLAG_COMMAND | FLAG_CONTROL | FLAG_OPTION);
    assert_eq!(
        all.mods,
        Mods {
            command: true,
            control: true,
            option: true
        }
    );
    // Shift (1<<17), caps lock (1<<16), fn (1<<23) and device-dependent bits do not count.
    let noise = (1 << 16) | (1 << 17) | (1 << 23) | 0x108;
    assert_eq!(key_event(0, "a", noise), ev("a", 0, NONE));
}

#[test]
fn modifier_masks_tell_command_alone_from_no_modifier() {
    use claude_sessions::keyboard_mode::{FLAG_COMMAND, FLAG_CONTROL, FLAG_OPTION, FLAG_SHIFT};
    use claude_sessions::keyboard_mode::{command_only, no_modifiers};
    // Caps lock, fn and the device-dependent low bits do not count as modifiers.
    let noise = (1 << 16) | (1 << 23) | 0x102;
    assert!(command_only(FLAG_COMMAND | noise));
    assert!(!command_only(FLAG_COMMAND | FLAG_SHIFT));
    assert!(!command_only(FLAG_COMMAND | FLAG_OPTION));
    assert!(!command_only(FLAG_COMMAND | FLAG_CONTROL));
    assert!(!command_only(noise));
    assert!(no_modifiers(noise));
    assert!(!no_modifiers(FLAG_SHIFT));
    assert!(!no_modifiers(FLAG_COMMAND));
}

#[test]
fn give_back_after_settings_cases() {
    use claude_sessions::keyboard_mode::give_back_after_settings;
    assert!(give_back_after_settings(false, true));
    assert!(!give_back_after_settings(false, false));
    assert!(!give_back_after_settings(true, true));
    assert!(!give_back_after_settings(true, false));
}
