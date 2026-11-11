//! GUI-crate tests: `harness = false`. Set `CLAUDE_SESSIONS_GUI_TESTS=1` to run them.
//!
//! `defaults_reads_swift_values` uses only the throwaway suite `claude-sessions-test-<pid>` (removed at the end, also on
//! failure); it never touches the real `local.claude-sessions` domain and opens no window.

use std::process::Command;

use claude_sessions_core::kv::KeyValueStore;
use claude_sessions_macos::defaults::UserDefaultsStore;
use claude_sessions_macos::ui::read_png_pixel;
use serde_json::{Map, Value, json};

const CLOSED_SESSIONS: &[u8] =
    include_bytes!("../../claude-sessions-core/tests/fixtures/closed-sessions-swift.json");
const HOTKEY_COMBO: &[u8] =
    include_bytes!("../../claude-sessions-core/tests/fixtures/hotkey-combo-swift.json");

/// The plist Swift's `UserDefaults.set` writes for these values: doubles are `real`, `[String: [Double]]` a dict of real arrays.
const SWIFT_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>pillOrigins</key>
	<dict>
		<key>UUID-A</key>
		<array>
			<real>12</real>
			<real>34</real>
		</array>
	</dict>
	<key>pillX</key>
	<real>100</real>
	<key>pillY</key>
	<real>200</real>
	<key>pillWidth</key>
	<real>232</real>
	<key>bgOpacity</key>
	<real>0.5</real>
	<key>muted</key>
	<true/>
	<key>prPrompt</key>
	<string>x {url}</string>
	<key>jiraProjects</key>
	<dict>
		<key>ABC</key>
		<string>acme.atlassian.net</string>
	</dict>
	<key>hotkeyNext</key>
	<string>off</string>
</dict>
</plist>
"#;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn defaults(args: &[&str]) -> Result<String, String> {
    let out = Command::new("/usr/bin/defaults")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "defaults {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn defaults_reads_swift_values(suite: &str) -> Result<(), String> {
    let plist = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{suite}.plist"));
    std::fs::write(&plist, SWIFT_PLIST).map_err(|e| e.to_string())?;
    defaults(&["import", suite, plist.to_str().ok_or("path")?])?;
    std::fs::remove_file(&plist).ok();
    defaults(&[
        "write",
        suite,
        "closedSessions",
        "-data",
        &hex(CLOSED_SESSIONS),
    ])?;
    defaults(&["write", suite, "hotkeyOpen", "-data", &hex(HOTKEY_COMBO)])?;
    let swift_export = defaults(&["export", suite, "-"])?;

    let store = UserDefaultsStore::new(Some(suite)).ok_or("suite")?;
    let check = |ok: bool, what: &str| {
        if ok {
            Ok(())
        } else {
            Err(format!("read {what}"))
        }
    };
    check(
        store.get_dict("pillOrigins") == json!({"UUID-A": [12.0, 34.0]}).as_object().cloned(),
        "pillOrigins",
    )?;
    check(store.get_f64("pillX") == Some(100.0), "pillX")?;
    check(store.get_f64("pillY") == Some(200.0), "pillY")?;
    check(store.get_f64("pillWidth") == Some(232.0), "pillWidth")?;
    check(store.get_f64("bgOpacity") == Some(0.5), "bgOpacity")?;
    check(store.get_bool("muted") == Some(true), "muted")?;
    check(
        store.get_string("prPrompt").as_deref() == Some("x {url}"),
        "prPrompt",
    )?;
    check(
        store.get_dict("jiraProjects") == json!({"ABC": "acme.atlassian.net"}).as_object().cloned(),
        "jiraProjects",
    )?;
    check(
        store.get_bytes("closedSessions").as_deref() == Some(CLOSED_SESSIONS),
        "closedSessions",
    )?;
    check(
        store.get_bytes("hotkeyOpen").as_deref() == Some(HOTKEY_COMBO),
        "hotkeyOpen",
    )?;
    check(
        store.get_string("hotkeyNext").as_deref() == Some("off"),
        "hotkeyNext",
    )?;
    // A value of another kind reads as absent.
    check(store.get_f64("muted").is_none(), "muted as f64")?;
    check(store.get_string("pillX").is_none(), "pillX as string")?;
    check(store.get_dict("nothing").is_none(), "absent key")?;

    // Write each value through Rust: the exported plist has the same types as the Swift-typed one.
    for key in [
        "pillOrigins",
        "pillX",
        "pillY",
        "pillWidth",
        "bgOpacity",
        "muted",
        "prPrompt",
        "jiraProjects",
        "closedSessions",
        "hotkeyOpen",
        "hotkeyNext",
    ] {
        store.remove(key);
    }
    let origins: Map<String, Value> = json!({"UUID-A": [12.0, 34.0]})
        .as_object()
        .cloned()
        .unwrap();
    let jira: Map<String, Value> = json!({"ABC": "acme.atlassian.net"})
        .as_object()
        .cloned()
        .unwrap();
    store.set_dict("pillOrigins", &origins);
    store.set_f64("pillX", 100.0);
    store.set_f64("pillY", 200.0);
    store.set_f64("pillWidth", 232.0);
    store.set_f64("bgOpacity", 0.5);
    store.set_bool("muted", true);
    store.set_string("prPrompt", "x {url}");
    store.set_dict("jiraProjects", &jira);
    store.set_bytes("closedSessions", CLOSED_SESSIONS);
    store.set_bytes("hotkeyOpen", HOTKEY_COMBO);
    store.set_string("hotkeyNext", "off");
    let rust_export = defaults(&["export", suite, "-"])?;
    if rust_export != swift_export {
        return Err(format!(
            "plist types differ\n--- swift ---\n{swift_export}\n--- rust ---\n{rust_export}"
        ));
    }
    Ok(())
}

