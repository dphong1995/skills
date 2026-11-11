//! Reads each session's transcript incrementally on a worker thread and keeps the tickets and PR links it mentions.

use crate::jira;
use crate::pull_requests::{self, PRRef};
use crate::time::secs_since;
use crate::transcript::{self, flag, has, text_of};
use memchr::memmem::Finder;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::thread::JoinHandle;
use std::time::SystemTime;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TicketMention {
    pub key: String,
    pub by_user: bool,
    pub first: u64,
    pub last: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanResult {
    /// Every KEY-123 seen (URL or bare), unfiltered.
    pub tickets: HashMap<String, TicketMention>,
    /// Jira project prefix -> host.
    pub jira_hosts: HashMap<String, String>,
    /// PR-shaped URLs, first appearance order, one per PR.
    pub pr_links: Vec<Url>,
    /// Bitbucket refs, first appearance order, deduped.
    pub pr_refs: Vec<PRRef>,
    /// Bumped whenever `absorb` adds or changes anything, so callers can cache what they derive from it.
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanTarget {
    pub session_id: String,
    pub transcript: Option<PathBuf>,
    pub size: Option<u64>,
}

const CHUNK_BYTES: u64 = 1 << 20;
const MAX_LINE_BYTES: usize = 4 << 20;
const FORGET_AFTER: f64 = 30.0;

struct State {
    url: PathBuf,
    offset: u64,
    carry: Vec<u8>,
    skipping: bool,
    counter: u64,
    /// The current turn began with a prompt the user typed, or with the report of an agent launched in such a turn.
    your_turn: bool,
    /// Ids of main-thread tool_use blocks issued while `your_turn` was true.
    your_tool_uses: HashSet<String>,
    result: ScanResult,
    last_seen: SystemTime,
}

impl State {
    fn new(url: PathBuf, last_seen: SystemTime) -> Self {
        Self {
            url,
            offset: 0,
            carry: Vec::new(),
            skipping: false,
            counter: 0,
            your_turn: false,
            your_tool_uses: HashSet::new(),
            result: ScanResult::default(),
            last_seen,
        }
    }
}

type OnPublish = Box<dyn FnMut(&str) + Send>;

#[derive(Default)]
struct Shared {
    pending: Option<(Vec<ScanTarget>, SystemTime)>,
    snapshot: HashMap<String, ScanResult>,
    parsed: usize,
    shutdown: bool,
}

#[derive(Default)]
struct Inner {
    shared: Mutex<Shared>,
    wake: Condvar,
    /// Only touched inside `tick`, which runs one at a time.
    states: Mutex<HashMap<String, State>>,
    on_publish: Mutex<Option<OnPublish>>,
}

pub struct TranscriptScanner {
    inner: Arc<Inner>,
    worker: Option<JoinHandle<()>>,
}

impl TranscriptScanner {
    /// `threaded: false` runs each `update` inline on the caller's thread (tests); `true` starts the worker thread
    /// `claude-sessions.transcript-scanner`.
    pub fn new(threaded: bool) -> Self {
        let inner = Arc::new(Inner::default());
        let worker = threaded.then(|| {
            let inner = Arc::clone(&inner);
            std::thread::Builder::new()
                .name("claude-sessions.transcript-scanner".into())
                .spawn(move || inner.run())
                .expect("spawn transcript scanner thread")
        });
        Self { inner, worker }
    }

    /// Cheap: hands the latest targets to the worker, or scans them now when not threaded.
    pub fn update(&self, targets: Vec<ScanTarget>, now: SystemTime) {
        if self.worker.is_none() {
            return self.inner.tick(&targets, now);
        }
        self.inner
            .shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending = Some((targets, now));
        self.inner.wake.notify_one();
    }

    pub fn take_snapshot(&self) -> HashMap<String, ScanResult> {
        self.inner
            .shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }

    /// JSON parses done, for tests.
    pub fn parsed_lines(&self) -> usize {
        self.inner
            .shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .parsed
    }

    /// Called on the scan thread after each session's result is published, for tests.
    pub fn set_on_publish(&self, f: OnPublish) {
        *self
            .inner
            .on_publish
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(f);
    }
}

impl Drop for TranscriptScanner {
    fn drop(&mut self) {
        self.inner
            .shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .shutdown = true;
        self.inner.wake.notify_one();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

impl Inner {
    fn run(&self) {
        loop {
            let (targets, now) = {
                let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if shared.shutdown {
                        return;
                    }
                    if let Some(p) = shared.pending.take() {
                        break p;
                    }
                    shared = self.wake.wait(shared).unwrap_or_else(|e| e.into_inner());
                }
            };
            self.tick(&targets, now);
        }
    }

    fn tick(&self, targets: &[ScanTarget], now: SystemTime) {
        let mut states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        for t in targets {
            let (Some(url), Some(size)) = (&t.transcript, t.size) else {
                continue;
            };
            let known = states.remove(&t.session_id);
            let mut fresh = known.is_none();
            let mut s = known.unwrap_or_else(|| State::new(url.clone(), now));
            if s.url != *url || size < s.offset {
                s = State::new(url.clone(), now);
                fresh = true;
            }
            s.last_seen = now;
            let before = s.offset;
            let n = scan(&mut s, size);
            {
                let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
                // Copy the result only for a new state or when this scan read something.
                if fresh || s.offset != before {
                    shared
                        .snapshot
                        .insert(t.session_id.clone(), s.result.clone());
                }
                shared.parsed += n;
            }
            states.insert(t.session_id.clone(), s);
            if let Some(f) = self
                .on_publish
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_mut()
            {
                f(&t.session_id);
            }
        }
        let live: HashSet<&str> = targets.iter().map(|t| t.session_id.as_str()).collect();
        states.retain(|id, s| {
            live.contains(id.as_str()) || secs_since(now, s.last_seen) <= FORGET_AFTER
        });
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .retain(|id, _| states.contains_key(id));
    }
}

/// Reads `[offset, size)` in chunks; returns how many lines were parsed as JSON.
fn scan(s: &mut State, size: u64) -> usize {
    let mut parsed = 0;
    while s.offset < size {
        let mut chunk = transcript::read(&s.url, s.offset, size.min(s.offset + CHUNK_BYTES));
        if chunk.is_empty() {
            break;
        }
        s.offset += chunk.len() as u64;
        let buf = if s.skipping {
            let Some(nl) = memchr::memchr(b'\n', &chunk) else {
                continue;
            };
            s.skipping = false;
            chunk.drain(..=nl);
            chunk
        } else if s.carry.is_empty() {
            chunk
        } else {
            let mut b = std::mem::take(&mut s.carry);
            b.extend_from_slice(&chunk);
            b
        };
        if let Some(end) = memchr::memrchr(b'\n', &buf) {
            s.carry = buf[end + 1..].to_vec();
            for line in transcript::split_lines(&buf[..end]) {
                if line.len() > MAX_LINE_BYTES {
                    continue;
                }
                if s.your_turn {
                    record_tool_use(line, s);
                }
                if !keep(line) {
                    continue;
                }
                parsed += 1;
                absorb(line, s);
            }
        } else {
            s.carry = buf;
        }
        if s.carry.len() > MAX_LINE_BYTES {
            s.carry = Vec::new();
            s.skipping = true;
        }
    }
    parsed
}

const TOOL_USE_MARK: &str = "\"type\":\"tool_use\"";

fn finders(needles: &[&'static str]) -> Vec<Finder<'static>> {
    needles.iter().map(|n| Finder::new(n.as_bytes())).collect()
}

static WANT: LazyLock<Vec<Finder<'static>>> =
    LazyLock::new(|| finders(&["\"type\":\"user\"", "\"type\":\"assistant\""]));
static REJECT: LazyLock<Vec<Finder<'static>>> = LazyLock::new(|| {
    finders(&[
        TOOL_USE_MARK,
        "\"type\":\"tool_result\"",
        "\"type\":\"thinking\"",
        "\"isSidechain\":true",
        "\"isMeta\":true",
    ])
});
static ASSISTANT: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b"\"type\":\"assistant\""));
static TOOL_USE: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(TOOL_USE_MARK.as_bytes()));
static SIDECHAIN: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b"\"isSidechain\":true"));
/// How Claude Code writes a tool_use block's start; the id follows up to the next quote.
static TOOL_USE_ID_MARK: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b"\"type\":\"tool_use\",\"id\":\""));

