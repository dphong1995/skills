//! Per-display pill placement over a `MemoryStore`.

use claude_sessions::placement::{
    ORIGINS_KEY, load_origins, migrate_legacy, panel_frame, save_origin, sync,
};
use claude_sessions_core::kv::{KeyValueStore, MemoryStore};
use claude_sessions_core::layout::{CANVAS, COLLAPSED, HAnchor, MARGIN};
use claude_sessions_core::scene::{Point, Rect};
use claude_sessions_macos::screens::KeyedScreen;

const PRIMARY: Rect = Rect::new(0.0, 0.0, 1440.0, 900.0);
/// Above the primary screen (AppKit y grows upward).
const ABOVE: Rect = Rect::new(0.0, 900.0, 1920.0, 1080.0);

fn screen(key: &str, frame: Rect) -> KeyedScreen {
    KeyedScreen {
        key: key.into(),
        frame,
        visible_frame: Rect::new(frame.x, frame.y, frame.w, frame.h - 25.0),
    }
}

fn two_screens() -> Vec<KeyedScreen> {
    vec![screen("UUID-A", PRIMARY), screen("UUID-B", ABOVE)]
}

#[test]
fn pill_origin_restored_per_display() {
    let kv = MemoryStore::default();
    save_origin(&kv, "UUID-A", [30.0, 40.0]);
    save_origin(&kv, "UUID-B", [100.0, 50.0]);
    let placed = sync(&kv, &two_screens());

    // UUID-B: the saved offset from the secondary screen's corner, the card growing up and to the right.
    let b = placed[1];
    assert_eq!(b.origin, Point::new(100.0, 950.0));
    assert_eq!(b.offset, [100.0, 50.0]);
    assert_eq!(b.anchor.h, HAnchor::Leading);
    assert!(b.anchor.up);
    assert_eq!(
        b.frame,
        Rect::new(100.0 - MARGIN, 950.0 - MARGIN, CANVAS.w, CANVAS.h)
    );
    assert_eq!(placed[0].origin, Point::new(30.0, 40.0));
    // Nothing was rewritten for screens that already had an offset.
    assert_eq!(load_origins(&kv)["UUID-B"], vec![100.0, 50.0]);

    // An unknown display gets the default origin: top right of its visible frame, 16 pt in and 10 pt down.
    let mut screens = two_screens();
    screens.push(screen("UUID-C", Rect::new(1920.0, 0.0, 1000.0, 800.0)));
    let kv = MemoryStore::default();
    let placed = sync(&kv, &screens);
    let v = screens[0].visible_frame;
    assert_eq!(
        placed[0].origin,
        Point::new(
            v.max_x() - COLLAPSED.w - 16.0,
            v.max_y() - COLLAPSED.h - 10.0
        )
    );
    // The default is saved, and a display seen again keeps it.
    assert_eq!(load_origins(&kv).len(), 3);
    let again = sync(&kv, &screens);
    assert_eq!(again, placed);
}

#[test]
fn new_display_follows_the_first_saved_one() {
    let kv = MemoryStore::default();
    // The pill hugs the right edge of the primary screen.
    save_origin(&kv, "UUID-A", [PRIMARY.w - COLLAPSED.w - 20.0, 100.0]);
    let placed = sync(&kv, &two_screens());
    // The same distances to the right and bottom edges on the other screen.
    assert_eq!(placed[1].origin.x, ABOVE.max_x() - COLLAPSED.w - 20.0);
    assert_eq!(placed[1].origin.y, ABOVE.min_y() + 100.0);
}

#[test]
fn legacy_pill_origin_is_migrated_once() {
    let kv = MemoryStore::default();
    // Saved by the single-pill build at the old 232 pt width, on the secondary screen.
    kv.set_f64("pillX", 400.0);
    kv.set_f64("pillY", 1000.0);
    let screens = two_screens();
    migrate_legacy(&kv, &screens);
    let origins = load_origins(&kv);
    assert_eq!(origins.keys().collect::<Vec<_>>(), ["UUID-B"]);
    assert_eq!(origins["UUID-B"].len(), 2);
    assert_eq!(origins["UUID-B"][1], 100.0);
    assert!(kv.get_f64("pillX").is_none());
    assert!(kv.get_f64("pillY").is_none());
    assert!(kv.get_f64("pillWidth").is_none());

    // A second pass changes nothing, even when the old keys come back.
    let before = kv.get_dict(ORIGINS_KEY);
    kv.set_f64("pillX", 5.0);
    migrate_legacy(&kv, &screens);
    assert_eq!(kv.get_dict(ORIGINS_KEY), before);
    assert!(kv.get_f64("pillX").is_none());
}

#[test]
fn legacy_keys_do_not_override_saved_origins() {
    let kv = MemoryStore::default();
    save_origin(&kv, "UUID-A", [30.0, 40.0]);
    kv.set_f64("pillX", 400.0);
    kv.set_f64("pillY", 100.0);
    kv.set_f64("pillWidth", 126.0);
    migrate_legacy(&kv, &two_screens());
    assert_eq!(load_origins(&kv)["UUID-A"], vec![30.0, 40.0]);
    assert!(kv.get_f64("pillWidth").is_none());
}

#[test]
fn panel_frame_clamps_into_the_screen() {
    let p = panel_frame(Point::new(5000.0, -50.0), PRIMARY);
    assert_eq!(p.origin, Point::new(PRIMARY.w - COLLAPSED.w, 0.0));
    assert_eq!(p.anchor.h, HAnchor::Trailing);
}
