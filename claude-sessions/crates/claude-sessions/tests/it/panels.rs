//! The pointer tests of a scene: which row is under the mouse, and whether it is over the widget at all.

use claude_sessions::panels::{inside_widget, row_at};
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
