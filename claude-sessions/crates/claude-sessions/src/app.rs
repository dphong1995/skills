//! App state of the gate: the panels, the 1 s tick, the hotkey and the fake interactions.
//!
//! `AppState` lives in `Rc<RefCell<..>>` on the main thread. Every AppKit callback borrows, updates and releases;
//! anything that spins a nested run loop (menu, alert) or re-enters callbacks (`makeKeyWindow`, `orderOut`) runs on a
//! later main-queue turn with no borrow held.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::SystemTime;

use claude_sessions_core::layout;
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::scene::{HitId, Scene};
use claude_sessions_core::store::Snapshot;
use claude_sessions_macos::keys::{self, KEY_CODE_ESCAPE, KeyMonitor};
use claude_sessions_macos::main_queue::{self, Ticker};
use claude_sessions_macos::menu::{self, MenuItem};
use claude_sessions_macos::mouse::{self, MouseMonitors};
use claude_sessions_macos::text::CoreTextMeasure;
use claude_sessions_macos::ui::{UiEvent, WidgetPanelHandle};
use claude_sessions_macos::{NotificationObserver, alert, app as mac_app, hotkeys, screens};

use crate::panels::PanelHost;
use crate::views::{View, build};

type Shared = Rc<RefCell<AppState>>;
type WeakState = Weak<RefCell<AppState>>;

const HOTKEY_OPEN: u32 = 1;
const MENU_LOG: isize = 1;
const MENU_HIDE_SHOW: isize = 2;
const MENU_TOKEN: isize = 3;

struct AppState {
    panels: Vec<PanelHost>,
    snap: Snapshot,
    panel: PanelState,
    now: SystemTime,
}

/// The fixture state named by `CLAUDE_SESSIONS_FIXTURE` (default `07-mixed-hover`), built in a temp dir that is removed
/// before returning.
#[cfg(feature = "fixtures")]
fn initial_state() -> (Snapshot, PanelState, SystemTime) {
    use claude_sessions_core::fixtures::{STATES, fixed_now, view_now};
    let want = std::env::var("CLAUDE_SESSIONS_FIXTURE").unwrap_or_else(|_| "07-mixed-hover".into());
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
    (snap, panel, view_now(name, now))
}

/// Without the `fixtures` feature the gate app shows an empty list until the store is wired in.
#[cfg(not(feature = "fixtures"))]
fn initial_state() -> (Snapshot, PanelState, SystemTime) {
    let snap = Snapshot {
        sessions: Vec::new(),
        closed: Vec::new(),
        quota: None,
        unseen: Default::default(),
        sends: Default::default(),
        shown_commands: Default::default(),
        surface_misses: Default::default(),
        connections: Default::default(),
        issue_icons: Default::default(),
        pr_last_success: None,
        pr_stale: false,
        hidden: false,
        peeking: false,
        muted: false,
        bg_opacity: 0.92,
    };
    (snap, PanelState::new(), SystemTime::now())
}

/// The scene of a view, moved to the top-right corner of the canvas the panel window covers.
fn place(mut scene: Scene) -> Scene {
    let dx = layout::CANVAS.w - layout::MARGIN - scene.size.w;
    for node in &mut scene.nodes {
        node.frame.x += dx;
        node.frame.y += layout::MARGIN;
    }
    scene.size = layout::CANVAS;
    scene
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
    fn render(&mut self) {
        for host in &mut self.panels {
            let mut panel = self.panel.clone();
            panel.hovering = host.expanded;
            let scene = place(
                build(&View {
                    snap: &self.snap,
                    panel: &panel,
                    measure: &CoreTextMeasure,
                    now: self.now,
                    clock: &|_| "09:41".to_string(),
                })
                .scene,
            );
            host.window.set_scene(&scene);
            host.scene = scene;
        }
    }

    /// Expands under the pointer, collapses when it leaves, and only takes mouse events over the card.
    fn track_mouse(&mut self) {
        let mut changed = false;
        for host in &mut self.panels {
            let want = host.pointer_inside() || host.keyboard;
            if want != host.expanded {
                host.expanded = want;
                changed = true;
            }
        }
        if changed {
            self.render();
        }
        for host in &self.panels {
            let inside = host.pointer_inside();
            if host.window.ignores_mouse() == inside {
                host.window.set_ignores_mouse(!inside);
            }
        }
    }

    fn on_tick(&mut self) {
        self.track_mouse();
    }
}

fn log_props(host: &PanelHost, when: &str) {
    eprintln!("panel {} {when}: {:?}", host.key, host.window.props());
    eprintln!("panel {} frame: {:?}", host.key, host.window.frame());
}

