//! GUI-crate tests: `harness = false`. Set `CLAUDE_SESSIONS_GUI_TESTS=1` to run them.
//!
//! `defaults_reads_swift_values` uses only the throwaway suite `claude-sessions-test-<pid>` (removed at the end, also on
//! failure); it never touches the real `local.claude-sessions` domain and opens no window.
//!
//! `CLAUDE_SESSIONS_FOCUS_TESTS=1` also runs the cases that take key status or run a modal session on the real window
//! server (they pull keystrokes from the app you are typing in): run it only when nobody is using the machine.

use std::process::Command;

use claude_sessions_core::kv::KeyValueStore;
use claude_sessions_macos::defaults::UserDefaultsStore;
use claude_sessions_macos::ui::read_png_pixel;
use objc2_quartz_core::CAMediaTiming;
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
            material: None,
        }],
        images: Default::default(),
        grow_origin: Default::default(),
    }
}

/// The views draw the field's capsule and ring themselves: the native field is borderless, with no background and no focus ring.
fn fields_are_borderless() -> Result<(), String> {
    use claude_sessions_macos::ui::SceneView;
    use objc2_app_kit::NSFocusRingType;
    use objc2_foundation::{MainThreadMarker, NSSize};

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 60.0));
    view.set_scene(&field_scene("", "Email"));
    let field = view.first_field().ok_or("no field was created")?;
    if field.isBezeled() {
        return Err("the field is bezeled".into());
    }
    if field.isBordered() {
        return Err("the field is bordered".into());
    }
    if field.drawsBackground() {
        return Err("the field draws a background".into());
    }
    if field.focusRingType() != NSFocusRingType::None {
        return Err("the field draws a focus ring".into());
    }
    Ok(())
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

fn two_field_scene(with_second: bool) -> claude_sessions_core::scene::Scene {
    use claude_sessions_core::scene::{Field, Rect};
    let mut scene = field_scene("", "Email");
    if with_second {
        scene.nodes[0].fields.push(Field {
            key: "other".into(),
            rect: Rect::new(10.0, 36.0, 180.0, 30.0),
            text: String::new(),
            placeholder: "Other".into(),
        });
    }
    scene
}

/// A widget panel with `scene` room, offscreen. Only the key variant is ordered front and made key (keyboard mode); the
/// other stays hidden, so normal gui runs never put a window on the screen.
fn field_panel(key: bool) -> Result<claude_sessions_macos::ui::WidgetPanelHandle, String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::WidgetPanelHandle;
    let panel = WidgetPanelHandle::new(
        Rect::new(-3000.0, -3000.0, 200.0, 80.0),
        std::rc::Rc::new(|_| {}),
    )
    .ok_or("not on the main thread")?;
    if key {
        panel.show();
        claude_sessions_macos::keys::enter_keyboard(&panel);
        if !panel.is_key() {
            return Err("the panel did not become key".into());
        }
    }
    Ok(panel)
}

/// What a gated case returns without `CLAUDE_SESSIONS_FOCUS_TESTS=1`; the harness prints it instead of `ok`.
const SKIPPED: &str = "SKIPPED (set CLAUDE_SESSIONS_FOCUS_TESTS=1)";

/// `Ok` when `CLAUDE_SESSIONS_FOCUS_TESTS=1` is set; otherwise the case ends with `SKIPPED`.
fn focus_tests_enabled() -> Result<(), String> {
    match std::env::var_os("CLAUDE_SESSIONS_FOCUS_TESTS") {
        Some(_) => Ok(()),
        None => Err(SKIPPED.into()),
    }
}

/// The alert window is above the panel while its modal session runs, by `NSWindow.level` and by the window server's
/// layer, and it is key (AppKit puts the alert back to the modal level 8 when it becomes key, which the first version
/// of this test missed: it read `level()` 0.1 s into a session without activation and passed while the real alert sat
/// under the panel). The test aborts the session itself. The alert is invisible (alpha 0, mouse-transparent) but not
/// offscreen: AppKit centres it again when the session starts (read as origin (626, 579) inside the session, despite
/// the hook's `setFrameOrigin`). The hook activates the app like the real dialogs do (AppKit re-levels the alert only
/// once it is key), so the app is briefly frontmost and focus goes back to the previous app afterwards. It is a real
/// modal session, so it is opt-in: set `CLAUDE_SESSIONS_FOCUS_TESTS=1` (besides `CLAUDE_SESSIONS_GUI_TESTS=1`) to run
/// it, like every case that makes a window key.
fn alert_level_is_above_panel_while_modal() -> Result<(), String> {
    focus_tests_enabled()?;
    let seen = claude_sessions_macos::ui::testing::alert_level_during_modal()?;
    if seen.alpha != 0.0 {
        return Err(format!(
            "alert alpha {} is not 0 inside the session",
            seen.alpha
        ));
    }
    let panel = objc2_app_kit::NSStatusWindowLevel;
    if seen.level <= panel {
        return Err(format!(
            "alert level {} is not above the panel level {panel}",
            seen.level
        ));
    }
    if !seen.key {
        return Err("the alert is not the key window inside the session".into());
    }
    if !seen.modal_open {
        return Err("`modal_open` is false inside the modal session".into());
    }
    if claude_sessions_macos::app::modal_open() {
        return Err("`modal_open` is true outside a modal session".into());
    }
    match seen.server_layer {
        Some(layer) if layer > panel => Ok(()),
        Some(layer) => Err(format!(
            "the window server puts the alert at layer {layer}, not above the panel level {panel}"
        )),
        None => Err("the window server does not list the alert window".into()),
    }
}

/// `Esc` ends an alert session with the second button's code (`Huỷ`); a Caps Lock flag on the key does not stop it.
/// A synthetic `Esc` key-down is posted inside an invisible, activated modal session. Opt-in like the other modal cases.
fn escape_closes_the_alert(caps_lock: bool) -> Result<(), String> {
    focus_tests_enabled()?;
    let code = claude_sessions_macos::ui::testing::escape_response_during_modal(caps_lock)?;
    if code == objc2_app_kit::NSAlertSecondButtonReturn {
        Ok(())
    } else {
        Err(format!(
            "Esc (caps lock {caps_lock}) ended the session with {code}, not the second button's code"
        ))
    }
}

