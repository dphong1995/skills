use crate::support::FakeJira;
use claude_sessions_core::credentials::ConnectionState;
use claude_sessions_core::jira::{JiraError, JiraIssue};
use claude_sessions_core::jira_monitor::{JiraMonitor, JiraTarget};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

const HOST: &str = "talosix.atlassian.net";

fn t0() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

fn at(dt: f64) -> SystemTime {
    t0() + Duration::from_secs_f64(dt)
}

fn targets(keys: &[&str]) -> Vec<JiraTarget> {
    targets_on(keys, HOST)
}

fn targets_on(keys: &[&str], host: &str) -> Vec<JiraTarget> {
    keys.iter()
        .map(|k| JiraTarget {
            key: k.to_string(),
            host: host.to_string(),
        })
        .collect()
}

fn issue(title: Option<&str>, status: Option<&str>) -> JiraIssue {
    JiraIssue {
        title: title.map(String::from),
        status: status.map(String::from),
        ..Default::default()
    }
}

fn set_issues(api: &FakeJira, list: &[(&str, &str, Option<&str>)]) {
    let mut m = api.issues.lock().unwrap();
    for (k, t, s) in list {
        m.insert(k.to_string(), (t.to_string(), s.map(String::from)));
    }
}

fn monitor(api: &Arc<FakeJira>) -> JiraMonitor {
    JiraMonitor::new(api.clone(), false)
}

#[test]
fn connection_starts_checking() {
    let m = monitor(&Arc::new(FakeJira::default()));
    assert_eq!(m.take_snapshot().connection, ConnectionState::Checking);
}

#[test]
fn new_keys_batched_once() {
    let api = Arc::new(FakeJira::default());
    set_issues(
        &api,
        &[
            ("A-1", "one", Some("Open")),
            ("A-2", "two", None),
            ("A-3", "three", Some("Done")),
        ],
    );
    let m = monitor(&api);
    m.update(&targets(&["A-1", "A-2", "A-3"]), t0());
    assert_eq!(api.call_count(), 1);
    let mut keys = api.call_keys(0);
    keys.sort();
    assert_eq!(keys, ["A-1", "A-2", "A-3"]);
    let s = m.take_snapshot();
    assert_eq!(s.issues["A-1"], issue(Some("one"), Some("Open")));
    assert_eq!(s.issues["A-2"], issue(Some("two"), None));
    assert_eq!(s.connection, ConnectionState::Ok);
}

#[test]
fn cached_keys_not_refetched_before_900s() {
    let api = Arc::new(FakeJira::default());
    let m = monitor(&api);
    let t = targets(&["A-1", "A-2"]);
    m.update(&t, t0());
    m.update(&t, at(10.0));
    m.update(&t, at(899.0));
    assert_eq!(api.call_count(), 1);
    m.update(&t, at(900.0));
    assert_eq!(api.call_count(), 2);
    let mut keys = api.call_keys(1);
    keys.sort();
    assert_eq!(keys, ["A-1", "A-2"]);
}

#[test]
fn new_key_waits_five_seconds() {
    let api = Arc::new(FakeJira::default());
    let m = monitor(&api);
    m.update(&targets(&["A-1"]), t0());
    m.update(&targets(&["A-1", "A-2"]), at(2.0));
    assert_eq!(api.call_count(), 1);
    m.update(&targets(&["A-1", "A-2"]), at(4.9));
    assert_eq!(api.call_count(), 1);
    m.update(&targets(&["A-1", "A-2"]), at(5.0));
    assert_eq!(api.call_count(), 2);
    assert_eq!(api.call_keys(1), ["A-2"]);
}

#[test]
fn splits_at_100() {
    let api = Arc::new(FakeJira::default());
    let keys: Vec<String> = (1..=150).map(|i| format!("A-{i}")).collect();
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let m = monitor(&api);
    m.update(&targets(&refs), t0());
    let sizes: Vec<usize> = api
        .calls
        .lock()
        .unwrap()
        .iter()
        .map(|c| c.1.len())
        .collect();
    assert_eq!(sizes, [100, 50]);
}

