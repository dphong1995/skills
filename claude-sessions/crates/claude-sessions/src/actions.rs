//! What an action does outside the store (Swift `handle(_:)`): the state part goes through `Store::apply`, the effect part
//! through an `EffectRunner`. Effects that finish later (cmux lookups, AppleScript, sockets) post a `Completion` that
//! `complete` applies on the main thread. Nothing here touches AppKit, so tests drive it with a recording runner.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use claude_sessions_core::action::{Action, SendKey};
use claude_sessions_core::closed_sessions::ClosedSession;
use claude_sessions_core::cmux::{self, Key};
use claude_sessions_core::credentials::Service;
use claude_sessions_core::panel_state::EditKind;
use claude_sessions_core::pull_requests::Chime;
use claude_sessions_core::store::{Effect, LinkKind, Store};
use claude_sessions_macos::workspace;

use crate::focus::Located;
use url::Url;

/// Hands a `Completion` back to the app from any thread.
pub type Poster = Arc<dyn Fn(Completion) + Send + Sync>;

/// An action whose send button is claimed, waiting for cmux.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub action: Action,
    pub key: Option<SendKey>,
    /// The process of the session when the lookup started: a result for another process is not cached.
    pub pid: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Completion {
    Surface(Pending, Option<String>),
    Jumped(Option<SendKey>, bool),
    Sent(Pending, bool),
    Resumed(ClosedSession, SendKey, bool),
    Answered {
        key: SendKey,
        session: String,
        request_id: String,
        ok: bool,
    },
    Nudged(SendKey, bool),
    /// The kill of a process group finished: `true` when every target is gone.
    Killed(SendKey, bool),
    /// `Action::Connect`: the app opens the Settings window on a later run-loop turn.
    OpenSettings(Service),
    /// The token check and Keychain write of `connect` finished: `failure` is the notice to show, `None` when stored.
    /// Never carries the token.
    Connected {
        service: Service,
        account: String,
        failure: Option<String>,
    },
    /// The `Kiểm tra` of the Settings window finished: the confirmed account or the failure text. Never carries the token.
    Checked {
        service: Service,
        result: Result<String, String>,
    },
    /// The 2 s focused-pane check finished (`generation` tells a stale one).
    Focus {
        generation: u64,
        located: Vec<Located>,
        focused: Option<String>,
    },
    /// The `⌃⌥→` walk finished: `found` is the session and surface to jump to; `order` and `at` are the cycle it belongs to.
    NextWaiting {
        order: Vec<String>,
        at: SystemTime,
        located: Vec<Located>,
        found: Option<(String, String)>,
    },
    /// The cmux check behind a chime finished: `transitions` and `pr_chime` are those of the refresh that started it.
    Alert {
        transitions: HashMap<String, Chime>,
        pr_chime: Option<Chime>,
        located: Vec<Located>,
        focused: Option<String>,
    },
}

/// The side effects of actions. Every method only starts work; results come back as `Completion`s through `poster`.
pub trait EffectRunner {
    fn poster(&self) -> Poster;
    fn beep(&mut self);
    fn play(&mut self, chime: Chime);
    /// Opens `url` in the browser; `allow_http` is for a transcript link, every other link is `https` only.
    fn open_link(&mut self, url: &Url, allow_http: bool);
    fn copy(&mut self, text: &str);
    /// Looks the session's cmux surface up off the main thread, then posts `Completion::Surface(pending, ..)`.
    fn lookup_surface(&mut self, session: &str, pending: Pending);
    /// Activates cmux on `surface`, then posts `Completion::Jumped(key, ..)`.
    fn jump(&mut self, surface: &str, key: Option<SendKey>);
    /// Types `keys` into `surface` (pauses included), then posts `Completion::Sent(pending, ..)`.
    fn send_keys(&mut self, surface: &str, keys: Vec<Key>, pending: Pending);
    /// Opens a cmux workspace that resumes `closed`, then posts `Completion::Resumed`.
    fn resume(&mut self, closed: &ClosedSession, key: SendKey);
    /// Opens the token dialog of `service`.
    fn connect(&mut self, service: Service);
    /// Kills `targets` off the main thread, then posts `Completion::Killed(key, ok)`.
    fn kill_procs(&mut self, key: SendKey, targets: Vec<(i32, u64)>);
}

