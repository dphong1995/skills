//! One panel per display: creation, placement, the scene it shows and the pointer test (`syncPanels`, `trackMouse`).

use std::collections::HashMap;
use std::time::SystemTime;

use crate::fields;
use crate::hits::Targets;
use crate::placement::Placed;
use crate::rebuild::RebuildGate;
use crate::views::{View, build};
use claude_sessions_core::layout::{self, CANVAS};
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::scene::{Point, Rect, Scene, Size};
use claude_sessions_core::store::Snapshot;
use claude_sessions_macos::NotificationObserver;
use claude_sessions_macos::defaults::clock_text;
use claude_sessions_macos::keys::{self, KeyMonitor};
use claude_sessions_macos::mouse;
use claude_sessions_macos::screens::KeyedScreen;
use claude_sessions_macos::text::CoreTextMeasure;
use claude_sessions_macos::ui::{EventHandler, WidgetPanelHandle};

pub struct PanelHost {
    /// The display UUID (`"main"` when none could be read).
    pub key: String,
    pub window: WidgetPanelHandle,
    /// The screen frame the pill is placed on (AppKit coordinates).
    pub screen: Rect,
    pub state: PanelState,
    /// What the last scene was built from; keyboard handling decides on it.
    pub snapshot: Snapshot,
    /// The scene placed in the canvas the window covers.
    pub scene: Scene,
    /// Size of the pill or card alone (`Layout.size`).
    pub visible: Size,
    pub targets: Targets,
    /// Whether the last scene was the card; a change to the pill is a collapse.
    expanded: bool,
    pub gate: RebuildGate,
    pub key_monitor: Option<KeyMonitor>,
    _resign: NotificationObserver,
}

/// `scene` moved to where `anchor` puts a pill or card of its size in the canvas, and sized to the canvas.
pub fn place_scene(mut scene: Scene, anchor: layout::Anchor) -> Scene {
    let canvas = Rect::from_origin_size(Point::new(0.0, 0.0), CANVAS);
    let r = layout::pill(canvas, scene.size, anchor);
    // AppKit's y runs up, the scene's down.
    let (dx, dy) = (r.x, canvas.max_y() - r.max_y());
    for node in &mut scene.nodes {
        node.frame.x += dx;
        node.frame.y += dy;
    }
    scene.size = CANVAS;
    scene
}

/// Whether `local` (scene points) is over the pill or the card: the first node is the card's background or the pill, and the
/// scrolled card frames lie past the viewport, so they must not be tested.
pub fn inside_widget(scene: &Scene, local: Point) -> bool {
    scene.nodes.first().is_some_and(|n| n.frame.contains(local))
}

/// The session whose card contains `local` (scene points). Cards after the scroll node live in the scrolled content: they only
/// answer inside the viewport, at `local` moved down by the scroll `offset` (`SceneView::scroll_offset`).
pub fn row_at(scene: &Scene, local: Point, offset: f64) -> Option<String> {
    let scrolled_from = scene.nodes.iter().position(|n| n.scroll.is_some());
    scene.nodes.iter().enumerate().find_map(|(i, node)| {
        let id = node.key.0.strip_prefix("card-")?;
        let at = match scrolled_from {
            Some(from) if i > from => {
                if !scene.nodes[from].frame.contains(local) {
                    return None;
                }
                Point::new(local.x, local.y + offset)
            }
            _ => local,
        };
        node.frame.contains(at).then(|| id.to_string())
    })
}

impl PanelHost {
    /// A hidden panel at `placed` on `screen`.
    pub fn new(
        screen: &KeyedScreen,
        placed: &Placed,
        state: PanelState,
        snapshot: Snapshot,
        on_event: EventHandler,
        on_resign_key: impl Fn() + 'static,
    ) -> Option<Self> {
        let window = WidgetPanelHandle::new(placed.frame, on_event)?;
        let resign = keys::observe_resign_key(&window, on_resign_key);
        let mut host = Self {
            key: screen.key.clone(),
            window,
            screen: screen.frame,
            state,
            snapshot,
            scene: Scene::default(),
            visible: Size::default(),
            targets: Targets::default(),
            expanded: false,
            gate: RebuildGate::default(),
            key_monitor: None,
            _resign: resign,
        };
        host.state.anchor = placed.anchor;
        Some(host)
    }

    /// Moves the panel to `placed` on `screen`.
    pub fn apply_placed(&mut self, screen: Rect, placed: &Placed) {
        self.screen = screen;
        self.state.anchor = placed.anchor;
        if self.window.frame() != placed.frame {
            self.window.set_frame(placed.frame);
        }
    }

    /// Builds the scene for `snap` and gives it to the renderer; `drafts` fill its text fields, `why` is only for the frame log.
    pub fn render(
        &mut self,
        snap: &Snapshot,
        drafts: &HashMap<String, String>,
        now: SystemTime,
        why: &str,
    ) {
        if crate::app::frame_log() {
            eprintln!("rebuild {} {why}", self.key);
        }
        let expanded = self.state.expanded(snap.peeking);
        if self.expanded && !expanded {
            // The widget collapsed: link menus and the `Vừa đóng` group close, answers of gone questions drop.
            let live: Vec<_> = snap
                .sessions
                .iter()
                .filter_map(|s| s.question.clone())
                .collect();
            self.state.on_collapse();
            self.state.prune_questions(&live);
        }
        self.expanded = expanded;
        let old: Vec<String> = self
            .snapshot
            .sessions
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let new: Vec<String> = snap.sessions.iter().map(|s| s.id.clone()).collect();
        if old != new {
            self.state.on_order_change(&old, &new);
        }
        let mut built = build(&View {
            snap,
            panel: &self.state,
            measure: &CoreTextMeasure,
            now,
            clock: &|t| {
                clock_text(
                    t.duration_since(SystemTime::UNIX_EPOCH)
                        .map_or(0.0, |d| d.as_secs_f64()),
                )
            },
        });
        fields::fill(&mut built.scene, drafts);
        self.visible = built.scene.size;
        let scene = place_scene(built.scene, self.state.anchor);
        self.window.set_scene(&scene);
        self.scene = scene;
        self.targets = built.targets;
        self.snapshot = snap.clone();
        self.gate.built(snap, &self.state, expanded, now);
    }

    /// Whether the pointer (screen points, bottom-left origin) is over the pill or the card.
    pub fn pointer_inside(&self) -> bool {
        self.local(mouse::pointer())
            .is_some_and(|local| inside_widget(&self.scene, local))
    }

    /// The session whose card is under the pointer.
    pub fn row_under_pointer(&self) -> Option<String> {
        let local = self.local(mouse::pointer())?;
        row_at(&self.scene, local, self.window.view().scroll_offset())
    }

    fn local(&self, pointer: Point) -> Option<Point> {
        let f = self.window.frame();
        f.contains(pointer).then_some(Point {
            x: pointer.x - f.x,
            y: f.y + f.h - pointer.y,
        })
    }
}