/// `open_https` with a recording opener: nothing is ever opened for real.
fn open_https_guards_the_scheme() -> Result<(), String> {
    use claude_sessions_macos::workspace::open_https_with;
    let opened = |input: &str| -> Result<bool, String> {
        let Ok(url) = url::Url::parse(input) else {
            return Ok(false);
        };
        let mut called = false;
        let ok = open_https_with(&url, |_| {
            called = true;
            true
        });
        if ok != called {
            return Err(format!("{input}: result {ok} but opener called {called}"));
        }
        Ok(called)
    };
    for bad in [
        "file:///tmp/x",
        "vscode://x",
        "-x",
        "http://example.com/",
        "https://u:p@example.com/",
    ] {
        if opened(bad)? {
            return Err(format!("{bad} reached the opener"));
        }
    }
    if !opened("https://example.com")? {
        return Err("https://example.com was refused".into());
    }
    Ok(())
}

/// Transcript links may be `http`; every other guard stays.
fn open_link_allow_http_guards() -> Result<(), String> {
    use claude_sessions_macos::workspace::open_link_allow_http_with;
    let opened = |input: &str| -> bool {
        let url = url::Url::parse(input).unwrap();
        let mut called = false;
        let ok = open_link_allow_http_with(&url, |_| {
            called = true;
            true
        });
        assert_eq!(ok, called);
        called
    };
    for good in ["http://example.com/a", "https://example.com/a"] {
        if !opened(good) {
            return Err(format!("{good} was refused"));
        }
    }
    for bad in [
        "file:///tmp/x",
        "ftp://example.com/",
        "http://u:p@example.com/",
        "https://u@example.com/",
    ] {
        if opened(bad) {
            return Err(format!("{bad} reached the opener"));
        }
    }
    Ok(())
}

fn field_scene(text: &str, label: &str) -> claude_sessions_core::scene::Scene {
    use claude_sessions_core::scene::{Field, Node, NodeKey, Rect, Scene, Size};
    Scene {
        size: Size { w: 200.0, h: 60.0 },
        nodes: vec![Node {
            key: NodeKey("form".into()),
            frame: Rect::new(0.0, 0.0, 200.0, 60.0),
            cmds: vec![],
            hits: vec![],
            fields: vec![Field {
                key: "email".into(),
                rect: Rect::new(10.0, 10.0, 180.0, 24.0),
                text: text.into(),
                placeholder: label.into(),
            }],
            spinner: None,
            tips: vec![],
            scroll: None,
            shadow: None,
        }],
        images: Default::default(),
    }
}

