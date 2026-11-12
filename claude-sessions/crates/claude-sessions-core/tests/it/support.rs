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

// ---- Bitbucket / CodeRabbit JSON builders (Swift `prJSON`, `rabbitStatus`, `rabbitComment`, ...) ----

/// Arguments of `pr_json`; the defaults are Swift's.
pub struct PrSpec {
    pub id: i64,
    pub repo: &'static str,
    pub branch: &'static str,
    pub state: &'static str,
    pub comments: i64,
    pub reviewers: usize,
    pub rabbit_approved: Option<bool>,
}

impl Default for PrSpec {
    fn default() -> Self {
        PrSpec {
            id: 0,
            repo: "t6engineering/talo6-dct-backend",
            branch: "bugfix/T6IR-14744",
            state: "OPEN",
            comments: 0,
            reviewers: 0,
            rabbit_approved: None,
        }
    }
}

/// A pull request as the Bitbucket list returns it.
pub fn pr_json(s: PrSpec) -> Value {
    let mut participants = Vec::new();
    for i in 0..s.reviewers {
        let state = match i {
            0 => json!("changes_requested"),
            1 => json!("approved"),
            _ => Value::Null,
        };
        participants.push(
            json!({"role": "REVIEWER", "approved": i == 1, "state": state,
            "user": {"uuid": format!("{{r{i}}}"), "display_name": format!("Reviewer {i}")}}),
        );
    }
    if let Some(approved) = s.rabbit_approved {
        participants.push(json!({"role": "REVIEWER", "approved": approved,
            "state": if approved { json!("approved") } else { Value::Null },
            "user": {"uuid": "{tin}", "display_name": "Tin Tran"}}));
    }
    participants.push(
        json!({"role": "PARTICIPANT", "approved": false, "state": null,
        "user": {"uuid": "{author}", "display_name": "Author"}}),
    );
    json!({"id": s.id, "title": "t", "state": s.state, "comment_count": s.comments,
        "source": {"branch": {"name": s.branch}, "repository": {"full_name": s.repo}, "commit": {"hash": "abc123"}},
        "links": {"html": {"href": format!("https://bitbucket.org/{}/pull-requests/{}", s.repo, s.id)}},
        "participants": participants})
}

pub fn comment(uuid: &str, name: &str, at: &str) -> Value {
    json!({"user": {"uuid": uuid, "display_name": name}, "created_on": at, "deleted": false})
}

pub const RABBIT_CREATED: &str = "2026-11-03T03:33:15.111541+00:00";

/// A CodeRabbit build status as Bitbucket returns it.
pub fn rabbit_status(
    state: &str,
    description: &str,
    commit: &str,
    created: &str,
    updated: Option<&str>,
) -> Value {
    json!({"key": "coderabbitai/6867", "name": "CodeRabbit", "state": state, "description": description,
        "created_on": created, "updated_on": updated.unwrap_or(created), "commit": {"hash": commit}})
}

/// A comment carrying CodeRabbit's marker, posted by a human account like the real ones.
pub fn rabbit_comment(raw: &str, at: &str) -> Value {
    json!({"user": {"uuid": "{tin}", "display_name": "Tin Tran"}, "created_on": at, "updated_on": at, "deleted": false,
        "content": {"raw": format!("[coderabbit-html-comment-v1-0]: https://coderabbit.ai/#html-comment-v1:abc\n{raw}")}})
}

/// An inline CodeRabbit comment: a finding, or a reply when `parent` is given.
pub fn rabbit_inline(id: i64, raw: &str, at: &str, parent: Option<i64>) -> Value {
    let mut c = rabbit_comment(raw, at);
    c["id"] = json!(id);
    c["inline"] = json!({"path": "a.ts"});
    if let Some(p) = parent {
        c["parent"] = json!({"id": p});
    }
    c
}

/// CodeRabbit's summary and one unanswered finding (Swift `pendingFinding`).
pub fn pending_finding(count: i64, at: &str) -> Vec<Value> {
    vec![
        rabbit_comment(&format!("**Actionable comments posted: {count}**"), at),
        rabbit_inline(1, "nit", at, None),
    ]
}

