//! The two global hotkeys' choices, their saved form (`hotkeyOpen` / `hotkeyNext`) and the `⌃⌥→` jump cycle. Registering them with
//! Carbon lives in `claude-sessions-macos::hotkeys`.

use crate::kv::KeyValueStore;
use crate::time::secs_since;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// Carbon `kVK_*` key codes and modifier masks (`Events.h`, `HIToolbox/Events.h`).
const K_VK_SPACE: u32 = 49;
const K_VK_RIGHT_ARROW: u32 = 124;
const K_VK_ANSI_J: u32 = 38;
const CMD_KEY: u32 = 0x100;
const OPTION_KEY: u32 = 0x800;
const CONTROL_KEY: u32 = 0x1000;

/// A key and its modifiers as Carbon takes them. Serialized as Swift's `JSONEncoder` writes `HotKeyCombo`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotKeyCombo {
    pub key_code: u32,
    pub modifiers: u32,
    pub label: String,
}

impl HotKeyCombo {
    fn new(key_code: u32, modifiers: u32, label: &str) -> Self {
        HotKeyCombo {
            key_code,
            modifiers,
            label: label.to_string(),
        }
    }

    /// The saved bytes (`Data` of JSON).
    pub fn to_defaults(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a combo is plain data")
    }

    /// Reads saved bytes; `None` when they are not a combo.
    pub fn from_defaults(data: &[u8]) -> Option<HotKeyCombo> {
        serde_json::from_slice(data).ok()
    }
}

pub fn open_choices() -> Vec<HotKeyCombo> {
    vec![
        HotKeyCombo::new(K_VK_SPACE, CONTROL_KEY | OPTION_KEY, "⌃⌥Space"),
        HotKeyCombo::new(K_VK_SPACE, CONTROL_KEY | OPTION_KEY | CMD_KEY, "⌃⌥⌘Space"),
        HotKeyCombo::new(K_VK_SPACE, OPTION_KEY, "⌥Space"),
    ]
}

pub fn next_choices() -> Vec<HotKeyCombo> {
    vec![
        HotKeyCombo::new(K_VK_RIGHT_ARROW, CONTROL_KEY | OPTION_KEY, "⌃⌥→"),
        HotKeyCombo::new(K_VK_ANSI_J, CONTROL_KEY | OPTION_KEY, "⌃⌥J"),
        HotKeyCombo::new(K_VK_RIGHT_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY, "⌃⌥⌘→"),
    ]
}

/// A hotkey that is on with a combo, or switched off (`Tắt`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotKeyChoice {
    Off,
    On(HotKeyCombo),
}

impl HotKeyChoice {
    /// Loads the saved choice: an absent key is `default`, the stored string `"off"` (or anything unreadable) is `Off`.
    pub fn load(store: &dyn KeyValueStore, key: &str, default: &HotKeyCombo) -> HotKeyChoice {
        if let Some(data) = store.get_bytes(key) {
            return HotKeyCombo::from_defaults(&data).map_or(HotKeyChoice::Off, HotKeyChoice::On);
        }
        if store.get_string(key).is_some() {
            return HotKeyChoice::Off;
        }
        HotKeyChoice::On(default.clone())
    }

    pub fn save(&self, store: &dyn KeyValueStore, key: &str) {
        match self {
            HotKeyChoice::On(c) => store.set_bytes(key, &c.to_defaults()),
            HotKeyChoice::Off => store.set_string(key, "off"),
        }
    }

    pub fn combo(&self) -> Option<&HotKeyCombo> {
        match self {
            HotKeyChoice::On(c) => Some(c),
            HotKeyChoice::Off => None,
        }
    }
}

/// Presses of the jump hotkey this close together (seconds) walk one saved order.
pub const CYCLE_WINDOW_SECS: f64 = 5.0;

/// One run of presses: the order seen on its first press, the session jumped to last, and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle {
    pub order: Vec<String>,
    pub id: String,
    pub at: SystemTime,
}

/// The order to walk and the session to jump to: within 5 s of the last press, the live one after `cycle.id` in the order saved
/// on the first press (a finished session leaves `fresh` once it is seen, so `fresh` cannot be used); else (also when every saved
/// session has died) the first of `fresh`.
pub fn advance(
    cycle: Option<&Cycle>,
    fresh: &[String],
    is_live: impl Fn(&str) -> bool,
    now: SystemTime,
) -> Option<(Vec<String>, String)> {
    if let Some(cycle) = cycle
        && secs_since(now, cycle.at) <= CYCLE_WINDOW_SECS
        && let Some(i) = cycle.order.iter().position(|o| *o == cycle.id)
    {
        let n = cycle.order.len();
        if let Some(id) = (1..=n)
            .map(|k| &cycle.order[(i + k) % n])
            .find(|id| is_live(id))
        {
            return Some((cycle.order.clone(), id.clone()));
        }
    }
    fresh.first().map(|first| (fresh.to_vec(), first.clone()))
}
