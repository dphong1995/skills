//! The view that shows a `Scene`: one `NodeView` per node key, diffed on every new scene, plus mouse hit-testing.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use claude_sessions_core::scene::{Cursor, HitId, NodeKey, Point, Scene, changed_nodes};
use objc2::Message;
use objc2::rc::Retained;
use objc2::rc::Weak;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSAnimatablePropertyContainer, NSCursor, NSEvent, NSTextField, NSView};
use objc2_app_kit::{NSScrollView, NSScrollerStyle, NSWindowOrderingMode};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::draw::{ns_rect, register_images};
use super::motion;
use super::node_view::NodeView;
use super::{ClampHandler, EventHandler, UiEvent};

/// Pointer travel (points) before a press becomes a drag, as Swift's `Layout.isDrag`.
const DRAG_THRESHOLD: f64 = 4.0;

#[derive(Clone, Copy)]
struct Drag {
    mouse0: NSPoint,
    origin0: NSPoint,
    moving: bool,
    /// The origin the pointer asked for and the one the window got after the clamp hook.
    raw: NSPoint,
    clamped: NSPoint,
}

pub struct SceneIvars {
    scene: RefCell<Scene>,
    nodes: RefCell<HashMap<NodeKey, Retained<NodeView>>>,
    handler: RefCell<Option<EventHandler>>,
    /// Maps the window origin a drag asks for to the one it gets (keeps the card on its screen).
    clamp: RefCell<Option<ClampHandler>>,
    drag: Cell<Option<Drag>>,
    /// The panel is on screen: only then may a spinner turn. Starts false until the panel is shown.
    live: Cell<bool>,
    reduce_observer: RefCell<Option<crate::NotificationObserver>>,
    /// The scroll host of the scene's scroll node, while it has one.
    scroll: RefCell<Option<ScrollHost>>,
}

/// The `NSScrollView` for a scroll node and its flipped document view, which holds the scrolled nodes.
struct ScrollHost {
    view: Retained<NSScrollView>,
    doc: Retained<NodeView>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = SceneIvars]
    pub struct SceneView;

    unsafe impl NSObjectProtocol for SceneView {}

    impl SceneView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            let Some(window) = self.window() else { return };
            self.ivars().drag.set(Some(Drag {
                mouse0: NSEvent::mouseLocation(),
                origin0: window.frame().origin,
                moving: false,
                raw: window.frame().origin,
                clamped: window.frame().origin,
            }));
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, _event: &NSEvent) {
            let (Some(window), Some(mut drag)) = (self.window(), self.ivars().drag.get()) else {
                return;
            };
            let now = NSEvent::mouseLocation();
            let (dx, dy) = (now.x - drag.mouse0.x, now.y - drag.mouse0.y);
            if !drag.moving && dx.hypot(dy) >= DRAG_THRESHOLD {
                drag.moving = true;
                self.ivars().drag.set(Some(drag));
            }
            if drag.moving {
                let raw = Point {
                    x: drag.origin0.x + dx,
                    y: drag.origin0.y + dy,
                };
                let clamp = self.ivars().clamp.borrow().clone();
                let clamped = clamp.map_or(raw, |c| c(raw));
                drag.raw = NSPoint::new(raw.x, raw.y);
                drag.clamped = NSPoint::new(clamped.x, clamped.y);
                self.ivars().drag.set(Some(drag));
                window.setFrameOrigin(drag.clamped);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let Some(drag) = self.ivars().drag.take() else { return };
            if drag.moving {
                let p = |p: NSPoint| Point { x: p.x, y: p.y };
                self.emit(UiEvent::Dragged {
                    start: p(drag.origin0),
                    raw: p(drag.raw),
                    clamped: p(drag.clamped),
                });
            } else if let Some(id) = self.hit_at(self.local_point(event)) {
                self.emit(UiEvent::Click(id));
            }
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            let at = self.local_point(event);
            self.emit(UiEvent::RightClick {
                hit: self.hit_at(at),
                at,
            });
        }

        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            let scene = self.ivars().scene.borrow();
            // Scrolled nodes keep the arrow: their cursor rects would have to live in the scrolled document.
            let scrolled_from = scene.nodes.iter().position(|n| n.scroll.is_some()).unwrap_or(usize::MAX);
            for (i, node) in scene.nodes.iter().enumerate() {
                if i >= scrolled_from {
                    continue;
                }
                for hit in node.hits.iter().filter(|h| h.cursor == Cursor::Pointer) {
                    let mut rect = hit.rect;
                    rect.x += node.frame.x;
                    rect.y += node.frame.y;
                    self.addCursorRect_cursor(ns_rect(rect), &NSCursor::pointingHandCursor());
                }
            }
        }
    }
);

