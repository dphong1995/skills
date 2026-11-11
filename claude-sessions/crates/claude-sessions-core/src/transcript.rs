//! Reading what Claude Code writes to a session's transcript (`~/.claude/projects/<dir>/<id>.jsonl`): prompts, replies,
//! tool calls, agents, links and tickets. Functions take the raw bytes of whole lines.

use crate::closed_sessions::folder as last_path_component;
use crate::shell::parse_iso_date;
use memchr::memmem::Finder;
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;
use unicode_segmentation::UnicodeSegmentation;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub tool_id: String,
    pub tool: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingState {
    One(Pending),
    Unclear { count: usize },
}

/// Where the main thread of a transcript stands: a turn in progress, or ended with background agents/workflows still pending.
#[derive(Clone, Debug, PartialEq)]
pub enum MainTurn {
    Running,
    Ended {
        background: i64,
        at: Option<SystemTime>,
    },
}

/// Main-thread `Agent`/`Task` calls opened (id, description) and tool-result ids closed, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentChanges {
    pub opened: Vec<(String, String)>,
    pub closed: Vec<String>,
}

/// How much of a transcript's start is read for the first prompt and branch.
pub const HEAD_BYTES: usize = 65_536;
/// The most a refresh reads of a busy session's transcript for the running-tool line.
pub const ACTIVITY_WINDOW: u64 = 262_144;
const APPROVABLE: [&str; 10] = [
    "Bash",
    "Edit",
    "Write",
    "MultiEdit",
    "NotebookEdit",
    "Read",
    "Glob",
    "Grep",
    "WebFetch",
    "WebSearch",
];

