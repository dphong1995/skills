//! The app: the real `Store`, one panel per display, the 1 s tick, drag, hotkeys and keyboard mode.
//!
//! `AppState` lives in `Rc<RefCell<..>>` on the main thread. Every AppKit callback borrows, updates and releases;
//! anything that spins a nested run loop (menu, alert) or re-enters callbacks (`makeKeyWindow`, `orderOut`) runs on a
//! later main-queue turn with no borrow held.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Instant, SystemTime};

use claude_sessions_core::action::Action;
use claude_sessions_core::bitbucket::BitbucketClient;
use claude_sessions_core::closed_sessions::ClosedStore;
use claude_sessions_core::credentials::{self, Service};
use claude_sessions_core::feed::{FeedMonitor, UnixFeedTransport};
use claude_sessions_core::hotkeys::advance;
use claude_sessions_core::jira::JiraClient;
use claude_sessions_core::jira_monitor::JiraMonitor;
use claude_sessions_core::keyboard::KeyOutcome;
use claude_sessions_core::layout;
use claude_sessions_core::panel_state::{HotkeyToggle, PanelState};
use claude_sessions_core::pr_monitor::PRMonitor;
use claude_sessions_core::quota::QuotaReader;
use claude_sessions_core::scene::HitId;
use claude_sessions_core::store::{Effect, Paths, Snapshot, Store, StoreOptions};
use claude_sessions_macos::defaults::{UserDefaultsStore, beep};
use claude_sessions_macos::keys::{self, KeyInfo, KeyMonitor};
use claude_sessions_macos::main_queue::{self, Ticker};
use claude_sessions_macos::menu::{self, MenuItem};
use claude_sessions_macos::mouse::{self, MouseMonitors};
use claude_sessions_macos::ui::{UiEvent, WidgetPanelHandle};
use claude_sessions_macos::{NotificationObserver, app as mac_app, screens};

use crate::hits::{Click, ClickEnv};
use crate::hotkey::{Hotkeys, Which};
use crate::panels::PanelHost;
use crate::settings::{self, Settings};
use crate::{drag, keyboard_mode, placement};

type Shared = Rc<RefCell<AppState>>;
type WeakState = Weak<RefCell<AppState>>;

/// Ticks a panel may stay off the active Space before it is rebuilt (Swift `rebuildPanelsOffSpace`).
const OFF_SPACE_TICKS: u32 = 3;

const MENU_QUIT: isize = 1;

/// A fixed snapshot shown instead of the store (`CLAUDE_SESSIONS_FIXTURE`, `fixtures` feature only).
struct Fixture {
    snap: Snapshot,
    panel: PanelState,
    base: SystemTime,
    started: Instant,
}

/// Keychain results, read off the main thread and handed to the tick.
struct Credentials {
    accounts: HashMap<Service, String>,
    bitbucket: Option<(String, String)>,
    jira: Option<(String, String)>,
}

pub struct AppState {
    pub store: Store,
    pub kv: UserDefaultsStore,
    pub panels: Vec<PanelHost>,
    pub hotkeys: Hotkeys,
    /// Per panel: ticks spent off the active Space since it was last on it; absent until it has been on it once.
    off_space_ticks: HashMap<String, u32>,
    fixture: Option<Fixture>,
    saved: Settings,
    pr_prompt: String,
    credentials: Option<Receiver<Credentials>>,
}

