//! The right-click menu as a plain model (Swift `.contextMenu` in `WidgetView`): `build` is the two-line menu, `hotkey_menu`
//! the choices of one hotkey (popped up from the Settings window). `claude-sessions-macos::menu::popup` only shows the entries.

use claude_sessions_core::hotkeys::{HotKeyChoice, next_choices, open_choices};
use claude_sessions_macos::menu::Entry;

use crate::hotkey::{Hotkeys, Which};

/// What a menu line does; the id of an `Entry::Item` is its index in `Menu::commands`.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Settings,
    Hotkey(Which, HotKeyChoice),
    Quit,
}

#[derive(Debug, PartialEq)]
pub struct Menu {
    pub entries: Vec<Entry>,
    pub commands: Vec<Command>,
}

const CONFLICT: &str = " (đang bị app khác dùng)";

/// `Cài đặt…`, a separator, `Thoát`.
pub fn build() -> Menu {
    Menu {
        entries: vec![
            Entry::Item {
                title: "Cài đặt…".into(),
                id: 0,
            },
            Entry::Separator,
            Entry::Item {
                title: "Thoát".into(),
                id: 1,
            },
        ],
        commands: vec![Command::Settings, Command::Quit],
    }
}

/// The combos of `which` and `Tắt`; the current one carries `✓ ` and, when its registration failed, the conflict note.
pub fn hotkey_menu(which: Which, hotkeys: &Hotkeys) -> Menu {
    let current = hotkeys.choice(which);
    let failed = hotkeys.failed.contains(which.name());
    let choices = match which {
        Which::Open => open_choices(),
        Which::Next => next_choices(),
    };
    let mut menu = Menu {
        entries: Vec::new(),
        commands: Vec::new(),
    };
    let mut add = |title: String, choice: HotKeyChoice| {
        menu.entries.push(Entry::Item {
            title,
            id: menu.commands.len() as isize,
        });
        menu.commands.push(Command::Hotkey(which, choice));
    };
    for combo in choices {
        let choice = HotKeyChoice::On(combo.clone());
        let on = *current == choice;
        let title = format!(
            "{}{}{}",
            if on { "✓ " } else { "" },
            combo.label,
            if on && failed { CONFLICT } else { "" }
        );
        add(title, choice);
    }
    let off = *current == HotKeyChoice::Off;
    add(
        format!("{}Tắt", if off { "✓ " } else { "" }),
        HotKeyChoice::Off,
    );
    menu
}