/// Plays `chime` unless muted (Swift `playChime`).
pub fn play_chime(chime: Option<Chime>, muted: bool, runner: &mut dyn EffectRunner) {
    if let Some(chime) = chime
        && !muted
    {
        runner.play(chime);
    }
}

/// `Store::apply`, then the effect when the store claimed the action. Returns the session whose editor to close.
pub fn handle(
    action: &Action,
    store: &mut Store,
    runner: &mut dyn EffectRunner,
    now: SystemTime,
) -> Option<String> {
    match store.apply(action, now) {
        Effect::Run(a) => run(&a, store, runner, now),
        Effect::Beep => {
            runner.beep();
            None
        }
        Effect::Nothing => None,
    }
}

/// Whether the action types into a cmux pane, so the store must be fresh before it runs.
pub fn sends_keys(action: &Action) -> bool {
    matches!(
        action,
        Action::Approve { .. }
            | Action::Deny { .. }
            | Action::AskClaude { .. }
            | Action::Reply { .. }
            | Action::Stop(_)
    )
}

/// The effect of an action the store already claimed (its send button is `Sending`). Returns the session whose editor to
/// close: a deny, reply or answer closes it at once so a second Enter cannot queue a second send.
pub fn run(
    action: &Action,
    store: &mut Store,
    runner: &mut dyn EffectRunner,
    now: SystemTime,
) -> Option<String> {
    let id = action.session_id().to_string();
    let key = action.send_key();
    match action {
        Action::MarkSeen(_) | Action::ShowCommand { .. } => {}
        Action::Connect(service) => {
            runner.connect(*service);
        }
        Action::OpenLink(_, url) => {
            store.mark_seen(&id);
            let allow_http = store
                .sessions
                .iter()
                .filter(|s| s.id == id)
                .flat_map(|s| &s.menu)
                .flat_map(|section| &section.items)
                .any(|item| item.kind == LinkKind::Link && item.url == *url);
            let opener = |u: &Url| {
                runner.open_link(u, allow_http);
                true
            };
            let opened = if allow_http {
                workspace::open_link_allow_http_with(url, opener)
            } else {
                workspace::open_https_with(url, opener)
            };
            if !opened {
                runner.beep();
            }
        }
        Action::NudgeRabbit(_, pr) => {
            let (Some(key), Some(monitor)) = (key, &store.pr_monitor) else {
                finish_unclaimed(store, action, runner, now);
                return None;
            };
            let poster = runner.poster();
            monitor.nudge(pr, move |ok| poster(Completion::Nudged(key, ok)));
        }
        Action::Answer {
            request_id,
            answers,
            other,
            ..
        } => {
            let question = store.question_of(&id).cloned();
            let (Some(key), Some(q), true, true) = (
                key,
                question,
                store.feed_monitor.is_some(),
                store.still_valid(action),
            ) else {
                finish_unclaimed(store, action, runner, now);
                return None;
            };
            // Optimistic: the monitor drops the item before `ok` arrives, which would flash `Trả lời trong cmux`.
            store.note_sent(&id, request_id);
            let poster = runner.poster();
            let (session, request_id) = (id.clone(), request_id.clone());
            if let Some(monitor) = &store.feed_monitor {
                monitor.reply(&q, answers, other, move |ok| {
                    poster(Completion::Answered {
                        key,
                        session,
                        request_id,
                        ok,
                    })
                });
            }
            return Some(id);
        }
        Action::Resume(closed) => {
            let (Some(key), true) = (key, closed.resume_command().is_some()) else {
                finish_unclaimed(store, action, runner, now);
                return None;
            };
            runner.resume(closed, key);
        }
        Action::KillProcs { targets, .. } => {
            let (Some(key), true) = (key, store.still_valid(action)) else {
                finish_unclaimed(store, action, runner, now);
                return None;
            };
            runner.kill_procs(key, targets.clone());
        }
        Action::Jump(_) => {
            store.mark_seen(&id);
            resolve_surface(
                Pending {
                    action: action.clone(),
                    key,
                    pid: store.pid_of(&id),
                },
                store,
                runner,
                now,
            );
        }
        Action::Approve { .. }
        | Action::Deny { .. }
        | Action::AskClaude { .. }
        | Action::Reply { .. }
        | Action::Stop(_) => {
            resolve_surface(
                Pending {
                    action: action.clone(),
                    key,
                    pid: store.pid_of(&id),
                },
                store,
                runner,
                now,
            );
            return matches!(action, Action::Deny { .. } | Action::Reply { .. }).then_some(id);
        }
    }
    None
}

