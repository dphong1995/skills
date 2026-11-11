//! One layer-backed view per scene node: draws its own commands and hosts its fields and spinner.

use std::cell::RefCell;
use std::sync::OnceLock;

use claude_sessions_core::scene::{
    Appearance, DrawCmd, Field, Rect, Tip, appeared_symbols, colors_only_changed,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAppearanceCustomization, NSColor, NSControl, NSControlTextEditingDelegate, NSEvent,
    NSTextField, NSTextFieldDelegate, NSTextView, NSView,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGPath;
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSNumber, NSObject, NSObjectProtocol, NSRect, NSString,
    ns_string,
};
use objc2_quartz_core::{
    CABasicAnimation, CALayer, CAMediaTiming, CAShapeLayer, CATransaction, CATransition,
};

use super::draw::{draw_cmds, ns_rect};
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
}

/// A native text field of the node, its delegate (AppKit keeps delegates unretained) and the text the scene last set.
struct FieldSlot {
    key: String,
    view: Retained<NSTextField>,
    _delegate: Retained<FieldDelegate>,
    applied: String,
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
            if command != sel!(insertNewline:) {
                return Bool::NO;
            }
            (self.ivars().sink)(UiEvent::FieldSubmit {
                key: self.ivars().key.clone(),
            });
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
        });
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: ns_rect(frame)] };
        this.setWantsLayer(true);
        this
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
    /// field's `FieldChanged` and `FieldSubmit` events.
    pub(crate) fn set_fields(&self, fields: &[Field], sink: &EventHandler) {
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
                    have.push(FieldSlot {
                        key: field.key.clone(),
                        view,
                        _delegate: delegate,
                        applied: field.text.clone(),
                    });
                    have.len() - 1
                }
            };
            let slot = &mut have[at];
            slot.view.setFrame(ns_rect(field.rect));
            slot.view
                .setPlaceholderString(Some(&NSString::from_str(&field.placeholder)));
            if slot.applied != field.text && slot.view.currentEditor().is_none() {
                slot.applied.clone_from(&field.text);
                if slot.view.stringValue().to_string() != field.text {
                    slot.view.setStringValue(&NSString::from_str(&field.text));
                }
            }
        }
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