/// Creates panels for new displays and closes those of removed ones.
fn sync_panels(shared: &Shared) {
    let Some(keyed) = screens::keyed_screens() else {
        return;
    };
    let weak = Rc::downgrade(shared);
    let mut state = shared.borrow_mut();
    state.panels.retain(|p| {
        let keep = keyed.iter().any(|s| s.key == p.key);
        if !keep {
            p.window.close();
        }
        keep
    });
    for screen in &keyed {
        if state.panels.iter().any(|p| p.key == screen.key) {
            continue;
        }
        eprintln!("screen {} frame {:?}", screen.key, screen.frame);
        let on_event: Rc<dyn Fn(UiEvent)> = {
            let (weak, key) = (weak.clone(), screen.key.clone());
            Rc::new(move |event| handle_event(&weak, &key, event))
        };
        let on_resign = {
            let (weak, key) = (weak.clone(), screen.key.clone());
            move || {
                main_queue::async_main({
                    let (weak, key) = (weak.clone(), key.clone());
                    move || leave_keyboard(&weak, &key)
                })
            }
        };
        let Some(host) = PanelHost::new(screen, on_event, on_resign) else {
            continue;
        };
        host.window.show();
        log_props(&host, "created");
        state.panels.push(host);
    }
    state.render();
}

fn handle_event(weak: &WeakState, key: &str, event: UiEvent) {
    match event {
        UiEvent::Click(HitId(id)) => eprintln!("click hit {id}"),
        UiEvent::Dragged => {
            eprintln!("dragged");
            with_state(weak, AppState::track_mouse);
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
            let weak = weak.clone();
            main_queue::async_main(move || {
                let items = [
                    MenuItem {
                        title: "Ghi log",
                        id: MENU_LOG,
                    },
                    MenuItem {
                        title: "Ẩn rồi hiện lại",
                        id: MENU_HIDE_SHOW,
                    },
                    MenuItem {
                        title: "Nhập token…",
                        id: MENU_TOKEN,
                    },
                ];
                menu::popup(&view, &items, at, move |id| on_menu_pick(&weak, id));
            });
        }
    }
}

fn on_menu_pick(weak: &WeakState, id: isize) {
    match id {
        MENU_LOG => eprintln!("menu: log item"),
        MENU_HIDE_SHOW => {
            with_state(weak, |s| {
                for host in &s.panels {
                    host.window.hide();
                }
                for host in &s.panels {
                    host.window.show();
                    log_props(host, "after hide/show");
                }
            });
        }
        MENU_TOKEN => main_queue::async_main(|| {
            // Only the length is logged: the text may be a token.
            match alert::ask_secret("Token", "Nhập giá trị thử (không dùng token thật)")
            {
                Some(text) => eprintln!("alert: OK, {} characters", text.chars().count()),
                None => eprintln!("alert: cancelled"),
            }
        }),
        _ => {}
    }
}

/// `⌃⌥Space`: expand the first panel and give it the keyboard.
fn enter_keyboard_mode(weak: &WeakState) {
    let window = with_state(weak, |s| {
        let host = s.panels.first_mut()?;
        host.keyboard = true;
        host.expanded = true;
        let window = host.window.clone();
        let key = host.key.clone();
        s.render();
        Some((window, key))
    })
    .flatten();
    let Some((window, key)) = window else { return };
    keys::enter_keyboard(&window);
    let monitor = {
        let (weak, key) = (weak.clone(), key.clone());
        KeyMonitor::new(move |code| {
            if code != KEY_CODE_ESCAPE {
                return false;
            }
            let (weak, key) = (weak.clone(), key.clone());
            main_queue::async_main(move || leave_keyboard(&weak, &key));
            true
        })
    };
    with_state(weak, |s| {
        if let Some(host) = s.panels.iter_mut().find(|p| p.key == key) {
            host.key_monitor = monitor;
        }
    });
}

/// Esc, or the panel lost key status: back to hover mode and hand the keys back.
fn leave_keyboard(weak: &WeakState, key: &str) {
    let window = with_state(weak, |s| {
        let host = s.panels.iter_mut().find(|p| p.key == key)?;
        if !host.keyboard {
            return None;
        }
        host.keyboard = false;
        host.key_monitor = None;
        let window: WidgetPanelHandle = host.window.clone();
        s.track_mouse();
        Some(window)
    })
    .flatten();
    if let Some(window) = window {
        keys::hand_back(&window);
    }
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
    let (snap, panel, now) = initial_state();
    let shared: Shared = Rc::new(RefCell::new(AppState {
        panels: Vec::new(),
        snap,
        panel,
        now,
    }));
    sync_panels(&shared);

    let weak = Rc::downgrade(&shared);
    let screens_observer = screens::observe_changes({
        let shared = shared.clone();
        move || {
            sync_panels(&shared);
            // NSScreen.screens can be stale while the notification fires.
            let shared = shared.clone();
            main_queue::after(0.5, move || sync_panels(&shared));
        }
    });
    let monitors = mouse::observe_moves({
        let weak = weak.clone();
        move || {
            with_state(&weak, AppState::track_mouse);
        }
    });
    let ticker = main_queue::every(1.0, {
        let weak = weak.clone();
        move || {
            with_state(&weak, AppState::on_tick);
        }
    });
    let open = {
        let weak = weak.clone();
        move || {
            let weak = weak.clone();
            main_queue::async_main(move || enter_keyboard_mode(&weak));
        }
    };
    if !hotkeys::register(
        HOTKEY_OPEN,
        hotkeys::KEY_SPACE,
        hotkeys::CONTROL | hotkeys::OPTION,
        open,
    ) {
        eprintln!("hotkey ⌃⌥Space is used by another app");
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
