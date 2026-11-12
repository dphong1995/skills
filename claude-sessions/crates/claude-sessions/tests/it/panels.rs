//! The pointer tests of a scene: which row is under the mouse, and whether it is over the widget at all.

use claude_sessions::panels::{inside_widget, place_scene, row_at};
use claude_sessions_core::layout::{Anchor, HAnchor};
use claude_sessions_core::scene::{Node, NodeKey, Point, Rect, Scene, Size};

fn node(key: &str, frame: Rect, scroll: Option<Size>) -> Node {
    Node {
        key: NodeKey(key.into()),
        frame,
        cmds: vec![],
        hits: vec![],
        fields: vec![],
        spinner: None,
        tips: vec![],
        scroll,
        shadow: None,
        material: None,
    }
}

/// A panel, then cards a and b directly under it.
fn plain() -> Scene {
    Scene {
        nodes: vec![
            node("panel", Rect::new(0.0, 0.0, 300.0, 200.0), None),
            node("card-a", Rect::new(10.0, 40.0, 280.0, 50.0), None),
            node("card-b", Rect::new(10.0, 100.0, 280.0, 50.0), None),
        ],
        ..Scene::default()
    }
}

#[test]
fn the_row_under_the_pointer_is_a_card_not_the_panel() {
    let scene = plain();
    assert_eq!(
        row_at(&scene, Point::new(20.0, 50.0), 0.0),
        Some("a".into())
    );
    assert_eq!(
        row_at(&scene, Point::new(20.0, 110.0), 0.0),
        Some("b".into())
    );
    assert_eq!(
        row_at(&scene, Point::new(20.0, 95.0), 0.0),
        None,
        "between cards"
    );
}

/// A panel, a viewport over y 60..160, and cards in the content: `a` at 60, `b` at 200 (past the viewport).
fn scrolling() -> Scene {
    Scene {
        nodes: vec![
            node("panel", Rect::new(0.0, 0.0, 300.0, 160.0), None),
            node(
                "scroll",
                Rect::new(0.0, 60.0, 300.0, 100.0),
                Some(Size::new(300.0, 300.0)),
            ),
            node("card-a", Rect::new(10.0, 60.0, 280.0, 50.0), None),
            node("card-b", Rect::new(10.0, 200.0, 280.0, 50.0), None),
        ],
        ..Scene::default()
    }
}

#[test]
fn a_scrolled_row_is_found_through_the_scroll_offset() {
    let scene = scrolling();
    assert_eq!(
        row_at(&scene, Point::new(20.0, 70.0), 0.0),
        Some("a".into())
    );
    // Scrolled by 120: `b` (content y 200) shows at y 80 of the viewport.
    assert_eq!(
        row_at(&scene, Point::new(20.0, 80.0), 120.0),
        Some("b".into())
    );
    assert_eq!(
        row_at(&scene, Point::new(20.0, 80.0), 0.0),
        Some("a".into())
    );
}

#[test]
fn a_scrolled_row_does_not_answer_outside_the_viewport() {
    let scene = scrolling();
    // Above the viewport, over the header: the content under it is not visible.
    assert_eq!(row_at(&scene, Point::new(20.0, 30.0), 60.0), None);
}

#[test]
fn inside_is_the_first_node_so_scrolled_frames_past_the_card_do_not_count() {
    let scene = scrolling();
    assert!(inside_widget(&scene, Point::new(20.0, 100.0)));
    assert!(
        !inside_widget(&scene, Point::new(20.0, 220.0)),
        "card-b's frame lies past the panel"
    );
    assert!(!inside_widget(&Scene::default(), Point::new(0.0, 0.0)));
}

#[test]
fn place_scene_moves_grow_origin() {
    for (key, size) in [
        ("panel", Size::new(400.0, 200.0)),
        ("pill", Size::new(126.0, 30.0)),
    ] {
        for h in [HAnchor::Leading, HAnchor::Center, HAnchor::Trailing] {
            for up in [false, true] {
                let scene = Scene {
                    size,
                    nodes: vec![node(key, Rect::new(0.0, 0.0, size.w, size.h), None)],
                    ..Scene::default()
                };
                let placed = place_scene(scene, Anchor { h, up });
                let f = placed.nodes[0].frame;
                let want = Point::new(
                    match h {
                        HAnchor::Leading => f.x,
                        HAnchor::Center => f.x + f.w / 2.0,
                        HAnchor::Trailing => f.x + f.w,
                    },
                    if up { f.y + f.h } else { f.y },
                );
                assert_eq!(placed.grow_origin, want, "{key} {h:?} up={up}");
            }
        }
    }
    // One absolute case: a top-right panel in the 508 x 640 canvas grows from its top-right corner.
    let scene = Scene {
        size: Size::new(400.0, 200.0),
        nodes: vec![node("panel", Rect::new(0.0, 0.0, 400.0, 200.0), None)],
        ..Scene::default()
    };
    let placed = place_scene(scene, Anchor::default());
    assert_eq!(placed.grow_origin, Point::new(494.0, 14.0));
}