/// Main-queue work runs while a dialog is open: the dialog is started the way the app starts it, a `main_queue::async_main`
/// block is queued during the session, and it must have run before the session ends. Invisible alert, opt-in like the
/// other modal case.
fn main_queue_runs_while_dialog_is_open() -> Result<(), String> {
    focus_tests_enabled()?;
    if claude_sessions_macos::ui::testing::main_queue_runs_during_modal()? {
        Ok(())
    } else {
        Err("a main-queue block or a `main_queue::every` tick did not run while the modal session was open".into())
    }
}

/// A field created by a scene update while the window is key takes the cursor; the newest one wins.
fn newly_created_field_takes_focus() -> Result<(), String> {
    focus_tests_enabled()?;
    let panel = field_panel(true)?;
    let view = panel.view();
    view.set_scene(&claude_sessions_core::scene::Scene::default());
    view.set_scene(&two_field_scene(false));
    let first = view.field("email").ok_or("no email field")?;
    if first.currentEditor().is_none() {
        return Err("the new field did not take the cursor".into());
    }
    view.set_scene(&two_field_scene(true));
    let second = view.field("other").ok_or("no second field")?;
    let result = if second.currentEditor().is_none() {
        Err("the second new field did not take the cursor".to_string())
    } else if first.currentEditor().is_some() {
        Err("the older field kept the cursor".to_string())
    } else {
        Ok(())
    };
    panel.close();
    result
}

/// A hidden (never key) window keeps its first responder: the field waits for `enter_keyboard`.
fn field_created_while_not_key_is_not_forced() -> Result<(), String> {
    let panel = field_panel(false)?;
    if panel.is_key() {
        return Err("the panel is key already".into());
    }
    let view = panel.view();
    view.set_scene(&two_field_scene(false));
    view.set_scene(&two_field_scene(true));
    let result = match (view.field("email"), view.field("other")) {
        (Some(a), Some(b)) if a.currentEditor().is_none() && b.currentEditor().is_none() => Ok(()),
        (Some(_), Some(_)) => {
            Err("a field took the cursor in a window that is not key".to_string())
        }
        _ => Err("fields missing".to_string()),
    };
    panel.close();
    result
}

/// Esc in a field (`cancelOperation:`) is consumed and reported with the field's key, for any field.
fn cancel_operation_emits_field_cancel() -> Result<(), String> {
    use claude_sessions_macos::ui::{SceneView, UiEvent};
    use objc2::sel;
    use objc2_app_kit::{NSControlTextEditingDelegate, NSTextView};
    use objc2_foundation::{MainThreadMarker, NSSize};

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 80.0));
    let events: std::rc::Rc<std::cell::RefCell<Vec<UiEvent>>> = Default::default();
    let sink = events.clone();
    view.set_handler(std::rc::Rc::new(move |e| sink.borrow_mut().push(e)));
    view.set_scene(&two_field_scene(true));
    let text_view = NSTextView::new(mtm);
    // The `Khác…` field has the same delegate path as the reply field, so the second field stands for it.
    for key in ["email", "other"] {
        let f = view.field(key).ok_or("no field")?;
        let delegate = f.delegate().ok_or("no delegate")?;
        // SAFETY: plain delegate calls with live objects.
        let consumed = unsafe {
            delegate.control_textView_doCommandBySelector(&f, &text_view, sel!(cancelOperation:))
        };
        if !consumed {
            return Err(format!("Esc in {key} was not consumed"));
        }
    }
    let want = vec![
        UiEvent::FieldCancel {
            key: "email".into(),
        },
        UiEvent::FieldCancel {
            key: "other".into(),
        },
    ];
    if *events.borrow() != want {
        return Err(format!("events: {:?}", events.borrow()));
    }
    Ok(())
}

/// A node removed by `set_scene` fades out for a moment: its field takes no input and emits nothing meanwhile.
fn fading_node_field_is_disabled() -> Result<(), String> {
    use claude_sessions_macos::ui::{SceneView, UiEvent};
    use objc2_foundation::{MainThreadMarker, NSSize};

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 80.0));
    let events: std::rc::Rc<std::cell::RefCell<Vec<UiEvent>>> = Default::default();
    let sink = events.clone();
    view.set_handler(std::rc::Rc::new(move |e| sink.borrow_mut().push(e)));
    view.set_scene(&field_scene("", "Email"));
    let field = view.field("email").ok_or("no field")?;
    if !field.isEnabled() || field.delegate().is_none() {
        return Err("the field starts without being enabled and delegated".into());
    }
    view.set_scene(&claude_sessions_core::scene::Scene::default());
    if view.field("email").is_some() {
        return Err("the removed node still answers for its field".into());
    }
    if field.isEnabled() {
        return Err("the fading field is still enabled".into());
    }
    if field.delegate().is_some() {
        return Err("the fading field still has its delegate".into());
    }
    if !events.borrow().is_empty() {
        return Err(format!("events: {:?}", events.borrow()));
    }
    Ok(())
}