/// A comment without the CodeRabbit marker, e.g. a human reply to a finding.
pub fn human_reply(
    id: i64,
    parent: Option<i64>,
    uuid: &str,
    raw: &str,
    at: &str,
    inline: bool,
) -> Value {
    let mut c = json!({"id": id, "user": {"uuid": uuid, "display_name": "Human"}, "created_on": at, "updated_on": at,
        "deleted": false, "content": {"raw": raw}});
    if inline {
        c["inline"] = json!({"path": "a.ts"});
    }
    if let Some(p) = parent {
        c["parent"] = json!({"id": p});
    }
    c
}

// ---- Local HTTP server (no network: 127.0.0.1 only) ----

/// Answers every request with `handler(request_head)` (raw bytes of the full response) and records the request heads.
pub struct TestHttpServer {
    pub port: u16,
    pub requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TestHttpServer {
    pub fn start(handler: impl Fn(&str) -> String + Send + 'static) -> Self {
        use std::io::Read;
        use std::sync::atomic::Ordering;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (reqs, stopped) = (requests.clone(), stop.clone());
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut conn) = conn else { continue };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match conn.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&buf).into_owned();
                let response = handler(&head);
                reqs.lock().unwrap().push(head);
                let _ = conn.write_all(response.as_bytes());
            }
        });
        TestHttpServer {
            port,
            requests,
            stop,
        }
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
    }
}

// ---- cmux Feed ----

use claude_sessions_core::feed::{FeedQuestion, FeedTransport, TransportError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One `question` item as `feed.list` returns it (snake_case; the session id is inside `workstream_id`).
pub fn feed_item(request_id: &str, session_id: &str, created: SystemTime) -> Value {
    use base64::Engine;
    let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
    json!({"kind": "question", "id": format!("ID-{request_id}"), "request_id": request_id, "status": "pending",
        "source": "claude", "cwd": "/x", "created_at": format_iso_date(created),
        "workstream_id": format!("cmux-feed-v1:{}:{}", b64("claude"), b64(session_id)),
        "questions": [feed_question_json("Pick a color", "Color", false, &["Red", "Blue", "Green"])]})
}

pub fn feed_question_json(prompt: &str, header: &str, multi: bool, labels: &[&str]) -> Value {
    let options: Vec<Value> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| json!({"id": format!("opt{i}"), "label": l, "description": format!("d-{l}")}))
        .collect();
    json!({"id": "q0", "prompt": prompt, "header": header, "multi_select": multi, "options": options})
}

pub fn feed_question(request_id: &str, created: SystemTime) -> FeedQuestion {
    use claude_sessions_core::feed::{FeedItem, FeedOption};
    FeedQuestion {
        request_id: request_id.into(),
        created_at: created,
        session_id: Some("a".into()),
        questions: vec![FeedItem {
            prompt: "Pick a color".into(),
            header: Some("Color".into()),
            multi_select: false,
            options: ["Red", "Blue"]
                .iter()
                .map(|l| FeedOption {
                    id: l.to_string(),
                    label: l.to_string(),
                    description: None,
                })
                .collect(),
        }],
    }
}

/// Canned `feed.list` / `feed.question.reply`; records every call. `fail_all` makes every call fail.
pub struct FakeFeedTransport {
    pub items: Mutex<Vec<Value>>,
    pub fail_all: Mutex<bool>,
    pub reply_result: Mutex<String>,
    pub calls: Mutex<Vec<(String, Value)>>,
}

impl FakeFeedTransport {
    pub fn new() -> Arc<Self> {
        Arc::new(FakeFeedTransport {
            items: Mutex::new(Vec::new()),
            fail_all: Mutex::new(false),
            reply_result: Mutex::new(r#"{"ok":true,"result":{"delivered":true}}"#.into()),
            calls: Mutex::new(Vec::new()),
        })
    }

    pub fn methods(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.0.clone())
            .collect()
    }
}

impl FeedTransport for FakeFeedTransport {
    fn request(
        &self,
        method: &str,
        params: &Value,
        _timeout: Duration,
    ) -> Result<Vec<u8>, TransportError> {
        self.calls
            .lock()
            .unwrap()
            .push((method.to_string(), params.clone()));
        if *self.fail_all.lock().unwrap() {
            return Err(TransportError);
        }
        if method == "feed.list" {
            let items = self.items.lock().unwrap().clone();
            return Ok(serde_json::to_vec(
                &json!({"ok": true, "id": "x", "result": {"items": items}}),
            )
            .unwrap());
        }
        Ok(self.reply_result.lock().unwrap().clone().into_bytes())
    }
}

/// What a `TestUnixServer` does with one request line.
pub enum Reply {
    Bytes(Vec<u8>),
    /// Reads the request and then says nothing until the client closes.
    Silent,
}

