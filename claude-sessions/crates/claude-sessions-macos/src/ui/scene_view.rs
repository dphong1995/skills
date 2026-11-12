//! The view that shows a `Scene`: one `NodeView` per node key, diffed on every new scene, plus mouse hit-testing.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use claude_sessions_core::scene::{
    Cursor, HitId, Material, NodeKey, Point, Rect, Scene, changed_nodes,
};
use objc2::Message;
use objc2::rc::Retained;
use objc2::rc::Weak;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSAnimatablePropertyContainer, NSCursor, NSEvent, NSTextField, NSView};
use objc2_app_kit::{NSScrollView, NSScrollerStyle, NSWindowOrderingMode};
use objc2_core_foundation::CGPoint;
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::draw::{ns_rect, register_images};
use super::motion;
use super::node_view::{GlassView, NodeView};
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
    /// The vibrancy view of each node with a `Material`: a sibling directly below the node's own view.
    glass: RefCell<HashMap<NodeKey, Retained<GlassView>>>,
    handler: RefCell<Option<EventHandler>>,
    /// Maps the window origin a drag asks for to the one it gets (keeps the card on its screen).
    clamp: RefCell<Option<ClampHandler>>,
    drag: Cell<Option<Drag>>,
    /// The panel is on screen: only then may a spinner turn. Starts false until the panel is shown.
    live: Cell<bool>,
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

/// Puts `glass` directly below `view` in `view`'s superview (a no-op when it already is).
fn seat_glass(view: &NodeView, glass: &GlassView) {
    // SAFETY: a plain getter, called on the main thread.
    let Some(parent) = (unsafe { view.superview() }) else {
        return;
    };
    let me: &NSView = view;
    let glass: &NSView = glass;
    let subs = parent.subviews();
    let seated = subs
        .iter()
        .position(|v| &*v == me)
        .is_some_and(|at| at > 0 && &*subs.objectAtIndex(at - 1) == glass);
    if !seated {
        glass.removeFromSuperview();
        parent.addSubview_positioned_relativeTo(glass, NSWindowOrderingMode::Below, Some(me));
    }
}

/// A material node's glass view at `frame`, hidden until `alpha` is set.
fn new_glass(mtm: MainThreadMarker, frame: Rect, corner: f64) -> Retained<GlassView> {
    let glass = GlassView::new(mtm, frame);
    glass.set_look(corner);
    glass
}