/// `CLAUDE_SESSIONS_FRAME_LOG=1` also logs every scene rebuild (the renderer logs every node redraw).
fn frame_log() -> bool {
    std::env::var_os("CLAUDE_SESSIONS_FRAME_LOG").is_some()
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// The fixture state named by `CLAUDE_SESSIONS_FIXTURE`, built in a temp dir that is removed before returning.
#[cfg(feature = "fixtures")]
fn load_fixture() -> Option<Fixture> {
    use claude_sessions_core::fixtures::{STATES, fixed_now, view_now};
    let want = std::env::var("CLAUDE_SESSIONS_FIXTURE").ok()?;
    let (name, build_state) = STATES.iter().find(|(n, _)| *n == want).unwrap_or_else(|| {
        let names: Vec<&str> = STATES.iter().map(|(n, _)| *n).collect();
        eprintln!("unknown fixture {want:?}; one of {}", names.join(", "));
        &STATES[0]
    });
    let dir = std::env::temp_dir().join(format!("claude-sessions-fixture-{}", std::process::id()));
    let now = fixed_now();
    let (snap, panel) = build_state(&dir, now);
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("fixture {name}");
    Some(Fixture {
        snap,
        panel,
        base: view_now(name, now),
        started: Instant::now(),
    })
}

#[cfg(not(feature = "fixtures"))]
fn load_fixture() -> Option<Fixture> {
    None
}

/// The store of a live run: `CLAUDE_SESSIONS_ROOT` holds `sessions/`, `projects/` and `tasks/` and turns off the quota;
/// otherwise the paths are `~/.claude/...` with the OMC quota cache and the saved closed list.
fn make_store(root: Option<PathBuf>) -> Store {
    match root {
        Some(root) => Store::new(
            Paths {
                sessions: root.join("sessions"),
                projects: root.join("projects"),
                tasks: root.join("tasks"),
            },
            StoreOptions {
                start_hidden: true,
                ..StoreOptions::default()
            },
        ),
        None => {
            let claude = home().join(".claude");
            Store::new(
                Paths {
                    sessions: claude.join("sessions"),
                    projects: claude.join("projects"),
                    tasks: claude.join("tasks"),
                },
                StoreOptions {
                    start_hidden: true,
                    quota: Some(QuotaReader::new(
                        claude.join("plugins/oh-my-claudecode/.usage-cache-anthropic.json"),
                    )),
                    closed_store: UserDefaultsStore::new(None)
                        .map(|kv| ClosedStore::new(Box::new(kv))),
                    ..StoreOptions::default()
                },
            )
        }
    }
}

/// Reads the Keychain on a worker thread (it spawns `security`); the tick picks the result up.
fn read_credentials() -> Receiver<Credentials> {
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new()
        .name("claude-sessions.credentials".into())
        .spawn(move || {
            let read = |s| credentials::read(s);
            let (bitbucket, jira) = (read(Service::Bitbucket), read(Service::Jira));
            let accounts = [(Service::Bitbucket, &bitbucket), (Service::Jira, &jira)]
                .into_iter()
                .filter_map(|(s, c)| c.as_ref().map(|(account, _)| (s, account.clone())))
                .collect();
            let _ = tx.send(Credentials {
                accounts,
                bitbucket,
                jira,
            });
        });
    if let Err(e) = spawned {
        eprintln!("credentials thread: {e}");
    }
    rx
}

/// Runs `f` with the state borrowed. A failed borrow is a re-entrancy bug: debug builds panic, release builds drop
/// the event.
fn with_state<R>(weak: &WeakState, f: impl FnOnce(&mut AppState) -> R) -> Option<R> {
    let shared = weak.upgrade()?;
    let Ok(mut state) = shared.try_borrow_mut() else {
        eprintln!("bug: AppState borrowed during a callback");
        debug_assert!(false, "AppState re-entered");
        return None;
    };
    Some(f(&mut state))
}

impl AppState {
    fn now(&self) -> SystemTime {
        match &self.fixture {
            Some(f) => f.base + f.started.elapsed(),
            None => SystemTime::now(),
        }
    }

    fn hidden(&self) -> bool {
        match &self.fixture {
            Some(f) => f.snap.hidden,
            None => self.store.hidden(),
        }
    }

    fn peeking(&self) -> bool {
        match &self.fixture {
            Some(f) => f.snap.peeking,
            None => self.store.peeking(),
        }
    }

    fn snapshot(&self) -> Snapshot {
        match &self.fixture {
            Some(f) => f.snap.clone(),
            None => self.store.snapshot(),
        }
    }

    /// Rebuilds every panel's scene; `why` is only for the frame log.
    fn render(&mut self, why: &str) {
        let (snap, now) = (self.snapshot(), self.now());
        for host in &mut self.panels {
            host.render(&snap, now, why);
        }
    }

    /// Expands under the pointer, collapses when it leaves, and only takes mouse events over the card (Swift `trackMouse`).
    fn track_mouse(&mut self) {
        let now = self.now();
        if self.hidden() {
            return;
        }
        let peeking = self.peeking();
        let mut changed = false;
        for host in &mut self.panels {
            let inside = host.pointer_inside();
            let want = host.state.pointer_wants(inside);
            let was = host.state.hovering;
            if host.state.editing.is_none() && want != was {
                host.state.hovering = want;
                if want {
                    self.store.peek_until = None;
                }
                changed = true;
            }
            if !host.state.hovering && host.state.keyboard && host.state.editing.is_none() {
                host.state.keyboard = false;
                changed = true;
            }
            let row = if inside && host.state.expanded(peeking) {
                host.row_under_pointer()
            } else {
                None
            };
            match (&row, host.state.hovered_row.clone()) {
                (Some(id), _) => host.state.note_row_hover(id, true, now),
                (None, Some(old)) => host.state.note_row_hover(&old, false, now),
                (None, None) => {}
            }
            // Only the card takes clicks, also while pinned open or editing: the rest of the canvas must reach the app beneath.
            if host.window.ignores_mouse() == inside {
                host.window.set_ignores_mouse(!inside);
            }
        }
        if changed {
            self.render("pointer");
        }
    }

    /// The 1 s tick: refresh the store, show or hide the panels, clear a dwelled mark, rebuild what changed.
    /// Returns true when a panel was dropped off its Space and the panels have to be synced again.
    fn on_tick(&mut self) -> bool {
        let now = self.now();
        self.pick_up_credentials();
        if self.fixture.is_none() {
            self.store.refresh(now);
            let (transitions, pr_chime) = (self.store.transitions.clone(), self.store.pr_chime);
            // The chime sound and the focused-pane check of the alert belong to phase 9.
            self.store
                .apply_alert(&transitions, pr_chime, &Default::default(), now);
            let store = &self.store;
            for host in &mut self.panels {
                if host
                    .state
                    .editing
                    .as_ref()
                    .is_some_and(|e| !store.allows(e))
                {
                    host.state.editing = None;
                }
            }
            settings::persist(
                &self.kv,
                &mut self.saved,
                settings::current(&self.store, self.pr_prompt.clone()),
            );
        }
        let snap = self.snapshot();
        let dropped = self.sync_visibility(&snap);
        self.track_mouse();
        let mut seen = Vec::new();
        for host in &mut self.panels {
            if let Some(id) = host
                .state
                .due_mark_seen(now, |id| snap.unseen.contains_key(id))
            {
                seen.push(Action::MarkSeen(id));
            }
        }
        for a in &seen {
            self.store.apply(a, now);
        }
        let snap = if seen.is_empty() {
            snap
        } else {
            self.snapshot()
        };
        let mut rebuilt = false;
        for host in &mut self.panels {
            let expanded = host.state.expanded(snap.peeking);
            if !seen.is_empty() || host.gate.needs_rebuild(&snap, expanded, now) {
                host.render(&snap, now, "tick");
                rebuilt = true;
            }
        }
        if rebuilt && frame_log() {
            eprintln!("tick rebuilt");
        }
        dropped
    }

    /// Shows or hides the panels with the store's `hidden`, and drops a panel that fell off the active Space.
    fn sync_visibility(&mut self, snap: &Snapshot) -> bool {
        for host in &self.panels {
            if snap.hidden && host.window.is_visible() {
                host.window.hide();
                host.window.set_ignores_mouse(true);
            } else if !snap.hidden && !host.window.is_visible() {
                host.window.show();
            }
        }
        // A `.canJoinAllSpaces` panel should always be on the active Space, but after a long run the WindowServer can drop
        // it from every Space while `isVisible` stays true; a new window brings it back. Only a panel seen on the active
        // Space counts, so a Space a new window cannot join either costs one rebuild, not one every few seconds.
        let mut dropped = false;
        let ticks = &mut self.off_space_ticks;
        self.panels.retain(|host| {
            if snap.hidden || !host.window.is_visible() {
                return true;
            }
            if host.window.is_on_active_space() {
                ticks.insert(host.key.clone(), 0);
                return true;
            }
            let Some(n) = ticks.get_mut(&host.key) else {
                return true;
            };
            *n += 1;
            if *n < OFF_SPACE_TICKS {
                return true;
            }
            host.window.close();
            ticks.remove(&host.key);
            dropped = true;
            false
        });
        dropped
    }

    fn pick_up_credentials(&mut self) {
        let Some(rx) = &self.credentials else { return };
        let Ok(found) = rx.try_recv() else { return };
        self.credentials = None;
        self.store.accounts = found.accounts;
        if let Some((account, token)) = found.bitbucket {
            self.store.pr_monitor = Some(PRMonitor::new(
                Arc::new(BitbucketClient::new(&account, &token)),
                true,
            ));
        }
        if let Some((account, token)) = found.jira {
            self.store.jira_monitor = Some(JiraMonitor::new(
                Arc::new(JiraClient::new(&account, &token)),
                true,
            ));
        }
    }
}

/// Creates panels for new displays, moves the others to their saved place and closes those of removed ones.
fn sync_panels(weak: &WeakState) {
    let Some(keyed) = screens::keyed_screens() else {
        return;
    };
    with_state(weak, |state| {
        state.panels.retain(|p| {
            let keep = keyed.iter().any(|s| s.key == p.key);
            if !keep {
                p.window.close();
                state.off_space_ticks.remove(&p.key);
            }
            keep
        });
        let placed = placement::sync(&state.kv, &keyed);
        let snap = state.snapshot();
        for (screen, placed) in keyed.iter().zip(&placed) {
            if let Some(host) = state.panels.iter_mut().find(|p| p.key == screen.key) {
                host.apply_placed(screen.frame, placed);
                continue;
            }
            eprintln!("screen {} frame {:?}", screen.key, screen.frame);
            let panel = state
                .fixture
                .as_ref()
                .map(|f| f.panel.clone())
                .unwrap_or_default();
            let on_event: Rc<dyn Fn(UiEvent)> = {
                let (weak, key) = (weak.clone(), screen.key.clone());
                Rc::new(move |event| handle_event(&weak, &key, event))
            };
            let on_resign = {
                let (weak, key) = (weak.clone(), screen.key.clone());
                move || {
                    let (weak, key) = (weak.clone(), key.clone());
                    main_queue::async_main(move || on_resign_key(&weak, &key));
                }
            };
            let Some(host) =
                PanelHost::new(screen, placed, panel, snap.clone(), on_event, on_resign)
            else {
                continue;
            };
            let clamp: claude_sessions_macos::ui::ClampHandler = {
                let (weak, key) = (weak.clone(), screen.key.clone());
                Rc::new(move |origin| {
                    with_state(&weak, |s| {
                        let host = s.panels.iter().find(|p| p.key == key)?;
                        Some(layout::clamp_drag(
                            origin,
                            layout::CANVAS,
                            host.visible,
                            host.state.anchor,
                            host.screen,
                        ))
                    })
                    .flatten()
                    .unwrap_or(origin)
                })
            };
            host.window.set_clamp(clamp);
            if !snap.hidden {
                host.window.show();
            }
            state.panels.push(host);
        }
        state.render("screens");
    });
}

fn handle_event(weak: &WeakState, key: &str, event: UiEvent) {
    match event {
        UiEvent::Click(id) => on_click(weak, key, id),
        UiEvent::Dragged {
            start,
            raw,
            clamped,
        } => {
            with_state(weak, |s| {
                let Some(host) = s.panels.iter_mut().find(|p| p.key == key) else {
                    return;
                };
                let placed = drag::settle(
                    &s.kv,
                    key,
                    (start, raw, clamped),
                    layout::CANVAS,
                    host.state.anchor,
                    host.screen,
                );
                let screen = host.screen;
                host.apply_placed(screen, &placed);
                s.render("drag");
                s.track_mouse();
            });
        }
        UiEvent::RightClick { at, .. } => {
            let Some(view) = with_state(weak, |s| {
                s.panels
                    .iter()
                    .find(|p| p.key == key)
                    .map(|p| p.window.view())
            })
            .flatten() else {
                return;
            };
            // phase 9 builds the real menu from the menu model; this one only quits.
            main_queue::async_main(move || {
                let items = [MenuItem {
                    title: "Thoát",
                    id: MENU_QUIT,
                }];
                menu::popup(&view, &items, at, |id| {
                    if id == MENU_QUIT {
                        mac_app::quit();
                    }
                });
            });
        }
    }
}

/// A click on a hit: an action goes through `Store::apply`, a panel change is applied here.
fn on_click(weak: &WeakState, key: &str, id: HitId) {
    let run = with_state(weak, |s| {
        let now = s.now();
        let pr_prompt = s.pr_prompt.clone();
        let host = s.panels.iter_mut().find(|p| p.key == key)?;
        let target = host.targets.get(id)?.clone();
        // The text of the field the click belongs to is read by phase 9 with the actions that send it.
        let click = target.click(&ClickEnv {
            snap: &host.snapshot,
            panel: &host.state,
            now,
            text: "",
            pr_prompt: &pr_prompt,
        });
        match click {
            Click::Stale => None,
            Click::Panel(change) => {
                let snap = host.snapshot.clone();
                change.apply(&mut host.state, &snap, now);
                s.render("click");
                None
            }
            Click::Action(a) => match s.store.apply(&a, now) {
                Effect::Run(a) => {
                    s.render("click");
                    Some(a)
                }
                Effect::Beep => {
                    beep();
                    None
                }
                Effect::Nothing => None,
            },
        }
    })
    .flatten();
    schedule_key_sync(weak);
    if let Some(action) = run {
        let weak = weak.clone();
        main_queue::async_main(move || {
            with_state(&weak, |s| s.run_action(&action));
        });
    }
}

impl AppState {
    /// An action whose effect lives outside the store. phase 9: cmux jump, keystrokes, socket answers, links, resume and the
    /// token dialog. Until then the send is marked failed and nothing is sent to any session.
    fn run_action(&mut self, action: &Action) {
        let now = self.now();
        eprintln!("action (phase 9, not run): {}", action_name(action));
        if let Action::Jump(id) = action {
            self.store.mark_seen(id);
        }
        if let Some(k) = action.send_key() {
            self.store.finish_send(&k, false, now);
        }
        self.render("action");
    }
}

/// The variant name only: actions can carry typed text.
fn action_name(a: &Action) -> &'static str {
    match a {
        Action::Jump(_) => "Jump",
        Action::Approve { .. } => "Approve",
        Action::Deny { .. } => "Deny",
        Action::OpenLink(..) => "OpenLink",
        Action::AskClaude { .. } => "AskClaude",
        Action::Reply { .. } => "Reply",
        Action::Stop(_) => "Stop",
        Action::Answer { .. } => "Answer",
        Action::NudgeRabbit(..) => "NudgeRabbit",
        Action::Resume(_) => "Resume",
        Action::Connect(_) => "Connect",
        Action::MarkSeen(_) => "MarkSeen",
    }
}