#[test]
fn one_call_per_host() {
    let api = Arc::new(FakeJira::default());
    let m = monitor(&api);
    let mut t = targets(&["A-1"]);
    t.extend(targets_on(&["B-1"], "other.atlassian.net"));
    m.update(&t, t0());
    assert_eq!(api.call_count(), 2);
    let hosts: std::collections::HashSet<String> = api
        .calls
        .lock()
        .unwrap()
        .iter()
        .map(|c| c.0.clone())
        .collect();
    assert_eq!(
        hosts,
        [HOST.to_string(), "other.atlassian.net".to_string()].into()
    );
}

#[test]
fn missing_key_has_nil_title() {
    let api = Arc::new(FakeJira::default());
    set_issues(&api, &[("A-1", "one", Some("Open"))]);
    let m = monitor(&api);
    m.update(&targets(&["A-1", "A-9"]), t0());
    let s = m.take_snapshot();
    assert_eq!(s.issues["A-9"], issue(None, None));
    assert_eq!(s.issues["A-1"].title.as_deref(), Some("one"));
}

#[test]
fn rejected_then_recovers() {
    let api = Arc::new(FakeJira::default());
    set_issues(&api, &[("A-1", "one", Some("Open"))]);
    *api.fail.lock().unwrap() = Some(JiraError::Http(401));
    let m = monitor(&api);
    m.update(&targets(&["A-1"]), t0());
    assert_eq!(m.take_snapshot().connection, ConnectionState::Rejected);
    m.update(&targets(&["A-1"]), at(59.0));
    assert_eq!(api.call_count(), 1);
    *api.fail.lock().unwrap() = None;
    m.update(&targets(&["A-1"]), at(60.0));
    assert_eq!(api.call_count(), 2);
    let s = m.take_snapshot();
    assert_eq!(s.connection, ConnectionState::Ok);
    assert_eq!(s.issues["A-1"].title.as_deref(), Some("one"));
}

#[test]
fn refresh_failure_retries_after_60s() {
    let api = Arc::new(FakeJira::default());
    set_issues(&api, &[("A-1", "one", Some("Open"))]);
    let m = monitor(&api);
    m.update(&targets(&["A-1"]), t0());
    assert_eq!(m.take_snapshot().connection, ConnectionState::Ok);
    *api.fail.lock().unwrap() = Some(JiraError::Http(401));
    m.update(&targets(&["A-1"]), at(900.0));
    assert_eq!(m.take_snapshot().connection, ConnectionState::Rejected);
    assert_eq!(api.call_count(), 2);
    *api.fail.lock().unwrap() = None;
    m.update(&targets(&["A-1"]), at(930.0));
    assert_eq!(api.call_count(), 2);
    m.update(&targets(&["A-1"]), at(960.0));
    assert_eq!(api.call_count(), 3);
    assert_eq!(m.take_snapshot().connection, ConnectionState::Ok);
}

#[test]
fn dropped_after_session_gone() {
    let api = Arc::new(FakeJira::default());
    set_issues(
        &api,
        &[("A-1", "one", Some("Open")), ("A-2", "two", Some("Open"))],
    );
    let m = monitor(&api);
    m.update(&targets(&["A-1", "A-2"]), t0());
    m.update(&targets(&["A-1"]), at(10.0));
    assert!(m.take_snapshot().issues.contains_key("A-2"));
    m.update(&targets(&["A-1"]), at(41.0));
    assert!(!m.take_snapshot().issues.contains_key("A-2"));
}

fn icon_a() -> Url {
    Url::parse("https://talosix.atlassian.net/avatar/1?size=medium").unwrap()
}

