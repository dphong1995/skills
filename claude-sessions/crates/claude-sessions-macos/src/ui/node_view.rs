//! One layer-backed view per scene node: draws its own commands and hosts its fields and spinner.

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use claude_sessions_core::scene::{
    Appearance, Color, DrawCmd, Field, FieldKind, Rect, Shadow, Tip, appeared_symbols,
    colors_only_changed,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSAppearanceCustomization,
    NSAutoresizingMaskOptions, NSColor, NSControl, NSControlTextEditingDelegate, NSEvent,
    NSFocusRingType, NSFont, NSScrollView, NSScrollerStyle, NSSecureTextField, NSTextDelegate,
    NSTextField, NSTextFieldDelegate, NSTextView, NSTextViewDelegate, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowOrderingMode,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGPath;
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSNumber, NSObject, NSObjectProtocol, NSPoint, NSRect,
    NSSize, NSString, ns_string,
};
use objc2_quartz_core::{
    CABasicAnimation, CALayer, CAMediaTiming, CAShapeLayer, CATransaction, CATransition,
    kCAFillRuleEvenOdd,
};

use super::draw::{draw_cmds, ns_color, ns_rect, rect, shadow_mask_path, shadow_path};
use super::motion;
use super::{EventHandler, UiEvent};

/// The views draw a field's capsule and its ring: the native field only shows the text, with no bezel, border, background
/// or focus ring of its own.
pub(crate) fn style_field(view: &NSTextField) {
    view.setBezeled(false);
    view.setBordered(false);
    view.setDrawsBackground(false);
    view.setFocusRingType(NSFocusRingType::None);
}

pub(crate) struct NodeIvars {
    key: String,
    cmds: RefCell<Vec<DrawCmd>>,
    fields: RefCell<Vec<FieldSlot>>,
    spinner: RefCell<Option<Retained<CAShapeLayer>>>,
    /// One-shot sparkle pulses still running; each removes itself when done.
    pulses: RefCell<Vec<Retained<CAShapeLayer>>>,
    /// AppKit does not retain a tooltip's owner, so the strings live here.
    tips: RefCell<Vec<Retained<NSString>>>,
    /// The view that casts the node's shadow, if it has one: a sibling below the node (and its glass).
    shadow: RefCell<Option<Retained<ShadowView>>>,
}

/// A native text control of the node and its delegate (AppKit keeps delegates unretained).
struct FieldSlot {
    key: String,
    kind: FieldKind,
    control: Control,
}

enum Control {
    /// A line field, or a secure one (`NSSecureTextField` is an `NSTextField`).
    Line {
        view: Retained<NSTextField>,
        _delegate: Retained<FieldDelegate>,
    },
    /// A text view inside the scroll view that sits at the field's rect.
    Multi {
        scroll: Retained<NSScrollView>,
        text: Retained<NSTextView>,
        _delegate: Retained<TextDelegate>,
    },
}

impl FieldSlot {
    /// The view that sits in the node and takes the field's rect.
    fn host(&self) -> Retained<NSView> {
        match &self.control {
            Control::Line { view, .. } => Retained::into_super(Retained::into_super(view.clone())),
            Control::Multi { scroll, .. } => Retained::into_super(scroll.clone()),
        }
    }

    fn text(&self) -> String {
        match &self.control {
            Control::Line { view, .. } => view.stringValue().to_string(),
            Control::Multi { text, .. } => text.string().to_string(),
        }
    }

    fn set_text(&self, text: &str) {
        match &self.control {
            Control::Line { view, .. } => view.setStringValue(&NSString::from_str(text)),
            Control::Multi { text: view, .. } => view.setString(&NSString::from_str(text)),
        }
    }

    fn is_editing(&self) -> bool {
        match &self.control {
            Control::Line { view, .. } => view.currentEditor().is_some(),
            Control::Multi { text, .. } => text
                .window()
                .and_then(|w| w.firstResponder())
                .is_some_and(|r| {
                    std::ptr::eq(
                        &*r as *const _ as *const u8,
                        &**text as *const _ as *const u8,
                    )
                }),
        }
    }

