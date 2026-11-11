//! One layer-backed view per scene node: draws its own commands and hosts its fields and spinner.

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use claude_sessions_core::scene::{
    Appearance, Color, DrawCmd, Field, Rect, Shadow, Tip, appeared_symbols, colors_only_changed,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSAppearanceCustomization, NSColor,
    NSControl, NSControlTextEditingDelegate, NSEvent, NSTextField, NSTextFieldDelegate, NSTextView,
    NSView,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGPath;
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSNumber, NSObject, NSObjectProtocol, NSRect, NSSize,
    NSString, ns_string,
};
use objc2_quartz_core::{
    CABasicAnimation, CALayer, CAMediaTiming, CAShapeLayer, CATransaction, CATransition,
};

use super::draw::{draw_cmds, ns_color, ns_rect, shadow_path};
use super::motion;
use super::{EventHandler, UiEvent};

pub(crate) struct NodeIvars {
    key: String,
    cmds: RefCell<Vec<DrawCmd>>,
    fields: RefCell<Vec<FieldSlot>>,
    spinner: RefCell<Option<Retained<CAShapeLayer>>>,
    /// One-shot sparkle pulses still running; each removes itself when done.
    pulses: RefCell<Vec<Retained<CAShapeLayer>>>,
    /// AppKit does not retain a tooltip's owner, so the strings live here.
    tips: RefCell<Vec<Retained<NSString>>>,
    /// The layer shadow the node casts, if any; `shadowPath` follows the view's size.
    shadow: Cell<Option<Shadow>>,
}

