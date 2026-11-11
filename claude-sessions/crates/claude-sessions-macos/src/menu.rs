//! The main menu (Edit items, so ⌘V reaches a text field) and the right-click popup.

use claude_sessions_core::scene::Point;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSMenuItem,
};
use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSString, ns_string,
};

use crate::ui::SceneView;

pub struct MenuItem<'a> {
    pub title: &'a str,
    pub id: isize,
}

struct TargetIvars {
    on_pick: Box<dyn Fn(isize)>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetIvars]
    struct MenuTarget;

    unsafe impl NSObjectProtocol for MenuTarget {}

    impl MenuTarget {
        #[unsafe(method(pick:))]
        fn pick(&self, sender: &NSMenuItem) {
            (self.ivars().on_pick)(sender.tag());
        }
    }
);

fn item(
    mtm: MainThreadMarker,
    title: &str,
    key: &str,
    action: Option<objc2::runtime::Sel>,
) -> Retained<NSMenuItem> {
    // SAFETY: the action is `None` or a selector the target answers; the strings are valid.
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    }
}

/// Installs a main menu with the standard Edit items. The menu bar stays hidden (accessory app) but the key
/// equivalents still reach the first responder.
pub fn install_main_menu() -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let bar = NSMenu::new(mtm);
    let edit_holder = item(mtm, "Edit", "", None);
    let edit = NSMenu::initWithTitle(mtm.alloc(), ns_string!("Edit"));
    for (title, key, action) in [
        ("Undo", "z", sel!(undo:)),
        ("Cut", "x", sel!(cut:)),
        ("Copy", "c", sel!(copy:)),
        ("Paste", "v", sel!(paste:)),
        ("Select All", "a", sel!(selectAll:)),
    ] {
        edit.addItem(&item(mtm, title, key, Some(action)));
    }
    edit_holder.setSubmenu(Some(&edit));
    bar.addItem(&edit_holder);
    NSApplication::sharedApplication(mtm).setMainMenu(Some(&bar));
    true
}

/// Pops up a menu at `at` (scene points) in `view` and calls `on_pick` with the chosen item's id. This spins a nested
/// run loop: call it from a later main-queue turn with no state borrowed. False when nothing could be shown.
pub fn popup(
    view: &SceneView,
    items: &[MenuItem],
    at: Point,
    on_pick: impl Fn(isize) + 'static,
) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let target = MenuTarget::alloc(mtm).set_ivars(TargetIvars {
        on_pick: Box::new(on_pick),
    });
    // SAFETY: NSObject's `init`.
    let target: Retained<MenuTarget> = unsafe { msg_send![super(target), init] };
    let menu = NSMenu::new(mtm);
    for entry in items {
        let entry_item = item(mtm, entry.title, "", Some(sel!(pick:)));
        // SAFETY: the target outlives the popup, which returns only after the action ran.
        unsafe { entry_item.setTarget(Some(&target as &AnyObject)) };
        entry_item.setTag(entry.id);
        menu.addItem(&entry_item);
    }
    menu.popUpMenuPositioningItem_atLocation_inView(None, NSPoint::new(at.x, at.y), Some(view))
}

/// Whether the main menu answers ⌘`key` (tests: the Edit items must handle the clipboard shortcuts).
pub fn handles_command_key(key: &str) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(menu) = NSApplication::sharedApplication(mtm).mainMenu() else {
        return false;
    };
    let ns_key = NSString::from_str(key);
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags::Command,
        0.0,
        0,
        None,
        &ns_key,
        &ns_key,
        false,
        0,
    )
    .is_some_and(|event| menu.performKeyEquivalent(&event))
}