    fn clear_delegate(&self) {
        // SAFETY: clearing the delegate is always valid; the slot still keeps the delegate object alive.
        match &self.control {
            Control::Line { view, .. } => unsafe { view.setDelegate(None) },
            Control::Multi { text, .. } => text.setDelegate(None),
        }
    }
}

pub(crate) struct DelegateIvars {
    key: String,
    sink: EventHandler,
}

define_class!(
    /// Turns what happens in one text field into `UiEvent`s.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    pub(crate) struct FieldDelegate;

    unsafe impl NSObjectProtocol for FieldDelegate {}

    unsafe impl NSControlTextEditingDelegate for FieldDelegate {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, note: &NSNotification) {
            let Some(object) = note.object() else { return };
            let Some(field) = object.downcast_ref::<NSTextField>() else {
                return;
            };
            (self.ivars().sink)(UiEvent::FieldChanged {
                key: self.ivars().key.clone(),
                text: field.stringValue().to_string(),
            });
        }

        #[unsafe(method(control:textView:doCommandBySelector:))]
        unsafe fn control_text_view_do_command_by_selector(
            &self,
            _control: &NSControl,
            _text_view: &NSTextView,
            command: Sel,
        ) -> Bool {
            if command == sel!(insertNewline:) {
                let key = self.ivars().key.clone();
                (self.ivars().sink)(UiEvent::FieldSubmit { key });
            } else if command == sel!(cancelOperation:) {
                let key = self.ivars().key.clone();
                (self.ivars().sink)(UiEvent::FieldCancel { key });
            } else {
                return Bool::NO;
            }
            Bool::YES
        }
    }

    unsafe impl NSTextFieldDelegate for FieldDelegate {}
);

define_class!(
    /// Turns what happens in one multi-line text view into `UiEvent`s.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    pub(crate) struct TextDelegate;

    unsafe impl NSObjectProtocol for TextDelegate {}

    unsafe impl NSTextDelegate for TextDelegate {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, note: &NSNotification) {
            let Some(object) = note.object() else { return };
            let Some(text) = object.downcast_ref::<NSTextView>() else {
                return;
            };
            (self.ivars().sink)(UiEvent::FieldChanged {
                key: self.ivars().key.clone(),
                text: text.string().to_string(),
            });
        }

        /// Losing first responder status submits the field.
        #[unsafe(method(textDidEndEditing:))]
        fn text_did_end_editing(&self, _note: &NSNotification) {
            let key = self.ivars().key.clone();
            (self.ivars().sink)(UiEvent::FieldSubmit { key });
        }
    }

    unsafe impl NSTextViewDelegate for TextDelegate {
        #[unsafe(method(textView:doCommandBySelector:))]
        unsafe fn text_view_do_command_by_selector(
            &self,
            _text_view: &NSTextView,
            command: Sel,
        ) -> Bool {
            if command == sel!(cancelOperation:) {
                let key = self.ivars().key.clone();
                (self.ivars().sink)(UiEvent::FieldCancel { key });
                Bool::YES
            } else {
                Bool::NO
            }
        }
    }
);

impl TextDelegate {
    fn new(mtm: MainThreadMarker, key: &str, sink: EventHandler) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars {
            key: key.to_owned(),
            sink,
        });
        // SAFETY: `init` is NSObject's designated initialiser.
        unsafe { msg_send![super(this), init] }
    }
}

impl FieldDelegate {
    fn new(mtm: MainThreadMarker, key: &str, sink: EventHandler) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars {
            key: key.to_owned(),
            sink,
        });
        // SAFETY: `init` is NSObject's designated initialiser.
        unsafe { msg_send![super(this), init] }
    }
}