/// Makes each panel's key status and key monitor match its state, on a later main-queue turn: a panel that wants keys takes
/// them; one that stopped wanting them hands them back with `orderOut` + `orderFrontRegardless` (Swift `main.swift:77-108`).
fn schedule_key_sync(weak: &WeakState) {
    let weak = weak.clone();
    main_queue::async_main(move || {
        enum Step {
            Take,
            HandBack,
        }
        let steps: Vec<(WidgetPanelHandle, Step)> = with_state(&weak, |s| {
            let hidden = s.hidden();
            let mut steps = Vec::new();
            for host in &mut s.panels {
                if host.state.keyboard && host.key_monitor.is_none() {
                    host.key_monitor = make_key_monitor(&weak, &host.key);
                } else if !host.state.keyboard {
                    host.key_monitor = None;
                }
                if host.state.wants_key() {
                    if !hidden && !host.window.is_key() {
                        steps.push((host.window.clone(), Step::Take));
                    }
                } else if host
                    .state
                    .should_hand_back_key(host.window.is_key(), hidden)
                {
                    steps.push((host.window.clone(), Step::HandBack));
                }
            }
            steps
        })
        .unwrap_or_default();
        for (window, step) in steps {
            match step {
                Step::Take => keys::enter_keyboard(&window),
                Step::HandBack => keys::hand_back(&window),
            }
        }
    });
}

