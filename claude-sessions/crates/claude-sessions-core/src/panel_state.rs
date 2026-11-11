//! State that belongs to one pill (one display); everything shared lives in the store. Includes the behaviour Swift keeps inside the
//! SwiftUI views: the hover dwell that clears a fresh mark, and keeping the hover and the selection valid when the row order changes.

use crate::feed::{self, FeedQuestion};
use crate::layout::{Anchor, SEEN_DWELL_SECS};
use crate::time::secs_since;
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Deny,
    Reply,
    Answer,
}

/// The text field a card has open (Swift `Store.Editing`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editing {
    pub id: String,
    pub kind: EditKind,
    /// The tool a `Deny` editor is for.
    pub tool_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyToggle {
    Open,
    Close,
    Keyboard,
}

#[derive(Clone, Debug, Default)]
pub struct PanelState {
    pub hovering: bool,
    /// Opened by the hotkey: `pointer_wants` keeps `hovering` true until the hotkey is pressed again or the mouse has entered and
    /// then left the card.
    pub pinned_open: bool,
    pub pinned_entered: bool,
    pub editing: Option<Editing>,
    pub anchor: Anchor,
    pub measured_height: Option<f64>,
    /// Height of the session rows alone, so the list can scroll when the panel is full.
    pub list_height: Option<f64>,
    /// Session id -> when its `Dừng` button was first clicked; a second click within 3 s sends the stop.
    pub stop_armed: HashMap<String, SystemTime>,
    /// Session ids whose menu list is expanded; cleared when the widget collapses.
    pub open_menus: HashSet<String>,
    /// The `Vừa đóng (N)` group is expanded; cleared when the widget collapses.
    pub closed_open: bool,
    /// Session id the mouse is over in the expanded card; drives the hover dwell that clears a fresh mark.
    pub hovered_row: Option<String>,
    /// Request id -> the options picked per question index, and the `Khác…` text per question index.
    pub answers: HashMap<String, Vec<Vec<String>>>,
    pub others: HashMap<String, HashMap<usize, String>>,
    /// The `Khác…` field that was opened, as `"<session id>#<question index>"`: only that question shows its field, and only
    /// while `editing` is that session's `Answer`.
    pub other_key: Option<String>,
    /// Opened or switched to by the hotkey: the panel is key and the arrow keys move `selected`.
    pub keyboard: bool,
    pub selected: Option<String>,
    /// When the mouse entered `hovered_row`, or when its mark appeared under the mouse.
    hover_since: Option<SystemTime>,
    /// The last `due_mark_seen` call saw the hovered row without a mark.
    hover_stale: bool,
}

