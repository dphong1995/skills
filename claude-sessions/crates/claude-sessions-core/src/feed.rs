//! The pending `AskUserQuestion` items of cmux Feed: parsing, answers, the control-socket transport and the monitor.

use crate::shell::parse_iso_date;
use crate::worker::Worker;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

/// A pending AskUserQuestion item of cmux Feed.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedQuestion {
    pub request_id: String,
    pub created_at: SystemTime,
    pub session_id: Option<String>,
    pub questions: Vec<FeedItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeedItem {
    pub prompt: String,
    pub header: Option<String>,
    pub multi_select: bool,
    pub options: Vec<FeedOption>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeedOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// How long the Feed hook waits for an answer, from the item's `created_at`.
pub const WINDOW: f64 = 120.0;
/// After a failed call the monitor waits this long before asking again.
pub const RETRY_AFTER: f64 = 15.0;
/// A reply line of cmux can never reach this size.
const MAX_REPLY: usize = 4_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockState {
    Form,
    Sent,
    Fallback,
}

/// What a card shows for a question: the form, `Đã gửi`, or only `Trả lời trong cmux`.
pub fn block_state(
    q: &FeedQuestion,
    live: bool,
    sent: bool,
    failed: bool,
    now: SystemTime,
) -> BlockState {
    if sent {
        return BlockState::Sent;
    }
    if live && !failed && supports(q) && remaining(q, now) > 0.0 {
        BlockState::Form
    } else {
        BlockState::Fallback
    }
}

/// Seconds left of the answer window (negative once it is over).
pub fn remaining(q: &FeedQuestion, now: SystemTime) -> f64 {
    let elapsed = match now.duration_since(q.created_at) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    };
    WINDOW - elapsed
}

/// `còn m:ss`, never below 0:00.
pub fn countdown(q: &FeedQuestion, now: SystemTime) -> String {
    let s = remaining(q, now).ceil().max(0.0) as u64;
    format!("còn {}:{:02}", s / 60, s % 60)
}

/// Every question has options to pick from: the shapes the spike showed cmux takes (single, multi, several questions, Other text).
pub fn supports(q: &FeedQuestion) -> bool {
    !q.questions.is_empty() && q.questions.iter().all(|i| !i.options.is_empty())
}

/// Every option of the card as `(question, option, number)`, numbered from 1 and continuous across the questions.
pub fn numbered_options(q: &FeedQuestion) -> Vec<(usize, usize, usize)> {
    q.questions
        .iter()
        .enumerate()
        .flat_map(|(i, item)| (0..item.options.len()).map(move |j| (i, j)))
        .enumerate()
        .map(|(n, (i, j))| (i, j, n + 1))
        .collect()
}

/// The number shown on an option; None past 9 (no digit key).
pub fn option_number(q: &FeedQuestion, question: usize, option: usize) -> Option<usize> {
    numbered_options(q)
        .into_iter()
        .find(|&(i, j, _)| i == question && j == option)
        .map(|(_, _, n)| n)
        .filter(|&n| n <= 9)
}

fn picks(answers: &[Vec<String>], i: usize) -> &[String] {
    answers.get(i).map_or(&[], Vec::as_slice)
}

/// The trimmed Other text of question `i`.
fn other_text(other: &HashMap<usize, String>, i: usize) -> &str {
    other.get(&i).map_or("", |s| s.trim())
}

/// Every question has a pick or non-blank Other text.
pub fn complete(q: &FeedQuestion, answers: &[Vec<String>], other: &HashMap<usize, String>) -> bool {
    (0..q.questions.len())
        .all(|i| !picks(answers, i).is_empty() || !other_text(other, i).is_empty())
}

/// The newest question of a session, matched by session id (the first of equally new ones).
pub fn match_session<'a>(items: &'a [FeedQuestion], session_id: &str) -> Option<&'a FeedQuestion> {
    items
        .iter()
        .filter(|q| q.session_id.as_deref() == Some(session_id))
        .fold(None, |best: Option<&FeedQuestion>, q| match best {
            Some(b) if b.created_at >= q.created_at => Some(b),
            _ => Some(q),
        })
}

/// `feed.question.reply` params: one string per answered question, in order: the label, the labels joined by `", "`
/// (multi-select), or the typed Other text; Other text follows the picks. Callers only send complete answers; a slot
/// without an answer sends `""`, so later answers keep their place.
pub fn reply_params(
    request_id: &str,
    question: Option<&FeedQuestion>,
    answers: &[Vec<String>],
    other: &HashMap<usize, String>,
) -> Value {
    let count = question.map_or_else(
        || answers.len().max(other.keys().max().map_or(0, |k| k + 1)),
        |q| q.questions.len(),
    );
    let selections: Vec<String> = (0..count)
        .map(|i| {
            let mut parts: Vec<&str> = picks(answers, i).iter().map(String::as_str).collect();
            let text = other_text(other, i);
            if !text.is_empty() {
                parts.push(text);
            }
            parts.join(", ")
        })
        .collect();
    json!({"request_id": request_id, "selections": selections})
}

