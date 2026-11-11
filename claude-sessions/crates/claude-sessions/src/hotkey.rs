//! The two global hotkeys: the saved choices, their registration and the conflict flags the `Phím tắt` menu shows (phase 9
//! builds that menu; it calls `Hotkeys::set`). Swift `AppDelegate.setUpHotKeys`.

use std::collections::BTreeSet;

use claude_sessions_core::hotkeys::{Cycle, HotKeyChoice, next_choices, open_choices};
use claude_sessions_core::kv::KeyValueStore;
use claude_sessions_macos::hotkeys;

pub const OPEN_ID: u32 = 1;
pub const NEXT_ID: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    /// `Mở widget`: opens the card and takes the keyboard.
    Open,
    /// `Nhảy session chờ`.
    Next,
}

impl Which {
    fn id(self) -> u32 {
        match self {
            Which::Open => OPEN_ID,
            Which::Next => NEXT_ID,
        }
    }

    fn defaults_key(self) -> &'static str {
        match self {
            Which::Open => "hotkeyOpen",
            Which::Next => "hotkeyNext",
        }
    }

    /// The name in `failed` (Swift `hotkeyFailed`).
    pub fn name(self) -> &'static str {
        match self {
            Which::Open => "open",
            Which::Next => "next",
        }
    }
}

pub struct Hotkeys {
    pub open: HotKeyChoice,
    pub next: HotKeyChoice,
    /// Hotkeys whose registration failed (another app owns the combo): `(đang bị app khác dùng)` in the menu.
    pub failed: BTreeSet<&'static str>,
    /// The last `⌃⌥→` press run, for the 5 s cycle.
    pub last_next: Option<Cycle>,
}

impl Hotkeys {
    /// The saved choices (absent key = default, stored `"off"` = none); nothing is registered yet.
    pub fn load(kv: &dyn KeyValueStore) -> Self {
        Hotkeys {
            open: HotKeyChoice::load(kv, Which::Open.defaults_key(), &open_choices()[0]),
            next: HotKeyChoice::load(kv, Which::Next.defaults_key(), &next_choices()[0]),
            failed: BTreeSet::new(),
            last_next: None,
        }
    }

    pub fn choice(&self, which: Which) -> &HotKeyChoice {
        match which {
            Which::Open => &self.open,
            Which::Next => &self.next,
        }
    }

    /// Registers the current choice (or unregisters for `Off`) and records a refusal in `failed`.
    pub fn register(&mut self, which: Which, handler: impl Fn() + 'static) {
        let ok = match self.choice(which).combo() {
            Some(c) => hotkeys::register(which.id(), c.key_code, c.modifiers, handler),
            None => {
                hotkeys::unregister(which.id());
                true
            }
        };
        if ok {
            self.failed.remove(which.name());
        } else {
            self.failed.insert(which.name());
        }
    }

    /// Changes and saves a choice, then registers it.
    pub fn set(
        &mut self,
        kv: &dyn KeyValueStore,
        which: Which,
        choice: HotKeyChoice,
        handler: impl Fn() + 'static,
    ) {
        choice.save(kv, which.defaults_key());
        match which {
            Which::Open => self.open = choice,
            Which::Next => self.next = choice,
        }
        self.register(which, handler);
    }
}
