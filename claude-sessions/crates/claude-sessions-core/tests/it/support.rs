//! Helpers shared by the ported tests (Swift `TestSupport.swift`). Temp dirs only: nothing here touches `~/.claude`.
#![allow(dead_code)]

use claude_sessions_core::shell::format_iso_date;
use claude_sessions_core::transcript::project_dir_name;
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const FRONTEND: &str = "/Users/phong/talo6-dct-frontend";

/// Fresh directory per call; the caller keeps the guard alive (the dir is removed on drop).
pub fn make_temp_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("claude-sessions-tests-")
        .tempdir()
        .unwrap()
}

pub fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A temp dir holding `.git/HEAD` with `head` as its content.
pub fn repo(head: &str) -> tempfile::TempDir {
    let d = make_temp_dir();
    write(&d.path().join(".git/HEAD"), head);
    d
}

pub fn append(path: &Path, s: &str) {
    let mut f = fs::OpenOptions::new().append(true).open(path).unwrap();
    f.write_all(s.as_bytes()).unwrap();
}

/// Writes `<dir>/<session>/<id>.json` like Claude Code's TaskCreate; without a `session` it writes straight into `dir`.
pub fn write_task(
    dir: &Path,
    session: Option<&str>,
    id: i64,
    status: &str,
    subject: &str,
    active_form: Option<&str>,
) {
    let d = match session {
        Some(s) => dir.join(s),
        None => dir.to_path_buf(),
    };
    fs::create_dir_all(&d).unwrap();
    let mut o = json!({"id": id.to_string(), "subject": subject, "status": status, "blocks": [], "blockedBy": []});
    if let Some(a) = active_form {
        o["activeForm"] = json!(a);
    }
    fs::write(
        d.join(format!("{id}.json")),
        serde_json::to_vec(&o).unwrap(),
    )
    .unwrap();
}

pub fn json_line(o: Value) -> String {
    serde_json::to_string(&o).unwrap()
}

/// One content block per line, like Claude Code writes them; `None` makes a tool-only entry.
pub fn assistant_line(text: Option<&str>, sidechain: bool) -> String {
    let block = match text {
        Some(t) => json!({"type": "text", "text": t}),
        None => json!({"type": "tool_use", "name": "Bash"}),
    };
    json_line(
        json!({"type": "assistant", "isSidechain": sidechain, "message": {"content": [block]}}),
    )
}

/// The main-turn end marker Claude Code appends; counts left `None` are omitted.
pub fn turn_duration_line(
    background: Option<i64>,
    workflows: Option<i64>,
    sidechain: bool,
    timestamp: Option<SystemTime>,
) -> String {
    let mut o = json!({"type": "system", "subtype": "turn_duration", "isSidechain": sidechain, "durationMs": 10});
    if let Some(b) = background {
        o["pendingBackgroundAgentCount"] = json!(b);
    }
    if let Some(w) = workflows {
        o["pendingWorkflowCount"] = json!(w);
    }
    if let Some(t) = timestamp {
        o["timestamp"] = json!(format_iso_date(t));
    }
    json_line(o)
}

pub fn write_transcript(projects: &Path, cwd: &str, id: &str, lines: &[String]) -> PathBuf {
    let d = projects.join(project_dir_name(cwd));
    fs::create_dir_all(&d).unwrap();
    let f = d.join(format!("{id}.jsonl"));
    fs::write(&f, lines.join("\n")).unwrap();
    f
}

pub fn tool_use_line(id: &str, name: &str, input: Value, sidechain: bool) -> String {
    json_line(json!({"type": "assistant", "isSidechain": sidechain,
        "message": {"content": [{"type": "tool_use", "id": id, "name": name, "input": input}]}}))
}

pub fn tool_result_line(id: &str) -> String {
    json_line(
        json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": id}]}}),
    )
}

pub fn user_line(text: &str, is_meta: bool, origin: Option<&str>) -> String {
    let mut o = json!({"type": "user", "message": {"content": text}});
    if is_meta {
        o["isMeta"] = json!(true);
    }
    if let Some(k) = origin {
        o["origin"] = json!({"kind": k});
    }
    json_line(o)
}

/// A background agent's report as Claude Code records it; `origin: false` is the older format without the field.
pub fn notification_line(result: &str, tool_use_id: Option<&str>, origin: bool) -> String {
    let id = tool_use_id
        .map(|i| format!("<tool-use-id>{i}</tool-use-id>\n"))
        .unwrap_or_default();
    user_line(
        &format!(
            "<task-notification>\n{id}<status>completed</status>\n<result>{result}</result>\n</task-notification>"
        ),
        false,
        origin.then_some("task-notification"),
    )
}

pub fn command_line(name: &str, args: &str) -> String {
    let message = &name[1..];
    user_line(
        &format!(
            "<command-name>{name}</command-name>\n<command-message>{message}</command-message>\n<command-args>{args}</command-args>"
        ),
        false,
        None,
    )
}

pub fn ai_title_line(title: &str) -> String {
    json_line(json!({"type": "ai-title", "aiTitle": title}))
}

/// A `type: "user"` line whose content is a `tool_result` block carrying `text`.
pub fn tool_result_text_line(text: &str) -> String {
    json_line(
        json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t1",
        "content": [{"type": "text", "text": text}], "text": text}]}}),
    )
}