/// The session id inside `cmux-feed-v1:<b64 source>:<b64 session id>`.
fn session_id_of(workstream: Option<&str>) -> Option<String> {
    use base64::Engine;
    let parts: Vec<&str> = workstream?.split(':').filter(|p| !p.is_empty()).collect();
    if parts.len() != 3 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(parts[2])
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// The pending questions of a `feed.list` response (snake_case items, as the spike showed); anything else (or a protocol change) gives an empty list.
pub fn parse_list(data: &[u8]) -> Vec<FeedQuestion> {
    let Ok(o) = serde_json::from_slice::<Value>(data) else {
        return Vec::new();
    };
    if o["ok"].as_bool() != Some(true) {
        return Vec::new();
    }
    let Some(items) = o["result"]["items"].as_array() else {
        return Vec::new();
    };
    items.iter().filter_map(parse_item).collect()
}

fn parse_item(item: &Value) -> Option<FeedQuestion> {
    if item["kind"].as_str() != Some("question")
        || item["source"].as_str() != Some("claude")
        || item["status"].as_str().unwrap_or("pending") != "pending"
    {
        return None;
    }
    let created_at = parse_iso_date(item["created_at"].as_str()?)?;
    let request_id = item["request_id"].as_str()?.to_string();
    let questions = item["questions"].as_array().map_or_else(Vec::new, |qs| {
        qs.iter().filter_map(parse_question).collect()
    });
    Some(FeedQuestion {
        request_id,
        created_at,
        session_id: session_id_of(item["workstream_id"].as_str()),
        questions,
    })
}

fn parse_question(q: &Value) -> Option<FeedItem> {
    let prompt = q["prompt"].as_str()?.to_string();
    let options = q["options"].as_array().map_or_else(Vec::new, |os| {
        os.iter()
            .filter_map(|o| {
                Some(FeedOption {
                    id: o["id"].as_str()?.to_string(),
                    label: o["label"].as_str()?.to_string(),
                    description: o["description"].as_str().map(String::from),
                })
            })
            .collect()
    });
    Some(FeedItem {
        prompt,
        header: q["header"].as_str().map(String::from),
        multi_select: q["multi_select"].as_bool().unwrap_or(false),
        options,
    })
}

/// One request line: the JSON through `serde_json` (so a newline inside an answer is escaped) plus the terminating `\n`.
pub fn encode_request(method: &str, params: &Value) -> Vec<u8> {
    let mut out = serde_json::to_vec(&json!({"id": uuid::Uuid::new_v4().to_string().to_uppercase(), "method": method, "params": params}))
        .unwrap_or_default();
    out.push(b'\n');
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportError;

pub trait FeedTransport: Send + Sync {
    /// Sends one request line and returns the one response line.
    fn request(
        &self,
        method: &str,
        params: &Value,
        timeout: Duration,
    ) -> Result<Vec<u8>, TransportError>;
}

/// cmux's control socket (`AF_UNIX` stream); no `auth` line is needed from outside cmux (spike).
pub struct UnixFeedTransport {
    path_file: std::path::PathBuf,
}

impl UnixFeedTransport {
    /// `path_file` holds the socket path on its first line; None when it does not exist.
    pub fn new(path_file: &Path) -> Option<Self> {
        path_file.exists().then(|| UnixFeedTransport {
            path_file: path_file.to_path_buf(),
        })
    }

    fn socket_path(&self) -> Option<String> {
        let text = std::fs::read_to_string(&self.path_file).ok()?;
        let line = text.split('\n').find(|l| !l.is_empty())?.trim().to_string();
        (!line.is_empty()).then_some(line)
    }
}

impl FeedTransport for UnixFeedTransport {
    fn request(
        &self,
        method: &str,
        params: &Value,
        timeout: Duration,
    ) -> Result<Vec<u8>, TransportError> {
        let path = self.socket_path().ok_or(TransportError)?;
        if path.len() >= 104 {
            return Err(TransportError);
        }
        // `lstat`: only a socket of this user, never a symlink to one.
        let meta = std::fs::symlink_metadata(&path).map_err(|_| TransportError)?;
        if !meta.file_type().is_socket() || meta.uid() != rustix::process::geteuid().as_raw() {
            return Err(TransportError);
        }
        let mut stream = UnixStream::connect(&path).map_err(|_| TransportError)?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|_| TransportError)?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|_| TransportError)?;
        stream
            .write_all(&encode_request(method, params))
            .map_err(|_| TransportError)?;
        let deadline = Instant::now() + timeout;
        let mut line = Vec::new();
        let mut chunk = vec![0u8; 65536];
        loop {
            // The socket timeout is per call: shrink it to what is left so a slow peer cannot hold the worker.
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(TransportError);
            }
            // Best effort: macOS refuses to set it once the peer has closed, and the deadline check above still ends the loop.
            let _ = stream.set_read_timeout(Some(left));
            let n = match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return Err(TransportError),
                Ok(n) => n,
            };
            let start = line.len();
            line.extend_from_slice(&chunk[..n]);
            if line.len() >= MAX_REPLY {
                return Err(TransportError);
            }
            if let Some(i) = chunk[..n].iter().position(|&b| b == b'\n') {
                line.truncate(start + i);
                return Ok(line);
            }
        }
    }
}