/// A scroll view at `rect` with a plain-text view in it that grows with its content (Return inserts a newline).
fn multiline_views(
    mtm: MainThreadMarker,
    rect: Rect,
) -> (Retained<NSScrollView>, Retained<NSTextView>) {
    let frame = ns_rect(rect);
    let scroll = NSScrollView::initWithFrame(mtm.alloc(), frame);
    scroll.setDrawsBackground(false);
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(false);
    scroll.setAutohidesScrollers(true);
    scroll.setScrollerStyle(NSScrollerStyle::Overlay);
    let text =
        NSTextView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::new(0.0, 0.0), frame.size));
    text.setRichText(false);
    text.setDrawsBackground(false);
    text.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    text.setFocusRingType(NSFocusRingType::None);
    text.setVerticallyResizable(true);
    text.setHorizontallyResizable(false);
    text.setMinSize(NSSize::new(0.0, frame.size.height));
    text.setMaxSize(NSSize::new(frame.size.width, f64::MAX));
    text.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    // SAFETY: a plain getter, called on the main thread.
    if let Some(container) = unsafe { text.textContainer() } {
        container.setContainerSize(NSSize::new(frame.size.width, f64::MAX));
        container.setWidthTracksTextView(true);
    }
    scroll.setDocumentView(Some(&text));
    (scroll, text)
}

/// `CLAUDE_SESSIONS_FRAME_LOG=1`: one stderr line per node redraw.
fn frame_log() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("CLAUDE_SESSIONS_FRAME_LOG").is_some())
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = NodeIvars]
    pub(crate) struct NodeView;

    unsafe impl NSObjectProtocol for NodeView {}

    impl NodeView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        // The panel never becomes active, so the first click must be delivered, not swallowed.
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if frame_log() {
                eprintln!("redraw {}", self.ivars().key);
            }
            let dark = self.effectiveAppearance().name().to_string().contains("Dark");
            let appearance = if dark { Appearance::Dark } else { Appearance::Light };
            draw_cmds(&self.ivars().cmds.borrow(), appearance);
        }
    }
);

pub(crate) struct GlassIvars {
    corner: Cell<f64>,
    /// The alpha the glass shows at when its node is on screen.
    alpha: Cell<f64>,
}

define_class!(
    /// The vibrancy behind a node with a `Material`: a sibling placed directly below the node view, so the node's drawn
    /// commands and subviews stay above it. It never takes a mouse event.
    #[unsafe(super = NSVisualEffectView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = GlassIvars]
    pub(crate) struct GlassView;

    unsafe impl NSObjectProtocol for GlassView {}

    impl GlassView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: CGPoint) -> Option<&NSView> {
            None
        }
    }
);

impl GlassView {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        frame: Rect,
        corner: f64,
        alpha: f64,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(GlassIvars {
            corner: Cell::new(corner),
            alpha: Cell::new(alpha),
        });
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: ns_rect(frame)] };
        this.setMaterial(NSVisualEffectMaterial::Popover);
        this.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        // The app is never the active one; any other state falls back to a flat grey.
        this.setState(NSVisualEffectState::Active);
        this.setWantsLayer(true);
        this.apply_look();
        this
    }

    /// Takes a new corner and alpha; the alpha shows at once.
    pub(crate) fn retarget(&self, corner: f64, alpha: f64) {
        self.ivars().corner.set(corner);
        self.ivars().alpha.set(alpha);
        self.apply_look();
        self.setAlphaValue(alpha);
    }

    /// The alpha this glass fades to when its node appears.
    pub(crate) fn target_alpha(&self) -> f64 {
        self.ivars().alpha.get()
    }

    /// Rounds the glass to its corner. Run again after the view moves in the hierarchy: AppKit may swap the backing layer.
    pub(crate) fn apply_look(&self) {
        if let Some(layer) = self.layer() {
            layer.setCornerRadius(self.ivars().corner.get());
            layer.setMasksToBounds(true);
        }
    }
}

pub(crate) struct ShadowIvars {
    shadow: Cell<Shadow>,
    /// Lets the shadow show only outside the node's shape; its path follows the view's size like `shadowPath`.
    mask: Retained<CAShapeLayer>,
}