/// Typed text reaches the app as events and survives scenes that did not change the field's text.
fn field_text_flows_and_survives() -> Result<(), String> {
    use claude_sessions_macos::ui::{SceneView, UiEvent};
    use objc2::sel;
    use objc2_app_kit::{
        NSControlTextDidChangeNotification, NSControlTextEditingDelegate, NSTextView,
    };
    use objc2_foundation::{MainThreadMarker, NSNotificationCenter, NSSize};
    use std::cell::RefCell;
    use std::rc::Rc;

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 60.0));
    let events: Rc<RefCell<Vec<UiEvent>>> = Rc::default();
    let sink = events.clone();
    view.set_handler(Rc::new(move |e| sink.borrow_mut().push(e)));
    view.set_scene(&field_scene("", "Email"));
    let field = view.first_field().ok_or("no field was created")?;

    // The user types; the scene is re-sent with the same (empty) text but another placeholder.
    field.setStringValue(&objc2_foundation::NSString::from_str("me@x.io"));
    // SAFETY: a plain notification post for the field, which is its own delegate source.
    unsafe {
        NSNotificationCenter::defaultCenter()
            .postNotificationName_object(NSControlTextDidChangeNotification, Some(&field));
    }
    // The draft mirrors what was typed, so the next scene carries it and leaves the field as it is.
    view.set_scene(&field_scene("me@x.io", "Email address"));
    if field.stringValue().to_string() != "me@x.io" {
        return Err(format!("typed text wiped: {:?}", field.stringValue()));
    }
    // The draft is cleared (a send) while the scene itself is the same as before: the field still follows it.
    view.set_scene(&field_scene("", "Email address"));
    view.set_scene(&field_scene("", "Email address"));
    if !field.stringValue().to_string().is_empty() {
        return Err(format!("cleared draft kept: {:?}", field.stringValue()));
    }
    field.setStringValue(&objc2_foundation::NSString::from_str("late"));
    view.set_scene(&field_scene("", "Address"));
    if !field.stringValue().to_string().is_empty() {
        return Err(format!("stale field text kept: {:?}", field.stringValue()));
    }
    // The scene asks for another text: it is applied.
    view.set_scene(&field_scene("other", "Email address"));
    if field.stringValue().to_string() != "other" {
        return Err(format!("scene text ignored: {:?}", field.stringValue()));
    }

    let delegate = field.delegate().ok_or("no delegate")?;
    let text_view = NSTextView::new(mtm);
    // SAFETY: plain delegate calls with live objects.
    let (newline, other) = unsafe {
        (
            delegate.control_textView_doCommandBySelector(&field, &text_view, sel!(insertNewline:)),
            delegate.control_textView_doCommandBySelector(&field, &text_view, sel!(moveLeft:)),
        )
    };
    if !newline || other {
        return Err(format!(
            "Return consumed {newline}, other keys consumed {other}"
        ));
    }
    let want = vec![
        UiEvent::FieldChanged {
            key: "email".into(),
            text: "me@x.io".into(),
        },
        UiEvent::FieldSubmit {
            key: "email".into(),
        },
    ];
    if *events.borrow() != want {
        return Err(format!("events: {:?}", events.borrow()));
    }
    Ok(())
}

