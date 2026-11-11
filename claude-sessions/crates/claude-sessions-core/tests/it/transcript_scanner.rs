use crate::support::*;
use claude_sessions_core::pull_requests::PRRef;
use claude_sessions_core::transcript_scanner::{ScanResult, ScanTarget, TranscriptScanner};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

fn size(url: &Path) -> u64 {
    fs::metadata(url).unwrap().len()
}

fn scan(scanner: &TranscriptScanner, url: &Path) -> ScanResult {
    scanner.update(
        vec![ScanTarget {
            session_id: "a".into(),
            transcript: Some(url.to_path_buf()),
            size: Some(size(url)),
        }],
        SystemTime::now(),
    );
    scanner.take_snapshot().remove("a").unwrap_or_default()
}

/// Keeps the temp dir alive next to the transcript path.
struct Transcript {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn transcript(lines: &[String]) -> Transcript {
    let dir = make_temp_dir();
    let mut all = lines.to_vec();
    all.push(String::new());
    let path = write_transcript(dir.path(), FRONTEND, "a", &all);
    Transcript { _dir: dir, path }
}

fn scanner() -> TranscriptScanner {
    TranscriptScanner::new(false)
}

fn keys(r: &ScanResult) -> Vec<String> {
    let mut k: Vec<String> = r.tickets.keys().cloned().collect();
    k.sort();
    k
}

fn pr_ref(repo: &str, id: i64) -> PRRef {
    PRRef {
        workspace: "t".into(),
        repo: repo.into(),
        id,
    }
}

fn link(repo: &str, id: i64) -> String {
    format!("https://bitbucket.org/t/{repo}/pull-requests/{id}")
}

fn user(text: &str) -> String {
    user_line(text, false, None)
}

fn human(text: &str) -> String {
    user_line(text, false, Some("human"))
}

fn assistant(text: &str) -> String {
    assistant_line(Some(text), false)
}

fn notification(result: &str) -> String {
    notification_line(result, None, true)
}

fn notification_for(result: &str, id: &str) -> String {
    notification_line(result, Some(id), true)
}

fn agent(id: &str, sidechain: bool) -> String {
    tool_use_line(id, "Agent", json!({}), sidechain)
}

#[test]
fn prefilter_skips_non_text_lines() {
    let t = transcript(&[
        tool_use_line("1", "Bash", json!({}), false),
        tool_result_line("1"),
        json_line(
            json!({"type": "assistant", "message": {"content": [{"type": "thinking", "thinking": "x"}]}}),
        ),
        assistant_line(Some("side"), true),
        user_line("meta", true, None),
        ai_title_line("t"),
        user("one"),
        assistant("two"),
    ]);
    let s = scanner();
    scan(&s, &t.path);
    assert_eq!(s.parsed_lines(), 2);
}

#[test]
fn incremental_reads_only_appended() {
    let t = transcript(&[user("T6IR-1")]);
    let s = scanner();
    assert_eq!(keys(&scan(&s, &t.path)), ["T6IR-1"]);
    append(&t.path, &(user("T6IR-2") + "\n"));
    let r = scan(&s, &t.path);
    assert_eq!(keys(&r), ["T6IR-1", "T6IR-2"]);
    assert_eq!(s.parsed_lines(), 2);
}

#[test]
fn partial_line_waits_for_newline() {
    let t = transcript(&[user("T6IR-1")]);
    append(&t.path, &user("T6IR-2"));
    let s = scanner();
    assert_eq!(keys(&scan(&s, &t.path)), ["T6IR-1"]);
    assert_eq!(s.parsed_lines(), 1);
    append(&t.path, "\n");
    assert_eq!(keys(&scan(&s, &t.path)), ["T6IR-1", "T6IR-2"]);
}

#[test]
fn oversized_line_skipped() {
    let big = user(&"x".repeat(5 << 20));
    let t = transcript(&[user("T6IR-1"), big, user("T6IR-2")]);
    assert_eq!(keys(&scan(&scanner(), &t.path)), ["T6IR-1", "T6IR-2"]);
}

#[test]
fn mentions_by_source() {
    let summary = json_line(
        json!({"type": "user", "isCompactSummary": true, "message": {"content": "summary T6IR-3"}}),
    );
    let t = transcript(&[user("T6IR-1"), assistant("T6IR-2 T6IR-1"), summary]);
    let r = scan(&scanner(), &t.path);
    let m = &r.tickets;
    assert!(m["T6IR-1"].by_user);
    assert!(!m["T6IR-2"].by_user && !m["T6IR-3"].by_user);
    assert!(m["T6IR-1"].first < m["T6IR-2"].first);
    assert!(m["T6IR-2"].first < m["T6IR-3"].first);
    assert!(m["T6IR-1"].last > m["T6IR-2"].first && m["T6IR-1"].last < m["T6IR-3"].first);
}

#[test]
fn learns_jira_hosts() {
    let t = transcript(&[
        user("https://talosix.atlassian.net/browse/T6IR-1"),
        assistant("https://talosix.atlassian.net/jira/boards/1?selectedIssue=OPS-2"),
    ]);
    let r = scan(&scanner(), &t.path);
    let mut hosts: Vec<_> = r
        .jira_hosts
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    hosts.sort();
    assert_eq!(
        hosts,
        [
            ("OPS", "talosix.atlassian.net"),
            ("T6IR", "talosix.atlassian.net")
        ]
    );
}

#[test]
fn jira_host_lowercased_and_atlassian_wins() {
    let t = transcript(&[
        user("https://Evil.Example/browse/T6IR-1"),
        user("https://TaloSix.Atlassian.net/browse/T6IR-2"),
        user("https://Other.Example/browse/OPS-1"),
    ]);
    let r = scan(&scanner(), &t.path);
    let mut hosts: Vec<_> = r
        .jira_hosts
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    hosts.sort();
    assert_eq!(
        hosts,
        [("OPS", "other.example"), ("T6IR", "talosix.atlassian.net")]
    );
}

#[test]
fn claude_tickets_only_in_turns_you_started() {
    let t = transcript(&[
        user("start"),
        assistant("T6IR-1"),
        notification("agent report T6IR-2"),
        assistant("master has commits of T6IR-3"),
        human("ok"),
        assistant("T6IR-4"),
        notification_line("T6IR-5", None, false),
        assistant("T6IR-6"),
    ]);
    assert_eq!(keys(&scan(&scanner(), &t.path)), ["T6IR-1", "T6IR-4"]);
}

#[test]
fn notification_turn_inherits_launcher() {
    let t = transcript(&[
        human("tạo ticket"),
        agent("toolu_A", false),
        agent("toolu_S", true),
        notification_for("report T6IR-2", "toolu_A"),
        assistant("Created [T6IR-14804](https://talosix.atlassian.net/browse/T6IR-14804)"),
        notification_for("x", "toolu_S"),
        assistant("T6IR-9"),
    ]);
    let r = scan(&scanner(), &t.path);
    assert_eq!(keys(&r), ["T6IR-14804"]);
    assert!(!r.tickets["T6IR-14804"].by_user);
}

#[test]
fn notification_turn_inherits_chain() {
    let t = transcript(&[
        human("go"),
        agent("toolu_A", false),
        notification_for("a", "toolu_A"),
        agent("toolu_B", false),
        notification_for("b", "toolu_B"),
        assistant("T6IR-1"),
        notification("c"),
        agent("toolu_C", false),
        notification_for("d", "toolu_C"),
        assistant("T6IR-2"),
        notification_line("e", Some("toolu_A"), false),
        assistant("T6IR-3"),
    ]);
    assert_eq!(keys(&scan(&scanner(), &t.path)), ["T6IR-1", "T6IR-3"]);
}

#[test]
fn notification_turn_inherits_across_scans() {
    let s = scanner();
    let t = transcript(&[human("go"), agent("toolu_A", false)]);
    scan(&s, &t.path);
    let summary = json_line(
        json!({"type": "user", "isCompactSummary": true, "message": {"content": "summary"}}),
    );
    append(&t.path, &(summary + "\n"));
    append(&t.path, &(notification_for("r", "toolu_A") + "\n"));
    append(&t.path, &(assistant("T6IR-5") + "\n"));
    assert!(scan(&s, &t.path).tickets.contains_key("T6IR-5"));

    let raw = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_R","name":"Agent","input":{}}]}}"#;
    append(&t.path, &(raw.to_string() + "\n"));
    append(&t.path, &(notification_for("r", "toolu_R") + "\n"));
    append(&t.path, &(assistant("T6IR-6") + "\n"));
    assert!(scan(&s, &t.path).tickets.contains_key("T6IR-6"));
}

#[test]
fn notification_turn_keeps_pr_links() {
    let t = transcript(&[
        user("start"),
        notification("done"),
        assistant(&format!("PR {} for T6IR-7", link("a", 9))),
    ]);
    let r = scan(&scanner(), &t.path);
    assert!(r.tickets.is_empty());
    assert_eq!(r.pr_refs, [pr_ref("a", 9)]);
}

#[test]
fn shell_output_and_reminders_ignored() {
    let t = transcript(&[
        user("<bash-stdout>T6IR-5</bash-stdout>"),
        assistant("ok <system-reminder>T6IR-6</system-reminder>"),
    ]);
    assert!(scan(&scanner(), &t.path).tickets.is_empty());
}

#[test]
fn pr_links_and_refs() {
    let lines = [
        user(&format!("plain {}", link("a", 1))),
        tool_result_text_line(&format!("listing {}", link("b", 2))),
        user_line(&format!("meta {}", link("c", 3)), true, None),
        assistant_line(Some(&format!("side {}", link("d", 4))), true),
        assistant(&format!(
            "main {} https://github.com/x/y/pull/1",
            link("e", 5)
        )),
        user(&format!("<bash-stdout>{}</bash-stdout>", link("f", 6))),
        user(&format!(
            "  <local-command-stdout>{}</local-command-stdout>",
            link("g", 7)
        )),
        user(&format!("real {}", link("h", 8))),
    ];
    let r = scan(&scanner(), &transcript(&lines).path);
    assert_eq!(r.pr_refs, [pr_ref("a", 1), pr_ref("e", 5), pr_ref("h", 8)]);
    let links: Vec<&str> = r.pr_links.iter().map(|u| u.as_str()).collect();
    assert_eq!(
        links,
        [
            link("a", 1),
            link("e", 5),
            "https://github.com/x/y/pull/1".to_string(),
            link("h", 8)
        ]
    );
}

#[test]
fn pr_links_deduped_by_identity() {
    let lines = [
        user(&format!(
            "{} then {}/diff and {}/",
            link("a", 12),
            link("a", 12),
            link("a", 12)
        )),
        user(&format!(
            "https://github.com/x/y/pull/1 https://github.com/x/y/pull/1/files {}",
            link("a", 1)
        )),
    ];
    let r = scan(&scanner(), &transcript(&lines).path);
    let links: Vec<&str> = r.pr_links.iter().map(|u| u.as_str()).collect();
    assert_eq!(
        links,
        [
            link("a", 12),
            "https://github.com/x/y/pull/1".to_string(),
            link("a", 1)
        ]
    );
}

#[test]
fn session_published_before_larger_one_finishes() {
    let dir = make_temp_dir();
    let small = write_transcript(
        dir.path(),
        FRONTEND,
        "small",
        &[user("T6IR-1"), String::new()],
    );
    let filler = user(&"word ".repeat(200));
    let mut big_lines = vec![filler; 40_000];
    big_lines.push(String::new());
    let big = write_transcript(dir.path(), FRONTEND, "big", &big_lines);
    let s = TranscriptScanner::new(true);
    let (published_tx, published_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    s.set_on_publish(Box::new(move |id| {
        if id == "small" {
            published_tx.send(()).unwrap();
            let _ = release_rx.recv();
        }
    }));
    s.update(
        vec![
            ScanTarget {
                session_id: "small".into(),
                transcript: Some(small.clone()),
                size: Some(size(&small)),
            },
            ScanTarget {
                session_id: "big".into(),
                transcript: Some(big.clone()),
                size: Some(size(&big)),
            },
        ],
        SystemTime::now(),
    );
    let got_published = published_rx.recv_timeout(Duration::from_secs(10));
    let early = s.take_snapshot();
    release_tx.send(()).unwrap();
    // Wait for the worker to finish the second session before the scanner drops.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !s.take_snapshot().contains_key("big") && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(got_published.is_ok());
    let mut small_keys: Vec<_> = early["small"].tickets.keys().cloned().collect();
    small_keys.sort();
    assert_eq!(small_keys, ["T6IR-1"]);
    assert!(!early.contains_key("big"));
}

#[test]
fn rewritten_file_rescanned() {
    let t = transcript(&[user(&format!("T6IR-1 {}", "x".repeat(200)))]);
    let s = scanner();
    assert_eq!(keys(&scan(&s, &t.path)), ["T6IR-1"]);
    fs::write(&t.path, user("T6IR-2") + "\n").unwrap();
    assert_eq!(keys(&scan(&s, &t.path)), ["T6IR-2"]);
}

#[test]
fn large_file_scan_time() {
    let dir = make_temp_dir();
    let path = dir.path().join("big.jsonl");
    let result = tool_result_text_line(&"r".repeat(2000)) + "\n";
    let mut data = Vec::new();
    while data.len() < 32 << 20 {
        data.extend_from_slice(result.as_bytes());
    }
    for i in 0..100 {
        data.extend_from_slice((user(&format!("T6IR-{}", i + 1)) + "\n").as_bytes());
    }
    fs::write(&path, &data).unwrap();
    let s = scanner();
    let start = Instant::now();
    let r = scan(&s, &path);
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(s.parsed_lines(), 100);
    assert_eq!(r.tickets.len(), 100);
}