define_class!(
    /// The shadow of a node with a `Shadow`: a sibling placed below the node's glass, so the shadow never darkens what a
    /// translucent node lets through. A `CALayer` shadow with an explicit path (the render server draws it, nothing is
    /// redrawn per frame), masked so that it falls outside the node's shape only. It never takes a mouse event.
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ShadowIvars]
    pub(crate) struct ShadowView;

    unsafe impl NSObjectProtocol for ShadowView {}

    impl ShadowView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: CGPoint) -> Option<&NSView> {
            None
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        // The shadow's path is the node's own rounded rect, so it must follow every size change.
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: NSView's own setter, with the same argument.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.sync_paths();
        }

        // Moving into a window can swap the backing layer, which drops the shadow set on the old one.
        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move_to_window(&self) {
            // SAFETY: NSView's own notification, no arguments.
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            self.reapply();
        }
    }
);

impl ShadowView {
    pub(crate) fn new(mtm: MainThreadMarker, frame: Rect, shadow: Shadow) -> Retained<Self> {
        let mask = CAShapeLayer::layer();
        // SAFETY: a plain extern constant.
        mask.setFillRule(unsafe { kCAFillRuleEvenOdd });
        let this = Self::alloc(mtm).set_ivars(ShadowIvars {
            shadow: Cell::new(shadow),
            mask,
        });
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: ns_rect(frame)] };
        this.setWantsLayer(true);
        this.apply(shadow);
        this
    }

    /// Sets the layer shadow and its mask again, after the view moved in the hierarchy (AppKit may swap the backing layer).
    pub(crate) fn reapply(&self) {
        self.apply(self.ivars().shadow.get());
    }

    /// Sets the layer shadow and its mask.
    pub(crate) fn apply(&self, shadow: Shadow) {
        self.ivars().shadow.set(shadow);
        let Some(layer) = self.layer() else { return };
        // The colour is opaque; the layer's opacity carries the alpha.
        let opaque = Color {
            alpha: 1.0,
            ..shadow.color
        };
        layer.setShadowColor(Some(&ns_color(opaque, Appearance::Dark).CGColor()));
        layer.setShadowOpacity(shadow.color.resolve(Appearance::Dark)[3] as f32);
        layer.setShadowRadius(shadow.radius);
        // A flipped view's backing layer has flipped geometry: there a positive offset already points down.
        let y = if layer.isGeometryFlipped() {
            shadow.offset.y
        } else {
            -shadow.offset.y
        };
        layer.setShadowOffset(CGSize::new(shadow.offset.x, y));
        // SAFETY: the mask is a layer owned by this view and used as no other layer's mask.
        unsafe { layer.setMask(Some(&self.ivars().mask)) };
        self.sync_paths();
    }

    /// The rounded rect of the view's bounds as the layer's `shadowPath`, and the mask that cuts it out of the shadow.
    fn sync_paths(&self) {
        let Some(layer) = self.layer() else { return };
        let shadow = self.ivars().shadow.get();
        let size = self.bounds().size;
        // Not animated: a standalone mask layer would otherwise get a 0.25 s implicit animation of `path`.
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        layer.setShadowPath(Some(&shadow_path(&shadow, size.width, size.height)));
        self.ivars()
            .mask
            .setPath(Some(&shadow_mask_path(&shadow, size.width, size.height)));
        CATransaction::commit();
    }

    /// Animates the frame (inside the caller's animation group) and the shadow path and mask with it, so the shadow never
    /// lags.
    pub(crate) fn animate_frame(&self, frame: Rect) {
        // From what is on screen, so overlapping animations do not make the shadow jump.
        let old = self.layer().and_then(|l| {
            // SAFETY: a plain getter; the presentation layer is read on the main thread and not kept.
            unsafe { l.presentationLayer() }
                .and_then(|p| p.shadowPath())
                .or_else(|| l.shadowPath())
        });
        self.animator().setFrame(ns_rect(frame));
        let (Some(old), Some(layer)) = (old, self.layer()) else {
            return;
        };
        let shadow = self.ivars().shadow.get();
        // The target size, not the view's: the animated frame has not reached it yet.
        let new = shadow_path(&shadow, frame.w, frame.h);
        layer.setShadowPath(Some(&new));
        let old_size = CGPath::bounding_box(Some(&old)).size;
        let new_mask = shadow_mask_path(&shadow, frame.w, frame.h);
        let mask = &self.ivars().mask;
        mask.setPath(Some(&new_mask));
        let old_mask = shadow_mask_path(&shadow, old_size.width, old_size.height);
        let context = NSAnimationContext::currentContext();
        let animate = |target: &CALayer, key: &NSString, from: &CGPath, to: &CGPath| {
            let animation = CABasicAnimation::animationWithKeyPath(Some(key));
            // SAFETY: a CGPath is the value type of both key paths; Core Animation takes it as an object.
            unsafe {
                animation.setFromValue(Some(&*(from as *const _ as *const AnyObject)));
                animation.setToValue(Some(&*(to as *const _ as *const AnyObject)));
            }
            animation.setDuration(context.duration());
            animation.setTimingFunction(context.timingFunction().as_deref());
            target.addAnimation_forKey(&animation, Some(key));
        };
        animate(&layer, ns_string!("shadowPath"), &old, &new);
        animate(mask, ns_string!("path"), &old_mask, &new_mask);
    }
}