/// A scroll host that went away takes its scroll view out of the view tree.
fn scroll_host_is_removed() -> Result<(), String> {
    use claude_sessions_core::scene::{Node, Rect, Size};
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 60.0));
    let mut scene = field_scene("", "x");
    let plain = scene.clone();
    scene.nodes.insert(
        0,
        Node {
            key: "scroller".into(),
            frame: Rect::new(0.0, 0.0, 200.0, 60.0),
            cmds: vec![],
            hits: vec![],
            fields: vec![],
            spinner: None,
            tips: vec![],
            scroll: Some(Size { w: 200.0, h: 400.0 }),
            shadow: None,
        },
    );
    let base = view.subviews().len();
    for _ in 0..3 {
        view.set_scene(&scene);
        view.set_scene(&plain);
    }
    let after = view.subviews().len();
    // Without the scroll node every subview is a node view (the faded-out ones may still be leaving).
    let nodes = plain.nodes.len();
    if after > base + nodes {
        return Err(format!(
            "{after} subviews after 3 cycles, started with {base}"
        ));
    }
    Ok(())
}

/// Scene with the real list's node order: panel background, header, the scroll node, one scrolled card.
fn panel_scroll_scene(with_scroll: bool) -> claude_sessions_core::scene::Scene {
    use claude_sessions_core::scene::{Node, Rect, Scene, Size};
    let node = |key: &str, frame: Rect, scroll: Option<Size>| Node {
        key: key.into(),
        frame,
        cmds: vec![],
        hits: vec![],
        fields: vec![],
        spinner: None,
        tips: vec![],
        scroll,
        shadow: None,
    };
    let mut nodes = vec![
        node("panel", Rect::new(0.0, 0.0, 200.0, 100.0), None),
        node("header", Rect::new(0.0, 0.0, 200.0, 20.0), None),
    ];
    if with_scroll {
        nodes.push(node(
            "scroll",
            Rect::new(0.0, 20.0, 200.0, 80.0),
            Some(Size { w: 200.0, h: 400.0 }),
        ));
        nodes.push(node("card", Rect::new(0.0, 20.0, 200.0, 40.0), None));
    }
    Scene {
        size: Size { w: 200.0, h: 100.0 },
        nodes,
        images: Default::default(),
    }
}

/// The SceneView's subviews bottom to top as "scroll" or "node".
fn subview_kinds(view: &claude_sessions_macos::ui::SceneView) -> Vec<&'static str> {
    use objc2::ClassType;
    use objc2_app_kit::NSScrollView;
    use objc2_foundation::NSObjectProtocol;
    view.subviews()
        .iter()
        .map(|v| {
            if v.isKindOfClass(NSScrollView::class()) {
                "scroll"
            } else {
                "node"
            }
        })
        .collect()
}

/// Views stack in scene node order: panel, header, then the scroll view (the scroll node's place); the scroll view must not sit
/// under the panel background.
fn scroll_view_stacks_above_panel_when_in_first_scene() -> Result<(), String> {
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&panel_scroll_scene(true));
    let kinds = subview_kinds(&view);
    if kinds != ["node", "node", "scroll"] {
        return Err(format!(
            "subviews bottom to top {kinds:?}, want [\"node\", \"node\", \"scroll\"] (panel, header, scroll)"
        ));
    }
    Ok(())
}

/// The same order when the scroll node only appears in a later scene.
fn scroll_view_stacks_above_panel_when_added_later() -> Result<(), String> {
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&panel_scroll_scene(false));
    view.set_scene(&panel_scroll_scene(true));
    let kinds = subview_kinds(&view);
    if kinds != ["node", "node", "scroll"] {
        return Err(format!(
            "subviews bottom to top {kinds:?}, want [\"node\", \"node\", \"scroll\"] (panel, header, scroll)"
        ));
    }
    Ok(())
}

/// A node added in a later scene before the scroll node in scene order stays under the scroll view.
fn scroll_view_stays_above_a_node_added_later_before_it() -> Result<(), String> {
    use claude_sessions_core::scene::{Node, Rect};
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    let mut scene = panel_scroll_scene(true);
    view.set_scene(&scene);
    scene.nodes.insert(
        2,
        Node {
            key: "banner".into(),
            frame: Rect::new(0.0, 0.0, 200.0, 10.0),
            cmds: vec![],
            hits: vec![],
            fields: vec![],
            spinner: None,
            tips: vec![],
            scroll: None,
            shadow: None,
        },
    );
    view.set_scene(&scene);
    let kinds = subview_kinds(&view);
    if kinds != ["node", "node", "node", "scroll"] {
        return Err(format!(
            "subviews bottom to top {kinds:?}, want node, node, node, scroll"
        ));
    }
    Ok(())
}