/// A native text field of the node and its delegate (AppKit keeps delegates unretained).
struct FieldSlot {
    key: String,
    view: Retained<NSTextField>,
    _delegate: Retained<FieldDelegate>,
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
            let key = self.ivars().key.clone();
            if command == sel!(insertNewline:) {
                (self.ivars().sink)(UiEvent::FieldSubmit { key });
            } else if command == sel!(cancelOperation:) {
                (self.ivars().sink)(UiEvent::FieldCancel { key });
            } else {
                return Bool::NO;
            }
            Bool::YES
        }
    }

    unsafe impl NSTextFieldDelegate for FieldDelegate {}
);

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

        // The shadow's path is the node's own rounded rect, so it must follow every size change.
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: NSView's own setter, with the same argument.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.sync_shadow_path();
        }

        // Moving into a window can swap the backing layer, which drops the shadow set on the old one.
        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move_to_window(&self) {
            // SAFETY: NSView's own notification, no arguments.
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            self.set_shadow(self.ivars().shadow.get());
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

impl NodeView {
    pub(crate) fn new(mtm: MainThreadMarker, key: &str, frame: Rect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(NodeIvars {
            key: key.to_owned(),
            cmds: RefCell::new(Vec::new()),
            fields: RefCell::new(Vec::new()),
            spinner: RefCell::new(None),
            pulses: RefCell::new(Vec::new()),
            tips: RefCell::new(Vec::new()),
            shadow: Cell::new(None),
        });
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: ns_rect(frame)] };
        this.setWantsLayer(true);
        this
    }

    /// Sets (or clears) the layer shadow. It is a `CALayer` shadow with an explicit path: the render server draws it and
    /// nothing is redrawn per frame.
    pub(crate) fn set_shadow(&self, shadow: Option<Shadow>) {
        self.ivars().shadow.set(shadow);
        let Some(layer) = self.layer() else { return };
        match shadow {
            Some(shadow) => {
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
            }
            None => {
                layer.setShadowOpacity(0.0);
                layer.setShadowPath(None);
            }
        }
        self.sync_shadow_path();
    }

    /// The rounded rect of the view's bounds as the layer's `shadowPath`.
    fn sync_shadow_path(&self) {
        let (Some(shadow), Some(layer)) = (self.ivars().shadow.get(), self.layer()) else {
            return;
        };
        let size = self.bounds().size;
        layer.setShadowPath(Some(&shadow_path(&shadow, size.width, size.height)));
    }

    /// Drops an explicit `shadowPath` animation still heading for an old target (a direct resize must not show it).
    pub(crate) fn cancel_shadow_animation(&self) {
        if let Some(layer) = self.layer() {
            layer.removeAnimationForKey(ns_string!("shadowPath"));
        }
    }

    /// Animates the frame (inside the caller's animation group) and the shadow path with it, so the shadow never lags.
    pub(crate) fn animate_frame(&self, frame: Rect) {
        // From what is on screen, so overlapping animations do not make the shadow jump.
        let old = self.layer().and_then(|l| {
            // SAFETY: a plain getter; the presentation layer is read on the main thread and not kept.
            unsafe { l.presentationLayer() }
                .and_then(|p| p.shadowPath())
                .or_else(|| l.shadowPath())
        });
        self.animator().setFrame(ns_rect(frame));
        let (Some(old), Some(shadow), Some(layer)) = (old, self.ivars().shadow.get(), self.layer())
        else {
            return;
        };
        // The target size, not the view's: the animated frame has not reached it yet.
        let new = shadow_path(&shadow, frame.w, frame.h);
        layer.setShadowPath(Some(&new));
        let animation = CABasicAnimation::animationWithKeyPath(Some(ns_string!("shadowPath")));
        // SAFETY: a CGPath is the value type of the `shadowPath` key path; Core Animation takes it as an object.
        unsafe {
            animation.setFromValue(Some(&*(&*old as *const _ as *const AnyObject)));
            animation.setToValue(Some(&*(&*new as *const _ as *const AnyObject)));
        }
        let context = NSAnimationContext::currentContext();
        animation.setDuration(context.duration());
        animation.setTimingFunction(context.timingFunction().as_deref());
        layer.addAnimation_forKey(&animation, Some(ns_string!("shadowPath")));
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
    /// field's `FieldChanged`, `FieldSubmit` and `FieldCancel` events. Returns the last field this call created.
    pub(crate) fn set_fields(
        &self,
        fields: &[Field],
        sink: &EventHandler,
    ) -> Option<Retained<NSTextField>> {
        let mut created = None;
        let mtm = self.mtm();
        let mut have = self.ivars().fields.borrow_mut();
        have.retain(|slot| {
            let keep = fields.iter().any(|f| f.key == slot.key);
            if !keep {
                slot.view.removeFromSuperview();
            }
            keep
        });
        for field in fields {
            let at = match have.iter().position(|slot| slot.key == field.key) {
                Some(at) => at,
                None => {
                    let view = NSTextField::initWithFrame(mtm.alloc(), ns_rect(field.rect));
                    let delegate = FieldDelegate::new(mtm, &field.key, sink.clone());
                    // SAFETY: the delegate lives in the slot as long as the field does.
                    unsafe { view.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
                    self.addSubview(&view);
                    view.setStringValue(&NSString::from_str(&field.text));
                    created = Some(view.clone());
                    have.push(FieldSlot {
                        key: field.key.clone(),
                        view,
                        _delegate: delegate,
                    });
                    have.len() - 1
                }
            };
            let slot = &mut have[at];
            slot.view.setFrame(ns_rect(field.rect));
            slot.view
                .setPlaceholderString(Some(&NSString::from_str(&field.placeholder)));
            // Typed text reaches the draft (and so the scene) event by event, so a difference means the scene moved on
            // (a draft cleared after a send); an unchanged text is left alone to keep the cursor and selection.
            if slot.view.stringValue().to_string() != field.text {
                slot.view.setStringValue(&NSString::from_str(&field.text));
            }
        }
        created
    }

    /// Whether a native field shows other text than the scene's, so `set_fields` has to run although the scene is the same.
    pub(crate) fn field_text_differs(&self, fields: &[Field]) -> bool {
        let have = self.ivars().fields.borrow();
        fields.iter().any(|f| {
            have.iter()
                .find(|slot| slot.key == f.key)
                .is_some_and(|slot| slot.view.stringValue().to_string() != f.text)
        })
    }

    /// The native text field with `Field.key` `key`, if this node has it.
    pub(crate) fn field(&self, key: &str) -> Option<Retained<NSTextField>> {
        self.ivars()
            .fields
            .borrow()
            .iter()
            .find(|slot| slot.key == key)
            .map(|slot| slot.view.clone())
    }

    /// The first native text field of this node, if any.
    pub(crate) fn first_field(&self) -> Option<Retained<NSTextField>> {
        self.ivars()
            .fields
            .borrow()
            .first()
            .map(|slot| slot.view.clone())
    }

    /// A ring in `rect`; `None` removes it. With `spin` it rotates (in the render server), otherwise it stands still: the
    /// caller passes `spin` only while the node is visible in an expanded panel and Reduce Motion is off.
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