/// A field that is being edited when its node is removed ends the edit: the window keeps no field editor for it. The
/// window is never ordered in, so nothing shows and the app is not activated.
fn fading_node_ends_field_edit() -> Result<(), String> {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = claude_sessions_macos::ui::SceneView::new(mtm, NSSize::new(200.0, 80.0));
    // SAFETY: NSWindow's designated initialiser with valid arguments; the window is owned by `Retained` and released
    // normally, never ordered in.
    let window: Retained<NSWindow> = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            NSRect::new(NSPoint::new(-10000.0, -10000.0), NSSize::new(200.0, 80.0)),
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: the window is owned by `Retained`; closing must not release it a second time.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setAlphaValue(0.0);
    window.setContentView(Some(&view));
    view.set_scene(&field_scene("", "Email"));
    let field = view.field("email").ok_or("no field")?;
    if !window.makeFirstResponder(Some(&field)) {
        return Err("the window refused the field as first responder".into());
    }
    if field.currentEditor().is_none() {
        return Err("the field got no editor, so the editing branch cannot run".into());
    }
    view.set_scene(&claude_sessions_core::scene::Scene::default());
    if field.currentEditor().is_some() {
        return Err("the removed field is still being edited".into());
    }
    let first = window.firstResponder();
    let is_field = first.as_ref().is_some_and(|r| {
        std::ptr::eq(
            &**r as *const _ as *const u8,
            &*field as *const _ as *const u8,
        )
    });
    if is_field {
        return Err("the field is still the first responder".into());
    }
    if first.is_some_and(|r| r.downcast::<objc2_app_kit::NSText>().is_ok()) {
        return Err("a field editor is still the first responder".into());
    }
    window.setContentView(None);
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
            material: None,
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
        material: None,
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
        grow_origin: Default::default(),
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
            material: None,
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
            material: None,
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

/// A scroll node that is not at the origin (the panel sits `MARGIN` in from the window edge, the header above the list): the
/// scrolled node is drawn where the scene says, the offset is 0 at the top, and a click at the node's hit finds it.
fn scrolled_node_keeps_its_place_and_hit_off_origin() -> Result<(), String> {
    use claude_sessions_core::scene::{Cursor, Hit, HitId, Point, Rect};
    use claude_sessions_macos::ui::SceneView;
    use objc2::rc::Retained;
    use objc2_app_kit::NSScrollView;
    use objc2_foundation::{MainThreadMarker, NSPoint, NSSize};

    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(228.0, 128.0));
    let mut scene = panel_scroll_scene(true);
    for n in &mut scene.nodes {
        n.frame.x += 14.0;
        n.frame.y += 14.0;
    }
    let card = scene.nodes.last_mut().ok_or("no card")?;
    card.hits = vec![Hit {
        rect: Rect::new(10.0, 5.0, 50.0, 20.0),
        id: HitId(7),
        cursor: Cursor::Pointer,
    }];
    let scene_frame = card.frame;
    if scene
        .nodes
        .iter()
        .any(|n| n.scroll.is_some_and(|_| n.frame.y < 30.0))
    {
        return Err("fixture: the scroll node must sit at y >= 30".into());
    }
    view.set_scene(&scene);
    let host: Retained<NSScrollView> = view
        .subviews()
        .iter()
        .find_map(|v| v.downcast::<NSScrollView>().ok())
        .ok_or("no scroll view")?;
    let doc = host.documentView().ok_or("no document view")?;
    let card_view = doc.subviews().firstObject().ok_or("no scrolled view")?;
    let seen = view.convertRect_fromView(card_view.frame(), Some(&doc));
    if (seen.origin.x, seen.origin.y) != (scene_frame.x, scene_frame.y) {
        return Err(format!(
            "scrolled node drawn at ({}, {}), scene says ({}, {})",
            seen.origin.x, seen.origin.y, scene_frame.x, scene_frame.y
        ));
    }
    if view.scroll_offset() != 0.0 {
        return Err(format!("offset at the top {}", view.scroll_offset()));
    }
    let centre = |scrolled: f64| Point {
        x: scene_frame.x + 10.0 + 25.0,
        y: scene_frame.y + 5.0 + 10.0 - scrolled,
    };
    if view.hit_at(centre(0.0)) != Some(HitId(7)) {
        return Err(format!("no hit at the top: {:?}", view.hit_at(centre(0.0))));
    }
    host.contentView().scrollToPoint(NSPoint::new(0.0, 10.0));
    if view.scroll_offset() != 10.0 {
        return Err(format!("scrolled offset {}", view.scroll_offset()));
    }
    if view.hit_at(centre(10.0)) != Some(HitId(7)) {
        return Err(format!(
            "no hit after scrolling: {:?}",
            view.hit_at(centre(10.0))
        ));
    }
    // The reuse path: the same scroll node moved down and given a different content height keeps its host, is drawn
    // at its new place, and answers hits there.
    let mut moved = scene.clone();
    for n in moved
        .nodes
        .iter_mut()
        .filter(|n| n.key.0 == "scroll" || n.key.0 == "card")
    {
        n.frame.y += 20.0;
        if let Some(size) = &mut n.scroll {
            size.h = 300.0;
        }
    }
    let moved_frame = moved.nodes.last().ok_or("no card")?.frame;
    view.set_scene(&moved);
    let host2: Retained<NSScrollView> = view
        .subviews()
        .iter()
        .find_map(|v| v.downcast::<NSScrollView>().ok())
        .ok_or("no scroll view after the move")?;
    if host != host2 {
        return Err("the scroll host was not reused".into());
    }
    let doc = host2.documentView().ok_or("no document view")?;
    let card_view = doc.subviews().firstObject().ok_or("no scrolled view")?;
    let seen = view.convertRect_fromView(card_view.frame(), Some(&doc));
    let offset = view.scroll_offset();
    if (seen.origin.x, seen.origin.y + offset) != (moved_frame.x, moved_frame.y) {
        return Err(format!(
            "moved node drawn at ({}, {}) with offset {offset}, scene says ({}, {})",
            seen.origin.x, seen.origin.y, moved_frame.x, moved_frame.y
        ));
    }
    if offset != 10.0 {
        return Err(format!("offset after the move {offset}, was 10"));
    }
    let centre = |scrolled: f64| Point {
        x: moved_frame.x + 10.0 + 25.0,
        y: moved_frame.y + 5.0 + 10.0 - scrolled,
    };
    if view.hit_at(centre(10.0)) != Some(HitId(7)) {
        return Err(format!(
            "no hit after the move: {:?}",
            view.hit_at(centre(10.0))
        ));
    }
    host2.contentView().scrollToPoint(NSPoint::new(0.0, 15.0));
    if view.scroll_offset() != 15.0 {
        return Err(format!(
            "scrolled offset after the move {}",
            view.scroll_offset()
        ));
    }
    if view.hit_at(centre(15.0)) != Some(HitId(7)) {
        return Err(format!(
            "no hit after scrolling the moved node: {:?}",
            view.hit_at(centre(15.0))
        ));
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
            material: None,
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
        material: None,
    }
}

