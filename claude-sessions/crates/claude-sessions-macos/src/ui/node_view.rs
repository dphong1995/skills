//! One layer-backed view per scene node: draws its own commands and hosts its fields and spinner.

use std::cell::RefCell;
use std::sync::OnceLock;

use claude_sessions_core::scene::{Appearance, DrawCmd, Field, Rect};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSAppearanceCustomization, NSColor, NSEvent, NSTextField, NSView};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGPath;
use objc2_foundation::{MainThreadMarker, NSNumber, NSObjectProtocol, NSRect, NSString, ns_string};
use objc2_quartz_core::{CABasicAnimation, CAMediaTiming, CAShapeLayer};

use super::draw::{draw_cmds, ns_rect};

pub(crate) struct NodeIvars {
    key: String,
    cmds: RefCell<Vec<DrawCmd>>,
    fields: RefCell<Vec<(String, Retained<NSTextField>)>>,
    spinner: RefCell<Option<Retained<CAShapeLayer>>>,
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
        });
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: ns_rect(frame)] };
        this.setWantsLayer(true);
        this
    }

    pub(crate) fn set_cmds(&self, cmds: &[DrawCmd]) {
        *self.ivars().cmds.borrow_mut() = cmds.to_vec();
        self.setNeedsDisplay(true);
    }

    /// Adds, moves and removes the native text fields; a field being edited keeps the user's text.
    pub(crate) fn set_fields(&self, fields: &[Field]) {
        let mtm = self.mtm();
        let mut have = self.ivars().fields.borrow_mut();
        have.retain(|(key, view)| {
            let keep = fields.iter().any(|f| &f.key == key);
            if !keep {
                view.removeFromSuperview();
            }
            keep
        });
        for field in fields {
            let existing = have
                .iter()
                .find(|(key, _)| *key == field.key)
                .map(|(_, v)| v.clone());
            let view = existing.unwrap_or_else(|| {
                let view = NSTextField::initWithFrame(mtm.alloc(), ns_rect(field.rect));
                self.addSubview(&view);
                have.push((field.key.clone(), view.clone()));
                view
            });
            view.setFrame(ns_rect(field.rect));
            view.setPlaceholderString(Some(&NSString::from_str(&field.placeholder)));
            if view.currentEditor().is_none() && view.stringValue().to_string() != field.text {
                view.setStringValue(&NSString::from_str(&field.text));
            }
        }
    }

    /// The first native text field of this node, if any.
    pub(crate) fn first_field(&self) -> Option<Retained<NSTextField>> {
        self.ivars().fields.borrow().first().map(|(_, v)| v.clone())
    }

    /// A rotating ring in `rect`; `None` removes it. The rotation runs in the render server.
    pub(crate) fn set_spinner(&self, rect: Option<Rect>) {
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
        host.addSublayer(&layer);
        *slot = Some(layer);
    }
}
