use claude_sessions_core::cmux::*;
use std::path::Path;

const LINE: &str = "claude 7d0fcbef-0908-41eb-9779-ea86d5bf838a  workspace=4E6B8144-0E40-4E20-A810-62245241C5A0  surface=20FAA4D5-E0C5-4F59-9D8E-277C0038558C  cwd=/Users/phong/.claude/skills-repo  active_ws=no  active_surface=yes  updated=2026-11-03T13:47:06.593Z  scratch=no  session_dir=-  fork_command=yes  fork=yes  pid_exists=yes";
const IDENTIFY: &str = r#"{"focused": {"surface_id": "6E39B188-F067-48FA-852D-3CE78A94EF92", "surface_type": "terminal", "pane_id": "P"}, "caller": null}"#;

fn raw(s: &str) -> Key {
    Key::Raw(s.into())
}

fn paste(s: &str) -> Key {
    Key::Paste(s.into())
}

#[test]
fn surface_found() {
    assert_eq!(
        surface_from_sessions_output(LINE).as_deref(),
        Some("20FAA4D5-E0C5-4F59-9D8E-277C0038558C")
    );
}

#[test]
fn surface_missing() {
    assert_eq!(surface_from_sessions_output(""), None);
}

#[test]
fn surface_first_line() {
    let second = LINE.replace(
        "20FAA4D5-E0C5-4F59-9D8E-277C0038558C",
        "AAAAAAAA-E0C5-4F59-9D8E-277C0038558C",
    );
    assert_eq!(
        surface_from_sessions_output(&format!("{LINE}\n{second}")).as_deref(),
        Some("20FAA4D5-E0C5-4F59-9D8E-277C0038558C")
    );
}

#[test]
fn approve_keys_is_one() {
    assert_eq!(approve_keys(), [raw("1")]);
}

#[test]
fn deny_keys_without_reason() {
    assert_eq!(deny_keys(""), [raw("\\x1b")]);
    assert_eq!(deny_keys(" \n "), [raw("\\x1b")]);
}

#[test]
fn deny_keys_with_reason() {
    assert_eq!(
        deny_keys("chỉ chạy make clean"),
        [
            raw("\\x1b"),
            Key::Pause(0.3),
            clear_input(),
            paste("chỉ chạy make clean"),
            raw("\\r")
        ]
    );
}

// A half-typed draft in Claude's input must not be sent with the deny reason: Ctrl-E, then Ctrl-U + Backspace per line.
#[test]
fn clear_input_empties_multi_line_draft() {
    assert_eq!(
        clear_input(),
        Key::Raw(format!("\\x05{}", "\\x15\\x7f".repeat(20)))
    );
}

#[test]
fn script_raw() {
    let s = script("S", &raw("\\x1b"));
    assert!(s.contains(r#"perform action "text:\\x1b" on (first terminal whose id is "S")"#));
}

#[test]
fn script_escapes_paste() {
    let s = script("S", &paste("a \"b\" \\ c\nd"));
    assert!(
        s.contains("input text \"a \\\"b\\\" \\\\ c\nd\" to (first terminal whose id is \"S\")")
    );
}

#[test]
fn script_pause_is_empty() {
    assert!(script("S", &Key::Pause(0.3)).is_empty());
}

#[test]
fn ask_claude_keys_clear_paste_enter() {
    assert_eq!(
        ask_claude_keys("Fix PR"),
        [clear_input(), paste("Fix PR"), raw("\\r")]
    );
}

#[test]
fn reply_keys_clear_paste_enter() {
    assert_eq!(
        reply_keys("say hi"),
        [clear_input(), paste("say hi"), raw("\\r")]
    );
    assert_eq!(
        reply_keys("a\nb"),
        [clear_input(), paste("a\nb"), raw("\\r")]
    );
}

#[test]
fn stop_keys_is_escape() {
    assert_eq!(stop_keys(), [raw("\\x1b")]);
}

#[test]
fn focused_surface_from_identify_terminal() {
    assert_eq!(
        focused_surface_from_identify(IDENTIFY.as_bytes()).as_deref(),
        Some("6E39B188-F067-48FA-852D-3CE78A94EF92")
    );
}

#[test]
fn focused_surface_ignores_browser() {
    let browser = IDENTIFY.replace("terminal", "browser");
    assert_eq!(focused_surface_from_identify(browser.as_bytes()), None);
}

#[test]
fn focused_surface_bad_json() {
    assert_eq!(focused_surface_from_identify(b"nope"), None);
    assert_eq!(focused_surface_from_identify(b"{}"), None);
}

#[test]
fn cli_path_is_inside_the_bundle() {
    assert_eq!(
        cli_path(Path::new("/Applications/cmux.app")),
        Path::new("/Applications/cmux.app/Contents/Resources/bin/cmux")
    );
}

#[test]
fn open_link_sends_no_keys() {
    use claude_sessions_core::action::Action;
    let url = url::Url::parse("https://x.y").unwrap();
    assert!(keys_for(&Action::OpenLink("s".into(), url)).is_empty());
}

#[test]
fn answer_sends_no_keystrokes() {
    use claude_sessions_core::action::Action;
    let a = Action::Answer {
        session: "a".into(),
        request_id: "R".into(),
        answers: vec![vec!["x".into()]],
        other: Default::default(),
    };
    assert!(keys_for(&a).is_empty());
}