/// Remembers the tool_use ids of a main-thread assistant line, so a later report of that agent continues the turn.
fn record_tool_use(line: &[u8], s: &mut State) {
    if !has(line, &TOOL_USE) || !has(line, &ASSISTANT) || has(line, &SIDECHAIN) {
        return;
    }
    let mark_len = b"\"type\":\"tool_use\",\"id\":\"".len();
    let mut from = 0;
    let mut found = false;
    while let Some(at) = TOOL_USE_ID_MARK.find(&line[from..]) {
        found = true;
        let rest = &line[from + at + mark_len..];
        let Some(close) = memchr::memchr(b'"', rest) else {
            return;
        };
        s.your_tool_uses
            .insert(String::from_utf8_lossy(&rest[..close]).into_owned());
        from += at + mark_len + close;
    }
    if found {
        return;
    }
    // Another key order (not Claude Code's own): parse the line.
    let Ok(Value::Object(o)) = serde_json::from_slice::<Value>(line) else {
        return;
    };
    let Some(blocks) = o
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for b in blocks {
        if b.get("type").and_then(Value::as_str) == Some("tool_use")
            && let Some(id) = b.get("id").and_then(Value::as_str)
        {
            s.your_tool_uses.insert(id.to_string());
        }
    }
}

fn keep(line: &[u8]) -> bool {
    WANT.iter().any(|f| has(line, f)) && !REJECT.iter().any(|f| has(line, f))
}