fn one_node_scene(node: claude_sessions_core::scene::Node) -> claude_sessions_core::scene::Scene {
    use claude_sessions_core::scene::{Scene, Size};
    Scene {
        size: Size { w: 200.0, h: 100.0 },
        nodes: vec![node],
        images: Default::default(),
        grow_origin: Default::default(),
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

/// `(width, height)` of a shadow path.
type PathSize = (f64, f64);

/// The `(from, to)` sizes of the layer's explicit `shadowPath` animation.
fn shadow_animation_sizes(
    layer: &objc2_quartz_core::CALayer,
) -> Result<(PathSize, PathSize), String> {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_core_foundation::CFType;
    use objc2_core_graphics::CGPath;
    use objc2_foundation::ns_string;
    use objc2_quartz_core::CABasicAnimation;
    // SAFETY: a plain getter on a layer owned here.
    let animation = unsafe { layer.animationForKey(ns_string!("shadowPath")) }
        .ok_or("no shadowPath animation")?;
    let animation: Retained<CABasicAnimation> = animation
        .downcast()
        .map_err(|_| "the shadowPath animation is not a CABasicAnimation".to_string())?;
    let size_of = |value: Option<Retained<AnyObject>>| -> Result<(f64, f64), String> {
        let value = value.ok_or("animation without a value")?;
        // SAFETY: every Objective-C object is a valid CF object for `CFGetTypeID`; `downcast_ref` checks the type.
        let cf = unsafe { &*(&*value as *const AnyObject as *const CFType) };
        let path = cf
            .downcast_ref::<CGPath>()
            .ok_or("animation value is not a CGPath")?;
        let b = CGPath::bounding_box(Some(path));
        Ok((b.size.width, b.size.height))
    };
    Ok((
        size_of(animation.fromValue())?,
        size_of(animation.toValue())?,
    ))
}

/// A frame change through `animate_frame`: the layer gets an explicit `shadowPath` animation from the old size to the new.
fn shadow_path_animates_with_the_frame() -> Result<(), String> {
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
    if !claude_sessions_macos::ui::testing::animate_first_node_frame(
        &view,
        Rect::new(10.0, 10.0, 100.0, 70.0),
    ) {
        return Err("no node view".into());
    }
    let layer = node_layer(&view)?;
    let (from, to) = shadow_animation_sizes(&layer)?;
    if from != (100.0, 40.0) || to != (100.0, 70.0) {
        return Err(format!("animation runs from {from:?} to {to:?}"));
    }
    let model = layer.shadowPath().ok_or("no shadowPath")?;
    let b = CGPath::bounding_box(Some(&model));
    if (b.size.width, b.size.height) != (100.0, 70.0) {
        return Err(format!("model shadowPath bounds {b:?}"));
    }
    Ok(())
}

/// `set_scene` animates a resize: the layer gets a `shadowPath` animation from the old size to the new.
fn set_scene_animates_a_resize() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
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
    let (from, to) = shadow_animation_sizes(&layer)?;
    if from != (100.0, 40.0) || to != (100.0, 70.0) {
        return Err(format!("animation runs from {from:?} to {to:?}"));
    }
    Ok(())
}

/// A node's spinner turns while the panel is live and stops when it is hidden.
fn spinner_spins_on_a_live_panel() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize, ns_string};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    let mut node = shadow_node(Rect::new(10.0, 10.0, 100.0, 40.0), false);
    node.spinner = Some(Rect::new(4.0, 4.0, 16.0, 16.0));
    view.set_scene(&one_node_scene(node));
    let spinning = || -> Result<bool, String> {
        let layer = node_layer(&view)?;
        // SAFETY: plain getters on layers owned here.
        unsafe {
            let ring = layer
                .sublayers()
                .and_then(|l| l.iter().next())
                .ok_or("no spinner layer")?;
            Ok(ring.animationForKey(ns_string!("spin")).is_some())
        }
    };
    view.set_motion_live(true);
    if !spinning()? {
        return Err("the spinner does not spin on a live panel".into());
    }
    view.set_motion_live(false);
    if spinning()? {
        return Err("the spinner still spins on a hidden panel".into());
    }
    Ok(())
}

/// A spinner created while the panel is already live spins at once.
fn spinner_created_on_a_live_panel_spins() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize, ns_string};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_motion_live(true);
    let mut node = shadow_node(Rect::new(10.0, 10.0, 100.0, 40.0), false);
    node.spinner = Some(Rect::new(4.0, 4.0, 16.0, 16.0));
    view.set_scene(&one_node_scene(node));
    let layer = node_layer(&view)?;
    // SAFETY: plain getters on layers owned here.
    let spins = unsafe {
        layer
            .sublayers()
            .and_then(|l| l.iter().next())
            .ok_or("no spinner layer")?
            .animationForKey(ns_string!("spin"))
            .is_some()
    };
    if !spins {
        return Err("a spinner created on a live panel does not spin".into());
    }
    Ok(())
}