/// The local key monitor of a panel in keyboard mode. It answers synchronously from the panel's snapshot; an action it
/// produces is applied on a later turn.
fn make_key_monitor(weak: &WeakState, key: &str) -> Option<KeyMonitor> {
    let (weak, key) = (weak.clone(), key.to_string());
    KeyMonitor::new(move |info: &KeyInfo| {
        let outcome = with_state(&weak, |s| {
            let host = s.panels.iter_mut().find(|p| p.key == key)?;
            let outcome = keyboard_mode::handle(
                info.key_code,
                &info.chars,
                info.flags,
                &host.snapshot,
                &mut host.state,
            );
            if matches!(outcome, KeyOutcome::Consumed(_)) {
                s.render("key");
            }
            Some(outcome)
        })
        .flatten();
        match outcome {
            None | Some(KeyOutcome::PassThrough) => false,
            Some(KeyOutcome::Beep) => {
                beep();
                true
            }
            Some(KeyOutcome::Consumed(action)) => {
                let weak = weak.clone();
                main_queue::async_main(move || {
                    if let Some(a) = action {
                        apply_action(&weak, &a);
                    }
                    schedule_key_sync(&weak);
                });
                true
            }
        }
    })
}

/// `Store::apply` then the effect, as a click does.
fn apply_action(weak: &WeakState, action: &Action) {
    with_state(weak, |s| match s.store.apply(action, s.now()) {
        Effect::Run(a) => s.run_action(&a),
        Effect::Beep => beep(),
        Effect::Nothing => s.render("action"),
    });
}