/// Claude Code names a project directory after the cwd with every non-alphanumeric char replaced by "-".
pub fn project_dir_name(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

pub fn find(session_id: &str, cwd: &str, projects: &Path) -> Option<PathBuf> {
    let direct = projects
        .join(project_dir_name(cwd))
        .join(format!("{session_id}.jsonl"));
    if direct.exists() {
        return Some(direct);
    }
    let dirs = std::fs::read_dir(projects).ok()?;
    dirs.filter_map(|e| e.ok())
        .map(|e| e.path().join(format!("{session_id}.jsonl")))
        .find(|p| p.exists())
}

/// The last `bytes` of the file.
pub fn tail(path: &Path, bytes: u64) -> Vec<u8> {
    let Ok(mut f) = File::open(path) else {
        return Vec::new();
    };
    let Ok(end) = f.seek(SeekFrom::End(0)) else {
        return Vec::new();
    };
    if f.seek(SeekFrom::Start(end.saturating_sub(bytes))).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    buf
}

/// The bytes `[from, to)` of the file.
pub fn read(path: &Path, from: u64, to: u64) -> Vec<u8> {
    if to <= from {
        return Vec::new();
    }
    let Ok(mut f) = File::open(path) else {
        return Vec::new();
    };
    if f.seek(SeekFrom::Start(from)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    let _ = f.take(to - from).read_to_end(&mut buf);
    buf
}

/// The first `bytes` of the file; when the file is longer, everything after the last newline is dropped.
pub fn head(path: &Path, bytes: usize) -> Vec<u8> {
    let Ok(f) = File::open(path) else {
        return Vec::new();
    };
    let mut d = Vec::new();
    if f.take(bytes as u64 + 1).read_to_end(&mut d).is_err() || d.is_empty() {
        return Vec::new();
    }
    if d.len() <= bytes {
        return d;
    }
    d.truncate(bytes);
    match d.iter().rposition(|b| *b == b'\n') {
        Some(nl) => {
            d.truncate(nl + 1);
            d
        }
        None => Vec::new(),
    }
}

pub fn strip_reminders(text: &str) -> String {
    const OPEN: &str = "<system-reminder>";
    const CLOSE: &str = "</system-reminder>";
    let mut t = text.to_string();
    while let Some(a) = t.find(OPEN) {
        let Some(b) = t[a + OPEN.len()..].find(CLOSE) else {
            break;
        };
        t.replace_range(a..a + OPEN.len() + b + CLOSE.len(), "");
    }
    t
}

/// The Jira project of a ticket key: `T6IR` for `T6IR-12`.
pub fn project_prefix(key: &str) -> Option<String> {
    key.split('-').find(|s| !s.is_empty()).map(String::from)
}

/// A one-line prompt for a row title, or `None` when the text is not something the user typed.
pub fn clean_prompt(text: &str) -> Option<String> {
    let mut t = strip_reminders(text).trim().to_string();
    if t.starts_with("<local-command") || t.starts_with("Caveat:") {
        return None;
    }
    if t.starts_with("<command-")
        && let Some(name) = inner(&t, "command-name")
    {
        let args = inner(&t, "command-args")
            .map(|a| a.trim().to_string())
            .unwrap_or_default();
        t = if args.is_empty() {
            String::new()
        } else {
            format!("{name} {args}")
        };
    }
    let collapsed = one_line(&t);
    (!collapsed.is_empty()).then_some(collapsed)
}

fn inner(t: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let a = t.find(&open)? + open.len();
    let b = a + t[a..].find(&format!("</{tag}>"))?;
    Some(t[a..b].to_string())
}

/// `https?://` up to whitespace, brackets, parentheses or quotes; trailing punctuation is stripped by `url_ranges`.
static URL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s<>()\[\]"'`]+"#).unwrap());
static MARKDOWN_LINK_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\[([^\]]*)\]\((https?://[^\s)]+)\)"#).unwrap());
static PR_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"/pull-requests/(\d+)",
        r"/pull/(\d+)",
        r"/merge_requests/(\d+)",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});
static JIRA_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/browse/([A-Z][A-Z0-9]{1,9}-\d+)").unwrap());
static ISSUE_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/issues/(\d+)").unwrap());

/// The URLs in `s` with their byte ranges, trailing `.,;:!?*` stripped.
pub fn url_ranges(s: &str) -> Vec<(Range<usize>, Url)> {
    URL_PATTERN
        .find_iter(s)
        .filter_map(|m| {
            let end = m.start()
                + m.as_str()
                    .trim_end_matches(['.', ',', ';', ':', '!', '?', '*'])
                    .len();
            let url = Url::parse(&s[m.start()..end]).ok()?;
            Some((m.start()..end, url))
        })
        .collect()
}

/// Every `KEY-123` in `text`, in order of appearance (a key repeated appears repeatedly). `KEY` is an uppercase letter
/// plus 1-9 uppercase letters or digits; it must not follow a letter, digit or `-`, and the number must not be followed by a letter or digit.
pub fn ticket_keys(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_uppercase()
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'-'))
        {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_uppercase() || b[j].is_ascii_digit()) {
                j += 1;
            }
            let class_len = j - i - 1;
            if (1..=9).contains(&class_len) && b.get(j) == Some(&b'-') {
                let mut k = j + 1;
                while k < b.len() && b[k].is_ascii_digit() {
                    k += 1;
                }
                if k > j + 1 && !b.get(k).is_some_and(u8::is_ascii_alphanumeric) {
                    out.push(text[i..k].to_string());
                    i = k;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// The Jira key and host of a `/browse/KEY-N` path or a `selectedIssue=KEY-N` query item.
pub fn jira_ticket(url: &Url) -> Option<(String, String)> {
    let host = url.host_str()?;
    if let Some(m) = JIRA_PATTERN.captures(url.path()) {
        return Some((m[1].to_string(), host.to_string()));
    }
    let item = url
        .query_pairs()
        .find(|(k, _)| k == "selectedIssue")
        .map(|(_, v)| v.into_owned())?;
    (ticket_keys(&item) == [item.as_str()]).then(|| (item, host.to_string()))
}

pub fn is_pull_request(url: &Url) -> bool {
    PR_PATTERNS.iter().any(|re| re.is_match(url.path()))
}

/// Host plus the path up to the PR number: one key for `/pull-requests/12` and `/pull-requests/12/diff`.
pub fn pull_request_key(url: &Url) -> Option<String> {
    let path = url.path();
    PR_PATTERNS
        .iter()
        .find_map(|re| re.find(path))
        .map(|m| format!("{}{}", url.host_str().unwrap_or(""), &path[..m.end()]))
}

/// A short label for a link button or inline URL.
pub fn label_for(url: &Url) -> String {
    let path = url.path();
    let capture = |re: &Regex| re.captures(path).map(|c| c[1].to_string());
    if let Some(n) = PR_PATTERNS.iter().find_map(capture) {
        return format!("PR #{n}");
    }
    if let Some(k) = capture(&JIRA_PATTERN) {
        return k;
    }
    if let Some(n) = capture(&ISSUE_PATTERN) {
        return format!("#{n}");
    }
    let host = url.host_str().unwrap_or(url.as_str());
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

/// The first `n` characters of `s`, plus `…` when `s` is longer.
fn cut(s: &str, n: usize) -> String {
    if s.graphemes(true).count() > n {
        format!("{}…", s.graphemes(true).take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Markdown links become their text, bare URLs their label; then cut to `limit` characters plus `…`.
pub fn display_text(text: &str, limit: usize) -> String {
    let t = MARKDOWN_LINK_PATTERN.replace_all(text, "$1");
    let mut out = String::new();
    let mut last = 0;
    for (range, url) in url_ranges(&t) {
        out += &t[last..range.start];
        out += &label_for(&url);
        last = range.end;
    }
    out += &t[last..];
    cut(&out, limit)
}

/// Complete lines of `data` (no empty ones), the way Swift's `Data.split(separator: 0x0A)` yields them.
pub fn split_lines(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0;
    for nl in memchr::memchr_iter(b'\n', data) {
        if nl > start {
            out.push(&data[start..nl]);
        }
        start = nl + 1;
    }
    if start < data.len() {
        out.push(&data[start..]);
    }
    out
}

fn parse_line(line: &[u8]) -> Option<Map<String, Value>> {
    match serde_json::from_slice(line) {
        Ok(Value::Object(o)) => Some(o),
        _ => None,
    }
}

/// `message.content` as a list of objects; `None` when it is anything else.
fn content_blocks(o: &Map<String, Value>) -> Option<Vec<&Map<String, Value>>> {
    o.get("message")?
        .get("content")?
        .as_array()?
        .iter()
        .map(Value::as_object)
        .collect()
}

pub fn flag(o: &Map<String, Value>, key: &str) -> bool {
    o.get(key).and_then(Value::as_bool) == Some(true)
}

fn str_field<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    o.get(key).and_then(Value::as_str)
}

pub fn text_of(blocks: &[&Map<String, Value>]) -> String {
    blocks
        .iter()
        .filter(|b| str_field(b, "type") == Some("text"))
        .filter_map(|b| str_field(b, "text"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn first_prompt(data: &[u8]) -> Option<String> {
    for line in split_lines(data) {
        let Some(o) = parse_line(line) else { continue };
        if str_field(&o, "type") != Some("user") || flag(&o, "isMeta") || flag(&o, "isSidechain") {
            continue;
        }
        let Some(content) = o.get("message").and_then(|m| m.get("content")) else {
            continue;
        };
        let text = match content {
            Value::String(s) => s.clone(),
            Value::Array(_) => match content_blocks(&o) {
                Some(blocks) => text_of(&blocks),
                None => continue,
            },
            _ => continue,
        };
        if let Some(p) = clean_prompt(&text) {
            return Some(p);
        }
    }
    None
}

pub fn last_ai_title(data: &[u8]) -> Option<String> {
    for line in split_lines(data).into_iter().rev() {
        let Some(o) = parse_line(line) else { continue };
        if str_field(&o, "type") == Some("ai-title")
            && let Some(t) = str_field(&o, "aiTitle").filter(|t| !t.is_empty())
        {
            return Some(t.to_string());
        }
    }
    None
}

pub fn last_reply(data: &[u8]) -> Option<String> {
    for line in split_lines(data).into_iter().rev() {
        let Some(o) = parse_line(line) else { continue };
        if str_field(&o, "type") != Some("assistant") || flag(&o, "isSidechain") {
            continue;
        }
        let Some(content) = content_blocks(&o) else {
            continue;
        };
        let collapsed = one_line(&text_of(&content));
        if !collapsed.is_empty() {
            return Some(collapsed);
        }
    }
    None
}

/// The tool calls still waiting for a result. Exactly one approvable call is `One`; anything else open is `Unclear`.
pub fn pending(data: &[u8]) -> Option<PendingState> {
    struct Open {
        id: String,
        name: String,
        input: Map<String, Value>,
    }
    let mut open: Vec<Open> = Vec::new();
    for line in split_lines(data) {
        let Some(o) = parse_line(line) else { continue };
        let Some(content) = content_blocks(&o) else {
            continue;
        };
        match str_field(&o, "type") {
            Some("assistant") if !flag(&o, "isSidechain") => {
                for c in content
                    .iter()
                    .filter(|c| str_field(c, "type") == Some("tool_use"))
                {
                    let (Some(id), Some(name)) = (str_field(c, "id"), str_field(c, "name")) else {
                        continue;
                    };
                    let input = c
                        .get("input")
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default();
                    open.push(Open {
                        id: id.into(),
                        name: name.into(),
                        input,
                    });
                }
            }
            Some("user") => {
                for c in content
                    .iter()
                    .filter(|c| str_field(c, "type") == Some("tool_result"))
                {
                    if let Some(id) = str_field(c, "tool_use_id") {
                        open.retain(|o| o.id != id);
                    }
                }
            }
            _ => {}
        }
    }
    let first = open.first()?;
    if open.len() != 1 || !APPROVABLE.contains(&first.name.as_str()) {
        return Some(PendingState::Unclear { count: open.len() });
    }
    match str_field(&first.input, input_key(&first.name)).filter(|t| !t.is_empty()) {
        Some(text) => Some(PendingState::One(Pending {
            tool_id: first.id.clone(),
            tool: first.name.clone(),
            text: text.into(),
        })),
        None => Some(PendingState::Unclear { count: 1 }),
    }
}

/// The input field that says what a tool call acts on.
pub fn input_key(tool: &str) -> &'static str {
    match tool {
        "Bash" => "command",
        "WebFetch" => "url",
        "WebSearch" => "query",
        "Glob" | "Grep" => "pattern",
        _ => "file_path",
    }
}

/// One line for a tool call, cut at 120 characters plus `…`. `input` is the call's input object.
pub fn tool_summary(name: &str, input: &Value) -> String {
    let field = |key: &str| {
        input
            .get(key)
            .and_then(Value::as_str)
            .map(one_line)
            .filter(|s| !s.is_empty())
    };
    let s = match name {
        "Bash" | "Glob" | "Grep" | "WebFetch" | "WebSearch" => {
            field(input_key(name)).map_or(name.to_string(), |f| format!("{name}: {f}"))
        }
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => field("file_path")
            .map_or(name.to_string(), |f| {
                format!("{name} {}", last_path_component(&f))
            }),
        "Agent" | "Task" => {
            field("description").map_or(name.to_string(), |f| format!("Agent: {f}"))
        }
        "Skill" => field("skill").map_or(name.to_string(), |f| format!("Skill: {f}")),
        _ if name.starts_with("mcp__") => {
            let parts: Vec<&str> = name.split("__").collect();
            if parts.len() > 2 {
                parts[2..].join("__")
            } else {
                name.to_string()
            }
        }
        _ => name.to_string(),
    };
    cut(&s, 120)
}

static TOOL_USE: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"\"tool_use\""));
static OPEN_MARKERS: LazyLock<[Finder<'static>; 2]> = LazyLock::new(|| {
    [
        Finder::new(b"\"name\":\"Agent\""),
        Finder::new(b"\"name\":\"Task\""),
    ]
});
static TURN_MARKERS: LazyLock<[Finder<'static>; 3]> = LazyLock::new(|| {
    [
        Finder::new(b"\"turn_duration\""),
        Finder::new(b"\"type\":\"user\""),
        Finder::new(b"\"type\":\"assistant\""),
    ]
});

pub(crate) fn has(line: &[u8], f: &Finder) -> bool {
    f.find(line).is_some()
}

/// The last main-thread tool call in `data`; only lines holding `"tool_use"` are parsed.
pub fn last_tool_use(data: &[u8]) -> Option<String> {
    for line in split_lines(data)
        .into_iter()
        .rev()
        .filter(|l| has(l, &TOOL_USE))
    {
        let Some(o) = parse_line(line) else { continue };
        if str_field(&o, "type") != Some("assistant") || flag(&o, "isSidechain") {
            continue;
        }
        let Some(content) = content_blocks(&o) else {
            continue;
        };
        let Some(call) = content
            .iter()
            .rfind(|c| str_field(c, "type") == Some("tool_use"))
        else {
            continue;
        };
        let Some(name) = str_field(call, "name") else {
            continue;
        };
        return Some(tool_summary(
            name,
            call.get("input").unwrap_or(&Value::Null),
        ));
    }
    None
}

fn tool_use_id_mark(id: &str) -> Vec<u8> {
    format!("\"tool_use_id\":\"{id}\"").into_bytes()
}

/// Main-thread `Agent`/`Task` calls opened and tool results seen in `lines`, in order. A line is parsed only when it holds
/// `"name":"Agent"`/`"name":"Task"` (compact JSON, as Claude writes it) or, with agents open (`open`, plus those opened in
/// `lines`), the `"tool_use_id":"<open id>"` of one of them.
pub fn agent_changes(lines: &[u8], open: &[String]) -> AgentChanges {
    let mut changes = AgentChanges::default();
    let mut open_marks: HashMap<String, Finder<'static>> = open
        .iter()
        .map(|id| (id.clone(), Finder::new(&tool_use_id_mark(id)).into_owned()))
        .collect();
    for line in split_lines(lines) {
        let opens = OPEN_MARKERS.iter().any(|m| has(line, m));
        if !opens && !open_marks.values().any(|m| has(line, m)) {
            continue;
        }
        let Some(o) = parse_line(line) else { continue };
        if flag(&o, "isSidechain") {
            continue;
        }
        let Some(content) = content_blocks(&o) else {
            continue;
        };
        match str_field(&o, "type") {
            Some("assistant") => {
                for c in content
                    .iter()
                    .filter(|c| str_field(c, "type") == Some("tool_use"))
                {
                    let (Some(id), Some(name)) = (str_field(c, "id"), str_field(c, "name")) else {
                        continue;
                    };
                    if name != "Agent" && name != "Task" {
                        continue;
                    }
                    let desc = c
                        .get("input")
                        .and_then(|i| i.get("description"))
                        .and_then(Value::as_str)
                        .map(one_line)
                        .unwrap_or_default();
                    changes.opened.push((
                        id.to_string(),
                        if desc.is_empty() {
                            name.to_string()
                        } else {
                            desc
                        },
                    ));
                    open_marks.insert(
                        id.to_string(),
                        Finder::new(&tool_use_id_mark(id)).into_owned(),
                    );
                }
            }
            Some("user") => {
                for c in content
                    .iter()
                    .filter(|c| str_field(c, "type") == Some("tool_result"))
                {
                    if let Some(id) = str_field(c, "tool_use_id") {
                        changes.closed.push(id.to_string());
                        open_marks.remove(id);
                    }
                }
            }
            _ => {}
        }
    }
    changes
}

/// The state given by the last main-thread user/assistant/`turn_duration` line of `data`; `None` when it has none.
pub fn main_turn(data: &[u8]) -> Option<MainTurn> {
    for line in split_lines(data)
        .into_iter()
        .rev()
        .filter(|l| TURN_MARKERS.iter().any(|m| has(l, m)))
    {
        let Some(o) = parse_line(line) else { continue };
        if flag(&o, "isSidechain") {
            continue;
        }
        match str_field(&o, "type") {
            Some("user" | "assistant") => return Some(MainTurn::Running),
            Some("system") if str_field(&o, "subtype") == Some("turn_duration") => {
                let count = |key: &str| o.get(key).and_then(Value::as_i64).unwrap_or(0);
                return Some(MainTurn::Ended {
                    background: count("pendingBackgroundAgentCount")
                        + count("pendingWorkflowCount"),
                    at: str_field(&o, "timestamp").and_then(parse_iso_date),
                });
            }
            _ => continue,
        }
    }
    None
}