/// A second resize during a running open animation leaves the `shadowPath` animation heading to the newest size.
fn second_resize_retargets_the_shadow_animation() -> Result<(), String> {
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
    view.set_scene(&one_node_scene(shadow_node(
        Rect::new(10.0, 10.0, 100.0, 90.0),
        true,
    )));
    let (from, to) = shadow_animation_sizes(&layer)?;
    // No presentation layer exists yet, so the animation starts from the model's size.
    if from != (100.0, 70.0) {
        return Err(format!("the shadowPath animation starts from {from:?}"));
    }
    if to != (100.0, 90.0) {
        return Err(format!("the shadowPath animation heads to {to:?}"));
    }
    let model = layer.shadowPath().ok_or("no shadowPath")?;
    let b = CGPath::bounding_box(Some(&model));
    if (b.size.width, b.size.height) != (100.0, 90.0) {
        return Err(format!("model shadowPath bounds {b:?}"));
    }
    // The mask that cuts the shape out of the shadow follows the same path.
    let mask = layer.mask().ok_or("the shadow layer has no mask")?;
    let path = animation_for(&mask, "path").ok_or("no path animation on the mask")?;
    if path.fromValue().is_none() || path.toValue().is_none() {
        return Err("the mask's path animation has no values".into());
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

/// A zero-alpha shadow draws nothing: no ring of the shape's fill is left at its curved edge.
fn zero_alpha_shadow_leaves_the_png_transparent() -> Result<(), String> {
    use claude_sessions_core::scene::{Appearance, Rect};
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("zero-shadow.png");
    let scale = 2.0;
    let mut node = shadow_node(Rect::new(40.0, 20.0, 100.0, 40.0), true);
    node.shadow.as_mut().ok_or("no shadow")?.color.alpha = 0.0;
    claude_sessions_macos::ui::render_png(&one_node_scene(node), scale, Appearance::Light, &path)?;
    // The top-left corner arc, 14 pt of margin in: the box around it holds the anti-aliased edge.
    for y in (30.0 * scale) as usize..(50.0 * scale) as usize {
        for x in (50.0 * scale) as usize..(70.0 * scale) as usize {
            let (_, p) = read_png_pixel(&path, x, y)?;
            if p[3] > 0.004 {
                return Err(format!("pixel ({x}, {y}) is not transparent: {p:?}"));
            }
        }
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

fn glass_node(
    key: &str,
    frame: claude_sessions_core::scene::Rect,
    material: Option<claude_sessions_core::scene::Material>,
) -> claude_sessions_core::scene::Node {
    let mut node = shadow_node(frame, false);
    node.key = key.into();
    node.material = material;
    node
}

fn glass(corner: f64, alpha: f64) -> Option<claude_sessions_core::scene::Material> {
    Some(claude_sessions_core::scene::Material::Glass { corner, alpha })
}

/// The effect views among the scene view's direct subviews.
fn glass_views(
    view: &claude_sessions_macos::ui::SceneView,
) -> Vec<objc2::rc::Retained<objc2_app_kit::NSVisualEffectView>> {
    view.subviews()
        .iter()
        .filter_map(|v| v.downcast::<objc2_app_kit::NSVisualEffectView>().ok())
        .collect()
}

/// The direct subviews that are not effect views (the node views).
fn node_views(
    view: &claude_sessions_macos::ui::SceneView,
) -> Vec<objc2::rc::Retained<objc2_app_kit::NSView>> {
    view.subviews()
        .iter()
        .filter(|v| {
            v.downcast_ref::<objc2_app_kit::NSVisualEffectView>()
                .is_none()
        })
        .collect()
}

fn glass_view_for_material_node() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_app_kit::{NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState};
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    let frame = Rect::new(10.0, 10.0, 100.0, 40.0);
    view.set_scene(&one_node_scene(glass_node(
        "panel",
        frame,
        glass(20.0, 1.0),
    )));
    let glasses = glass_views(&view);
    let [g] = glasses.as_slice() else {
        return Err(format!("{} effect views", glasses.len()));
    };
    if g.material() != NSVisualEffectMaterial::Popover {
        return Err(format!("material {:?}", g.material()));
    }
    if g.blendingMode() != NSVisualEffectBlendingMode::BehindWindow {
        return Err(format!("blending {:?}", g.blendingMode()));
    }
    if g.state() != NSVisualEffectState::Active {
        return Err(format!("state {:?}", g.state()));
    }
    let layer = g.layer().ok_or("no glass layer")?;
    if layer.cornerRadius() != 20.0 || !layer.masksToBounds() {
        return Err(format!(
            "corner {} masks {}",
            layer.cornerRadius(),
            layer.masksToBounds()
        ));
    }
    let f = g.frame();
    if (f.origin.x, f.origin.y, f.size.width, f.size.height) != (10.0, 10.0, 100.0, 40.0) {
        return Err(format!("glass frame {f:?}"));
    }
    if g.alphaValue() != 1.0 {
        return Err(format!("alpha {}", g.alphaValue()));
    }
    Ok(())
}

/// A node's commands may extend past its frame (`Canvas::row_shape` does), and the PNG clips them to the frame: the live
/// view must too, or the spill covers the neighbouring node. `clipsToBounds` defaults to NO since macOS 14.
fn node_view_clips_to_its_frame() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&one_node_scene(glass_node(
        "row",
        Rect::new(10.0, 10.0, 100.0, 40.0),
        None,
    )));
    let nodes = node_views(&view);
    let [n] = nodes.as_slice() else {
        return Err(format!("{} node views", nodes.len()));
    };
    if !n.clipsToBounds() {
        return Err("the node view does not clip to its bounds".into());
    }
    Ok(())
}

fn glass_alpha_follows_material() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    let frame = Rect::new(10.0, 10.0, 100.0, 40.0);
    view.set_scene(&one_node_scene(glass_node(
        "pill",
        frame,
        glass(15.0, 0.27),
    )));
    let alpha = || glass_views(&view).first().map(|g| g.alphaValue());
    if alpha() != Some(0.27) {
        return Err(format!("alpha {:?}", alpha()));
    }
    view.set_scene(&one_node_scene(glass_node("pill", frame, glass(15.0, 0.0))));
    if alpha() != Some(0.0) {
        return Err(format!("alpha after 0.0: {:?}", alpha()));
    }
    Ok(())
}

fn glass_sits_below_node_content() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_app_kit::NSVisualEffectView;
    use objc2_foundation::{MainThreadMarker, NSPoint, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&one_node_scene(glass_node(
        "panel",
        Rect::new(10.0, 10.0, 100.0, 40.0),
        glass(20.0, 1.0),
    )));
    let subs = view.subviews();
    let kinds: Vec<bool> = subs
        .iter()
        .map(|v| v.downcast_ref::<NSVisualEffectView>().is_some())
        .collect();
    if kinds != [true, false] {
        return Err(format!("subview order (true = glass): {kinds:?}"));
    }
    // The view is flipped, so a scene point is a point of its own coordinates.
    let hit = view
        .hitTest(NSPoint::new(50.0, 30.0))
        .ok_or("nothing at the click point")?;
    if hit.downcast_ref::<NSVisualEffectView>().is_some() {
        return Err("the glass took the hit".into());
    }
    Ok(())
}

