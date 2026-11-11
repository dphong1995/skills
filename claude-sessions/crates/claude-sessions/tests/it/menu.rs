//! The right-click menu model and what choosing a line does: no popup, no AppKit.

use claude_sessions::hotkey::{Hotkeys, Which};
use claude_sessions::menu::{self, Command, Menu, Outcome};
use claude_sessions::settings::{self, BG_OPACITIES};
use claude_sessions_core::credentials::Service;
use claude_sessions_core::hotkeys::HotKeyChoice;
use claude_sessions_core::kv::{KeyValueStore, MemoryStore};
use claude_sessions_core::store::{Paths, Store, StoreOptions};
use claude_sessions_macos::menu::Entry;

fn store(dir: &std::path::Path) -> Store {
    Store::new(
        Paths {
            sessions: dir.join("sessions"),
            projects: dir.join("projects"),
            tasks: dir.join("tasks"),
        },
        StoreOptions::default(),
    )
}

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

fn sub<'a>(entries: &'a [Entry], name: &str) -> &'a [Entry] {
    entries
        .iter()
        .find_map(|e| match e {
            Entry::Submenu { title, items } if title == name => Some(items.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no submenu {name}"))
}

fn menu_of(store: &Store, hotkeys: &Hotkeys) -> Menu {
    menu::build(store, hotkeys)
}

#[test]
fn the_model_lists_the_swift_items() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let h = Hotkeys::load(&MemoryStore::default());
    let m = menu_of(&s, &h);
    assert_eq!(
        titles(&m.entries),
        [
            "Độ đậm nền >",
            "Tắt tiếng",
            "Phím tắt >",
            "Prompt Nhờ Claude…",
            "-",
            "Bitbucket: ✗ chưa có token",
            "Jira: ✗ chưa có token",
            "-",
            "Thoát"
        ]
    );
    let opacity = titles(sub(&m.entries, "Độ đậm nền"));
    assert_eq!(opacity.len(), BG_OPACITIES.len());
    for (title, pct) in opacity.iter().zip(["0%", "25%", "50%", "75%", "92%"]) {
        assert!(title.ends_with(pct), "{title}");
    }
    // The current opacity carries the check mark.
    assert_eq!(opacity.iter().filter(|t| t.starts_with("✓ ")).count(), 1);
    let keys = sub(&m.entries, "Phím tắt");
    assert_eq!(titles(keys), ["Mở widget >", "Nhảy session chờ >"]);
    for name in ["Mở widget", "Nhảy session chờ"] {
        let items = titles(sub(keys, name));
        assert_eq!(items.last().unwrap(), "Tắt");
        assert!(items.len() >= 3, "alternatives and Tắt: {items:?}");
        assert_eq!(items.iter().filter(|t| t.starts_with("✓ ")).count(), 1);
    }
}

#[test]
fn mute_line_follows_the_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    s.muted = true;
    let m = menu_of(&s, &Hotkeys::load(&MemoryStore::default()));
    assert_eq!(titles(&m.entries)[1], "Bật tiếng");
}

#[test]
fn choosing_an_opacity_writes_bg_opacity() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    let kv = MemoryStore::default();
    let mut saved = settings::load(&kv, &mut s);
    let m = menu_of(&s, &Hotkeys::load(&kv));
    let id = sub(&m.entries, "Độ đậm nền")
        .iter()
        .find_map(|e| match e {
            Entry::Item { title, id } if title.ends_with("50%") => Some(*id),
            _ => None,
        })
        .unwrap();
    let out = menu::choose(&m.commands[id as usize], &mut s, &kv, &mut saved, "p");
    assert_eq!(out, Outcome::Done);
    assert_eq!(s.bg_opacity, 0.5);
    assert_eq!(kv.get_f64("bgOpacity"), Some(0.5));
}

#[test]
fn choosing_mute_toggles_and_saves() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    let kv = MemoryStore::default();
    let mut saved = settings::load(&kv, &mut s);
    menu::choose(&Command::ToggleMute, &mut s, &kv, &mut saved, "p");
    assert!(s.muted);
    assert_eq!(kv.get_bool("muted"), Some(true));
}

#[test]
fn other_commands_are_left_to_the_app() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    let kv = MemoryStore::default();
    let mut saved = settings::load(&kv, &mut s);
    let mut run = |c| menu::choose(&c, &mut s, &kv, &mut saved, "p");
    assert_eq!(run(Command::EditPrompt), Outcome::EditPrompt);
    assert_eq!(run(Command::Quit), Outcome::Quit);
    assert_eq!(
        run(Command::Connect(Service::Jira)),
        Outcome::Connect(Service::Jira)
    );
    assert_eq!(
        run(Command::Hotkey(Which::Next, HotKeyChoice::Off)),
        Outcome::Hotkey(Which::Next, HotKeyChoice::Off)
    );
    // Nothing was written for them.
    assert_eq!(kv.get_bool("muted"), None);
}

#[test]
fn the_conflict_flag_marks_only_the_chosen_combo() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let mut h = Hotkeys::load(&MemoryStore::default());
    let flagged = |m: &Menu, name: &str| {
        let keys = sub(&m.entries, "Phím tắt");
        titles(sub(keys, name))
            .into_iter()
            .filter(|t| t.contains("(đang bị app khác dùng)"))
            .collect::<Vec<_>>()
    };
    assert!(flagged(&menu_of(&s, &h), "Mở widget").is_empty());
    h.failed.insert("open");
    let m = menu_of(&s, &h);
    let hit = flagged(&m, "Mở widget");
    assert_eq!(hit.len(), 1);
    assert!(hit[0].starts_with("✓ "));
    assert!(flagged(&m, "Nhảy session chờ").is_empty());
}

#[test]
fn off_is_checked_when_the_hotkey_is_off() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let mut h = Hotkeys::load(&MemoryStore::default());
    h.next = HotKeyChoice::Off;
    let m = menu_of(&s, &h);
    let items = titles(sub(sub(&m.entries, "Phím tắt"), "Nhảy session chờ"));
    assert_eq!(items.last().unwrap(), "✓ Tắt");
    assert_eq!(items.iter().filter(|t| t.starts_with("✓ ")).count(), 1);
}