/// The panel lost key status: editing and keyboard mode end, as Swift's `didResignKey` sink does.
fn on_resign_key(weak: &WeakState, key: &str) {
    with_state(weak, |s| {
        if let Some(host) = s.panels.iter_mut().find(|p| p.key == key) {
            host.state.editing = None;
            host.state.keyboard = false;
        }
        s.track_mouse();
        s.render("resign");
    });
    schedule_key_sync(weak);
}

/// `⌃⌥Space`: closes an open card; otherwise opens and pins the one on the screen with the mouse and takes the keyboard.
fn toggle_open(weak: &WeakState) {
    let ok = with_state(weak, |s| {
        if s.hidden() {
            return true;
        }
        let open: Vec<usize> = (0..s.panels.len())
            .filter(|&i| s.panels[i].state.pinned_open || s.panels[i].state.hovering)
            .collect();
        let states: Vec<&PanelState> = open.iter().map(|&i| &s.panels[i].state).collect();
        match PanelState::hotkey_toggle(&states) {
            HotkeyToggle::Close => {
                for host in &mut s.panels {
                    host.state.close();
                }
            }
            HotkeyToggle::Keyboard => s.panels[open[0]].state.pin_with_keyboard(),
            HotkeyToggle::Open => {
                let at = mouse::pointer();
                let i = s
                    .panels
                    .iter()
                    .position(|p| p.screen.contains(at))
                    .or((!s.panels.is_empty()).then_some(0));
                let Some(i) = i else { return false };
                s.panels[i].state.pin_with_keyboard();
            }
        }
        s.render("hotkey");
        s.track_mouse();
        true
    })
    .unwrap_or(true);
    if !ok {
        beep();
    }
    schedule_key_sync(weak);
}