/// The shadow falls outside the node's shape only: the layer that casts it is masked by an even-odd path with the rounded
/// rect cut out, so a translucent node (the glass) is not darkened from below.
fn shadow_leaves_the_shape_interior_clear() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_core_foundation::CGPoint;
    use objc2_core_graphics::CGPath;
    use objc2_foundation::{MainThreadMarker, NSSize};
    use objc2_quartz_core::CAShapeLayer;
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&one_node_scene(shadow_node(
        Rect::new(10.0, 10.0, 100.0, 40.0),
        true,
    )));
    let layer = node_layer(&view)?;
    let mask = layer.mask().ok_or("the shadow layer has no mask")?;
    let mask: objc2::rc::Retained<CAShapeLayer> = mask
        .downcast()
        .map_err(|_| "the mask is not a CAShapeLayer".to_string())?;
    let path = mask.path().ok_or("the mask has no path")?;
    // SAFETY: a null transform means identity.
    let inside = |x: f64, y: f64| unsafe {
        CGPath::contains_point(Some(&path), std::ptr::null(), CGPoint::new(x, y), true)
    };
    if inside(50.0, 20.0) {
        return Err("the mask lets the shadow through at the node centre".into());
    }
    if !inside(50.0, 44.0) {
        return Err("the mask hides the shadow 4 pt below the node".into());
    }
    Ok(())
}

/// A restack that moves the glass leaves its rounded corner in place.
fn glass_keeps_corner_after_restack() -> Result<(), String> {
    use claude_sessions_core::scene::{Rect, Scene, Size};
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    let a = glass_node("a", Rect::new(10.0, 10.0, 100.0, 40.0), glass(20.0, 1.0));
    let b = glass_node("b", Rect::new(10.0, 60.0, 100.0, 30.0), None);
    let scene = |nodes| Scene {
        size: Size { w: 200.0, h: 100.0 },
        nodes,
        images: Default::default(),
        grow_origin: Default::default(),
    };
    view.set_scene(&scene(vec![a.clone(), b.clone()]));
    // `a` now stacks above `b`: its glass has to move with it.
    view.set_scene(&scene(vec![b, a]));
    let subs = view.subviews();
    let at = subs
        .iter()
        .position(|v| {
            v.downcast_ref::<objc2_app_kit::NSVisualEffectView>()
                .is_some()
        })
        .ok_or("no glass")?;
    if at != 1 {
        return Err(format!("the glass sits at index {at}, not below its node"));
    }
    let g = glass_views(&view);
    let layer = g[0].layer().ok_or("no glass layer")?;
    if layer.cornerRadius() != 20.0 || !layer.masksToBounds() {
        return Err(format!(
            "corner {} masks {}",
            layer.cornerRadius(),
            layer.masksToBounds()
        ));
    }
    Ok(())
}

fn glass_follows_frame_change() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&one_node_scene(glass_node(
        "panel",
        Rect::new(10.0, 10.0, 100.0, 40.0),
        glass(20.0, 1.0),
    )));
    view.set_scene(&one_node_scene(glass_node(
        "panel",
        Rect::new(10.0, 10.0, 100.0, 70.0),
        glass(20.0, 1.0),
    )));
    // The frame animates; its model value lands when the run loop has run the animation.
    spin(0.6);
    let glasses = glass_views(&view);
    let g = glasses.first().ok_or("no glass")?;
    let node = node_views(&view).into_iter().next().ok_or("no node")?;
    if g.frame() != node.frame() || g.frame().size.height != 70.0 {
        return Err(format!("glass {:?} node {:?}", g.frame(), node.frame()));
    }
    Ok(())
}

fn glass_removed_with_material() -> Result<(), String> {
    use claude_sessions_core::scene::{Rect, Scene};
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    let frame = Rect::new(10.0, 10.0, 100.0, 40.0);
    view.set_scene(&one_node_scene(glass_node(
        "panel",
        frame,
        glass(20.0, 1.0),
    )));
    view.set_scene(&one_node_scene(glass_node("panel", frame, None)));
    if !glass_views(&view).is_empty() {
        return Err("the glass stayed after the material went away".into());
    }
    view.set_scene(&one_node_scene(glass_node(
        "panel",
        frame,
        glass(20.0, 1.0),
    )));
    if glass_views(&view).len() != 1 {
        return Err("the glass did not come back".into());
    }
    view.set_scene(&Scene::default());
    spin(0.5);
    if !view.subviews().is_empty() {
        return Err(format!("{} views left", view.subviews().len()));
    }
    Ok(())
}

fn node_without_material_has_no_glass() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(200.0, 100.0));
    view.set_scene(&one_node_scene(glass_node(
        "panel",
        Rect::new(10.0, 10.0, 100.0, 40.0),
        None,
    )));
    if !glass_views(&view).is_empty() || view.subviews().len() != 1 {
        return Err("a node without material got an effect view".into());
    }
    Ok(())
}

fn render_png_ignores_material() -> Result<(), String> {
    use claude_sessions_core::scene::{Appearance, Rect};
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let frame = Rect::new(40.0, 20.0, 100.0, 40.0);
    let (a, b) = (dir.join("material-off.png"), dir.join("material-on.png"));
    let render = |material, path: &std::path::Path| {
        let mut node = shadow_node(frame, true);
        node.material = material;
        claude_sessions_macos::ui::render_png(&one_node_scene(node), 2.0, Appearance::Dark, path)
    };
    render(None, &a)?;
    render(glass(20.0, 0.5), &b)?;
    let (x, y) = (
        std::fs::read(&a).map_err(|e| e.to_string())?,
        std::fs::read(&b).map_err(|e| e.to_string())?,
    );
    if x != y {
        return Err("material changed the PNG".into());
    }
    Ok(())
}

/// Runs the main run loop for `seconds`.
fn spin(seconds: f64) {
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop};
    // SAFETY: a plain run-loop mode constant.
    let mode = unsafe { NSDefaultRunLoopMode };
    let end = std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds);
    while std::time::Instant::now() < end {
        NSRunLoop::currentRunLoop()
            .runMode_beforeDate(mode, &NSDate::dateWithTimeIntervalSinceNow(0.02));
    }
}