impl SceneView {
    pub fn new(mtm: MainThreadMarker, size: NSSize) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(SceneIvars {
            scene: RefCell::new(Scene::default()),
            nodes: RefCell::new(HashMap::new()),
            handler: RefCell::new(None),
            clamp: RefCell::new(None),
            drag: Cell::new(None),
            live: Cell::new(false),
            reduce_observer: RefCell::new(None),
            scroll: RefCell::new(None),
        });
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setWantsLayer(true);
        this
    }

    /// Re-applies the motion rules whenever the system's Reduce Motion setting changes.
    pub fn observe_reduce_motion(this: &Retained<Self>) {
        let weak = Weak::new(&**this);
        let observer = crate::accessibility::observe_reduce_motion(move || {
            if let Some(view) = weak.load() {
                view.apply_motion();
            }
        });
        *this.ivars().reduce_observer.borrow_mut() = Some(observer);
    }

    /// The panel went on or off screen: a hidden panel keeps no looping animation (spinners stop, pulses are dropped).
    pub fn set_motion_live(&self, live: bool) {
        self.ivars().live.set(live);
        self.apply_motion();
    }

    /// Spinners turn only while the panel is on screen and Reduce Motion is off; pulses go with either.
    fn apply_motion(&self) {
        let spin = self.ivars().live.get() && motion::enabled();
        for view in self.ivars().nodes.borrow().values() {
            view.set_spin(spin);
            if !spin {
                view.clear_pulses();
            }
        }
    }

    pub fn set_clamp(&self, clamp: ClampHandler) {
        *self.ivars().clamp.borrow_mut() = Some(clamp);
    }

    pub fn set_handler(&self, handler: EventHandler) {
        *self.ivars().handler.borrow_mut() = Some(handler);
    }

    /// How far the scrolled list is scrolled: the clip view's bounds origin y, 0 while the scene has no scroll host.
    pub fn scroll_offset(&self) -> f64 {
        self.ivars()
            .scroll
            .borrow()
            .as_ref()
            .map_or(0.0, |s| s.view.contentView().bounds().origin.y)
    }

    /// The first native text field in the scene, to give the keyboard to.
    pub fn first_field(&self) -> Option<Retained<NSTextField>> {
        self.ivars()
            .nodes
            .borrow()
            .values()
            .find_map(|n| n.first_field())
    }

    fn emit(&self, event: UiEvent) {
        let handler = self.ivars().handler.borrow().clone();
        if let Some(handler) = handler {
            handler(event);
        }
    }

    fn local_point(&self, event: &NSEvent) -> Point {
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        Point { x: p.x, y: p.y }
    }

    fn hit_at(&self, at: Point) -> Option<HitId> {
        let scene = self.ivars().scene.borrow();
        let scrolled_from = scene
            .nodes
            .iter()
            .position(|n| n.scroll.is_some())
            .unwrap_or(usize::MAX);
        let offset = self.scroll_offset();
        let viewport = scene.nodes.get(scrolled_from).map(|n| n.frame);
        scene.nodes.iter().enumerate().rev().find_map(|(i, node)| {
            if i == scrolled_from {
                return None;
            }
            // A scrolled node answers only inside the viewport, at the scrolled position.
            let shift = if i > scrolled_from {
                if !viewport.is_some_and(|v| v.contains(at)) {
                    return None;
                }
                offset
            } else {
                0.0
            };
            let local = Point {
                x: at.x - node.frame.x,
                y: at.y - node.frame.y + shift,
            };
            node.hits
                .iter()
                .find(|h| h.rect.contains(local))
                .map(|h| h.id)
        })
    }

    /// Creates, moves or keeps the `NSScrollView` of the scene's scroll node (`scroll_at`); an absent node keeps no host for the
    /// caller to drop once its nodes are back in this view.
    fn sync_scroll(&self, scene: &Scene, scroll_at: Option<usize>) {
        let Some(node) = scroll_at.map(|i| &scene.nodes[i]) else {
            return;
        };
        let Some(content) = node.scroll else { return };
        let mut slot = self.ivars().scroll.borrow_mut();
        let host = slot.get_or_insert_with(|| {
            let mtm = self.mtm();
            let view = NSScrollView::initWithFrame(mtm.alloc(), ns_rect(node.frame));
            view.setDrawsBackground(false);
            view.setHasVerticalScroller(true);
            view.setHasHorizontalScroller(false);
            view.setAutohidesScrollers(true);
            view.setScrollerStyle(NSScrollerStyle::Overlay);
            let doc = NodeView::new(mtm, "scroll-document", node.frame);
            view.setDocumentView(Some(&doc));
            self.addSubview(&view);
            ScrollHost { view, doc }
        });
        host.view.setFrame(ns_rect(node.frame));
        host.doc.setFrameSize(NSSize::new(content.w, content.h));
    }

    /// Shows `scene`, touching only what differs from the previous one: unchanged nodes do nothing, changed commands
    /// redraw that node alone, changed frames animate in Core Animation, new and removed nodes fade.
    pub fn set_scene(&self, scene: &Scene) {
        let mtm = self.mtm();
        let animate = motion::enabled();
        let spin = animate && self.ivars().live.get();
        let ivars = self.ivars();
        register_images(&scene.images);
        let old = ivars.scene.replace(scene.clone());
        let diff = changed_nodes(&old, scene);
        let scroll_at = scene.nodes.iter().position(|n| n.scroll.is_some());
        self.sync_scroll(scene, scroll_at);
        let top = scroll_at.map_or(0.0, |i| scene.nodes[i].frame.y);
        let scrolled: std::collections::HashSet<&NodeKey> = scroll_at
            .map(|i| scene.nodes[i + 1..].iter().map(|n| &n.key).collect())
            .unwrap_or_default();
        let place = |n: &claude_sessions_core::scene::Node| {
            let mut frame = n.frame;
            if scrolled.contains(&n.key) {
                frame.y -= top;
            }
            frame
        };
        let host: Option<Retained<NSView>> = ivars
            .scroll
            .borrow()
            .as_ref()
            .map(|h| Retained::into_super(h.doc.clone()));
        let sink: EventHandler = {
            let weak = Weak::new(self);
            Rc::new(move |event| {
                if let Some(view) = weak.load() {
                    view.emit(event);
                }
            })
        };
        let mut nodes = ivars.nodes.borrow_mut();
        let mut fading_out = Vec::new();
        // The last field this update created: it takes the cursor when the window is key.
        let mut created_field = None;

        for node in diff.added.iter().filter(|n| n.scroll.is_none()) {
            let mut start = place(node);
            if animate {
                start.y -= motion::APPEAR_SLIDE;
            }
            let view = NodeView::new(mtm, &node.key.0, start);
            view.set_cmds(&node.cmds, false);
            created_field = view.set_fields(&node.fields, &sink).or(created_field);
            view.set_spinner(node.spinner, spin);
            view.set_tips(&node.tips);
            if animate {
                view.setAlphaValue(0.0);
            }
            match (&host, scrolled.contains(&node.key)) {
                (Some(doc), true) => doc.addSubview(&view),
                _ => self.addSubview(&view),
            }
            // After the view has a superview: AppKit may swap the backing layer when it moves in.
            view.set_shadow(node.shadow);
            nodes.insert(node.key.clone(), view);
        }
        for key in &diff.removed {
            if let Some(view) = nodes.remove(*key) {
                view.set_spin(false);
                view.clear_pulses();
                fading_out.push(view);
            }
        }
        let before: HashMap<&NodeKey, &claude_sessions_core::scene::Node> =
            old.nodes.iter().map(|n| (&n.key, n)).collect();
        let mut moved = Vec::new();
        for update in &diff.updated {
            let Some(view) = nodes.get(&update.node.key) else {
                continue;
            };
            if update.cmds_changed {
                view.set_cmds(&update.node.cmds, animate);
            }
            // Only what differs from the node's previous version is touched.
            let prev = before.get(&update.node.key);
            if prev.is_none_or(|p| p.shadow != update.node.shadow) {
                view.set_shadow(update.node.shadow);
            }
            if prev.is_none_or(|p| p.fields != update.node.fields)
                || view.field_text_differs(&update.node.fields)
            {
                created_field = view
                    .set_fields(&update.node.fields, &sink)
                    .or(created_field);
            }
            if prev.is_none_or(|p| p.spinner != update.node.spinner) {
                view.set_spinner(update.node.spinner, spin);
            }
            if prev.is_none_or(|p| p.tips != update.node.tips) {
                view.set_tips(&update.node.tips);
            }
            if update.frame_changed {
                moved.push((view.clone(), place(update.node)));
            }
        }
        // A node that entered or left the scrolled content (the scroll node appeared or went away) changes parent.
        for node in scene.nodes.iter().filter(|n| n.scroll.is_none()) {
            let Some(view) = nodes.get(&node.key) else {
                continue;
            };
            let want: Retained<NSView> = match (&host, scrolled.contains(&node.key)) {
                (Some(doc), true) => doc.clone(),
                _ => {
                    let this: &NSView = self;
                    this.retain()
                }
            };
            // SAFETY: a plain getter, called on the main thread.
            let parent = unsafe { view.superview() };
            if parent.as_deref() != Some(&*want) {
                view.removeFromSuperview();
                want.addSubview(view);
                view.setFrame(ns_rect(place(node)));
            }
        }
        if scroll_at.is_none()
            && let Some(old) = ivars.scroll.borrow_mut().take()
        {
            // The nodes are back in this view; the scroll view, its clip view and its document must leave it too.
            old.view.removeFromSuperview();
        }
        // Direct subviews stack in scene order with the scroll view in the scroll node's place, so the panel
        // background never covers the list. Fading-out views stay where they are.
        let want: Vec<Retained<NSView>> = scene
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                if Some(i) == scroll_at {
                    let scroll = ivars.scroll.borrow();
                    scroll
                        .as_ref()
                        .map(|h| Retained::into_super(h.view.clone()))
                } else if scrolled.contains(&n.key) {
                    None
                } else {
                    nodes.get(&n.key).map(|v| Retained::into_super(v.clone()))
                }
            })
            .collect();
        let have = self.subviews();
        let have: Vec<_> = have.iter().filter(|v| want.contains(v)).collect();
        if have.len() != want.len() || have.iter().zip(&want).any(|(a, b)| **a != **b) {
            for pair in want.windows(2) {
                self.addSubview_positioned_relativeTo(
                    &pair[1],
                    NSWindowOrderingMode::Above,
                    Some(&pair[0]),
                );
            }
        }

        let appear: Vec<_> = diff
            .added
            .iter()
            .filter_map(|n| nodes.get(&n.key).map(|v| (v.clone(), place(n))))
            .collect();
        // A size change is the open/close spring; a pure move is a card sliding to its new place.
        let (resized, slid): (Vec<_>, Vec<_>) = moved
            .into_iter()
            .partition(|(view, frame)| view.frame().size != ns_rect(*frame).size);
        let remove = {
            let fading_out = fading_out.clone();
            move || {
                for view in &fading_out {
                    view.removeFromSuperview();
                }
            }
        };
        if !animate {
            for (view, frame) in resized.iter().chain(&slid) {
                view.cancel_shadow_animation();
                view.setFrame(ns_rect(*frame));
            }
            remove();
        } else {
            if !resized.is_empty() {
                motion::group(
                    motion::OPEN_SECS,
                    true,
                    move || {
                        for (view, frame) in &resized {
                            view.animate_frame(*frame);
                        }
                    },
                    || {},
                );
            }
            if !slid.is_empty() {
                motion::group(
                    motion::MOVE_SECS,
                    false,
                    move || {
                        for (view, frame) in &slid {
                            view.animator().setFrame(ns_rect(*frame));
                        }
                    },
                    || {},
                );
            }
            if !appear.is_empty() {
                motion::group(
                    motion::FADE_SECS,
                    false,
                    move || {
                        for (view, frame) in &appear {
                            view.animator().setAlphaValue(1.0);
                            view.animator().setFrame(ns_rect(*frame));
                        }
                    },
                    || {},
                );
            }
            if !fading_out.is_empty() {
                let gone = Rc::new(remove);
                motion::group(
                    motion::FADE_SECS,
                    false,
                    move || {
                        for view in &fading_out {
                            view.animator().setAlphaValue(0.0);
                        }
                    },
                    move || gone(),
                );
            }
        }
        if !(diff.added.is_empty() && diff.removed.is_empty() && diff.updated.is_empty())
            && let Some(window) = self.window()
        {
            window.invalidateCursorRectsForView(self);
        }
        // Only a field created by this update, and only in a key window (otherwise the cursor would move unseen).
        if let (Some(field), Some(window)) = (created_field, self.window())
            && window.isKeyWindow()
        {
            window.makeFirstResponder(Some(&field));
        }
    }
}
