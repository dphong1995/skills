//! The right-click menu model and what choosing a line does: no popup, no AppKit.

use claude_sessions::hits::Target;
use claude_sessions::hotkey::{Hotkeys, Which};
use claude_sessions::menu::{self, Command, MenuContext};
use claude_sessions_core::hotkeys::HotKeyChoice;
use claude_sessions_core::kv::MemoryStore;
use claude_sessions_core::pull_requests::PRRef;
use claude_sessions_macos::menu::Entry;
use url::Url;

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

fn pr(id: i64) -> PRRef {
    PRRef {
        workspace: "t6engineering".into(),
        repo: "talo6-dct-backend".into(),
        id,
    }
}

#[test]
fn menu_without_context_is_unchanged() {
    let m = menu::build(&MenuContext::default());
    assert_eq!(titles(&m.entries), ["Cài đặt…", "-", "Thoát"]);
    assert_eq!(m.commands, [Command::Settings, Command::Quit]);
}

#[test]
fn menu_with_hide_and_show_hidden() {
    let m = menu::build(&MenuContext {
        session: Some("s".into()),
        hide: Some(pr(7571)),
        hidden: 2,
    });
    assert_eq!(
        titles(&m.entries),
        [
            "Ẩn PR #7571 ở session này",
            "Hiện lại 2 PR đã ẩn",
            "-",
            "Cài đặt…",
            "-",
            "Thoát"
        ]
    );
    assert_eq!(
        m.commands,
        [
            Command::HidePr {
                session: "s".into(),
                pr: pr(7571)
            },
            Command::ShowHiddenPrs("s".into()),
            Command::Settings,
            Command::Quit
        ]
    );
    let ids: Vec<isize> = m
        .entries
        .iter()
        .filter_map(|e| match e {
            Entry::Item { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(ids, [0, 1, 2, 3]);
}

#[test]
fn menu_with_only_show_hidden() {
    let m = menu::build(&MenuContext {
        session: Some("s".into()),
        hide: None,
        hidden: 1,
    });
    assert_eq!(
        titles(&m.entries),
        ["Hiện lại 1 PR đã ẩn", "-", "Cài đặt…", "-", "Thoát"]
    );
    let zero = menu::build(&MenuContext {
        session: Some("s".into()),
        hide: None,
        hidden: 0,
    });
    assert_eq!(titles(&zero.entries), ["Cài đặt…", "-", "Thoát"]);
}

#[test]
fn target_pr_and_session() {
    let link = |u: &str| Target::Link {
        session: "s".into(),
        url: Url::parse(u).unwrap(),
    };
    let t = link("https://bitbucket.org/t6engineering/talo6-dct-backend/pull-requests/7571");
    assert_eq!(t.pr(), Some(pr(7571)));
    assert_eq!(t.session(), Some("s"));
    assert_eq!(link("https://example.com/x").pr(), None);
    let nudge = Target::Nudge {
        session: "s".into(),
        pr: pr(3),
    };
    assert_eq!(nudge.pr(), Some(pr(3)));
    let ask = Target::AskClaude {
        session: "s".into(),
        pr: pr(4),
    };
    assert_eq!(ask.pr(), Some(pr(4)));
    let jump = Target::Jump("s".into());
    assert_eq!(jump.pr(), None);
    assert_eq!(jump.session(), Some("s"));
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

fn link_to(id: i64) -> Target {
    Target::Link {
        session: "a".into(),
        url: Url::parse(&format!(
            "https://bitbucket.org/t6engineering/talo6-dct-backend/pull-requests/{id}"
        ))
        .unwrap(),
    }
}

#[test]
fn context_hides_a_tracked_pr_only() {
    let (store, _dir) = crate::support::pr_store(&[1, 2]);
    let ctx = MenuContext::new(Some(&link_to(2)), &store);
    assert_eq!(ctx.session.as_deref(), Some("a"));
    assert_eq!(ctx.hide, Some(pr(2)));
    assert_eq!(ctx.hidden, 0);
    let untracked = MenuContext::new(Some(&link_to(999)), &store);
    assert_eq!(untracked.session.as_deref(), Some("a"));
    assert_eq!(untracked.hide, None);
}

#[test]
fn context_counts_hidden_prs_on_a_jump() {
    let (mut store, _dir) = crate::support::pr_store(&[1, 2]);
    store.hide_pr("a", &pr(1));
    store.hide_pr("a", &pr(2));
    let ctx = MenuContext::new(Some(&Target::Jump("a".into())), &store);
    assert_eq!(ctx.hidden, 2);
    assert_eq!(ctx.hide, None);
    assert_eq!(ctx.session.as_deref(), Some("a"));
}

#[test]
fn context_without_target_is_empty() {
    let (store, _dir) = crate::support::pr_store(&[1]);
    let ctx = MenuContext::new(None, &store);
    assert_eq!(ctx.session, None);
    assert_eq!(ctx.hide, None);
    assert_eq!(ctx.hidden, 0);
}
