use crate::support::*;
use claude_sessions_core::feed::{
    self, BlockState, FeedItem, FeedMonitor, FeedOption, FeedTransport, UnixFeedTransport,
};
use claude_sessions_core::shell::parse_iso_date;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// A `feed.list` response captured from a live cmux (haiku session asking `Pick a color`), trimmed to what differs per run.
const CAPTURED_LIST: &str = r#"{"id":"37705c5d","ok":true,"result":{"items":[{"kind":"question","id":"6103AC13","question_multi_select":false,"question_options":[{"id":"opt0","description":"Chọn màu đỏ","label":"Red"}],"cwd":"/tmp/spike-a","question_prompt":"Pick a color","title":"AskUserQuestion","questions":[{"multi_select":false,"id":"q0","header":"Color","options":[{"description":"Chọn màu đỏ","label":"Red","id":"opt0"},{"id":"opt1","description":"Chọn màu xanh dương","label":"Blue"},{"description":"Chọn màu xanh lá","id":"opt2","label":"Green"}],"prompt":"Pick a color"}],"source":"claude","updated_at":"2026-11-06T12:02:56Z","created_at":"2026-11-06T12:02:56Z","workstream_id":"cmux-feed-v1:Y2xhdWRl:ODE0ODFlNDUtMGRkNS00MWM1LTlkODYtMGUyZjYwNmZkNGFk","request_id":"E329BEF6-5CC6-4EF4-BBE2-002B8D2460DD","status":"pending"},{"kind":"permissionRequest","request_id":"P1","status":"pending","workstream_id":"cmux-feed-v1:Y2xhdWRl:eA=="},{"kind":"question","request_id":"OLD","status":"resolved","workstream_id":"cmux-feed-v1:Y2xhdWRl:eA==","questions":[]}]}}"#;

fn after(t: SystemTime, secs: u64) -> SystemTime {
    t + Duration::from_secs(secs)
}

fn picks(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect()
}

fn other(pairs: &[(usize, &str)]) -> std::collections::HashMap<usize, String> {
    pairs.iter().map(|(i, s)| (*i, s.to_string())).collect()
}

