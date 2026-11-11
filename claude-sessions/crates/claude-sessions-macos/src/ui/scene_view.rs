//! The view that shows a `Scene`: one `NodeView` per node key, diffed on every new scene, plus mouse hit-testing.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr::NonNull;

use block2::RcBlock;
use claude_sessions_core::scene::{Cursor, HitId, NodeKey, Point, Scene, changed_nodes};
use objc2::Message;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSCursor, NSEvent, NSTextField, NSView,
    NSWorkspace,
};
use objc2_app_kit::{NSScrollView, NSScrollerStyle};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::draw::{ns_rect, register_images};
use super::node_view::NodeView;
use super::{EventHandler, UiEvent};

/// Pointer travel (points) before a press becomes a drag, as Swift's `Layout.isDrag`.
const DRAG_THRESHOLD: f64 = 4.0;
const ANIMATION_SECONDS: f64 = 0.2;

#[derive(Clone, Copy)]
struct Drag {
    mouse0: NSPoint,
    origin0: NSPoint,
    moving: bool,
}

pub struct SceneIvars {
    scene: RefCell<Scene>,
    nodes: RefCell<HashMap<NodeKey, Retained<NodeView>>>,
    handler: RefCell<Option<EventHandler>>,
    drag: Cell<Option<Drag>>,
    /// The scroll host of the scene's scroll node, while it has one.
    scroll: RefCell<Option<ScrollHost>>,
}

/// The `NSScrollView` for a scroll node and its flipped document view, which holds the scrolled nodes.
struct ScrollHost {
    view: Retained<NSScrollView>,
    doc: Retained<NodeView>,
    /// Scene y of the viewport's top; a scrolled node sits at its frame.y - top in the document.
    top: f64,
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
                window.setFrameOrigin(NSPoint::new(drag.origin0.x + dx, drag.origin0.y + dy));
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let Some(drag) = self.ivars().drag.take() else { return };
            if drag.moving {
                self.emit(UiEvent::Dragged);
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
            drag: Cell::new(None),
            scroll: RefCell::new(None),
        });
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setWantsLayer(true);
        this
    }

    pub fn set_handler(&self, handler: EventHandler) {
        *self.ivars().handler.borrow_mut() = Some(handler);
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
        let scroll = self.ivars().scroll.borrow();
        let scrolled_from = scene
            .nodes
            .iter()
            .position(|n| n.scroll.is_some())
            .unwrap_or(usize::MAX);
        let offset = scroll
            .as_ref()
            .map_or(0.0, |s| s.view.contentView().bounds().origin.y);
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
            ScrollHost {
                view,
                doc,
                top: node.frame.y,
            }
        });
        host.top = node.frame.y;
        host.view.setFrame(ns_rect(node.frame));
        host.doc.setFrameSize(NSSize::new(content.w, content.h));
    }

    /// Shows `scene`, touching only what differs from the previous one: unchanged nodes do nothing, changed commands
    /// redraw that node alone, changed frames animate in Core Animation, new and removed nodes fade.
    pub fn set_scene(&self, scene: &Scene) {
        let mtm = self.mtm();
        let animate = !NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion();
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
        let mut nodes = ivars.nodes.borrow_mut();
        let mut fading_out = Vec::new();

        for node in diff.added.iter().filter(|n| n.scroll.is_none()) {
            let view = NodeView::new(mtm, &node.key.0, place(node));
            view.set_cmds(&node.cmds);
            view.set_fields(&node.fields);
            view.set_spinner(node.spinner);
            view.set_tips(&node.tips);
            if animate {
                view.setAlphaValue(0.0);
            }
            match (&host, scrolled.contains(&node.key)) {
                (Some(doc), true) => doc.addSubview(&view),
                _ => self.addSubview(&view),
            }
            nodes.insert(node.key.clone(), view);
        }
        for key in &diff.removed {
            if let Some(view) = nodes.remove(*key) {
                fading_out.push(view);
            }
        }
        let mut moved = Vec::new();
        for update in &diff.updated {
            let Some(view) = nodes.get(&update.node.key) else {
                continue;
            };
            if update.cmds_changed {
                view.set_cmds(&update.node.cmds);
            }
            view.set_fields(&update.node.fields);
            view.set_spinner(update.node.spinner);
            view.set_tips(&update.node.tips);
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
        if scroll_at.is_none() {
            *ivars.scroll.borrow_mut() = None;
        }

        let appear: Vec<_> = diff
            .added
            .iter()
            .filter_map(|n| nodes.get(&n.key).cloned())
            .collect();
        let changes = {
            let (moved, appear, fading_out) = (moved.clone(), appear.clone(), fading_out.clone());
            move || {
                for (view, frame) in &moved {
                    if animate {
                        view.animator().setFrame(ns_rect(*frame));
                    } else {
                        view.setFrame(ns_rect(*frame));
                    }
                }
                for view in &appear {
                    if animate {
                        view.animator().setAlphaValue(1.0);
                    }
                }
                for view in &fading_out {
                    if animate {
                        view.animator().setAlphaValue(0.0);
                    }
                }
            }
        };
        if animate && !(moved.is_empty() && appear.is_empty() && fading_out.is_empty()) {
            let group = RcBlock::new(move |ctx: NonNull<NSAnimationContext>| {
                // SAFETY: AppKit passes a live context for the length of the call.
                unsafe { ctx.as_ref() }.setDuration(ANIMATION_SECONDS);
                changes();
            });
            let done = RcBlock::new(move || {
                for view in &fading_out {
                    view.removeFromSuperview();
                }
            });
            NSAnimationContext::runAnimationGroup_completionHandler(&group, Some(&done));
        } else {
            changes();
            for view in &fading_out {
                view.removeFromSuperview();
            }
        }
        if !(diff.added.is_empty() && diff.removed.is_empty() && diff.updated.is_empty())
            && let Some(window) = self.window()
        {
            window.invalidateCursorRectsForView(self);
        }
    }
}