/// An action that cannot run although the store claimed it: release the button as failed and beep.
fn finish_unclaimed(
    store: &mut Store,
    action: &Action,
    runner: &mut dyn EffectRunner,
    now: SystemTime,
) {
    if let Some(k) = action.send_key() {
        store.finish_send(&k, false, now);
    }
    runner.beep();
}

/// The cached surface of the session, else a lookup that continues in `complete`.
fn resolve_surface(
    pending: Pending,
    store: &mut Store,
    runner: &mut dyn EffectRunner,
    now: SystemTime,
) {
    let id = pending.action.session_id().to_string();
    match store.surfaces.get(&id).cloned() {
        Some(surface) => with_surface(pending, Some(surface), store, runner, now),
        None => runner.lookup_surface(&id, pending),
    }
}

/// A jump goes to `surface`; a send is validated right before the first keystroke and then typed.
fn with_surface(
    pending: Pending,
    surface: Option<String>,
    store: &mut Store,
    runner: &mut dyn EffectRunner,
    now: SystemTime,
) {
    let action = pending.action.clone();
    if matches!(action, Action::Jump(_)) {
        match surface {
            Some(s) => runner.jump(&s, pending.key),
            None => finish_unclaimed(store, &action, runner, now),
        }
        return;
    }
    let Some(surface) = surface else {
        return finish_unclaimed(store, &action, runner, now);
    };
    let id = action.session_id();
    if store.surfaces.get(id) != Some(&surface) || !store.still_valid(&action) {
        return finish_unclaimed(store, &action, runner, now);
    }
    runner.send_keys(&surface, cmux::keys_for(&action), pending);
}

/// Applies the result of an effect that finished later.
pub fn complete(c: Completion, store: &mut Store, runner: &mut dyn EffectRunner, now: SystemTime) {
    match c {
        Completion::Surface(pending, surface) => {
            let id = pending.action.session_id().to_string();
            // The session's process changed while the lookup ran: the result may belong to the old one.
            if store.pid_of(&id) == pending.pid {
                match &surface {
                    Some(s) => {
                        store.surfaces.insert(id.clone(), s.clone());
                        store.surface_misses.remove(&id);
                    }
                    None => {
                        store.surface_misses.insert(id);
                    }
                }
            }
            with_surface(pending, surface, store, runner, now);
        }
        Completion::Jumped(key, ok) => {
            if let Some(k) = &key {
                store.finish_send(k, ok, now);
            }
            if !ok {
                runner.beep();
            }
        }
        Completion::Sent(pending, ok) => {
            if let Some(k) = &pending.key {
                store.finish_send(k, ok, now);
            }
            if !ok {
                return runner.beep();
            }
            let id = pending.action.session_id();
            store.mark_seen(id);
            let (kind, text) = match &pending.action {
                Action::Deny { reason, .. } => (EditKind::Deny, reason),
                Action::Reply { text, .. } => (EditKind::Reply, text),
                _ => return,
            };
            let draft = Store::draft_key(id, kind);
            if store.drafts.get(&draft) == Some(text) {
                store.drafts.remove(&draft);
            }
        }
        Completion::Resumed(closed, key, ok) => {
            store.finish_send(&key, ok, now);
            if ok {
                store.revive_closed(&[closed.id.clone()].into());
                return;
            }
            if let Some(command) = closed.shell_command() {
                runner.copy(&command);
            }
            runner.beep();
        }
        Completion::Answered {
            key,
            session,
            request_id,
            ok,
        } => {
            store.finish_send(&key, ok, now);
            if !ok {
                store.note_send_failed(&session, &request_id);
                runner.beep();
            }
        }
        Completion::Nudged(key, ok) | Completion::Killed(key, ok) => {
            store.finish_send(&key, ok, now);
            if !ok {
                runner.beep();
            }
        }
        // The app owns these: they need its Settings window and its cmux state.
        Completion::OpenSettings(_)
        | Completion::Connected { .. }
        | Completion::Checked { .. }
        | Completion::Focus { .. }
        | Completion::NextWaiting { .. }
        | Completion::Alert { .. } => {}
    }
}
