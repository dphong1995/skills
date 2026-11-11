//! Swift `StoreTests.swift` send-feedback tests (`beginSendBlocksSameSession` through `sendStateOfSessionTakesNewestKey`).

use super::*;
use claude_sessions_core::pull_requests::PRRef;
use claude_sessions_core::store::{SendKey, SendKind, SendState};

fn key(session: &str, kind: SendKind) -> SendKey {
    SendKey::new(session, kind)
}

#[test]
fn begin_send_blocks_same_session() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t = SystemTime::now();
    assert!(store.begin_send(key("s1", SendKind::Reply), t));
    assert!(!store.begin_send(key("s1", SendKind::Stop), t));
    assert!(store.begin_send(key("s2", SendKind::Stop), t));
}

#[test]
fn finish_send_shows_three_seconds() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    store.refresh(t0);
    let k = key("s1", SendKind::Approve);
    assert!(store.begin_send(k.clone(), t0));
    assert_eq!(store.send_state(&k), Some(SendState::Sending));
    store.finish_send(&k, true, t0);
    store.refresh(secs(t0, 2));
    assert_eq!(store.send_state(&k), Some(SendState::Sent));
    store.refresh(secs(t0, 3));
    assert_eq!(store.send_state(&k), None);
    let f = key("s1", SendKind::Deny);
    store.finish_send(&f, false, t0);
    store.refresh(secs(t0, 1));
    assert_eq!(store.send_state(&f), Some(SendState::Failed));
    // Finished sends no longer block the session.
    assert!(store.begin_send(key("s1", SendKind::Reply), secs(t0, 1)));
}

#[test]
fn send_times_out_to_failed() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    store.refresh(t0);
    let k = key("s1", SendKind::Reply);
    assert!(store.begin_send(k.clone(), t0));
    store.refresh(secs(t0, 14));
    assert_eq!(store.send_state(&k), Some(SendState::Sending));
    store.refresh(secs(t0, 15));
    assert_eq!(store.send_state(&k), Some(SendState::Failed));
    assert!(store.begin_send(key("s1", SendKind::Stop), secs(t0, 15)));
}

#[test]
fn send_key_distinguishes_pr() {
    let pr = |id| PRRef {
        workspace: "w".into(),
        repo: "r".into(),
        id,
    };
    let with = |id| SendKey {
        pr: Some(pr(id)),
        ..key("s", SendKind::AskClaude)
    };
    assert_ne!(with(1), with(2));
}

#[test]
fn approve_send_is_per_prompt() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    store.refresh(t0);
    let approve = |tool: &str| {
        Action::Approve {
            session: "s1".into(),
            tool_id: tool.into(),
        }
        .send_key()
        .unwrap()
    };
    let (a, b) = (approve("tA"), approve("tB"));
    assert_ne!(a, b);
    store.finish_send(&a, true, t0);
    store.refresh(secs(t0, 1));
    assert_eq!(store.send_state(&a), Some(SendState::Sent));
    // A new prompt (tool B) shows no send state on its `Cho phép`; the session-level line still shows tool A's result.
    assert_eq!(store.send_state(&b), None);
    let deny_b = Action::Deny {
        session: "s1".into(),
        tool_id: "tB".into(),
        reason: String::new(),
    }
    .send_key()
    .unwrap();
    assert_eq!(store.send_state(&deny_b), None);
    assert_eq!(
        store.send_state_of("s1", &[SendKind::Approve, SendKind::Deny]),
        Some(SendState::Sent)
    );
}

#[test]
fn send_state_of_session_takes_newest_key() {
    let dir = make_temp_dir();
    let mut store = make_store(dir.path());
    let t0 = SystemTime::now();
    store.refresh(t0);
    store.finish_send(&key("s1", SendKind::Reply), true, t0);
    store.finish_send(&key("s1", SendKind::Stop), false, secs(t0, 1));
    store.finish_send(&key("s2", SendKind::Stop), true, secs(t0, 1));
    store.refresh(secs(t0, 1));
    assert_eq!(
        store.send_state_of("s1", &[SendKind::Reply, SendKind::Stop]),
        Some(SendState::Failed)
    );
    assert_eq!(
        store.send_state_of("s1", &[SendKind::Reply]),
        Some(SendState::Sent)
    );
    assert_eq!(store.send_state_of("s1", &[SendKind::Approve]), None);
}
