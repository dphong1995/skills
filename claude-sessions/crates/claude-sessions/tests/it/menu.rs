//! The right-click menu model and what choosing a line does: no popup, no AppKit.

use claude_sessions::hotkey::{Hotkeys, Which};
use claude_sessions::menu::{self, Command};
use claude_sessions_core::hotkeys::HotKeyChoice;
use claude_sessions_core::kv::MemoryStore;
use claude_sessions_macos::menu::Entry;

fn titles(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .map(|e| match e {
            Entry::Item { title, .. } => title.clone(),
            Entry::Submenu { title, .. } => format!("{title} >"),
            Entry::Separator => "-".into(),
        })
        .collect()
}

#[test]
fn menu_has_only_settings_and_quit() {
    let m = menu::build();
    assert_eq!(titles(&m.entries), ["Cài đặt…", "-", "Thoát"]);
    assert_eq!(m.commands, [Command::Settings, Command::Quit]);
}

#[test]
fn hotkey_menu_marks_the_choice_and_the_conflict() {
    let mut h = Hotkeys::load(&MemoryStore::default());
    let plain = titles(&menu::hotkey_menu(Which::Open, &h).entries);
    assert_eq!(plain.last().unwrap(), "Tắt");
    assert!(plain.len() >= 3, "alternatives and Tắt: {plain:?}");
    assert_eq!(plain.iter().filter(|t| t.starts_with("✓ ")).count(), 1);
    assert!(plain.iter().all(|t| !t.contains("(đang bị app khác dùng)")));
    h.failed.insert("open");
    let m = menu::hotkey_menu(Which::Open, &h);
    let flagged: Vec<_> = titles(&m.entries)
        .into_iter()
        .filter(|t| t.contains("(đang bị app khác dùng)"))
        .collect();
    assert_eq!(flagged.len(), 1);
    assert!(flagged[0].starts_with("✓ "));
    assert!(matches!(m.commands[0], Command::Hotkey(Which::Open, _)));
    // The other hotkey is not flagged.
    let next = titles(&menu::hotkey_menu(Which::Next, &h).entries);
    assert!(next.iter().all(|t| !t.contains("(đang bị app khác dùng)")));
}

#[test]
fn off_is_checked_when_the_hotkey_is_off() {
    let mut h = Hotkeys::load(&MemoryStore::default());
    h.next = HotKeyChoice::Off;
    let m = menu::hotkey_menu(Which::Next, &h);
    let items = titles(&m.entries);
    assert_eq!(items.last().unwrap(), "✓ Tắt");
    assert_eq!(items.iter().filter(|t| t.starts_with("✓ ")).count(), 1);
    assert_eq!(
        m.commands.last(),
        Some(&Command::Hotkey(Which::Next, HotKeyChoice::Off))
    );
}
