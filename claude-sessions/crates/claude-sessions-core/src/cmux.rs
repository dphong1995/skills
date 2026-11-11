//! The cmux side of a jump or a keystroke: the CLI calls and the AppleScript bodies. Running the AppleScript and finding the
//! app by its bundle id need AppKit and belong to the app layer.

use crate::action::Action;
use crate::closed_sessions::ClosedSession;
use crate::shell::run_process;
use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

pub const BUNDLE_ID: &str = "com.cmuxterm.app";

/// The surface id of the first `surface=<uuid>` in `cmux sessions` output.
pub fn surface_from_sessions_output(out: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new("surface=[0-9A-Fa-f-]{36}").expect("valid regex"));
    re.find(out)
        .map(|m| m.as_str()["surface=".len()..].to_string())
}

/// The focused surface of `cmux identify`, only when it is a terminal.
pub fn focused_surface_from_identify(data: &[u8]) -> Option<String> {
    let o: Value = serde_json::from_slice(data).ok()?;
    let focused = &o["focused"];
    if focused["surface_type"].as_str() != Some("terminal") {
        return None;
    }
    focused["surface_id"].as_str().map(String::from)
}

/// The `cmux` CLI inside the app bundle at `app`.
pub fn cli_path(app: &Path) -> PathBuf {
    app.join("Contents/Resources/bin/cmux")
}

/// Blocking. None when the output has no surface for the session.
pub fn surface(cmux: &Path, session_id: &str) -> Option<String> {
    let out = run_process(
        cmux,
        &["sessions", "--session", session_id, "--agent", "claude"],
        Duration::from_secs(2),
        false,
        None,
    )?;
    surface_from_sessions_output(&out)
}

/// Blocking. Opens a focused cmux workspace in `cwd` running `command`; false on any failure.
pub fn new_workspace(cmux: &Path, cwd: &str, command: &str) -> bool {
    run_process(
        cmux,
        &[
            "new-workspace",
            "--cwd",
            cwd,
            "--command",
            command,
            "--focus",
            "true",
        ],
        Duration::from_secs(5),
        true,
        None,
    )
    .is_some()
}

/// Blocking. The focused terminal surface of cmux; None when the focus is not a terminal.
pub fn focused_surface(cmux: &Path) -> Option<String> {
    let out = run_process(
        cmux,
        &["--id-format", "uuids", "identify", "--no-caller"],
        Duration::from_secs(2),
        true,
        None,
    )?;
    focused_surface_from_identify(out.as_bytes())
}

/// Blocking. Reopens a closed session in a cmux workspace; false when its id is not a canonical UUID or cmux fails.
pub fn resume(cmux: &Path, session: &ClosedSession) -> bool {
    match session.resume_command() {
        Some(command) => new_workspace(cmux, &session.cwd, &command),
        None => false,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Raw(String),
    Paste(String),
    Pause(f64),
}

/// Empties Claude's input so a half-typed draft is not sent with a deny reason: Ctrl-E, then Ctrl-U + Backspace
/// for up to 20 lines (Ctrl-U alone stops at the line start). Ctrl-Y in Claude brings the draft back.
pub fn clear_input() -> Key {
    Key::Raw(format!("\\x05{}", "\\x15\\x7f".repeat(20)))
}

pub fn approve_keys() -> Vec<Key> {
    vec![Key::Raw("1".into())]
}

pub fn deny_keys(reason: &str) -> Vec<Key> {
    let r = reason.trim();
    if r.is_empty() {
        vec![Key::Raw("\\x1b".into())]
    } else {
        vec![
            Key::Raw("\\x1b".into()),
            Key::Pause(0.3),
            clear_input(),
            Key::Paste(r.into()),
            Key::Raw("\\r".into()),
        ]
    }
}

pub fn ask_claude_keys(prompt: &str) -> Vec<Key> {
    vec![
        clear_input(),
        Key::Paste(prompt.into()),
        Key::Raw("\\r".into()),
    ]
}

pub fn reply_keys(text: &str) -> Vec<Key> {
    vec![
        clear_input(),
        Key::Paste(text.into()),
        Key::Raw("\\r".into()),
    ]
}

pub fn stop_keys() -> Vec<Key> {
    vec![Key::Raw("\\x1b".into())]
}

/// The keystrokes an action types into its session; none for actions that only open things or go through cmux's socket.
pub fn keys_for(a: &Action) -> Vec<Key> {
    match a {
        Action::Approve { .. } => approve_keys(),
        Action::Deny { reason, .. } => deny_keys(reason),
        Action::AskClaude { prompt, .. } => ask_claude_keys(prompt),
        Action::Reply { text, .. } => reply_keys(text),
        Action::Stop(_) => stop_keys(),
        Action::Jump(_)
        | Action::OpenLink(..)
        | Action::Connect(_)
        | Action::NudgeRabbit(..)
        | Action::Resume(_)
        | Action::Answer { .. }
        | Action::MarkSeen(_) => vec![],
    }
}

/// An AppleScript string literal: backslashes and double quotes escaped.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn target(surface: &str) -> String {
    format!("(first terminal whose id is {})", quote(surface))
}

/// The AppleScript body of a jump: activates cmux and focuses the terminal.
pub fn jump_script(surface: &str) -> String {
    format!(
        "tell application id \"{BUNDLE_ID}\"\nactivate\nfocus {}\nend tell",
        target(surface)
    )
}

/// The AppleScript body for one key. Pauses are handled by the sender, so they have no script.
pub fn script(surface: &str, key: &Key) -> String {
    match key {
        Key::Raw(r) => format!(
            "tell application id \"{BUNDLE_ID}\" to perform action {} on {}",
            quote(&format!("text:{r}")),
            target(surface)
        ),
        Key::Paste(t) => format!(
            "tell application id \"{BUNDLE_ID}\" to input text {} to {}",
            quote(t),
            target(surface)
        ),
        Key::Pause(_) => String::new(),
    }
}