/// `scroll_offset` is 0 without a scroll host and follows the clip view's bounds origin once the list scrolls.
fn scroll_offset_follows_the_clip_view() -> Result<(), String> {
    use claude_sessions_core::scene::{Node, Rect, Size};
    use claude_sessions_macos::ui::SceneView;
    use objc2::rc::Retained;
    use objc2_app_kit::NSScrollView;
    use objc2_foundation::{MainThreadMarker, NSPoint, NSSize};

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 60.0));
    let mut scene = field_scene("", "x");
    if view.scroll_offset() != 0.0 {
        return Err("offset without a scene".into());
    }
    scene.nodes.insert(
        0,
        Node {
            key: "scroller".into(),
            frame: Rect::new(0.0, 0.0, 200.0, 60.0),
            cmds: vec![],
            hits: vec![],
            fields: vec![],
            spinner: None,
            tips: vec![],
            scroll: Some(Size { w: 200.0, h: 400.0 }),
            shadow: None,
        },
    );
    view.set_scene(&scene);
    if view.scroll_offset() != 0.0 {
        return Err(format!("unscrolled offset {}", view.scroll_offset()));
    }
    let host: Retained<NSScrollView> = view
        .subviews()
        .iter()
        .find_map(|v| v.downcast::<NSScrollView>().ok())
        .ok_or("no scroll view")?;
    host.contentView().scrollToPoint(NSPoint::new(0.0, 40.0));
    if view.scroll_offset() != 40.0 {
        return Err(format!("scrolled offset {}", view.scroll_offset()));
    }
    Ok(())
}

/// Fields draw their text and nodes after the scroll node stay inside its viewport.
fn png_draws_fields_and_clips_scroll() -> Result<(), String> {
    use claude_sessions_core::scene::{Appearance, Rect, Size};
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let blank = dir.join("field-blank.png");
    let filled = dir.join("field-filled.png");
    claude_sessions_macos::ui::render_png(&field_scene("", ""), 1.0, Appearance::Light, &blank)?;
    claude_sessions_macos::ui::render_png(
        &field_scene("hello", ""),
        1.0,
        Appearance::Light,
        &filled,
    )?;
    let diff = claude_sessions_macos::ui::compare_png(&blank, &filled, 8)?;
    if diff.differing == 0 {
        return Err("a field with text renders like an empty one".into());
    }
    // A node after the scroll node, partly below the viewport, must not paint outside it.
    let mut scene = field_scene("", "");
    scene.nodes.clear();
    let node = |key: &str, frame: Rect, scroll: Option<Size>, fill: bool| {
        use claude_sessions_core::scene::{DrawCmd, Node, ThemeColor};
        Node {
            key: key.into(),
            frame,
            cmds: if fill {
                vec![DrawCmd::RoundedRect {
                    rect: Rect::new(0.0, 0.0, frame.w, frame.h),
                    radius: 0.0,
                    fill: Some(ThemeColor::Blue.opaque()),
                    stroke: None,
                }]
            } else {
                vec![]
            },
            hits: vec![],
            fields: vec![],
            spinner: None,
            tips: vec![],
            scroll,
            shadow: None,
        }
    };
    scene.nodes.push(node(
        "v",
        Rect::new(0.0, 0.0, 200.0, 30.0),
        Some(Size { w: 200.0, h: 60.0 }),
        false,
    ));
    scene
        .nodes
        .push(node("c", Rect::new(0.0, 0.0, 200.0, 60.0), None, true));
    let clipped = dir.join("scroll-clipped.png");
    claude_sessions_macos::ui::render_png(&scene, 1.0, Appearance::Light, &clipped)?;
    let (_, inside) = read_png_pixel(&clipped, 114, 24)?;
    let (_, outside) = read_png_pixel(&clipped, 114, 64)?;
    if inside == outside {
        return Err("content below the viewport was painted".into());
    }
    Ok(())
}