fn icon_api(url: &Url) -> Arc<FakeJira> {
    let api = Arc::new(FakeJira::default());
    set_issues(
        &api,
        &[("A-1", "one", Some("Open")), ("A-2", "two", Some("Open"))],
    );
    for k in ["A-1", "A-2"] {
        api.types
            .lock()
            .unwrap()
            .insert(k.into(), ("Story".into(), url.to_string()));
    }
    api.icon_data
        .lock()
        .unwrap()
        .insert(url.clone(), vec![1, 2, 3]);
    api
}

#[test]
fn icon_fetched_once_per_url() {
    let api = icon_api(&icon_a());
    let m = monitor(&api);
    for dt in [0.0, 10.0, 20.0] {
        m.update(&targets(&["A-1", "A-2"]), at(dt));
    }
    assert_eq!(api.icon_call_count(), 1);
    assert_eq!(m.take_snapshot().icons[&icon_a()], [1, 2, 3]);
}

#[test]
fn icon_retries_after_60s_on_tick() {
    let api = icon_api(&icon_a());
    *api.icon_fail.lock().unwrap() = Some(JiraError::Transport);
    let m = monitor(&api);
    m.update(&targets(&["A-1"]), t0());
    *api.icon_fail.lock().unwrap() = None;
    m.update(&targets(&["A-1"]), at(30.0));
    assert_eq!(api.icon_call_count(), 1);
    m.update(&targets(&["A-1"]), at(60.0));
    assert_eq!(api.icon_call_count(), 2);
    assert!(m.take_snapshot().icons.contains_key(&icon_a()));
    assert_eq!(api.call_count(), 1);
}

#[test]
fn icons_skipped_while_rejected() {
    let api = icon_api(&icon_a());
    *api.icon_fail.lock().unwrap() = Some(JiraError::Transport);
    let m = monitor(&api);
    m.update(&targets(&["A-1"]), t0());
    assert_eq!(api.icon_call_count(), 1);
    *api.icon_fail.lock().unwrap() = None;
    *api.fail.lock().unwrap() = Some(JiraError::Http(401));
    for dt in [900.0, 960.0, 1020.0] {
        m.update(&targets(&["A-1"]), at(dt));
    }
    assert_eq!(m.take_snapshot().connection, ConnectionState::Rejected);
    assert_eq!(api.icon_call_count(), 1);
}

#[test]
fn icon_skips_foreign_host() {
    let foreign = Url::parse("https://cdn.example/x.png").unwrap();
    let api = icon_api(&foreign);
    let m = monitor(&api);
    m.update(&targets(&["A-1"]), t0());
    m.update(&targets(&["A-1"]), at(120.0));
    assert_eq!(api.icon_call_count(), 0);
    assert!(m.take_snapshot().icons.is_empty());
}

#[test]
fn icon_skips_non_https() {
    let plain = Url::parse("http://talosix.atlassian.net/avatar/1").unwrap();
    let api = icon_api(&plain);
    let m = monitor(&api);
    m.update(&targets(&["A-1"]), t0());
    m.update(&targets(&["A-1"]), at(120.0));
    assert_eq!(api.icon_call_count(), 0);
    assert!(m.take_snapshot().icons.is_empty());
}

#[test]
fn icons_survive_forget() {
    let api = icon_api(&icon_a());
    let m = monitor(&api);
    m.update(&targets(&["A-1"]), t0());
    m.update(&[], at(1.0));
    m.update(&[], at(40.0));
    assert!(m.take_snapshot().issues.is_empty());
    assert!(m.take_snapshot().icons.contains_key(&icon_a()));
}

#[test]
fn threaded_monitor_hands_results_back() {
    let api = Arc::new(FakeJira::default());
    set_issues(&api, &[("A-1", "one", Some("Open"))]);
    let m = JiraMonitor::new(api.clone(), true);
    m.update(&targets(&["A-1"]), t0());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while m.take_snapshot().issues.is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        m.take_snapshot().issues["A-1"].title.as_deref(),
        Some("one")
    );
}
