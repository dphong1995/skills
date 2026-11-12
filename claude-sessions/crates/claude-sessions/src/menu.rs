//! The right-click menu as a plain model (Swift `.contextMenu` in `WidgetView`): `build` is the two-line menu, `hotkey_menu`
//! the choices of one hotkey (popped up from the Settings window). `claude-sessions-macos::menu::popup` only shows the entries.

use claude_sessions_core::hotkeys::{HotKeyChoice, next_choices, open_choices};
use claude_sessions_core::pull_requests::PRRef;
use claude_sessions_macos::menu::Entry;

use crate::hotkey::{Hotkeys, Which};

/// What a menu line does; the id of an `Entry::Item` is its index in `Menu::commands`.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    HidePr { session: String, pr: PRRef },
    ShowHiddenPrs(String),
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

/// What the right-click hit offers: hiding the PR under the pointer and restoring the session's hidden ones.
#[derive(Default)]
pub struct MenuContext {
    pub hide: Option<(String, PRRef)>,
    pub show_hidden: Option<(String, usize)>,
}

/// `Ẩn PR #<id> ở session này` and `Hiện lại <N> PR đã ẩn` (each when `ctx` has it) and a separator, then `Cài đặt…`, a
/// separator, `Thoát`.
pub fn build(ctx: &MenuContext) -> Menu {
    let mut menu = Menu {
        entries: Vec::new(),
        commands: Vec::new(),
    };
    let add = |menu: &mut Menu, title: String, command: Command| {
        menu.entries.push(Entry::Item {
            title,
            id: menu.commands.len() as isize,
        });
        menu.commands.push(command);
    };
    let mut extra = false;
    if let Some((session, pr)) = &ctx.hide {
        let title = format!("Ẩn PR #{} ở session này", pr.id);
        let command = Command::HidePr {
            session: session.clone(),
            pr: pr.clone(),
        };
        add(&mut menu, title, command);
        extra = true;
    }
    if let Some((session, n)) = ctx.show_hidden.as_ref().filter(|(_, n)| *n > 0) {
        add(
            &mut menu,
            format!("Hiện lại {n} PR đã ẩn"),
            Command::ShowHiddenPrs(session.clone()),
        );
        extra = true;
    }
    if extra {
        menu.entries.push(Entry::Separator);
    }
    add(&mut menu, "Cài đặt…".into(), Command::Settings);
    menu.entries.push(Entry::Separator);
    add(&mut menu, "Thoát".into(), Command::Quit);
    menu
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