fn shadow_node(
    frame: claude_sessions_core::scene::Rect,
    shadow: bool,
) -> claude_sessions_core::scene::Node {
    use claude_sessions_core::scene::{Color, Node, Point, Shadow, ThemeColor};
    Node {
        key: "card".into(),
        frame,
        cmds: vec![],
        hits: vec![],
        fields: vec![],
        spinner: None,
        tips: vec![],
        scroll: None,
        shadow: shadow.then_some(Shadow {
            color: Color {
                token: ThemeColor::Black,
                alpha: 0.4,
            },
            radius: 10.0,
            offset: Point { x: 0.0, y: 3.0 },
            corner: 12.0,
        }),
    }
}

fn one_node_scene(node: claude_sessions_core::scene::Node) -> claude_sessions_core::scene::Scene {
    use claude_sessions_core::scene::{Scene, Size};
    Scene {
        size: Size { w: 200.0, h: 100.0 },
        nodes: vec![node],
        images: Default::default(),
    }
}

/// The node view's layer (the one subview of a one-node scene).
fn node_layer(
    view: &claude_sessions_macos::ui::SceneView,
) -> Result<objc2::rc::Retained<objc2_quartz_core::CALayer>, String> {
    view.subviews()
        .iter()
        .next()
        .and_then(|v| v.layer())
        .ok_or_else(|| "no node layer".to_string())
}

fn node_shadow_sets_layer_shadow_path() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_core_graphics::CGPath;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&one_node_scene(shadow_node(
        Rect::new(10.0, 10.0, 100.0, 40.0),
        true,
    )));
    let layer = node_layer(&view)?;
    if layer.shadowOpacity() <= 0.0 {
        return Err(format!("shadowOpacity {}", layer.shadowOpacity()));
    }
    if layer.shadowRadius() != 10.0 {
        return Err(format!("shadowRadius {}", layer.shadowRadius()));
    }
    let path = layer.shadowPath().ok_or("no shadowPath")?;
    let b = CGPath::bounding_box(Some(&path));
    if (b.size.width, b.size.height) != (100.0, 40.0) {
        return Err(format!("shadowPath bounds {b:?}"));
    }
    Ok(())
}

fn shadow_path_follows_frame_change() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_core_graphics::CGPath;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&one_node_scene(shadow_node(
        Rect::new(10.0, 10.0, 100.0, 40.0),
        true,
    )));
    view.set_scene(&one_node_scene(shadow_node(
        Rect::new(10.0, 10.0, 100.0, 70.0),
        true,
    )));
    let layer = node_layer(&view)?;
    let path = layer.shadowPath().ok_or("no shadowPath")?;
    let b = CGPath::bounding_box(Some(&path));
    if (b.size.width, b.size.height) != (100.0, 70.0) {
        return Err(format!("shadowPath bounds {b:?}"));
    }
    Ok(())
}

fn node_without_shadow_has_none() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&one_node_scene(shadow_node(
        Rect::new(10.0, 10.0, 100.0, 40.0),
        false,
    )));
    let layer = node_layer(&view)?;
    if layer.shadowOpacity() != 0.0 {
        return Err(format!("shadowOpacity {}", layer.shadowOpacity()));
    }
    Ok(())
}

