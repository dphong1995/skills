//! The saved hotkey choices and the conflict flag; the Carbon registration itself needs the main thread.

use claude_sessions::hotkey::{Hotkeys, Which};
use claude_sessions_core::hotkeys::{HotKeyChoice, next_choices, open_choices};
use claude_sessions_core::kv::{KeyValueStore, MemoryStore};

#[test]
fn absent_keys_load_the_defaults() {
    let h = Hotkeys::load(&MemoryStore::default());
    assert_eq!(h.open, HotKeyChoice::On(open_choices()[0].clone()));
    assert_eq!(h.next, HotKeyChoice::On(next_choices()[0].clone()));
    assert!(h.failed.is_empty());
}

#[test]
fn saved_choices_and_off_load_as_saved() {
    let kv = MemoryStore::default();
    kv.set_bytes("hotkeyOpen", &open_choices()[2].to_defaults());
    kv.set_string("hotkeyNext", "off");
    let h = Hotkeys::load(&kv);
    assert_eq!(h.open, HotKeyChoice::On(open_choices()[2].clone()));
    assert_eq!(h.next, HotKeyChoice::Off);
}

#[test]
fn a_refused_registration_sets_the_conflict_flag_and_off_clears_it() {
    // A test thread is not the main thread, so Carbon refuses the registration.
    let kv = MemoryStore::default();
    let mut h = Hotkeys::load(&kv);
    h.register(Which::Open, || {});
    assert!(h.failed.contains("open"));
    assert!(!h.failed.contains("next"));
    h.set(&kv, Which::Open, HotKeyChoice::Off, || {});
    assert!(h.failed.is_empty());
    // The choice is saved in the Swift form.
    assert_eq!(kv.get_string("hotkeyOpen").as_deref(), Some("off"));
}