impl NodeView {
    pub(crate) fn new(mtm: MainThreadMarker, key: &str, frame: Rect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(NodeIvars {
            key: key.to_owned(),
            cmds: RefCell::new(Vec::new()),
            fields: RefCell::new(Vec::new()),
            spinner: RefCell::new(None),
            pulses: RefCell::new(Vec::new()),
            tips: RefCell::new(Vec::new()),
            shadow: RefCell::new(None),
        });
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: ns_rect(frame)] };
        this.setWantsLayer(true);
        // Commands may extend past the frame (a group's end rows do) and rely on the clip, as `render_png` applies;
        // `clipsToBounds` defaults to NO since macOS 14.
        this.setClipsToBounds(true);
        this
    }

    /// Sets (or clears) the shadow: its view is created below this one, in this view's superview, and removed again.
    pub(crate) fn set_shadow(&self, shadow: Option<Shadow>) {
        let mut slot = self.ivars().shadow.borrow_mut();
        match (shadow, slot.as_ref()) {
            (Some(shadow), Some(view)) => view.apply(shadow),
            (Some(shadow), None) => {
                let view = ShadowView::new(self.mtm(), rect(self.frame()), shadow);
                view.setAlphaValue(self.alphaValue());
                // SAFETY: a plain getter, called on the main thread.
                if let Some(parent) = unsafe { self.superview() } {
                    parent.addSubview_positioned_relativeTo(
                        &view,
                        NSWindowOrderingMode::Below,
                        Some(self),
                    );
                    view.reapply();
                }
                *slot = Some(view);
            }
            (None, _) => {
                if let Some(view) = slot.take() {
                    view.removeFromSuperview();
                }
            }
        }
    }

    /// The view that casts this node's shadow, if it has one.
    pub(crate) fn shadow_view(&self) -> Option<Retained<ShadowView>> {
        self.ivars().shadow.borrow().clone()
    }

    /// Animates the frame (inside the caller's animation group) and the shadow's with it.
    pub(crate) fn animate_frame(&self, frame: Rect) {
        self.animator().setFrame(ns_rect(frame));
        if let Some(shadow) = self.shadow_view() {
            shadow.animate_frame(frame);
        }
    }

    /// With `animate`, a pure recolour cross-fades and a sparkle that was not there pulses once.
    pub(crate) fn set_cmds(&self, cmds: &[DrawCmd], animate: bool) {
        let old = std::mem::replace(&mut *self.ivars().cmds.borrow_mut(), cmds.to_vec());
        if animate {
            if colors_only_changed(&old, cmds)
                && let Some(layer) = self.layer()
            {
                // A transition's default type is a fade; it blends the layer's old and new contents.
                let fade = CATransition::animation();
                fade.setDuration(motion::COLOR_SECS);
                layer.addAnimation_forKey(&fade, Some(ns_string!("colour")));
            }
            for rect in appeared_symbols(&old, cmds, "sparkle") {
                self.pulse(rect);
            }
        }
        self.setNeedsDisplay(true);
    }

    /// A ring that grows and fades once at `rect`, then removes itself. Runs in the render server.
    fn pulse(&self, rect: Rect) {
        let Some(host) = self.layer() else { return };
        let ring = CAShapeLayer::layer();
        let frame = CGRect::new(CGPoint::new(rect.x, rect.y), CGSize::new(rect.w, rect.h));
        let inner = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(rect.w, rect.h));
        // SAFETY: a null transform means identity.
        let path = unsafe { CGPath::with_ellipse_in_rect(inner, std::ptr::null()) };
        ring.setPath(Some(&path));
        ring.setFillColor(None);
        ring.setStrokeColor(Some(&NSColor::systemYellowColor().CGColor()));
        ring.setLineWidth(1.5);
        ring.setFrame(frame);
        ring.setOpacity(0.0);
        let number = |v: f64| -> Retained<AnyObject> { NSNumber::new_f64(v).into() };
        let animation = |key_path: &objc2_foundation::NSString, from: f64, to: f64| {
            let a = CABasicAnimation::animationWithKeyPath(Some(key_path));
            // SAFETY: from/to are NSNumbers, which these key paths take.
            unsafe {
                a.setFromValue(Some(&number(from)));
                a.setToValue(Some(&number(to)));
            }
            a.setDuration(motion::SPARKLE_SECS);
            a
        };
        host.addSublayer(&ring);
        let mut pulses = self.ivars().pulses.borrow_mut();
        pulses.retain(|l| l.superlayer().is_some());
        pulses.push(ring.clone());
        drop(pulses);
        let me = ring.clone();
        let done = block2::RcBlock::new(move || me.removeFromSuperlayer());
        CATransaction::begin();
        // SAFETY: the block is copied by the transaction and only removes a layer on the main thread.
        unsafe { CATransaction::setCompletionBlock(Some(&done)) };
        ring.addAnimation_forKey(
            &animation(ns_string!("transform.scale"), 1.0, 1.9),
            Some(ns_string!("pulse-scale")),
        );
        ring.addAnimation_forKey(
            &animation(ns_string!("opacity"), 0.9, 0.0),
            Some(ns_string!("pulse-fade")),
        );
        CATransaction::commit();
    }

    /// Drops every running one-shot pulse.
    pub(crate) fn clear_pulses(&self) {
        for layer in self.ivars().pulses.borrow_mut().drain(..) {
            layer.removeAllAnimations();
            layer.removeFromSuperlayer();
        }
    }

    /// Replaces the tooltips; the owner of each is its text (AppKit shows the owner's description).
    pub(crate) fn set_tips(&self, tips: &[Tip]) {
        self.removeAllToolTips();
        let mut owners = self.ivars().tips.borrow_mut();
        owners.clear();
        for tip in tips {
            let owner = NSString::from_str(&tip.text);
            // SAFETY: the owner is kept alive in `owners` until the tooltips are removed; no user data is used.
            unsafe {
                self.addToolTipRect_owner_userData(ns_rect(tip.rect), &owner, std::ptr::null_mut());
            }
            owners.push(owner);
        }
    }

    /// Adds, moves and removes the native text fields. The text is set only when the scene's text for a field changed and
    /// the field is not being edited, so what the user typed survives a scene that did not change it; `sink` gets the
    /// field's `FieldChanged`, `FieldSubmit` and `FieldCancel` events. A `Secure` field never takes the scene's text: it is
    /// emptied only by being replaced under a new key. Returns the last `Line` field this call created.
    pub(crate) fn set_fields(
        &self,
        fields: &[Field],
        sink: &EventHandler,
    ) -> Option<Retained<NSTextField>> {
        let mut created = None;
        let mtm = self.mtm();
        let mut have = self.ivars().fields.borrow_mut();
        have.retain(|slot| {
            let keep = fields
                .iter()
                .any(|f| f.key == slot.key && f.kind == slot.kind);
            if !keep {
                slot.host().removeFromSuperview();
            }
            keep
        });
        for field in fields {
            let at = match have.iter().position(|slot| slot.key == field.key) {
                Some(at) => at,
                None => {
                    let control = match field.kind {
                        FieldKind::Line | FieldKind::Secure => {
                            let view = if field.kind == FieldKind::Secure {
                                Retained::into_super(NSSecureTextField::initWithFrame(
                                    mtm.alloc(),
                                    ns_rect(field.rect),
                                ))
                            } else {
                                NSTextField::initWithFrame(mtm.alloc(), ns_rect(field.rect))
                            };
                            style_field(&view);
                            let delegate = FieldDelegate::new(mtm, &field.key, sink.clone());
                            // SAFETY: the delegate lives in the slot as long as the field does.
                            unsafe { view.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
                            self.addSubview(&view);
                            if field.kind == FieldKind::Line {
                                view.setStringValue(&NSString::from_str(&field.text));
                                created = Some(view.clone());
                            }
                            Control::Line {
                                view,
                                _delegate: delegate,
                            }
                        }
                        FieldKind::Multiline => {
                            let (scroll, text) = multiline_views(mtm, field.rect);
                            let delegate = TextDelegate::new(mtm, &field.key, sink.clone());
                            text.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
                            text.setString(&NSString::from_str(&field.text));
                            self.addSubview(&scroll);
                            Control::Multi {
                                scroll,
                                text,
                                _delegate: delegate,
                            }
                        }
                    };
                    have.push(FieldSlot {
                        key: field.key.clone(),
                        kind: field.kind,
                        control,
                    });
                    have.len() - 1
                }
            };
            let slot = &mut have[at];
            slot.host().setFrame(ns_rect(field.rect));
            if let Control::Line { view, .. } = &slot.control {
                view.setPlaceholderString(Some(&NSString::from_str(&field.placeholder)));
            }
            // Typed text reaches the draft (and so the scene) event by event, so a difference means the scene moved on
            // (a draft cleared after a send); an unchanged text is left alone to keep the cursor and selection.
            if field.kind != FieldKind::Secure && slot.text() != field.text {
                slot.set_text(&field.text);
            }
        }
        created
    }

    /// Whether a native field shows other text than the scene's, so `set_fields` has to run although the scene is the same.
    /// A secure field's text is never the scene's, so it never counts.
    pub(crate) fn field_text_differs(&self, fields: &[Field]) -> bool {
        let have = self.ivars().fields.borrow();
        fields.iter().any(|f| {
            f.kind != FieldKind::Secure
                && have
                    .iter()
                    .find(|slot| slot.key == f.key)
                    .is_some_and(|slot| slot.text() != f.text)
        })
    }

    /// The native text field (line or secure) with `Field.key` `key`, if this node has it.
    pub(crate) fn field(&self, key: &str) -> Option<Retained<NSTextField>> {
        self.ivars()
            .fields
            .borrow()
            .iter()
            .find(|slot| slot.key == key)
            .and_then(|slot| match &slot.control {
                Control::Line { view, .. } => Some(view.clone()),
                Control::Multi { .. } => None,
            })
    }

    /// The text view of the multi-line field with `Field.key` `key`, if this node has it.
    pub(crate) fn text_view(&self, key: &str) -> Option<Retained<NSTextView>> {
        self.ivars()
            .fields
            .borrow()
            .iter()
            .find(|slot| slot.key == key)
            .and_then(|slot| match &slot.control {
                Control::Multi { text, .. } => Some(text.clone()),
                Control::Line { .. } => None,
            })
    }

    /// The control with `Field.key` `key` as a first responder, and the view that has to be visible for it.
    pub(crate) fn focus_target(&self, key: &str) -> Option<(Retained<NSView>, Retained<NSView>)> {
        self.ivars()
            .fields
            .borrow()
            .iter()
            .find(|slot| slot.key == key)
            .map(|slot| {
                let responder = match &slot.control {
                    Control::Line { view, .. } => {
                        Retained::into_super(Retained::into_super(view.clone()))
                    }
                    Control::Multi { text, .. } => {
                        Retained::into_super(Retained::into_super(text.clone()))
                    }
                };
                (responder, slot.host())
            })
    }

    /// The first native line field of this node, if any.
    pub(crate) fn first_field(&self) -> Option<Retained<NSTextField>> {
        self.ivars()
            .fields
            .borrow()
            .iter()
            .find_map(|slot| match &slot.control {
                Control::Line { view, .. } => Some(view.clone()),
                Control::Multi { .. } => None,
            })
    }

    /// The node is leaving: its fields stop taking input and emit no more events. A field that is being edited first ends
    /// the edit; the window becomes first responder, as AppKit does when a first-responder view leaves the window.
    pub(crate) fn retire_fields(&self) {
        let fields = self.ivars().fields.borrow();
        // A multi-line field submits when the edit ends: that is not an event of a node that is leaving.
        for slot in fields.iter() {
            if matches!(slot.control, Control::Multi { .. }) {
                slot.clear_delegate();
            }
        }
        if let Some(window) = self.window()
            && fields.iter().any(|slot| slot.is_editing())
        {
            window.makeFirstResponder(None);
        }
        for slot in fields.iter() {
            if let Control::Line { view, .. } = &slot.control {
                view.setEnabled(false);
            } else if let Control::Multi { text, .. } = &slot.control {
                text.setEditable(false);
            }
            slot.clear_delegate();
        }
    }

    /// A ring in `rect`; `None` removes it. With `spin` it rotates (in the render server), otherwise it stands still: the
    /// caller passes `spin` only while the panel is on screen.
    pub(crate) fn set_spinner(&self, rect: Option<Rect>, spin: bool) {
        let mut slot = self.ivars().spinner.borrow_mut();
        let Some(rect) = rect else {
            if let Some(layer) = slot.take() {
                layer.removeAllAnimations();
                layer.removeFromSuperlayer();
            }
            return;
        };
        let frame = CGRect::new(CGPoint::new(rect.x, rect.y), CGSize::new(rect.w, rect.h));
        if let Some(layer) = slot.as_ref() {
            layer.setFrame(frame);
            set_spinning(layer, spin);
            return;
        }
        let Some(host) = self.layer() else {
            return;
        };
        let layer = CAShapeLayer::layer();
        let ring = CGRect::new(
            CGPoint::new(1.0, 1.0),
            CGSize::new(rect.w - 2.0, rect.h - 2.0),
        );
        // SAFETY: a null transform means identity.
        let path = unsafe { CGPath::with_ellipse_in_rect(ring, std::ptr::null()) };
        layer.setPath(Some(&path));
        layer.setFillColor(None);
        layer.setStrokeColor(Some(&NSColor::systemBlueColor().CGColor()));
        layer.setLineWidth(2.0);
        layer.setStrokeEnd(0.75);
        layer.setFrame(frame);
        set_spinning(&layer, spin);
        host.addSublayer(&layer);
        *slot = Some(layer);
    }

    /// Starts or stops the rotation of the spinner this node has, if any.
    pub(crate) fn set_spin(&self, spin: bool) {
        if let Some(layer) = self.ivars().spinner.borrow().as_ref() {
            set_spinning(layer, spin);
        }
    }
}

/// Adds the endless rotation to `layer`, or removes every animation from it: a stopped ring costs nothing.
fn set_spinning(layer: &CAShapeLayer, spin: bool) {
    let layer: &CALayer = layer;
    if !spin {
        layer.removeAnimationForKey(ns_string!("spin"));
        return;
    }
    // SAFETY: a plain getter.
    if unsafe { layer.animationForKey(ns_string!("spin")) }.is_some() {
        return;
    }
    let spin = CABasicAnimation::animationWithKeyPath(Some(ns_string!("transform.rotation.z")));
    let turn = |angle: f64| -> Retained<AnyObject> { NSNumber::new_f64(angle).into() };
    // SAFETY: from/to are NSNumbers, which the rotation key path takes.
    unsafe {
        spin.setFromValue(Some(&turn(0.0)));
        spin.setToValue(Some(&turn(std::f64::consts::TAU)));
    }
    spin.setDuration(1.0);
    spin.setRepeatCount(f32::INFINITY);
    layer.addAnimation_forKey(&spin, Some(ns_string!("spin")));
}