/// An `AF_UNIX` listener: reads one line per connection, answers as `handler` says.
pub struct TestUnixServer {
    pub requests: Arc<Mutex<Vec<Vec<u8>>>>,
    path: PathBuf,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl TestUnixServer {
    pub fn start(path: &Path, handler: impl Fn(&[u8]) -> Reply + Send + Sync + 'static) -> Self {
        use std::io::{BufRead, BufReader, Read};
        use std::os::unix::net::UnixListener;
        use std::sync::atomic::Ordering;
        let listener = UnixListener::bind(path).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (reqs, stopped, handler) = (requests.clone(), stop.clone(), Arc::new(handler));
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                let (reqs, handler) = (reqs.clone(), handler.clone());
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(&conn);
                    let mut line = Vec::new();
                    if reader.read_until(b'\n', &mut line).is_err() {
                        return;
                    }
                    reqs.lock().unwrap().push(line.clone());
                    match handler(&line) {
                        Reply::Bytes(b) => {
                            let _ = (&conn).write_all(&b);
                        }
                        Reply::Silent => {
                            let mut rest = Vec::new();
                            let _ = reader.read_to_end(&mut rest);
                        }
                    }
                });
            }
        });
        TestUnixServer {
            requests,
            path: path.to_path_buf(),
            stop,
        }
    }

    pub fn last_request(&self) -> Vec<u8> {
        self.requests
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

impl Drop for TestUnixServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::os::unix::net::UnixStream::connect(&self.path);
        let _ = fs::remove_file(&self.path);
    }
}

/// A short temp dir for socket paths (`sun_path` holds about 100 bytes); the caller keeps the guard alive.
pub fn make_socket_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("fs-")
        .tempdir_in("/tmp")
        .unwrap()
}

// ---- Bitbucket / Jira fakes (Swift `FakeBitbucket`, `FakeJira`) ----

use claude_sessions_core::bitbucket::{BitbucketApi, BitbucketError};
use claude_sessions_core::jira::{JiraApi, JiraError};
use std::collections::HashMap;
use url::Url;

/// Canned Bitbucket. A route matches when the requested path contains its key; the longest matching key wins.
/// Every request is recorded; `fail_all` makes every call fail.
#[derive(Default)]
pub struct FakeBitbucket {
    routes: Mutex<HashMap<String, Result<Vec<u8>, BitbucketError>>>,
    pub fail_all: Mutex<Option<BitbucketError>>,
    requests: Mutex<Vec<String>>,
    posts: Mutex<Vec<(String, Value)>>,
    pub post_result: Mutex<Option<Result<Vec<u8>, BitbucketError>>>,
    /// When set, `post` blocks until the sender side sends or drops.
    pub post_gate: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
}

impl FakeBitbucket {
    pub fn set(&self, key: &str, json: Value) {
        self.routes
            .lock()
            .unwrap()
            .insert(key.to_string(), Ok(serde_json::to_vec(&json).unwrap()));
    }

    pub fn set_error(&self, key: &str, e: BitbucketError) {
        self.routes.lock().unwrap().insert(key.to_string(), Err(e));
    }

    pub fn remove(&self, key: &str) {
        self.routes.lock().unwrap().remove(key);
    }

    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    pub fn requests_containing(&self, s: &str) -> usize {
        self.requests().iter().filter(|r| r.contains(s)).count()
    }

    pub fn posts(&self) -> Vec<(String, Value)> {
        self.posts.lock().unwrap().clone()
    }
}

impl BitbucketApi for FakeBitbucket {
    fn get(&self, path: &str) -> Result<Vec<u8>, BitbucketError> {
        self.requests.lock().unwrap().push(path.to_string());
        if let Some(e) = *self.fail_all.lock().unwrap() {
            return Err(e);
        }
        let routes = self.routes.lock().unwrap();
        let key = routes
            .keys()
            .filter(|k| path.contains(k.as_str()))
            .max_by_key(|k| k.len())
            .ok_or(BitbucketError::Http(404))?;
        routes[key].clone()
    }

    fn post(&self, path: &str, json: &Value) -> Result<Vec<u8>, BitbucketError> {
        self.posts
            .lock()
            .unwrap()
            .push((path.to_string(), json.clone()));
        if let Some(gate) = self.post_gate.lock().unwrap().take() {
            let _ = gate.recv();
        }
        self.post_result
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| Ok(b"{}".to_vec()))
    }
}