/// The `CABasicAnimation` on `layer` for `key_path` (any key), if there is one.
fn animation_for(
    layer: &objc2_quartz_core::CALayer,
    key_path: &str,
) -> Option<objc2::rc::Retained<objc2_quartz_core::CABasicAnimation>> {
    use objc2_quartz_core::CABasicAnimation;
    let keys = layer.animationKeys()?;
    keys.iter().find_map(|key| {
        // SAFETY: a plain getter on a layer owned here.
        let a = unsafe { layer.animationForKey(&key) }?;
        let a: objc2::rc::Retained<CABasicAnimation> = a.downcast().ok()?;
        (a.keyPath()?.to_string() == key_path).then_some(a)
    })
}

/// Whether `value` (the from or to transform of a zoom) leaves the scene point `fixed` where it is; returns its scale.
fn zoom_check(
    layer: &objc2_quartz_core::CALayer,
    value: &objc2_foundation::NSValue,
    fixed: objc2_core_foundation::CGPoint,
) -> Result<f64, String> {
    use objc2_quartz_core::{CATransform3DIdentity, NSValueCATransform3DAdditions};
    // SAFETY: the value was made from a CATransform3D by `valueWithCATransform3D:`.
    let m = unsafe { value.CATransform3DValue() };
    // The layer point that sits at `fixed` (a layer of a view in no window has no superlayer: `None` is the top of its
    // own tree, where the scene's coordinates are).
    let local = layer.convertPoint_fromLayer(fixed, None);
    layer.setTransform(m);
    let after = layer.convertPoint_toLayer(local, None);
    // SAFETY: a plain extern constant.
    layer.setTransform(unsafe { CATransform3DIdentity });
    if (after.x - fixed.x).abs() > 0.5 || (after.y - fixed.y).abs() > 0.5 {
        return Err(format!("the fixed point moved from {fixed:?} to {after:?}"));
    }
    Ok(m.m11)
}

/// Checks one view's zoom animation: the 0.96 scale (at the `closing` end or the start) fixed at `origin` (scene
/// point), `seconds` long.
fn check_zoom(
    layer: &objc2_quartz_core::CALayer,
    origin: claude_sessions_core::scene::Point,
    closing: bool,
    seconds: f64,
) -> Result<(), String> {
    use objc2_core_foundation::CGPoint;
    use objc2_foundation::NSValue;
    let a = animation_for(layer, "transform").ok_or("no transform animation")?;
    if (a.duration() - seconds).abs() > 1e-6 {
        return Err(format!("zoom lasts {}", a.duration()));
    }
    let value = |v: Option<objc2::rc::Retained<objc2::runtime::AnyObject>>| {
        v.and_then(|v| v.downcast::<NSValue>().ok())
            .ok_or("no transform value".to_string())
    };
    let shrunk = if closing { a.toValue() } else { a.fromValue() };
    let scale = zoom_check(layer, &*value(shrunk)?, CGPoint::new(origin.x, origin.y))?;
    if (scale - 0.96).abs() > 1e-6 {
        return Err(format!("zoom scale {scale}"));
    }
    Ok(())
}

fn pop_scenes(
    origin: claude_sessions_core::scene::Point,
) -> (
    claude_sessions_core::scene::Scene,
    claude_sessions_core::scene::Scene,
) {
    use claude_sessions_core::scene::{Rect, Scene, Size};
    // The panel and the pill cast a shadow, each from a view of its own below the node's glass.
    let shadowed = |mut node: claude_sessions_core::scene::Node| {
        node.shadow = shadow_node(node.frame, true).shadow;
        node
    };
    let closed = Scene {
        size: Size { w: 300.0, h: 200.0 },
        nodes: vec![shadowed(glass_node(
            "pill",
            Rect::new(10.0, 10.0, 100.0, 30.0),
            glass(15.0, 1.0),
        ))],
        images: Default::default(),
        grow_origin: origin,
    };
    let open = Scene {
        size: Size { w: 300.0, h: 200.0 },
        nodes: vec![
            shadowed(glass_node(
                "panel",
                Rect::new(10.0, 10.0, 200.0, 150.0),
                glass(20.0, 1.0),
            )),
            glass_node("header", Rect::new(18.0, 18.0, 180.0, 38.0), None),
        ],
        images: Default::default(),
        grow_origin: origin,
    };
    (closed, open)
}

/// Every added view (node and glass) zooms from 0.96 about `origin`, 0.28 s, with a 0.18 s fade.
fn check_open_zoom(origin: claude_sessions_core::scene::Point) -> Result<(), String> {
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(300.0, 200.0));
    let (closed, open) = pop_scenes(origin);
    view.set_scene(&closed);
    let leaving: Vec<_> = view.subviews().iter().collect();
    view.set_scene(&open);
    let mut checked = 0;
    for v in view.subviews().iter() {
        if leaving.contains(&v) {
            continue;
        }
        let layer = v.layer().ok_or("no layer")?;
        check_zoom(&layer, origin, false, 0.28)?;
        let fade = animation_for(&layer, "opacity").ok_or("no fade animation")?;
        if (fade.duration() - 0.18).abs() > 1e-6 {
            return Err(format!("fade lasts {}", fade.duration()));
        }
        checked += 1;
    }
    // panel + its glass and shadow, header.
    if checked != 4 {
        return Err(format!("{checked} views zoomed"));
    }
    Ok(())
}

fn open_zooms_from_grow_origin() -> Result<(), String> {
    check_open_zoom(claude_sessions_core::scene::Point { x: 210.0, y: 10.0 })
}

fn open_origin_follows_anchor() -> Result<(), String> {
    use claude_sessions_core::scene::Point;
    check_open_zoom(Point { x: 10.0, y: 160.0 })?;
    check_open_zoom(Point { x: 110.0, y: 10.0 })
}