fn tool_use_id(text: &str) -> Option<&str> {
    let open = text.find("<tool-use-id>")? + "<tool-use-id>".len();
    let close = open + text[open..].find("</tool-use-id>")?;
    Some(&text[open..close])
}

/// Parses one kept line and folds its text into `s.result`.
fn absorb(line: &[u8], s: &mut State) {
    let Ok(Value::Object(o)) = serde_json::from_slice::<Value>(line) else {
        return;
    };
    if flag(&o, "isSidechain") || flag(&o, "isMeta") {
        return;
    }
    let (Some(ty), Some(content)) = (
        o.get("type").and_then(Value::as_str),
        o.get("message").and_then(|m| m.get("content")),
    ) else {
        return;
    };
    let is_user = ty == "user";
    let text = match content {
        Value::String(str) => {
            if is_user && pull_requests::is_shell_output(str) {
                return;
            }
            str.clone()
        }
        Value::Array(items) => {
            let Some(blocks) = items
                .iter()
                .map(Value::as_object)
                .collect::<Option<Vec<&Map<String, Value>>>>()
            else {
                return;
            };
            let kept: Vec<&Map<String, Value>> = if is_user {
                blocks
                    .into_iter()
                    .filter(|b| {
                        !pull_requests::is_shell_output(
                            b.get("text").and_then(Value::as_str).unwrap_or(""),
                        )
                    })
                    .collect()
            } else {
                blocks
            };
            text_of(&kept)
        }
        _ => return,
    };
    let clean = transcript::strip_reminders(&text);
    let summary = flag(&o, "isCompactSummary");
    let by_user = is_user && !summary;
    let mut is_notification = false;
    if by_user {
        // Older transcripts have no `origin`; their agent reports are recognised by the tag.
        let kind = o
            .get("origin")
            .and_then(|k| k.get("kind"))
            .and_then(Value::as_str);
        is_notification = kind.map_or_else(
            || text.starts_with("<task-notification>"),
            |k| k == "task-notification",
        );
        s.your_turn = if is_notification {
            tool_use_id(&text).is_some_and(|id| s.your_tool_uses.contains(id))
        } else {
            kind.is_none_or(|k| k == "human")
        };
    }
    let keys = if !is_notification && (s.your_turn || summary) {
        transcript::ticket_keys(&clean)
    } else {
        Vec::new()
    };
    for key in keys {
        s.counter += 1;
        s.result.version += 1;
        let counter = s.counter;
        s.result
            .tickets
            .entry(key.clone())
            .and_modify(|m| {
                m.by_user = m.by_user || by_user;
                m.last = counter;
            })
            .or_insert(TicketMention {
                key,
                by_user,
                first: counter,
                last: counter,
            });
    }
    for (_, url) in transcript::url_ranges(&clean) {
        if let Some((key, host)) = transcript::jira_ticket(&url)
            && let Some(prefix) = transcript::project_prefix(&key)
        {
            let host = host.to_lowercase();
            if jira::replaces(s.result.jira_hosts.get(&prefix).map(String::as_str), &host) {
                s.result.jira_hosts.insert(prefix, host);
                s.result.version += 1;
            }
        }
        if !transcript::is_pull_request(&url) {
            continue;
        }
        for r in pull_requests::refs(url.as_str()) {
            if !s.result.pr_refs.contains(&r) {
                s.result.pr_refs.push(r);
                s.result.version += 1;
            }
        }
        if let Some(key) = transcript::pull_request_key(&url)
            && !s
                .result
                .pr_links
                .iter()
                .any(|l| transcript::pull_request_key(l).as_deref() == Some(key.as_str()))
        {
            s.result.pr_links.push(url);
            s.result.version += 1;
        }
    }
}
