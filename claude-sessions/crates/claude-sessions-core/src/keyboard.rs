//! Keyboard mode: the key table of the selected card (Swift `KeyCommand` and `WidgetView.handleKey`).

use crate::action::Action;
use crate::feed::{self, BlockState};
use crate::panel_state::{EditKind, Editing, PanelState};
use crate::store::{CommandFit, Snapshot, command_fit};
use crate::transcript::Pending;
use unicode_segmentation::UnicodeSegmentation;

/// Virtual key codes the keyboard mode reads.
pub mod key_code {
    pub const ESCAPE: u16 = 53;
    pub const UP: u16 = 126;
    pub const DOWN: u16 = 125;
    pub const RETURN: u16 = 36;
    pub const KEYPAD_ENTER: u16 = 76;
}

/// The modifiers the key table looks at; shift and caps lock are not part of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub command: bool,
    pub control: bool,
    pub option: bool,
}

impl Mods {
    fn is_empty(&self) -> bool {
        *self == Mods::default()
    }

    fn only_command(&self) -> bool {
        *self
            == Mods {
                command: true,
                ..Mods::default()
            }
    }
}

/// One key press; `chars` are the characters ignoring modifiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key_code: u16,
    pub chars: String,
    pub mods: Mods,
}

/// What one key press does in keyboard mode.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyCommand {
    Action(Action),
    Edit(Editing),
    Pick {
        question: usize,
        label: String,
    },
    Move(i64),
    Close,
    /// A bound key that cannot apply now.
    Beep,
    /// Not ours: the event goes on.
    Pass,
}

/// What `handle_key` did with the event.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyOutcome {
    /// The key was ours; `Some` when it asks the app to run an action.
    Consumed(Option<Action>),
    Beep,
    PassThrough,
}

fn command_shown(snapshot: &Snapshot, p: &Pending) -> bool {
    match command_fit(&p.text) {
        CommandFit::Short => true,
        CommandFit::Long => snapshot.shown_commands.contains(&p.tool_id),
        CommandFit::TooLong => false,
    }
}

impl KeyCommand {
    /// The decision for a key press.
    pub fn resolve(event: &KeyEvent, snapshot: &Snapshot, panel: &PanelState) -> KeyCommand {
        use key_code::*;
        if !panel.keyboard || panel.editing.is_some() {
            return KeyCommand::Pass;
        }
        let mods = event.mods;
        if event.key_code == ESCAPE {
            return KeyCommand::Close;
        }
        if event.key_code == UP || event.key_code == DOWN {
            return if mods.is_empty() {
                KeyCommand::Move(if event.key_code == UP { -1 } else { 1 })
            } else {
                KeyCommand::Pass
            };
        }
        let is_return = event.key_code == RETURN || event.key_code == KEYPAD_ENTER;
        let key = if is_return {
            "\r".to_string()
        } else {
            event.chars.to_lowercase()
        };
        if !(is_return || (mods.is_empty() && key.graphemes(true).count() == 1)) {
            return KeyCommand::Pass;
        }
        let digit = key
            .parse::<usize>()
            .ok()
            .filter(|n| key.len() == 1 && (1..=9).contains(n));
        let bound = if is_return {
            mods.is_empty() || mods.only_command()
        } else {
            mods.is_empty() && (matches!(key.as_str(), "a" | "d" | "r") || digit.is_some())
        };
        if !bound {
            return KeyCommand::Pass;
        }
        let Some(s) = panel
            .selected
            .as_ref()
            .and_then(|id| snapshot.sessions.iter().find(|s| &s.id == id))
        else {
            return KeyCommand::Beep;
        };
        let id = s.id.clone();
        let form = s
            .question
            .as_ref()
            .filter(|_| s.block == Some(BlockState::Form));
        if is_return {
            if mods.is_empty() {
                return KeyCommand::Action(Action::Jump(id));
            }
            let Some(q) = form else {
                return KeyCommand::Beep;
            };
            let answers = panel
                .answers
                .get(&q.request_id)
                .cloned()
                .unwrap_or_default();
            let other = panel.others.get(&q.request_id).cloned().unwrap_or_default();
            return if feed::complete(q, &answers, &other) {
                KeyCommand::Action(Action::Answer {
                    session: id,
                    request_id: q.request_id.clone(),
                    answers,
                    other,
                })
            } else {
                KeyCommand::Beep
            };
        }
        if let Some(n) = digit {
            let Some((question, label)) = form.and_then(|q| PanelState::option_at(q, n)) else {
                return KeyCommand::Beep;
            };
            return KeyCommand::Pick { question, label };
        }
        match key.as_str() {
            "a" => match s.approvable().filter(|p| command_shown(snapshot, p)) {
                Some(p) => KeyCommand::Action(Action::Approve {
                    session: id,
                    tool_id: p.tool_id.clone(),
                }),
                None => KeyCommand::Beep,
            },
            // The `Từ chối…` button shows under the same condition.
            "d" => match s.approvable().filter(|p| command_shown(snapshot, p)) {
                Some(p) => KeyCommand::Edit(Editing {
                    id,
                    kind: EditKind::Deny,
                    tool_id: Some(p.tool_id.clone()),
                }),
                None => KeyCommand::Beep,
            },
            _ => {
                if s.can_ask_claude() && !snapshot.surface_misses.contains(&id) {
                    KeyCommand::Edit(Editing {
                        id,
                        kind: EditKind::Reply,
                        tool_id: None,
                    })
                } else {
                    KeyCommand::Beep
                }
            }
        }
    }
}

/// Keyboard mode: runs the key table on the selected card; `Close`, `Move`, `Edit` and `Pick` change `panel` here.
pub fn handle_key(event: &KeyEvent, snapshot: &Snapshot, panel: &mut PanelState) -> KeyOutcome {
    match KeyCommand::resolve(event, snapshot, panel) {
        KeyCommand::Pass => return KeyOutcome::PassThrough,
        KeyCommand::Close => panel.close(),
        KeyCommand::Move(delta) => {
            let ids: Vec<String> = snapshot.sessions.iter().map(|s| s.id.clone()).collect();
            panel.move_selection(delta, &ids);
        }
        KeyCommand::Action(a) => return KeyOutcome::Consumed(Some(a)),
        KeyCommand::Edit(editing) => panel.editing = Some(editing),
        KeyCommand::Pick { question, label } => {
            let q = panel.selected.as_ref().and_then(|id| {
                snapshot
                    .sessions
                    .iter()
                    .find(|s| &s.id == id)
                    .and_then(|s| s.question.as_ref())
            });
            if let Some(q) = q {
                panel.pick(q, question, &label);
            }
        }
        KeyCommand::Beep => return KeyOutcome::Beep,
    }
    KeyOutcome::Consumed(None)
}