impl PanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The panel has to be the key window: a text field is open or keyboard mode is on.
    pub fn wants_key(&self) -> bool {
        self.editing.is_some() || self.keyboard
    }

    /// Whether a panel that stopped wanting the key should give key status back: only a key, visible panel that wants it no more.
    pub fn should_hand_back_key(&self, is_key: bool, hidden: bool) -> bool {
        is_key && !hidden && !self.wants_key()
    }

    /// Collapses the card and ends keyboard mode and any open editor.
    pub fn close(&mut self) {
        self.pinned_open = false;
        self.pinned_entered = false;
        self.hovering = false;
        self.editing = None;
        self.keyboard = false;
        self.selected = None;
    }

    /// The widget collapsed: closes the open link menus and the `Vừa đóng` group (the caller also calls `prune_questions`).
    pub fn on_collapse(&mut self) {
        self.open_menus.clear();
        self.closed_open = false;
    }

    /// What the open hotkey does given the panels that are open: nothing open opens one with the keyboard; one in keyboard mode closes;
    /// any other open one (hover, or pinned after keyboard mode ended) switches to keyboard mode.
    pub fn hotkey_toggle(open: &[&PanelState]) -> HotkeyToggle {
        if open.is_empty() {
            HotkeyToggle::Open
        } else if open.iter().any(|p| p.keyboard) {
            HotkeyToggle::Close
        } else {
            HotkeyToggle::Keyboard
        }
    }

    /// One mouse tick: updates the pin and returns whether the card should be hovered. A pinned card the mouse entered closes on the
    /// next leave, except in keyboard mode, which keeps it open until the keyboard mode ends.
    pub fn pointer_wants(&mut self, inside: bool) -> bool {
        if !self.pinned_open {
            return inside;
        }
        if inside {
            self.pinned_entered = true;
        }
        if !inside && self.pinned_entered && !self.keyboard {
            self.pinned_open = false;
            self.pinned_entered = false;
            return false;
        }
        true
    }

    /// Pins the card open and takes the keyboard, as the hotkey does.
    pub fn pin_with_keyboard(&mut self) {
        self.pinned_open = true;
        self.pinned_entered = false;
        self.keyboard = true;
    }

    /// Moves `selected` by `delta` within `ids`, clamped at both ends; no selection yet picks the first.
    pub fn move_selection(&mut self, delta: i64, ids: &[String]) {
        let at = self
            .selected
            .as_ref()
            .and_then(|cur| ids.iter().position(|i| i == cur));
        let Some(i) = at else {
            self.selected = ids.first().cloned();
            return;
        };
        let to = (i as i64 + delta).clamp(0, ids.len() as i64 - 1);
        self.selected = Some(ids[to as usize].clone());
    }

    /// Keeps `selected` valid after the id order changed: a vanished id gives way to the one at its index, else the last, else nothing.
    pub fn reconcile_selection(&mut self, old: &[String], new: &[String]) {
        let Some(cur) = &self.selected else {
            return;
        };
        if new.contains(cur) {
            return;
        }
        self.selected = match old.iter().position(|i| i == cur) {
            Some(i) if !new.is_empty() => Some(new[i.min(new.len() - 1)].clone()),
            _ => new.last().cloned(),
        };
    }

    /// The question index and label for digit key `number`, counted continuously across the questions.
    pub fn option_at(q: &FeedQuestion, number: usize) -> Option<(usize, String)> {
        let (question, option, _) = feed::numbered_options(q)
            .into_iter()
            .find(|&(_, _, n)| n == number)?;
        Some((
            question,
            q.questions[question].options[option].label.clone(),
        ))
    }

    /// Picks `label` for question `i`: replaces the pick of a single-select question (and drops its Other text), toggles it in a
    /// multi-select one.
    pub fn pick(&mut self, q: &FeedQuestion, i: usize, label: &str) {
        let Some(item) = q.questions.get(i) else {
            return;
        };
        let all = self.answers.entry(q.request_id.clone()).or_default();
        while all.len() < q.questions.len() {
            all.push(Vec::new());
        }
        if item.multi_select {
            match all[i].iter().position(|l| l == label) {
                Some(at) => {
                    all[i].remove(at);
                }
                None => all[i].push(label.to_string()),
            }
        } else {
            all[i] = vec![label.to_string()];
            if let Some(o) = self.others.get_mut(&q.request_id) {
                o.remove(&i);
            }
        }
    }

    /// Sets the `Khác…` text of question `i`; typing in a single-select question drops its pick.
    pub fn set_other(&mut self, q: &FeedQuestion, i: usize, text: &str) {
        let Some(item) = q.questions.get(i) else {
            return;
        };
        self.others
            .entry(q.request_id.clone())
            .or_default()
            .insert(i, text.to_string());
        if !item.multi_select
            && !text.is_empty()
            && let Some(all) = self.answers.get_mut(&q.request_id)
            && let Some(slot) = all.get_mut(i)
        {
            slot.clear();
        }
    }

    /// Drops the answers and the open `Khác…` field of questions that are gone.
    pub fn prune_questions(&mut self, live: &[FeedQuestion]) {
        let ids: HashSet<&str> = live.iter().map(|q| q.request_id.as_str()).collect();
        self.answers.retain(|k, _| ids.contains(k.as_str()));
        self.others.retain(|k, _| ids.contains(k.as_str()));
        let sessions: HashSet<&str> = live
            .iter()
            .filter_map(|q| q.session_id.as_deref())
            .collect();
        if let Some(key) = &self.other_key
            && !sessions.contains(key.split('#').next().unwrap_or(""))
        {
            self.other_key = None;
        }
    }

    /// The mouse entered (`inside`) or left row `id`; an enter of another row overrides a missed exit. `at` starts the dwell.
    pub fn note_row_hover(&mut self, id: &str, inside: bool, at: SystemTime) {
        if inside {
            if self.hovered_row.as_deref() != Some(id) {
                self.hovered_row = Some(id.to_string());
                self.hover_since = Some(at);
                self.hover_stale = false;
            }
        } else if self.hovered_row.as_deref() == Some(id) {
            self.hovered_row = None;
            self.hover_since = None;
        }
    }

    /// Clears `hovered_row` when its position in the id order changed or it vanished.
    pub fn note_order_change(&mut self, old: &[String], new: &[String]) {
        let Some(id) = &self.hovered_row else {
            return;
        };
        if old.iter().position(|i| i == id) != new.iter().position(|i| i == id) {
            self.hovered_row = None;
            self.hover_since = None;
        }
    }

    /// The ids changed order (`WidgetView.onChange(of: store.sessions.map(\.id))`): hover and selection stay valid.
    pub fn on_order_change(&mut self, old: &[String], new: &[String]) {
        self.note_order_change(old, new);
        self.reconcile_selection(old, new);
    }

    /// The hovered row when it carries a fresh mark.
    pub fn dwell_target(&self, is_fresh: impl Fn(&str) -> bool) -> Option<String> {
        self.hovered_row.clone().filter(|id| is_fresh(id))
    }

    /// The row whose mark clears now: the mouse has rested on a fresh row for `SEEN_DWELL_SECS`. A mark that appears under the
    /// mouse starts its own second (SwiftUI's `.task(id: dwellTarget)` restarts when the target changes).
    pub fn due_mark_seen(
        &mut self,
        at: SystemTime,
        is_fresh: impl Fn(&str) -> bool,
    ) -> Option<String> {
        let id = self.hovered_row.clone()?;
        if !is_fresh(&id) {
            self.hover_stale = true;
            return None;
        }
        if self.hover_stale {
            self.hover_stale = false;
            self.hover_since = Some(at);
        }
        let since = self.hover_since?;
        (secs_since(at, since) >= SEEN_DWELL_SECS).then_some(id)
    }

    /// Whether the card is shown expanded: hovered, editing, or peeking (Swift `expanded(_ store:)`).
    pub fn expanded(&self, peeking: bool) -> bool {
        self.hovering || self.editing.is_some() || peeking
    }
}