fn render_png_pads_margin_and_draws_shadow() -> Result<(), String> {
    use claude_sessions_core::scene::{Appearance, Rect};
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shadow.png");
    let scale = 2.0;
    let scene = one_node_scene(shadow_node(Rect::new(40.0, 20.0, 100.0, 40.0), true));
    claude_sessions_macos::ui::render_png(&scene, scale, Appearance::Light, &path)?;
    // Node bottom edge at scene y 60, so bitmap y (60 + 14 + 4) pt; centred horizontally at (40 + 50 + 14) pt.
    let (size, below) = read_png_pixel(&path, (104.0 * scale) as usize, (78.0 * scale) as usize)?;
    if size
        != (
            ((200.0 + 28.0) * scale) as usize,
            ((100.0 + 28.0) * scale) as usize,
        )
    {
        return Err(format!("png size {size:?}"));
    }
    if below[3] <= 0.05 {
        return Err(format!("no shadow below the node: {below:?}"));
    }
    let (_, corner) = read_png_pixel(&path, 0, 0)?;
    if corner[3] >= 0.02 {
        return Err(format!("corner is not transparent: {corner:?}"));
    }
    Ok(())
}

/// Rows (in px) where a render of `"H"` in a rect of height `h` has ink, as (first, last).
fn ink_rows(h: f64, name: &str) -> Result<(usize, usize), String> {
    use claude_sessions_core::scene::{
        Align, Appearance, DrawCmd, FontSpec, Rect, Size, ThemeColor, Weight,
    };
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let scale = 4.0;
    let mut node = shadow_node(Rect::new(0.0, 0.0, 60.0, h), false);
    node.cmds = vec![DrawCmd::Text {
        rect: Rect::new(0.0, 0.0, 60.0, h),
        text: "H".into(),
        font: FontSpec {
            size: 13.0,
            weight: Weight::Regular,
            mono: false,
        },
        color: ThemeColor::Text.opaque(),
        max_lines: 1,
        align: Align::Left,
    }];
    let mut scene = one_node_scene(node);
    scene.size = Size { w: 60.0, h };
    claude_sessions_macos::ui::render_png(&scene, scale, Appearance::Light, &path)?;
    // The PNG may be padded around the scene; find the pad from its width.
    let (size, _) = read_png_pixel(&path, 0, 0)?;
    let margin = (size.0 - (60.0 * scale) as usize) / 2;
    let rows = (h * scale) as usize;
    let mut first = None;
    let mut last = 0;
    for y in 0..rows {
        let mut ink = false;
        for x in 0..(20.0 * scale) as usize {
            let (_, p) = read_png_pixel(&path, margin + x, margin + y)?;
            ink |= p[3] > 0.3;
        }
        if ink {
            first.get_or_insert(y);
            last = y;
        }
    }
    first.map(|f| (f, last)).ok_or_else(|| "no ink".to_string())
}

fn text_centres_vertically_in_taller_rect() -> Result<(), String> {
    let (first, last) = ink_rows(40.0, "centre.png")?;
    let centre = (first + last + 1) as f64 / 2.0 / 4.0;
    if (centre - 20.0).abs() > 0.5 {
        return Err(format!("ink centre {centre} pt, rect centre 20"));
    }
    Ok(())
}

fn single_line_text_keeps_position() -> Result<(), String> {
    // A rect one line tall: 13 pt system font, line height 16.
    let (first, _) = ink_rows(16.0, "one-line.png")?;
    let top = first as f64 / 4.0;
    if (top - PINNED_INK_TOP).abs() > 0.5 {
        return Err(format!("ink top {top} pt, pinned {PINNED_INK_TOP}"));
    }
    Ok(())
}

/// Measured on the code before vertical centring existed.
const PINNED_INK_TOP: f64 = 3.75;

fn chimes_use_the_swift_sounds() -> Result<(), String> {
    use claude_sessions_core::pull_requests::Chime;
    use claude_sessions_macos::sound::sound_name;
    let got = (sound_name(Chime::Done), sound_name(Chime::NeedsYou));
    (got == ("Glass", "Funk"))
        .then_some(())
        .ok_or(format!("{got:?}"))
}

/// The login item is never registered here; only its gate is checked.
fn login_item_gate() -> Result<(), String> {
    use claude_sessions_macos::login_item::allowed;
    use std::path::Path;
    let home = Path::new("/Users/u");
    let ok = allowed(
        false,
        Path::new("/Users/u/Applications/ClaudeSessions.app"),
        home,
    ) && !allowed(
        true,
        Path::new("/Users/u/Applications/ClaudeSessions.app"),
        home,
    ) && !allowed(false, Path::new("/tmp/build/ClaudeSessions.app"), home)
        && !allowed(false, Path::new("/Applications/ClaudeSessions.app"), home);
    ok.then_some(()).ok_or("gate".into())
}

