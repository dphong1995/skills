//! Progress of a session's task list (`~/.claude/tasks/<sessionId>/*.json`).

use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub done: usize,
    pub total: usize,
    pub current: Option<String>,
}

impl Todo {
    pub fn label(&self) -> String {
        let n = format!("{}/{}", self.done, self.total);
        match &self.current {
            Some(c) => format!("{n} · {c}"),
            None => n,
        }
    }
}

struct Item {
    id: i64,
    name: String,
    subject: Option<String>,
    active_form: Option<String>,
    status: String,
}

/// The task files in `dir`, sorted by name; dot-files are bookkeeping.
pub fn files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json") && !file_name(p).starts_with('.'))
        .collect();
    out.sort_by_key(|p| file_name(p));
    out
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn read(dir: &Path) -> Option<Todo> {
    read_files(&files(dir))
}

/// Progress of the task list in `files`, or `None` when it is empty or all done.
pub fn read_files(files: &[PathBuf]) -> Option<Todo> {
    let mut items: Vec<Item> = Vec::new();
    for f in files {
        // A file can be mid-write: skip what fails to decode, the next refresh reads it again.
        let Ok(data) = std::fs::read(f) else { continue };
        let Ok(Value::Object(o)) = serde_json::from_slice::<Value>(&data) else {
            continue;
        };
        let Some(status) = o.get("status").and_then(Value::as_str) else {
            continue;
        };
        if !["pending", "in_progress", "completed"].contains(&status) {
            continue;
        }
        let id = o
            .get("id")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(i64::MAX);
        items.push(Item {
            id,
            name: file_name(f),
            subject: non_empty(o.get("subject")),
            active_form: non_empty(o.get("activeForm")),
            status: status.to_string(),
        });
    }
    items.sort_by(|a, b| (a.id, &a.name).cmp(&(b.id, &b.name)));
    let done = items.iter().filter(|i| i.status == "completed").count();
    if done >= items.len() {
        return None;
    }
    let current = match items.iter().find(|i| i.status == "in_progress") {
        Some(i) => i.active_form.clone().or_else(|| i.subject.clone()),
        None => items
            .iter()
            .find(|i| i.status == "pending")
            .and_then(|i| i.subject.clone()),
    };
    Some(Todo {
        done,
        total: items.len(),
        current,
    })
}

fn non_empty(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
}