/// `⌃⌥→`: the session that has waited longest; presses within 5 s move on. Sessions outside cmux are skipped.
fn jump_to_next_waiting(weak: &WeakState) {
    let picked = with_state(weak, |s| {
        let now = SystemTime::now();
        let store = &s.store;
        let (order, first) = advance(
            s.hotkeys.last_next.as_ref(),
            &store.waiting_order(),
            |id| store.pid_of(id).is_some(),
            now,
        )?;
        s.hotkeys.last_next = Some(claude_sessions_core::hotkeys::Cycle {
            order,
            id: first.clone(),
            at: now,
        });
        Some(first)
    })
    .flatten();
    match picked {
        Some(id) => apply_action(weak, &Action::Jump(id)),
        None => beep(),
    }
}

/// Registers both hotkeys with the saved choices.
fn register_hotkeys(weak: &WeakState) {
    with_state(weak, |s| {
        for (which, run) in [
            (Which::Open, toggle_open as fn(&WeakState)),
            (Which::Next, jump_to_next_waiting as fn(&WeakState)),
        ] {
            let weak = weak.clone();
            s.hotkeys.register(which, move || {
                let weak = weak.clone();
                main_queue::async_main(move || run(&weak));
            });
        }
        for name in &s.hotkeys.failed {
            eprintln!("hotkey {name} is used by another app");
        }
    });
}