impl SceneView {
    pub fn new(mtm: MainThreadMarker, size: NSSize) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(SceneIvars {
            scene: RefCell::new(Scene::default()),
            nodes: RefCell::new(HashMap::new()),
            glass: RefCell::new(HashMap::new()),
            handler: RefCell::new(None),
            clamp: RefCell::new(None),
            drag: Cell::new(None),
            live: Cell::new(false),
            scroll: RefCell::new(None),
        });
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setWantsLayer(true);
        this
    }

    /// The panel went on or off screen: a hidden panel keeps no looping animation (spinners stop, pulses are dropped).
    pub fn set_motion_live(&self, live: bool) {
        self.ivars().live.set(live);
        self.apply_motion();
    }

    /// Spinners turn only while the panel is on screen; pulses go with them.
    fn apply_motion(&self) {
        let live = self.ivars().live.get();
        for view in self.ivars().nodes.borrow().values() {
            view.set_spin(live);
            if !live {
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

    /// The native text field with `Field.key` `key`, wherever it sits in the scene (scroll content included).
    pub fn field(&self, key: &str) -> Option<Retained<NSTextField>> {
        self.ivars()
            .nodes
            .borrow()
            .values()
            .find_map(|n| n.field(key))
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
        let live = self.ivars().live.get();
        let ivars = self.ivars();
        register_images(&scene.images);
        let old = ivars.scene.replace(scene.clone());
        let diff = changed_nodes(&old, scene);
        // The panel opens when the pill goes away and closes when it comes back (never from an empty scene).
        let has_pill = |s: &Scene| s.nodes.iter().any(|n| n.key.0 == "pill");
        let opening = has_pill(&old) && !has_pill(scene);
        let closing = !old.nodes.is_empty() && !has_pill(&old) && has_pill(scene);
        let origin = scene.grow_origin;
        let origin = CGPoint::new(origin.x, origin.y);
        let had_scroll = ivars.scroll.borrow().is_some();
        let scroll_at = scene.nodes.iter().position(|n| n.scroll.is_some());
        self.sync_scroll(scene, scroll_at);
        let new_host = if opening && !had_scroll {
            ivars.scroll.borrow().as_ref().map(|h| h.view.clone())
        } else {
            None
        };
        if let Some(host) = &new_host {
            host.setAlphaValue(0.0);
        }
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
        let mut glasses = ivars.glass.borrow_mut();
        let mut fading_out = Vec::new();
        // The last field this update created: it takes the cursor when the window is key. The winner across nodes follows
        // iteration order (added nodes before updated ones).
        let mut created_field = None;

        for node in diff.added.iter().filter(|n| n.scroll.is_none()) {
            let mut start = place(node);
            if !opening && !closing {
                start.y -= motion::APPEAR_SLIDE;
            }
            let view = NodeView::new(mtm, &node.key.0, start);
            view.set_cmds(&node.cmds, false);
            created_field = view.set_fields(&node.fields, &sink).or(created_field);
            view.set_spinner(node.spinner, live);
            view.set_tips(&node.tips);
            view.setAlphaValue(0.0);
            match (&host, scrolled.contains(&node.key)) {
                (Some(doc), true) => doc.addSubview(&view),
                _ => self.addSubview(&view),
            }
            // After the view has a superview: AppKit may swap the backing layer when it moves in.
            view.set_shadow(node.shadow);
            if let Some(Material::Glass { corner, .. }) = node.material {
                let glass = new_glass(mtm, start, corner);
                glass.setAlphaValue(0.0);
                self.addSubview(&glass);
                glasses.insert(node.key.clone(), glass);
            }
            nodes.insert(node.key.clone(), view);
        }
        for key in &diff.removed {
            if let Some(view) = nodes.remove(*key) {
                view.set_spin(false);
                view.clear_pulses();
                view.retire_fields();
                fading_out.push((view, glasses.remove(*key)));
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
                view.set_cmds(&update.node.cmds, live);
            }
            // Only what differs from the node's previous version is touched.
            let prev = before.get(&update.node.key);
            if prev.is_none_or(|p| p.shadow != update.node.shadow) {
                view.set_shadow(update.node.shadow);
            }
            if prev.is_none_or(|p| p.material != update.node.material) {
                match update.node.material {
                    Some(Material::Glass { corner, alpha }) => {
                        let glass = glasses.entry(update.node.key.clone()).or_insert_with(|| {
                            let f = view.frame();
                            let rect =
                                Rect::new(f.origin.x, f.origin.y, f.size.width, f.size.height);
                            let glass = new_glass(mtm, rect, corner);
                            self.addSubview(&glass);
                            glass
                        });
                        glass.set_look(corner);
                        glass.setAlphaValue(alpha);
                    }
                    None => {
                        if let Some(glass) = glasses.remove(&update.node.key) {
                            glass.removeFromSuperview();
                        }
                    }
                }
            }
            if prev.is_none_or(|p| p.fields != update.node.fields)
                || view.field_text_differs(&update.node.fields)
            {
                created_field = view
                    .set_fields(&update.node.fields, &sink)
                    .or(created_field);
            }
            if prev.is_none_or(|p| p.spinner != update.node.spinner) {
                view.set_spinner(update.node.spinner, live);
            }
            if prev.is_none_or(|p| p.tips != update.node.tips) {
                view.set_tips(&update.node.tips);
            }
            if update.frame_changed {
                moved.push((
                    view.clone(),
                    glasses.get(&update.node.key).cloned(),
                    place(update.node),
                ));
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
        let mut dropped_host = None;
        if scroll_at.is_none()
            && let Some(old) = ivars.scroll.borrow_mut().take()
        {
            // The nodes are back in this view; the scroll view, its clip view and its document must leave it too.
            if closing {
                dropped_host = Some(old.view);
            } else {
                old.view.removeFromSuperview();
            }
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

        for node in &scene.nodes {
            if let (Some(view), Some(glass)) = (nodes.get(&node.key), glasses.get(&node.key)) {
                seat_glass(view, glass);
            }
        }

        let appear: Vec<_> = diff
            .added
            .iter()
            .filter_map(|n| {
                let glass = glasses.get(&n.key).map(|g| {
                    let alpha = match n.material {
                        Some(Material::Glass { alpha, .. }) => alpha,
                        None => 1.0,
                    };
                    (g.clone(), alpha)
                });
                nodes.get(&n.key).map(|v| (v.clone(), glass, place(n)))
            })
            .collect();
        // A size change is the open/close spring; a pure move is a card sliding to its new place.
        let (resized, slid): (Vec<_>, Vec<_>) = moved
            .into_iter()
            .partition(|(view, _, frame)| view.frame().size != ns_rect(*frame).size);
        if !resized.is_empty() {
            motion::group(
                motion::OPEN_SECS,
                true,
                move || {
                    for (view, glass, frame) in &resized {
                        view.animate_frame(*frame);
                        if let Some(glass) = glass {
                            glass.animator().setFrame(ns_rect(*frame));
                        }
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
                    for (view, glass, frame) in &slid {
                        view.animator().setFrame(ns_rect(*frame));
                        if let Some(glass) = glass {
                            glass.animator().setFrame(ns_rect(*frame));
                        }
                    }
                },
                || {},
            );
        }
        if opening {
            // Everything new grows out of the grow origin; the scrolled cards ride on their scroll view.
            for (view, glass, _) in &appear {
                // SAFETY: a plain getter, called on the main thread.
                if unsafe { view.superview() }.as_deref() != Some(self) {
                    continue;
                }
                for layer in [view.layer(), glass.as_ref().and_then(|(g, _)| g.layer())]
                    .into_iter()
                    .flatten()
                {
                    motion::zoom_in(&layer, origin);
                }
            }
            if let Some(layer) = new_host.as_ref().and_then(|h| h.layer()) {
                motion::zoom_in(&layer, origin);
            }
        }
        if !appear.is_empty() || new_host.is_some() {
            let changes = move || {
                for (view, glass, frame) in &appear {
                    view.animator().setAlphaValue(1.0);
                    if let Some((glass, alpha)) = glass {
                        glass.animator().setAlphaValue(*alpha);
                    }
                    if !(opening || closing) {
                        view.animator().setFrame(ns_rect(*frame));
                        if let Some((glass, _)) = glass {
                            glass.animator().setFrame(ns_rect(*frame));
                        }
                    }
                }
                if let Some(host) = &new_host {
                    host.animator().setAlphaValue(1.0);
                }
            };
            if opening || closing {
                motion::pop_group(motion::POP_FADE_SECS, opening, changes, || {});
            } else {
                motion::group(motion::FADE_SECS, false, changes, || {});
            }
        }
        // The scroll host of a closing panel goes with it, not at once.
        let closing_host = if closing { dropped_host } else { None };
        if let Some(host) = &closing_host
            && let Some(layer) = host.layer()
        {
            motion::zoom_out(&layer, origin);
        }
        if !fading_out.is_empty() || closing_host.is_some() {
            if closing {
                for (view, glass) in &fading_out {
                    // SAFETY: a plain getter, called on the main thread.
                    if unsafe { view.superview() }.as_deref() != Some(self) {
                        continue;
                    }
                    for layer in [view.layer(), glass.as_ref().and_then(|g| g.layer())]
                        .into_iter()
                        .flatten()
                    {
                        motion::zoom_out(&layer, origin);
                    }
                }
            }
            let leaving = fading_out.clone();
            let gone = closing_host.clone();
            let changes = move || {
                for (view, glass) in &fading_out {
                    view.animator().setAlphaValue(0.0);
                    if let Some(glass) = glass {
                        glass.animator().setAlphaValue(0.0);
                    }
                }
                if let Some(host) = &closing_host {
                    host.animator().setAlphaValue(0.0);
                }
            };
            let done = move || {
                for (view, glass) in &leaving {
                    view.removeFromSuperview();
                    if let Some(glass) = glass {
                        glass.removeFromSuperview();
                    }
                }
                if let Some(host) = &gone {
                    host.removeFromSuperview();
                }
            };
            if closing {
                motion::pop_group(motion::POP_CLOSE_SECS, false, changes, done);
            } else {
                motion::group(motion::FADE_SECS, false, changes, done);
            }
        }
        if !(diff.added.is_empty() && diff.removed.is_empty() && diff.updated.is_empty())
            && let Some(window) = self.window()
        {
            window.invalidateCursorRectsForView(self);
        }
        // Only a field created by this update, and only in a key window (otherwise the cursor would move unseen).
        if let Some(field) = created_field
            && let Some(window) = self.window()
            && window.isKeyWindow()
        {
            window.makeFirstResponder(Some(&field));
        }
    }
}
