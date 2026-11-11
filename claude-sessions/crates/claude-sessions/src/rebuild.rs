//! When the 1 s tick has to rebuild a panel's scene. A scene only changes when the snapshot changed, the panel went between
//! pill and card, or a visible time label (elapsed, `bấm lại`, countdown, sent note) reaches its next text. Everything else
//! is an idle tick: no scene, no diff, no redraw.

use crate::hits::STOP_ARM_SECS;
use claude_sessions_core::feed::{self, BlockState};
use claude_sessions_core::layout::question_state;
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::store::{SendState, Snapshot};
use claude_sessions_core::text::{next_closed_change, next_label_change};
use claude_sessions_core::time::{add_secs, secs_since};
use std::time::{Duration, SystemTime};

/// The next time a scene built at `now` would show different text for the same snapshot, or `None` if it never would.
/// The collapsed pill shows no time, so it has none.
pub fn next_rebuild(
    snap: &Snapshot,
    panel: &PanelState,
    expanded: bool,
    now: SystemTime,
) -> Option<SystemTime> {
    if !expanded {
        return None;
    }
    let mut due: Option<SystemTime> = None;
    let mut at = |t: SystemTime| due = Some(due.map_or(t, |d| d.min(t)));
    for s in &snap.sessions {
        let armed = panel
            .stop_armed
            .get(&s.id)
            .filter(|t| s.shown_status() == "busy" && secs_since(now, **t) < STOP_ARM_SECS);
        if let Some(t) = armed {
            at(add_secs(*t, STOP_ARM_SECS));
        } else if let Some(since) = s.since {
            at(now + next_label_change(since, now));
        }
        for pr in &s.prs {
            if let Some(c) = &pr.last_other {
                at(now + next_label_change(c.at, now));
            }
        }
        if let Some(q) = &s.question
            && question_state(s, q) == BlockState::Form
            && feed::remaining(q, now) > 0.0
        {
            at(now + Duration::from_secs(1));
        }
    }
    if snap.pr_stale
        && let Some(t) = snap.pr_stale_since
    {
        at(now + next_label_change(t, now));
    }
    if panel.closed_open {
        for item in &snap.closed {
            at(now + next_closed_change(item.closed_at, now));
        }
    }
    for (state, sent_at) in snap.sends.values() {
        if *state != SendState::Sending && secs_since(now, *sent_at) < 3.0 {
            at(add_secs(*sent_at, 3.0));
        }
    }
    due
}

/// What one panel's last scene was built from.
#[derive(Default)]
pub struct RebuildGate {
    built: Option<(Snapshot, bool)>,
    due: Option<SystemTime>,
}

impl RebuildGate {
    /// Whether the tick at `now` has to rebuild: nothing built yet, the snapshot or the expanded state differs, or a label is due.
    pub fn needs_rebuild(&self, snap: &Snapshot, expanded: bool, now: SystemTime) -> bool {
        match &self.built {
            None => true,
            Some((s, e)) => *e != expanded || s != snap || self.due.is_some_and(|d| d <= now),
        }
    }

    /// Records that a scene was built from these inputs; call after every rebuild, tick-driven or not.
    pub fn built(&mut self, snap: &Snapshot, panel: &PanelState, expanded: bool, now: SystemTime) {
        if self.built.as_ref().is_none_or(|(s, _)| s != snap) {
            self.built = Some((snap.clone(), expanded));
        }
        if let Some((_, e)) = &mut self.built {
            *e = expanded;
        }
        self.due = next_rebuild(snap, panel, expanded, now);
    }
}