struct Running {
    _shared: Shared,
    _observers: (
        Option<NotificationObserver>,
        Option<MouseMonitors>,
        Option<Ticker>,
    ),
}

fn start() -> Running {
    mac_app::set_accessory_policy();
    menu::install_main_menu();
    let fixture = load_fixture();
    let root = std::env::var_os("CLAUDE_SESSIONS_ROOT").map(PathBuf::from);
    let live = fixture.is_none() && root.is_none();
    let kv = UserDefaultsStore::new(None).expect("UserDefaults on the main thread");
    let mut store = match &fixture {
        Some(_) => make_store(Some(std::env::temp_dir().join("claude-sessions-no-root"))),
        None => make_store(root),
    };
    let saved = settings::load(&kv, &mut store);
    if live {
        let socket = home().join(".local/state/cmux/last-socket-path");
        if let Some(transport) = UnixFeedTransport::new(&socket) {
            store.feed_monitor = Some(FeedMonitor::new(Arc::new(transport)));
        }
    }
    let hotkeys = Hotkeys::load(&kv);
    let shared: Shared = Rc::new(RefCell::new(AppState {
        store,
        panels: Vec::new(),
        hotkeys,
        off_space_ticks: HashMap::new(),
        fixture,
        pr_prompt: saved.pr_prompt.clone(),
        saved,
        credentials: live.then(read_credentials),
        kv,
    }));
    let weak = Rc::downgrade(&shared);
    {
        let state = shared.borrow();
        let screens = screens::keyed_screens().unwrap_or_default();
        placement::migrate_legacy(&state.kv, &screens);
    }
    sync_panels(&weak);

    let screens_observer = screens::observe_changes({
        let weak = weak.clone();
        move || {
            sync_panels(&weak);
            // NSScreen.screens can be stale while the notification fires.
            let weak = weak.clone();
            main_queue::after(0.5, move || sync_panels(&weak));
        }
    });
    let monitors = mouse::observe_moves({
        let weak = weak.clone();
        move || {
            with_state(&weak, AppState::track_mouse);
        }
    });
    register_hotkeys(&weak);
    let ticker = main_queue::every(1.0, {
        let weak = weak.clone();
        move || {
            if with_state(&weak, |s| s.on_tick()) == Some(true) {
                sync_panels(&weak);
            }
        }
    });
    // The first refresh does not wait for the first tick.
    if with_state(&weak, |s| s.on_tick()) == Some(true) {
        sync_panels(&weak);
    }
    Running {
        _shared: shared,
        _observers: (screens_observer, monitors, ticker),
    }
}

pub fn run() {
    let _running = start();
    mac_app::run();
}