fn selections(p: &Value) -> Vec<String> {
    p["selections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn item(prompt: &str, labels: &[&str]) -> FeedItem {
    FeedItem {
        prompt: prompt.into(),
        header: None,
        multi_select: false,
        options: labels
            .iter()
            .map(|l| FeedOption {
                id: l.to_string(),
                label: l.to_string(),
                description: None,
            })
            .collect(),
    }
}

/// A reply outcome a completion callback fills in.
fn outcome() -> (Arc<Mutex<Option<bool>>>, impl FnOnce(bool) + Send + 'static) {
    let cell = Arc::new(Mutex::new(None));
    let c = cell.clone();
    (cell, move |ok| *c.lock().unwrap() = Some(ok))
}

#[test]
fn parse_real_item() {
    let list = feed::parse_list(CAPTURED_LIST.as_bytes());
    assert_eq!(list.len(), 1);
    let q = &list[0];
    assert_eq!(q.request_id, "E329BEF6-5CC6-4EF4-BBE2-002B8D2460DD");
    assert_eq!(
        q.session_id.as_deref(),
        Some("81481e45-0dd5-41c5-9d86-0e2f606fd4ad")
    );
    assert_eq!(Some(q.created_at), parse_iso_date("2026-11-06T12:02:56Z"));
    assert_eq!(q.questions.len(), 1);
    let q0 = &q.questions[0];
    assert_eq!(q0.prompt, "Pick a color");
    assert_eq!(q0.header.as_deref(), Some("Color"));
    assert!(!q0.multi_select);
    let labels: Vec<&str> = q0.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(labels, ["Red", "Blue", "Green"]);
    assert_eq!(q0.options[1].id, "opt1");
    assert_eq!(
        q0.options[1].description.as_deref(),
        Some("Chọn màu xanh dương")
    );
}

#[test]
fn parse_skips_other_sources_and_missing_date() {
    let now = SystemTime::now();
    let mut other = feed_item("X1", "a", now);
    other["source"] = json!("codex");
    let mut unsourced = feed_item("X3", "a", now);
    unsourced.as_object_mut().unwrap().remove("source");
    let mut undated = feed_item("X2", "a", now);
    undated.as_object_mut().unwrap().remove("created_at");
    let data = serde_json::to_vec(&json!({"ok": true, "result": {"items": [other, unsourced, undated, feed_item("OK", "a", now)]}})).unwrap();
    let ids: Vec<String> = feed::parse_list(&data)
        .into_iter()
        .map(|q| q.request_id)
        .collect();
    assert_eq!(ids, ["OK"]);
}

#[test]
fn parse_garbage_is_empty() {
    assert!(feed::parse_list(b"not json").is_empty());
    assert!(feed::parse_list(br#"{"ok":false,"error":{"code":"x"}}"#).is_empty());
    assert!(feed::parse_list(br#"{"ok":true,"result":{}}"#).is_empty());
}

#[test]
fn reply_params_shapes() {
    let none = other(&[]);
    // (a) one question, single select
    let p = feed::reply_params("R", None, &picks(&[&["Blue"]]), &none);
    assert_eq!(p["request_id"], "R");
    assert_eq!(selections(&p), ["Blue"]);
    // (b) multi-select: the labels joined by ", " (one string per question)
    let p = feed::reply_params("R", None, &picks(&[&["Red", "Blue"]]), &none);
    assert_eq!(selections(&p), ["Red, Blue"]);
    // (c) several questions: one string each, in order
    let p = feed::reply_params("R", None, &picks(&[&["Red"], &["Large"]]), &none);
    assert_eq!(selections(&p), ["Red", "Large"]);
    // (d) Other free text, trimmed
    let p = feed::reply_params("R", None, &picks(&[&[]]), &other(&[(0, "  Purple ")]));
    assert_eq!(selections(&p), ["Purple"]);
    // Other after the picks; an unanswered question sends "" (callers only send complete answers)
    let p = feed::reply_params("R", None, &picks(&[&["Red"], &[]]), &other(&[(0, "x")]));
    assert_eq!(selections(&p), ["Red, x", ""]);
    // With the question known, the slot count is its question count.
    let mut q = feed_question("R1", SystemTime::now());
    q.questions.extend(q.questions.clone());
    let p = feed::reply_params("R", Some(&q), &picks(&[&["Red"]]), &none);
    assert_eq!(selections(&p), ["Red", ""]);
    // An unanswered question in the middle keeps the later answers on their own prompts.
    let p = feed::reply_params("R", None, &picks(&[&[], &["Large"]]), &none);
    assert_eq!(selections(&p), ["", "Large"]);
}

#[test]
fn supports_and_complete() {
    let now = SystemTime::now();
    let with = |questions: Vec<FeedItem>| {
        let mut q = feed_question("R1", now);
        q.questions = questions;
        q
    };
    assert!(feed::supports(&feed_question("R1", now)));
    assert!(!feed::supports(&with(vec![])));
    assert!(!feed::supports(&with(vec![item("P", &[])])));
    let two = with(vec![item("A", &["x"]), item("B", &["y"])]);
    assert!(!feed::complete(&two, &picks(&[&["x"], &[]]), &other(&[])));
    assert!(!feed::complete(
        &two,
        &picks(&[&["x"], &[]]),
        &other(&[(1, "   ")])
    ));
    assert!(feed::complete(
        &two,
        &picks(&[&["x"], &[]]),
        &other(&[(1, "z")])
    ));
    assert!(feed::complete(&two, &picks(&[&["x"], &["y"]]), &other(&[])));
    assert!(!feed::complete(&two, &[], &other(&[])));
}

#[test]
fn window_and_countdown() {
    let t0 = SystemTime::now();
    let q = feed_question("R1", t0);
    assert_eq!(feed::WINDOW, 120.0);
    assert_eq!(feed::countdown(&q, t0), "còn 2:00");
    assert_eq!(feed::countdown(&q, after(t0, 61)), "còn 0:59");
    assert_eq!(feed::countdown(&q, after(t0, 200)), "còn 0:00");
    let state = |live, sent, failed, now| feed::block_state(&q, live, sent, failed, now);
    assert_eq!(state(true, false, false, after(t0, 119)), BlockState::Form);
    assert_eq!(
        state(true, false, false, after(t0, 120)),
        BlockState::Fallback
    );
    assert_eq!(state(false, false, false, t0), BlockState::Fallback);
    assert_eq!(state(true, false, true, t0), BlockState::Fallback);
    assert_eq!(state(true, true, false, t0), BlockState::Sent);
    assert_eq!(state(false, true, false, after(t0, 500)), BlockState::Sent);
    let mut odd = feed_question("R1", t0);
    odd.questions.clear();
    assert_eq!(
        feed::block_state(&odd, true, false, false, t0),
        BlockState::Fallback
    );
}

#[test]
fn monitor_polls_only_when_active() {
    let t = FakeFeedTransport::new();
    t.items
        .lock()
        .unwrap()
        .push(feed_item("R1", "a", SystemTime::now()));
    let m = FeedMonitor::without_worker(t.clone());
    let now = SystemTime::now();
    m.update(false, now);
    assert!(t.methods().is_empty());
    assert!(m.take_snapshot().is_empty());
    m.update(true, now);
    assert_eq!(t.methods(), ["feed.list"]);
    assert_eq!(t.calls.lock().unwrap()[0].1["pending_only"], true);
    let ids: Vec<String> = m
        .take_snapshot()
        .into_iter()
        .map(|q| q.request_id)
        .collect();
    assert_eq!(ids, ["R1"]);
    // Going inactive empties the snapshot and stops polling.
    m.update(false, after(now, 1));
    assert_eq!(t.methods().len(), 1);
    assert!(m.take_snapshot().is_empty());
}

#[test]
fn transport_error_disables() {
    let t = FakeFeedTransport::new();
    *t.fail_all.lock().unwrap() = true;
    let m = FeedMonitor::without_worker(t.clone());
    let now = SystemTime::now();
    m.update(true, now);
    assert!(m.take_snapshot().is_empty());
    assert_eq!(t.methods().len(), 1);
    // Backs off instead of hammering a broken socket, then retries.
    m.update(true, after(now, 1));
    assert_eq!(t.methods().len(), 1);
    *t.fail_all.lock().unwrap() = false;
    t.items
        .lock()
        .unwrap()
        .push(feed_item("R1", "a", SystemTime::now()));
    m.update(true, now + Duration::from_secs_f64(feed::RETRY_AFTER + 1.0));
    assert_eq!(m.take_snapshot().len(), 1);
}

#[test]
fn reply_only_for_listed_request_id() {
    let t = FakeFeedTransport::new();
    t.items
        .lock()
        .unwrap()
        .push(feed_item("R1", "a", SystemTime::now()));
    let m = FeedMonitor::without_worker(t.clone());
    m.update(true, SystemTime::now());
    let red = picks(&[&["Red"]]);
    let (ok, done) = outcome();
    m.reply(
        &feed_question("R-unknown", SystemTime::now()),
        &red,
        &other(&[]),
        done,
    );
    assert_eq!(*ok.lock().unwrap(), Some(false));
    assert_eq!(t.methods(), ["feed.list"]);
    let (ok, done) = outcome();
    m.reply(
        &feed_question("R1", SystemTime::now()),
        &red,
        &other(&[]),
        done,
    );
    assert_eq!(*ok.lock().unwrap(), Some(true));
    let last = t.calls.lock().unwrap().last().cloned().unwrap();
    assert_eq!(last.0, "feed.question.reply");
    assert_eq!(last.1["request_id"], "R1");
    assert_eq!(selections(&last.1), ["Red"]);
    // A delivered answer leaves the snapshot at once, so the next card build cannot show the form again.
    assert!(m.take_snapshot().is_empty());
    *t.reply_result.lock().unwrap() =
        r#"{"ok":false,"error":{"code":"not_found","message":"x"}}"#.into();
    m.update(true, after(SystemTime::now(), 1));
    let (ok, done) = outcome();
    m.reply(
        &feed_question("R1", SystemTime::now()),
        &red,
        &other(&[]),
        done,
    );
    assert_eq!(*ok.lock().unwrap(), Some(false));
    m.update(true, after(SystemTime::now(), 2));
    *t.fail_all.lock().unwrap() = true;
    let (ok, done) = outcome();
    m.reply(
        &feed_question("R1", SystemTime::now()),
        &red,
        &other(&[]),
        done,
    );
    assert_eq!(*ok.lock().unwrap(), Some(false));
}

#[test]
fn socket_round_trip() {
    let dir = make_socket_dir();
    let sock = dir.path().join("s.sock");
    let path_file = dir.path().join("last-socket-path");
    std::fs::write(&path_file, format!("{}\nignored\n", sock.display())).unwrap();
    let server = TestUnixServer::start(&sock, |req| {
        if req.windows(7).any(|w| w == b"silent!") {
            Reply::Silent
        } else {
            Reply::Bytes(b"{\"ok\":true,\"result\":{\"items\":[]}}\n".to_vec())
        }
    });
    let transport = UnixFeedTransport::new(&path_file).expect("no transport");
    let data = transport
        .request(
            "feed.list",
            &json!({"pending_only": true}),
            Duration::from_secs(2),
        )
        .unwrap();
    assert!(String::from_utf8_lossy(&data).contains("\"items\""));
    let received = String::from_utf8(server.last_request()).unwrap();
    assert!(received.ends_with('\n'));
    let o: Value = serde_json::from_str(&received).unwrap();
    assert_eq!(o["method"], "feed.list");
    assert_eq!(o["params"]["pending_only"], true);
    assert!(o["id"].is_string());
    // Silent server: times out.
    let t0 = SystemTime::now();
    assert!(
        transport
            .request(
                "feed.list",
                &json!({"x": "silent!"}),
                Duration::from_millis(400)
            )
            .is_err()
    );
    assert!(t0.elapsed().unwrap() < Duration::from_secs(2));
}

#[test]
fn socket_transport_needs_path_file() {
    assert!(
        UnixFeedTransport::new(std::path::Path::new("/nonexistent/last-socket-path")).is_none()
    );
}

#[test]
fn option_number_continuous() {
    let n = |count: usize| {
        let labels: Vec<String> = (0..count).map(|i| format!("L{i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        item("p", &refs)
    };
    let with = |questions: Vec<FeedItem>| {
        let mut q = feed_question("R1", SystemTime::now());
        q.questions = questions;
        q
    };
    let three = with(vec![n(3), n(3)]);
    assert_eq!(feed::option_number(&three, 0, 0), Some(1));
    assert_eq!(feed::option_number(&three, 0, 2), Some(3));
    assert_eq!(feed::option_number(&three, 1, 0), Some(4));
    assert_eq!(feed::option_number(&three, 1, 2), Some(6));
    let five = with(vec![n(5), n(5)]);
    assert_eq!(feed::option_number(&five, 1, 4), None);
}

#[test]
fn answer_with_newline_sends_one_line() {
    let dir = make_socket_dir();
    let sock = dir.path().join("s.sock");
    let path_file = dir.path().join("last-socket-path");
    std::fs::write(&path_file, format!("{}\n", sock.display())).unwrap();
    let list = serde_json::to_vec(
        &json!({"ok": true, "result": {"items": [feed_item("R1", "a", SystemTime::now())]}}),
    )
    .unwrap();
    let server = TestUnixServer::start(&sock, move |req| {
        let line: Value = serde_json::from_slice(req).unwrap_or(Value::Null);
        let mut answer = if line["method"] == "feed.list" {
            list.clone()
        } else {
            br#"{"ok":true,"result":{"delivered":true}}"#.to_vec()
        };
        answer.push(b'\n');
        Reply::Bytes(answer)
    });
    let transport = Arc::new(UnixFeedTransport::new(&path_file).unwrap());
    let m = FeedMonitor::without_worker(transport);
    m.update(true, SystemTime::now());
    let evil = "x\n{\"method\":\"x\"}";
    let (ok, done) = outcome();
    m.reply(
        &feed_question("R1", SystemTime::now()),
        &picks(&[&[]]),
        &other(&[(0, evil)]),
        done,
    );
    assert_eq!(*ok.lock().unwrap(), Some(true));
    let sent = server.last_request();
    assert_eq!(sent.iter().filter(|b| **b == b'\n').count(), 1);
    assert_eq!(sent.last(), Some(&b'\n'));
    let o: Value = serde_json::from_slice(&sent).unwrap();
    assert_eq!(o["method"], "feed.question.reply");
    assert_eq!(selections(&o["params"]), [evil.trim()]);
    // The encoder alone, for any params.
    let bytes = feed::encode_request("feed.question.reply", &json!({"selections": [evil]}));
    assert_eq!(bytes.iter().filter(|b| **b == b'\n').count(), 1);
    assert!(serde_json::from_slice::<Value>(&bytes).is_ok());
}

#[test]
fn oversized_reply_is_rejected() {
    let dir = make_socket_dir();
    let sock = dir.path().join("s.sock");
    let path_file = dir.path().join("last-socket-path");
    std::fs::write(&path_file, format!("{}\n", sock.display())).unwrap();
    let _server = TestUnixServer::start(&sock, |_| {
        let mut big = vec![b'a'; 4_000_001];
        big.push(b'\n');
        Reply::Bytes(big)
    });
    let transport = UnixFeedTransport::new(&path_file).unwrap();
    let r = transport.request("feed.list", &json!({}), Duration::from_secs(5));
    assert!(r.is_err());
    // A reply just under the cap is fine.
    let dir2 = make_socket_dir();
    let sock2 = dir2.path().join("s.sock");
    let path_file2 = dir2.path().join("last-socket-path");
    std::fs::write(&path_file2, format!("{}\n", sock2.display())).unwrap();
    let _server2 = TestUnixServer::start(&sock2, |_| {
        let mut ok = vec![b'a'; 3_000_000];
        ok.push(b'\n');
        Reply::Bytes(ok)
    });
    let ok = UnixFeedTransport::new(&path_file2).unwrap().request(
        "feed.list",
        &json!({}),
        Duration::from_secs(5),
    );
    assert_eq!(ok.map(|d| d.len()), Ok(3_000_000));
}

#[test]
fn refuses_a_socket_path_that_is_not_a_socket() {
    let dir = make_socket_dir();
    let not_socket = dir.path().join("plain-file");
    std::fs::write(&not_socket, "x").unwrap();
    let path_file = dir.path().join("last-socket-path");
    std::fs::write(&path_file, format!("{}\n", not_socket.display())).unwrap();
    let transport = UnixFeedTransport::new(&path_file).unwrap();
    assert!(
        transport
            .request("feed.list", &json!({}), Duration::from_secs(1))
            .is_err()
    );
    // A symlink to a live socket is refused too (`lstat`, not `stat`).
    let sock = dir.path().join("s.sock");
    let _server = TestUnixServer::start(&sock, |_| Reply::Bytes(b"{\"ok\":true}\n".to_vec()));
    let link = dir.path().join("link.sock");
    std::os::unix::fs::symlink(&sock, &link).unwrap();
    std::fs::write(&path_file, format!("{}\n", link.display())).unwrap();
    assert!(
        transport
            .request("feed.list", &json!({}), Duration::from_secs(1))
            .is_err()
    );
    std::fs::write(&path_file, format!("{}\n", sock.display())).unwrap();
    assert!(
        transport
            .request("feed.list", &json!({}), Duration::from_secs(1))
            .is_ok()
    );
}

#[test]
fn vanished_item_falls_back() {
    let dir = make_temp_dir();
    let t = FakeFeedTransport::new();
    *t.items.lock().unwrap() = vec![feed_item("R1", "a", SystemTime::now())];
    let mut store = make_store_with(
        dir.path(),
        StoreSetup {
            feed: Some(FeedMonitor::without_worker(t.clone())),
            ..Default::default()
        },
    );
    write_session_with(
        dir.path(),
        "a",
        me(),
        "waiting",
        0.0,
        "a",
        FRONTEND,
        SessionOpts {
            waiting_for: Some("input needed"),
            ..SessionOpts::default()
        },
    );
    let t0 = SystemTime::now();
    store.refresh(t0);
    let s = store.sessions[0].clone();
    assert!(s.question.as_ref().map(|q| q.request_id.as_str()) == Some("R1") && s.question_live);
    assert_eq!(
        feed::block_state(
            s.question.as_ref().unwrap(),
            s.question_live,
            false,
            false,
            t0
        ),
        BlockState::Form
    );
    // The item leaves feed.list (answered in the terminal, or the 120 s hook gave up) while the session still waits.
    t.items.lock().unwrap().clear();
    let t1 = after(t0, 1);
    store.refresh(t1);
    let s = store.sessions[0].clone();
    assert!(s.question.as_ref().map(|q| q.request_id.as_str()) == Some("R1") && !s.question_live);
    assert_eq!(
        feed::block_state(
            s.question.as_ref().unwrap(),
            s.question_live,
            false,
            false,
            t1
        ),
        BlockState::Fallback
    );
    // The session runs again: the block is gone.
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(after(t0, 2));
    assert!(store.sessions[0].question.is_none());
}

/// The first request panics; later ones return a captured list.
struct PanicOnce(std::sync::atomic::AtomicBool);

impl FeedTransport for PanicOnce {
    fn request(
        &self,
        _method: &str,
        _params: &Value,
        _timeout: Duration,
    ) -> Result<Vec<u8>, claude_sessions_core::feed::TransportError> {
        if !self.0.swap(true, std::sync::atomic::Ordering::SeqCst) {
            panic!("injected panic in a worker job");
        }
        Ok(CAPTURED_LIST.as_bytes().to_vec())
    }
}

#[test]
fn panicking_poll_does_not_stop_the_worker() {
    let m = FeedMonitor::new(Arc::new(PanicOnce(Default::default())));
    let now = SystemTime::now();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while m.take_snapshot().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "the worker stopped after a panicking job"
        );
        m.update(true, now);
        std::thread::sleep(Duration::from_millis(20));
    }
}