#[derive(Default)]
struct State {
    snapshot: Vec<FeedQuestion>,
    pending: Option<(bool, SystemTime)>,
    paused_until: Option<SystemTime>,
}

struct Inner {
    transport: Arc<dyn FeedTransport>,
    state: Mutex<State>,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tick(&self, active: bool, now: SystemTime) {
        if !active {
            self.lock().snapshot.clear();
            return;
        }
        if self.lock().paused_until.is_some_and(|p| now < p) {
            return;
        }
        match self.transport.request(
            "feed.list",
            &json!({"pending_only": true}),
            Duration::from_secs(2),
        ) {
            Ok(data) => self.lock().snapshot = parse_list(&data),
            Err(_) => {
                let mut s = self.lock();
                s.snapshot.clear();
                s.paused_until = Some(now + Duration::from_secs_f64(RETRY_AFTER));
            }
        }
    }

    fn reply(
        &self,
        q: &FeedQuestion,
        answers: &[Vec<String>],
        other: &HashMap<usize, String>,
    ) -> bool {
        if !self
            .lock()
            .snapshot
            .iter()
            .any(|s| s.request_id == q.request_id)
        {
            return false;
        }
        let params = reply_params(&q.request_id, Some(q), answers, other);
        let Ok(data) =
            self.transport
                .request("feed.question.reply", &params, Duration::from_secs(5))
        else {
            return false;
        };
        let ok =
            serde_json::from_slice::<Value>(&data).is_ok_and(|o| o["ok"].as_bool() == Some(true));
        if ok {
            self.lock()
                .snapshot
                .retain(|s| s.request_id != q.request_id);
        }
        ok
    }
}

/// Polls `feed.list` off the main thread while a session may be asking, and sends answers. Any transport or parse
/// error empties the snapshot and pauses polling for `RETRY_AFTER`, so a cmux protocol change only turns the feature off.
pub struct FeedMonitor {
    inner: Arc<Inner>,
    worker: Worker,
}

impl FeedMonitor {
    pub fn new(transport: Arc<dyn FeedTransport>) -> Self {
        Self::with_worker(transport, Worker::spawn("claude-sessions.feed-monitor"))
    }

    /// Runs everything inline on the calling thread (tests).
    pub fn without_worker(transport: Arc<dyn FeedTransport>) -> Self {
        Self::with_worker(transport, Worker::inline())
    }

    fn with_worker(transport: Arc<dyn FeedTransport>, worker: Worker) -> Self {
        FeedMonitor {
            inner: Arc::new(Inner {
                transport,
                state: Mutex::new(State::default()),
            }),
            worker,
        }
    }

    /// Cheap: queues one poll when `active`; otherwise forgets the questions. Calls made while one is queued replace its arguments.
    pub fn update(&self, active: bool, now: SystemTime) {
        if self.worker.is_inline() {
            return self.inner.tick(active, now);
        }
        let queued = self.inner.lock().pending.replace((active, now)).is_some();
        if queued {
            return;
        }
        let inner = self.inner.clone();
        let on_dead = self.inner.clone();
        self.worker.submit_or_else(
            move || {
                let p = inner.lock().pending.take();
                if let Some((active, now)) = p {
                    inner.tick(active, now);
                }
            },
            move || on_dead.lock().pending = None,
        );
    }

    pub fn take_snapshot(&self) -> Vec<FeedQuestion> {
        self.inner.lock().snapshot.clone()
    }

    /// Answers `q` if it is still in the last list; `completion` runs on the worker (the calling thread without one).
    pub fn reply(
        &self,
        q: &FeedQuestion,
        answers: &[Vec<String>],
        other: &HashMap<usize, String>,
        completion: impl FnOnce(bool) + Send + 'static,
    ) {
        let inner = self.inner.clone();
        let (q, answers, other) = (q.clone(), answers.to_vec(), other.clone());
        // `completion` is reachable from the job and from the dead-worker path; whichever runs first takes it.
        let completion = Arc::new(Mutex::new(Some(completion)));
        let completion_dead = completion.clone();
        self.worker.submit_or_else(
            move || {
                let ok = inner.reply(&q, &answers, &other);
                if let Some(c) = completion.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    c(ok);
                }
            },
            move || {
                if let Some(c) = completion_dead
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
                {
                    c(false);
                }
            },
        );
    }
}