/// The CLI path is looked up by bundle id and never through `PATH`; absent when cmux is not installed.
fn cmux_cli_comes_from_the_bundle() -> Result<(), String> {
    match claude_sessions_macos::workspace::cmux_cli_path() {
        Some(p) if !p.ends_with("Contents/Resources/bin/cmux") => Err(format!("{p:?}")),
        _ => Ok(()),
    }
}

fn main() {
    if std::env::var_os("CLAUDE_SESSIONS_GUI_TESTS").is_none() {
        println!("gui tests skipped (set CLAUDE_SESSIONS_GUI_TESTS=1)");
        return;
    }
    // `CmuxJumpTests` has no case that executes AppleScript (all are pure and ported to the core `cmux` tests), so there
    // is nothing to gate behind `CLAUDE_SESSIONS_CMUX_TESTS=1` ("needs cmux + Automation") here.
    let suite = format!("claude-sessions-test-{}", std::process::id());
    let mut results = vec![(
        "defaults_reads_swift_values",
        defaults_reads_swift_values(&suite),
    )];
    UserDefaultsStore::remove_suite(&suite);
    let _ = defaults(&["delete", &suite]);
    // The domain leaves an empty plist behind; it belongs to the throwaway suite only.
    if let Some(home) = std::env::var_os("HOME") {
        let _ = std::fs::remove_file(
            std::path::Path::new(&home).join(format!("Library/Preferences/{suite}.plist")),
        );
    }
    results.push((
        "open_https_guards_the_scheme",
        open_https_guards_the_scheme(),
    ));
    results.push(("open_link_allow_http_guards", open_link_allow_http_guards()));
    results.push((
        "field_text_flows_and_survives",
        field_text_flows_and_survives(),
    ));
    results.push(("scroll_host_is_removed", scroll_host_is_removed()));
    results.push((
        "scroll_offset_follows_the_clip_view",
        scroll_offset_follows_the_clip_view(),
    ));
    results.push((
        "scroll_view_stacks_above_panel_when_in_first_scene",
        scroll_view_stacks_above_panel_when_in_first_scene(),
    ));
    results.push((
        "scroll_view_stacks_above_panel_when_added_later",
        scroll_view_stacks_above_panel_when_added_later(),
    ));
    results.push((
        "scroll_view_stays_above_a_node_added_later_before_it",
        scroll_view_stays_above_a_node_added_later_before_it(),
    ));
    results.push((
        "png_draws_fields_and_clips_scroll",
        png_draws_fields_and_clips_scroll(),
    ));
    results.push((
        "node_shadow_sets_layer_shadow_path",
        node_shadow_sets_layer_shadow_path(),
    ));
    results.push((
        "shadow_path_follows_frame_change",
        shadow_path_follows_frame_change(),
    ));
    results.push((
        "node_without_shadow_has_none",
        node_without_shadow_has_none(),
    ));
    results.push((
        "render_png_pads_margin_and_draws_shadow",
        render_png_pads_margin_and_draws_shadow(),
    ));
    results.push((
        "text_centres_vertically_in_taller_rect",
        text_centres_vertically_in_taller_rect(),
    ));
    results.push((
        "single_line_text_keeps_position",
        single_line_text_keeps_position(),
    ));
    results.push(("chimes_use_the_swift_sounds", chimes_use_the_swift_sounds()));
    results.push(("login_item_gate", login_item_gate()));
    results.push((
        "cmux_cli_comes_from_the_bundle",
        cmux_cli_comes_from_the_bundle(),
    ));
    let mut failed = false;
    for (name, result) in results {
        match result {
            Ok(()) => println!("test {name} ... ok"),
            Err(e) => {
                println!("test {name} ... FAILED: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
