//! The right-click menu as a plain model (Swift `.contextMenu` in `WidgetView`): `build` lists the items from the store and the
//! hotkey state, `choose` applies the picked one. `claude-sessions-macos::menu::popup` only shows the entries.

use claude_sessions_core::credentials::Service;
use claude_sessions_core::hotkeys::{HotKeyChoice, next_choices, open_choices};
use claude_sessions_core::kv::KeyValueStore;
use claude_sessions_core::store::Store;
use claude_sessions_macos::menu::Entry;

use crate::hotkey::{Hotkeys, Which};
use crate::settings::{self, BG_OPACITIES, Settings};

/// What a menu line does; the id of an `Entry::Item` is its index in `Menu::commands`.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    BgOpacity(f64),
    ToggleMute,
    Hotkey(Which, HotKeyChoice),
    EditPrompt,
    Connect(Service),
    Quit,
}

#[derive(Debug, PartialEq)]
pub struct Menu {
    pub entries: Vec<Entry>,
    pub commands: Vec<Command>,
}

/// What the app still has to do after `choose`.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Applied and saved.
    Done,
    /// Register `choice` for `which` (saves it).
    Hotkey(Which, HotKeyChoice),
    EditPrompt,
    Connect(Service),
    Quit,
}

const CONFLICT: &str = " (đang bị app khác dùng)";

struct Builder {
    commands: Vec<Command>,
}

impl Builder {
    fn item(&mut self, title: String, command: Command) -> Entry {
        self.commands.push(command);
        Entry::Item {
            title,
            id: (self.commands.len() - 1) as isize,
        }
    }

    fn hotkey_items(&mut self, which: Which, current: &HotKeyChoice, failed: bool) -> Vec<Entry> {
        let choices = match which {
            Which::Open => open_choices(),
            Which::Next => next_choices(),
        };
        let mut items = Vec::new();
        for combo in choices {
            let choice = HotKeyChoice::On(combo.clone());
            let on = *current == choice;
            let title = format!(
                "{}{}{}",
                if on { "✓ " } else { "" },
                combo.label,
                if on && failed { CONFLICT } else { "" }
            );
            items.push(self.item(title, Command::Hotkey(which, choice)));
        }
        let off = *current == HotKeyChoice::Off;
        let title = format!("{}Tắt", if off { "✓ " } else { "" });
        items.push(self.item(title, Command::Hotkey(which, HotKeyChoice::Off)));
        items
    }
}

/// The menu for the current settings, hotkey choices (with the conflict flags) and connection lines.
pub fn build(store: &Store, hotkeys: &Hotkeys) -> Menu {
    let mut b = Builder {
        commands: Vec::new(),
    };
    let mut opacity = Vec::new();
    for v in BG_OPACITIES {
        let on = store.bg_opacity == v;
        let title = format!(
            "{}{}%",
            if on { "✓ " } else { "" },
            (v * 100.0).round() as i64
        );
        opacity.push(b.item(title, Command::BgOpacity(v)));
    }
    let mut entries = vec![Entry::Submenu {
        title: "Độ đậm nền".into(),
        items: opacity,
    }];
    let mute = if store.muted {
        "Bật tiếng"
    } else {
        "Tắt tiếng"
    };
    entries.push(b.item(mute.into(), Command::ToggleMute));
    let open = b.hotkey_items(
        Which::Open,
        &hotkeys.open,
        hotkeys.failed.contains(Which::Open.name()),
    );
    let next = b.hotkey_items(
        Which::Next,
        &hotkeys.next,
        hotkeys.failed.contains(Which::Next.name()),
    );
    entries.push(Entry::Submenu {
        title: "Phím tắt".into(),
        items: vec![
            Entry::Submenu {
                title: "Mở widget".into(),
                items: open,
            },
            Entry::Submenu {
                title: "Nhảy session chờ".into(),
                items: next,
            },
        ],
    });
    entries.push(b.item("Prompt Nhờ Claude…".into(), Command::EditPrompt));
    entries.push(Entry::Separator);
    for service in Service::ALL {
        entries.push(b.item(store.connection_text(service), Command::Connect(service)));
    }
    entries.push(Entry::Separator);
    entries.push(b.item("Thoát".into(), Command::Quit));
    Menu {
        entries,
        commands: b.commands,
    }
}

/// Applies the settings commands to `store` and saves them; the rest is returned for the app.
pub fn choose(
    command: &Command,
    store: &mut Store,
    kv: &dyn KeyValueStore,
    saved: &mut Settings,
    pr_prompt: &str,
) -> Outcome {
    match command {
        Command::BgOpacity(v) => store.bg_opacity = *v,
        Command::ToggleMute => store.muted = !store.muted,
        Command::Hotkey(which, choice) => return Outcome::Hotkey(*which, choice.clone()),
        Command::EditPrompt => return Outcome::EditPrompt,
        Command::Connect(service) => return Outcome::Connect(*service),
        Command::Quit => return Outcome::Quit,
    }
    settings::persist(kv, saved, settings::current(store, pr_prompt.to_string()));
    Outcome::Done
}