/// A `FakeBitbucket` whose calls wait while the test holds `gate`: keeps a threaded monitor's worker mid-cycle.
pub struct GatedApi {
    inner: Arc<FakeBitbucket>,
    pub gate: Mutex<()>,
}

impl GatedApi {
    pub fn new(inner: Arc<FakeBitbucket>) -> Self {
        GatedApi {
            inner,
            gate: Mutex::new(()),
        }
    }
}

impl BitbucketApi for GatedApi {
    fn get(&self, path: &str) -> Result<Vec<u8>, BitbucketError> {
        let _open = self.gate.lock().unwrap();
        self.inner.get(path)
    }

    fn post(&self, path: &str, json: &Value) -> Result<Vec<u8>, BitbucketError> {
        let _open = self.gate.lock().unwrap();
        self.inner.post(path, json)
    }
}

/// Routes for the user, an open list and default passing CI status / no comments.
pub fn fake(open: Vec<Value>) -> Arc<FakeBitbucket> {
    let api = Arc::new(FakeBitbucket::default());
    api.set("/2.0/user", json!({"uuid": "{me}"}));
    api.set(
        "/2.0/workspaces/t6engineering/pullrequests/",
        json!({"values": open}),
    );
    api.set("/statuses", json!({"values": [{"state": "SUCCESSFUL"}]}));
    api.set("/comments", json!({"values": []}));
    api
}

#[derive(Default)]
pub struct FakeJira {
    /// `(host, keys)` per `bulk_fetch` call.
    pub calls: Mutex<Vec<(String, Vec<String>)>>,
    pub issues: Mutex<HashMap<String, (String, Option<String>)>>,
    /// Key to `(issue type name, icon url)`.
    pub types: Mutex<HashMap<String, (String, String)>>,
    pub icon_data: Mutex<HashMap<Url, Vec<u8>>>,
    pub icon_fail: Mutex<Option<JiraError>>,
    pub icon_calls: Mutex<Vec<Url>>,
    pub fail: Mutex<Option<JiraError>>,
    /// Key to `statusCategory.key` (`new`, `indeterminate`, `done`).
    pub categories: Mutex<HashMap<String, String>>,
}

impl FakeJira {
    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    pub fn call_keys(&self, i: usize) -> Vec<String> {
        self.calls.lock().unwrap()[i].1.clone()
    }

    pub fn icon_call_count(&self) -> usize {
        self.icon_calls.lock().unwrap().len()
    }
}

impl JiraApi for FakeJira {
    fn bulk_fetch(&self, host: &str, keys: &[String]) -> Result<Vec<u8>, JiraError> {
        self.calls
            .lock()
            .unwrap()
            .push((host.to_string(), keys.to_vec()));
        if let Some(e) = *self.fail.lock().unwrap() {
            return Err(e);
        }
        let issues = self.issues.lock().unwrap();
        let types = self.types.lock().unwrap();
        let found: Vec<Value> = keys
            .iter()
            .filter_map(|k| {
                let (title, status) = issues.get(k)?;
                let mut fields = json!({"summary": title});
                if let Some(s) = status {
                    fields["status"] = json!({"name": s});
                    if let Some(c) = self.categories.lock().unwrap().get(k) {
                        fields["status"]["statusCategory"] = json!({"key": c});
                    }
                }
                if let Some((name, icon)) = types.get(k) {
                    fields["issuetype"] = json!({"name": name, "iconUrl": icon});
                }
                Some(json!({"key": k, "fields": fields}))
            })
            .collect();
        Ok(serde_json::to_vec(&json!({"issues": found})).unwrap())
    }

    fn myself(&self, _host: &str) -> Result<Vec<u8>, JiraError> {
        if let Some(e) = *self.fail.lock().unwrap() {
            return Err(e);
        }
        Ok(b"{}".to_vec())
    }

    fn icon(&self, url: &Url) -> Result<Vec<u8>, JiraError> {
        self.icon_calls.lock().unwrap().push(url.clone());
        if let Some(e) = *self.icon_fail.lock().unwrap() {
            return Err(e);
        }
        self.icon_data
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or(JiraError::Http(404))
    }
}

// ---- Store fixtures (Swift `TestSupport.swift`: `writeSession`, `makeStore`) ----

