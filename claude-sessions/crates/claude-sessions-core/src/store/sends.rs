//! Send states, the answered-question notes, and the state-only part of actions.

use super::*;

impl Store {
    /// Claims `k` for a send; `false` (the caller beeps) while another send to the same session is in flight.
    pub fn begin_send(&mut self, k: SendKey, at: SystemTime) -> bool {
        if self
            .sends
            .iter()
            .any(|(o, e)| o.session == k.session && e.0 == SendState::Sending)
        {
            return false;
        }
        self.sends.insert(k, (SendState::Sending, at));
        true
    }

    pub fn finish_send(&mut self, k: &SendKey, ok: bool, at: SystemTime) {
        let state = if ok {
            SendState::Sent
        } else {
            SendState::Failed
        };
        self.sends.insert(k.clone(), (state, at));
    }

    /// What the button of `k` shows: `Sending` until `prune_sends` times it out, `Sent`/`Failed` for 3 s, then `None`.
    pub fn send_state(&self, k: &SendKey) -> Option<SendState> {
        shown_send(self.sends.get(k)?, self.now)
    }

    /// The state of the newest shown send among the session's keys of `kinds`; for a slot that flips between buttons.
    pub fn send_state_of(&self, session: &str, kinds: &[SendKind]) -> Option<SendState> {
        self.sends
            .iter()
            .filter(|(k, _)| k.session == session && kinds.contains(&k.kind))
            .filter_map(|(_, e)| shown_send(e, self.now).map(|s| (s, e.1)))
            .max_by_key(|(_, at)| *at)
            .map(|(s, _)| s)
    }

    /// Turns a `Sending` older than 15 s into `Failed` and drops what is no longer shown.
    pub(super) fn prune_sends(&mut self) {
        let now = self.now;
        self.sends.retain(|_, e| {
            if e.0 == SendState::Sending && secs_since(now, e.1) >= SEND_TIMEOUT {
                *e = (SendState::Failed, now);
            }
            shown_send(e, now).is_some()
        });
    }

    pub(super) fn reblock(&mut self, id: &str) {
        let (sent, failed, now) = (&self.sent, &self.failed, self.now);
        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
            s.block = block_of(s, sent, failed, now);
        }
    }

    /// Remembers that the answer to `request_id` reached cmux, so the card reads `Đã gửi`.
    pub fn note_sent(&mut self, id: &str, request_id: &str) {
        self.sent.insert(id.to_string(), request_id.to_string());
        self.reblock(id);
    }

    /// Undoes `note_sent` after the send failed: the card falls back to `Trả lời trong cmux`.
    pub fn note_send_failed(&mut self, id: &str, request_id: &str) {
        if self.sent.get(id).map(String::as_str) == Some(request_id) {
            self.sent.remove(id);
        }
        self.failed.insert(id.to_string(), request_id.to_string());
        self.reblock(id);
    }

    pub fn still_valid(&self, a: &Action) -> bool {
        let session = |id: &str| self.sessions.iter().find(|s| s.id == id);
        match a {
            Action::Jump(_)
            | Action::OpenLink(..)
            | Action::Connect(_)
            | Action::NudgeRabbit(..)
            | Action::Resume(_)
            | Action::MarkSeen(_)
            | Action::ShowCommand { .. } => true,
            // Only a command the card shows whole can be approved or denied from the widget.
            Action::Approve {
                session: id,
                tool_id,
            }
            | Action::Deny {
                session: id,
                tool_id,
                ..
            } => {
                self.allows(&Editing {
                    id: id.clone(),
                    kind: EditKind::Deny,
                    tool_id: Some(tool_id.clone()),
                }) && session(id)
                    .and_then(Session::approvable)
                    .is_some_and(|p| command_is_shown(&self.shown_commands, p))
            }
            Action::AskClaude { session: id, .. } | Action::Reply { session: id, .. } => {
                session(id).is_some_and(Session::can_ask_claude)
            }
            Action::Stop(id) => session(id).is_some_and(|s| s.shown_status() == "busy"),
            Action::Answer {
                session: id,
                request_id,
                ..
            } => session(id).is_some_and(|s| {
                s.question_live && s.question.as_ref().map(|q| &q.request_id) == Some(request_id)
            }),
        }
    }

    /// The state-only part of an action: clears a mark, claims the send button. The app runs the returned effect.
    pub fn apply(&mut self, a: &Action, at: SystemTime) -> Effect {
        if let Action::MarkSeen(id) = a {
            self.mark_seen(id);
            return Effect::Nothing;
        }
        if let Action::ShowCommand { session, tool_id } = a {
            let open = self
                .sessions
                .iter()
                .find(|s| &s.id == session)
                .and_then(Session::approvable)
                .is_some_and(|p| &p.tool_id == tool_id);
            if !open {
                return Effect::Beep;
            }
            self.shown_commands.insert(tool_id.clone());
            // The app re-renders on `Run`, so the card grows at once.
            return Effect::Run(a.clone());
        }
        match a.send_key() {
            Some(k) if !self.begin_send(k.clone(), at) => Effect::Beep,
            _ => Effect::Run(a.clone()),
        }
    }
}