fn close_shrinks_to_grow_origin() -> Result<(), String> {
    use claude_sessions_core::scene::Point;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(300.0, 200.0));
    let origin = Point { x: 210.0, y: 10.0 };
    let (closed, open) = pop_scenes(origin);
    view.set_scene(&open);
    let leaving: Vec<_> = view.subviews().iter().collect();
    view.set_scene(&closed);
    for v in &leaving {
        let layer = v.layer().ok_or("no layer")?;
        check_zoom(&layer, origin, true, 0.16)?;
        let fade = animation_for(&layer, "opacity").ok_or("no fade animation")?;
        if (fade.duration() - 0.16).abs() > 1e-6 {
            return Err(format!("fade lasts {}", fade.duration()));
        }
    }
    let pills: Vec<_> = node_views(&view)
        .into_iter()
        .filter(|v| !leaving.contains(v))
        .collect();
    // The pill and its shadow.
    if pills.len() != 2 {
        return Err(format!("{} pill views", pills.len()));
    }
    for pill in &pills {
        if pill
            .layer()
            .and_then(|l| animation_for(&l, "opacity"))
            .is_none()
        {
            return Err("the pill or its shadow does not fade in".into());
        }
    }
    Ok(())
}

fn close_keeps_leaving_nodes_until_done() -> Result<(), String> {
    use claude_sessions_core::scene::Point;
    use claude_sessions_macos::ui::SceneView;
    use objc2_foundation::{MainThreadMarker, NSSize};
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let view = SceneView::new(mtm, NSSize::new(300.0, 200.0));
    let (closed, open) = pop_scenes(Point { x: 210.0, y: 10.0 });
    view.set_scene(&open);
    view.set_scene(&closed);
    // The panel, its glass, its shadow and the header, plus the pill, its glass and its shadow.
    if view.subviews().len() != 7 {
        return Err(format!("{} views right after", view.subviews().len()));
    }
    spin(0.6);
    // The pill, its glass and its shadow are left; the panel's shadow view went with the panel.
    if view.subviews().len() != 3 {
        return Err(format!("{} views after the close", view.subviews().len()));
    }
    Ok(())
}

fn in_panel_resize_keeps_spring() -> Result<(), String> {
    use claude_sessions_core::scene::Rect;
    use claude_sessions_macos::ui::SceneView;
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
    shadow_animation_sizes(&layer)?;
    if animation_for(&layer, "transform").is_some() {
        return Err("an in-panel resize zoomed".into());
    }
    Ok(())
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
    results.push(("fields_are_borderless", fields_are_borderless()));
    results.push((
        "alert_level_is_above_panel_while_modal",
        alert_level_is_above_panel_while_modal(),
    ));
    results.push(("escape_closes_the_alert", escape_closes_the_alert(false)));
    results.push((
        "escape_with_caps_lock_closes_the_alert",
        escape_closes_the_alert(true),
    ));
    results.push((
        "main_queue_runs_while_dialog_is_open",
        main_queue_runs_while_dialog_is_open(),
    ));
    results.push((
        "newly_created_field_takes_focus",
        newly_created_field_takes_focus(),
    ));
    results.push((
        "field_created_while_not_key_is_not_forced",
        field_created_while_not_key_is_not_forced(),
    ));
    results.push((
        "cancel_operation_emits_field_cancel",
        cancel_operation_emits_field_cancel(),
    ));
    results.push(("scroll_host_is_removed", scroll_host_is_removed()));
    results.push((
        "scroll_offset_follows_the_clip_view",
        scroll_offset_follows_the_clip_view(),
    ));
    results.push((
        "scrolled_node_keeps_its_place_and_hit_off_origin",
        scrolled_node_keeps_its_place_and_hit_off_origin(),
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
        "shadow_path_animates_with_the_frame",
        shadow_path_animates_with_the_frame(),
    ));
    results.push(("set_scene_animates_a_resize", set_scene_animates_a_resize()));
    results.push((
        "spinner_spins_on_a_live_panel",
        spinner_spins_on_a_live_panel(),
    ));
    results.push((
        "second_resize_retargets_the_shadow_animation",
        second_resize_retargets_the_shadow_animation(),
    ));
    results.push((
        "fading_node_field_is_disabled",
        fading_node_field_is_disabled(),
    ));
    results.push(("fading_node_ends_field_edit", fading_node_ends_field_edit()));
    results.push((
        "spinner_created_on_a_live_panel_spins",
        spinner_created_on_a_live_panel_spins(),
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
        "zero_alpha_shadow_leaves_the_png_transparent",
        zero_alpha_shadow_leaves_the_png_transparent(),
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
    results.push((
        "glass_view_for_material_node",
        glass_view_for_material_node(),
    ));
    results.push((
        "glass_alpha_follows_material",
        glass_alpha_follows_material(),
    ));
    results.push((
        "glass_sits_below_node_content",
        glass_sits_below_node_content(),
    ));
    results.push(("glass_follows_frame_change", glass_follows_frame_change()));
    results.push((
        "shadow_leaves_the_shape_interior_clear",
        shadow_leaves_the_shape_interior_clear(),
    ));
    results.push((
        "glass_keeps_corner_after_restack",
        glass_keeps_corner_after_restack(),
    ));
    results.push(("glass_removed_with_material", glass_removed_with_material()));
    results.push((
        "node_without_material_has_no_glass",
        node_without_material_has_no_glass(),
    ));
    results.push((
        "node_view_clips_to_its_frame",
        node_view_clips_to_its_frame(),
    ));
    results.push(("render_png_ignores_material", render_png_ignores_material()));
    results.push(("open_zooms_from_grow_origin", open_zooms_from_grow_origin()));
    results.push(("open_origin_follows_anchor", open_origin_follows_anchor()));
    results.push((
        "close_shrinks_to_grow_origin",
        close_shrinks_to_grow_origin(),
    ));
    results.push((
        "close_keeps_leaving_nodes_until_done",
        close_keeps_leaving_nodes_until_done(),
    ));
    results.push((
        "in_panel_resize_keeps_spring",
        in_panel_resize_keeps_spring(),
    ));
    let mut failed = false;
    for (name, result) in results {
        match result {
            Ok(()) => println!("test {name} ... ok"),
            Err(e) if e == SKIPPED => println!("test {name} ... {e}"),
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