use claude_sessions_core::jira_monitor::JiraMonitor;
use claude_sessions_core::pr_monitor::PRMonitor;
use claude_sessions_core::store::{Paths, Store, StoreOptions};
use claude_sessions_core::time::unix_secs;

pub const BACKEND: &str = "/Users/phong/talo6-dct-backend";

/// Our own pid keeps fake sessions alive (`kill(pid, 0)`); 999_999 is a dead one.
pub fn me() -> i32 {
    rustix::process::getpid().as_raw_nonzero().get()
}

#[derive(Default)]
pub struct SessionOpts<'a> {
    pub waiting_for: Option<&'a str>,
    pub updated_at: Option<SystemTime>,
    pub name_source: Option<&'a str>,
    pub entrypoint: Option<&'a str>,
}

/// Writes `<dir>/<file>.json` like Claude Code's session registry; the status changed `age_sec` ago unless `updated_at` is set.
pub fn write_session(
    dir: &Path,
    file: &str,
    pid: i32,
    status: &str,
    age_sec: f64,
    name: &str,
    cwd: &str,
) {
    write_session_with(
        dir,
        file,
        pid,
        status,
        age_sec,
        name,
        cwd,
        SessionOpts::default(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn write_session_with(
    dir: &Path,
    file: &str,
    pid: i32,
    status: &str,
    age_sec: f64,
    name: &str,
    cwd: &str,
    opts: SessionOpts,
) {
    let updated = opts
        .updated_at
        .map_or(unix_secs(SystemTime::now()) - age_sec, unix_secs);
    let mut o = json!({"pid": pid, "sessionId": file, "name": name, "cwd": cwd, "status": status,
                       "statusUpdatedAt": updated * 1000.0});
    if let Some(w) = opts.waiting_for {
        o["waitingFor"] = json!(w);
    }
    if let Some(n) = opts.name_source {
        o["nameSource"] = json!(n);
    }
    if let Some(e) = opts.entrypoint {
        o["entrypoint"] = json!(e);
    }
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join(format!("{file}.json")),
        serde_json::to_vec(&o).unwrap(),
    )
    .unwrap();
}

#[derive(Default)]
pub struct StoreSetup {
    pub projects: Option<PathBuf>,
    pub tasks: Option<PathBuf>,
    pub start_hidden: bool,
    pub prs: Option<PRMonitor>,
    pub jira: Option<JiraMonitor>,
    pub branch: Option<claude_sessions_core::store::BranchFn>,
    pub feed: Option<claude_sessions_core::feed::FeedMonitor>,
    pub closed_store: Option<claude_sessions_core::closed_sessions::ClosedStore>,
    pub is_running: Option<Box<dyn Fn(i32) -> bool>>,
}

/// A store whose transcript lookups never touch the real `~/.claude/projects` and whose branch reader never touches a real repo.
/// Without explicit `projects` / `tasks` it uses `_projects` / `_tasks` under `dir` (only `*.json` files there count as sessions).
pub fn make_store(dir: &Path) -> Store {
    make_store_with(dir, StoreSetup::default())
}

pub fn make_store_with(dir: &Path, setup: StoreSetup) -> Store {
    let paths = Paths {
        sessions: dir.to_path_buf(),
        projects: setup.projects.unwrap_or_else(|| dir.join("_projects")),
        tasks: setup.tasks.unwrap_or_else(|| dir.join("_tasks")),
    };
    Store::new(
        paths,
        StoreOptions {
            start_hidden: setup.start_hidden,
            prs: setup.prs,
            jira: setup.jira,
            feed: setup.feed,
            closed_store: setup.closed_store,
            is_running: setup.is_running,
            scanner: claude_sessions_core::transcript_scanner::TranscriptScanner::new(false),
            branch: Some(setup.branch.unwrap_or_else(|| Box::new(|_| None))),
            ..StoreOptions::default()
        },
    )
}

pub fn test_pr() -> claude_sessions_core::pull_requests::PRRef {
    claude_sessions_core::pull_requests::PRRef {
        workspace: "w".into(),
        repo: "r".into(),
        id: 1,
    }
}

/// `write_session` for a session named like its id in `FRONTEND`, with an optional `waitingFor`.
pub fn put(dir: &Path, id: &str, status: &str, waiting_for: Option<&str>, age_sec: f64) {
    write_session_with(
        dir,
        id,
        me(),
        status,
        age_sec,
        id,
        FRONTEND,
        SessionOpts {
            waiting_for,
            ..SessionOpts::default()
        },
    );
}
