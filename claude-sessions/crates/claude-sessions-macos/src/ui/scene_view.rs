//! The view that shows a `Scene`: one `NodeView` per node key, diffed on every new scene, plus mouse hit-testing.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr::NonNull;

use block2::RcBlock;
use claude_sessions_core::scene::{Cursor, HitId, NodeKey, Point, Scene, changed_nodes};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSCursor, NSEvent, NSTextField, NSView,
    NSWorkspace,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::draw::ns_rect;
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
            for node in &scene.nodes {
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
        scene.nodes.iter().rev().find_map(|node| {
            let local = Point {
                x: at.x - node.frame.x,
                y: at.y - node.frame.y,
            };
            node.hits
                .iter()
                .find(|h| h.rect.contains(local))
                .map(|h| h.id)
        })
    }

    /// Shows `scene`, touching only what differs from the previous one: unchanged nodes do nothing, changed commands
    /// redraw that node alone, changed frames animate in Core Animation, new and removed nodes fade.
    pub fn set_scene(&self, scene: &Scene) {
        let mtm = self.mtm();
        let animate = !NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion();
        let ivars = self.ivars();
        let old = ivars.scene.replace(scene.clone());
        let diff = changed_nodes(&old, scene);
        let mut nodes = ivars.nodes.borrow_mut();
        let mut fading_out = Vec::new();

        for node in &diff.added {
            let view = NodeView::new(mtm, &node.key.0, node.frame);
            view.set_cmds(&node.cmds);
            view.set_fields(&node.fields);
            view.set_spinner(node.spinner);
            if animate {
                view.setAlphaValue(0.0);
            }
            self.addSubview(&view);
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
            if update.frame_changed {
                moved.push((view.clone(), update.node.frame));
            }
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
