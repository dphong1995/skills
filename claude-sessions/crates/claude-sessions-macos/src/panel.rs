//! `WidgetPanel`: the non-activating panel the widget lives in, created like `main.swift:59-70`.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSPanel, NSScreen, NSStatusWindowLevel,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSRect};

#[derive(Default)]
pub struct PanelIvars {
    wants_key: Cell<bool>,
}

define_class!(
    #[unsafe(super = NSPanel)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PanelIvars]
    pub struct WidgetPanel;

    unsafe impl NSObjectProtocol for WidgetPanel {}

    impl WidgetPanel {
        /// Only while keyboard mode wants it, as Swift's `WidgetPanel.canBecomeKey`.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            self.ivars().wants_key.get()
        }

        /// The widget may sit anywhere, also partly off screen while dragged.
        #[unsafe(method(constrainFrameRect:toScreen:))]
        fn constrain_frame_rect(&self, frame: NSRect, _screen: Option<&NSScreen>) -> NSRect {
            frame
        }
    }
);

impl WidgetPanel {
    pub fn set_wants_key(&self, wants: bool) {
        self.ivars().wants_key.set(wants);
    }
}

/// A read-back of the properties the widget depends on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelProps {
    pub style_mask: usize,
    pub level: isize,
    pub collection_behavior: usize,
    pub has_shadow: bool,
    pub can_become_key: bool,
    pub hides_on_deactivate: bool,
    pub is_opaque: bool,
    pub ignores_cycle: bool,
}

/// A hidden-until-shown panel with the widget's properties; mouse events pass through until told otherwise.
pub fn new_widget_panel(mtm: MainThreadMarker, frame: NSRect) -> Retained<WidgetPanel> {
    let this = WidgetPanel::alloc(mtm).set_ivars(PanelIvars::default());
    // SAFETY: NSPanel's designated initialiser with valid arguments.
    let panel: Retained<WidgetPanel> = unsafe {
        msg_send![
            super(this),
            initWithContentRect: frame,
            styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    // SAFETY: the panel is owned by a `Retained` and never released by closing.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(false);
    panel.setLevel(NSStatusWindowLevel);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    panel.setHidesOnDeactivate(false);
    panel.setIgnoresMouseEvents(true);
    panel.setAcceptsMouseMovedEvents(true);
    panel
}

pub fn props(panel: &WidgetPanel) -> PanelProps {
    PanelProps {
        style_mask: panel.styleMask().0,
        level: panel.level(),
        collection_behavior: panel.collectionBehavior().0,
        has_shadow: panel.hasShadow(),
        can_become_key: panel.canBecomeKeyWindow(),
        hides_on_deactivate: panel.hidesOnDeactivate(),
        is_opaque: panel.isOpaque(),
        ignores_cycle: panel
            .collectionBehavior()
            .contains(NSWindowCollectionBehavior::IgnoresCycle),
    }
}
