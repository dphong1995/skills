//! The pure parts of the token check and save. No dialog is shown and no Keychain is touched: the writer is injected.

use std::cell::Cell;

use claude_sessions::connect;
use claude_sessions_core::bitbucket::BitbucketError;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::jira::JiraError;

#[test]
fn an_unsafe_token_or_email_is_rejected_before_anything_is_stored() {
    for bad in ["a\"b", "a\\b", "a\nb", "a\rb", "a\u{2028}b"] {
        assert_eq!(
            connect::precheck("me@example.com", bad),
            Err("✗ Email hoặc token có ký tự không hợp lệ".into()),
            "{bad:?}"
        );
        assert!(connect::precheck(bad, "token").is_err());
    }
}

#[test]
fn empty_fields_and_trimming() {
    assert_eq!(
        connect::precheck("  ", "t"),
        Err("✗ Nhập cả email và token".into())
    );
    assert_eq!(
        connect::precheck("a@b.c", " \n "),
        Err("✗ Nhập cả email và token".into())
    );
    assert_eq!(
        connect::precheck(" a@b.c ", " tok\n"),
        Ok(("a@b.c".into(), "tok".into()))
    );
}

#[test]
fn a_failed_check_never_reaches_the_writer() {
    let called = Cell::new(false);
    let failure = connect::store_credentials(
        Service::Bitbucket,
        "a@b.c",
        "tok",
        || Some("✗ nope".into()),
        |_, _, _| {
            called.set(true);
            true
        },
    );
    assert_eq!(failure.as_deref(), Some("✗ nope"));
    assert!(!called.get());
}

#[test]
fn a_passing_check_writes_once_and_a_failed_write_is_reported() {
    let seen = Cell::new(0);
    let ok = connect::store_credentials(
        Service::Jira,
        "a@b.c",
        "tok",
        || None,
        |s, a, t| {
            seen.set(seen.get() + 1);
            assert_eq!((s, a, t), (Service::Jira, "a@b.c", "tok"));
            true
        },
    );
    assert_eq!(ok, None);
    assert_eq!(seen.get(), 1);
    let bad = connect::store_credentials(Service::Jira, "a", "t", || None, |_, _, _| false);
    assert_eq!(bad.as_deref(), Some("✗ Không lưu được vào Keychain"));
}

#[test]
fn failure_lines_follow_the_status() {
    let line = |e: &dyn claude_sessions_core::credentials::ClientError| {
        connect::failure_text(Service::Bitbucket, "api.bitbucket.org", e)
    };
    assert_eq!(
        line(&BitbucketError::Transport),
        "✗ Không kết nối được tới api.bitbucket.org"
    );
    assert_eq!(
        line(&BitbucketError::Http(401)),
        "✗ Bitbucket trả về 401: email hoặc token sai"
    );
    assert_eq!(line(&BitbucketError::Http(403)), "✗ Lỗi HTTP 403");
    assert_eq!(
        connect::failure_text(Service::Jira, "x.atlassian.net", &JiraError::Http(401)),
        "✗ Jira trả về 401: email hoặc token sai"
    );
}

fn pair(a: &str, t: &str) -> Option<(String, String)> {
    Some((a.to_string(), t.to_string()))
}

#[test]
fn check_uses_the_typed_pair() {
    let seen = Cell::new(false);
    let got = connect::check(
        Service::Jira,
        pair("a@b.c", "typed"),
        Some("x.atlassian.net"),
        |_| {
            seen.set(true);
            None
        },
        |s, a, t, site| {
            assert_eq!(
                (s, a, t, site),
                (Service::Jira, "a@b.c", "typed", Some("x.atlassian.net"))
            );
            None
        },
    );
    assert_eq!(got, Ok("a@b.c".to_string()));
    assert!(
        !seen.get(),
        "the Keychain is not read when a token is typed"
    );
}

#[test]
fn check_reads_the_saved_pair_when_none_is_typed() {
    let got = connect::check(
        Service::Bitbucket,
        None,
        None,
        |s| {
            assert_eq!(s, Service::Bitbucket);
            pair("me@x.y", "saved")
        },
        |_, a, t, _| {
            assert_eq!((a, t), ("me@x.y", "saved"));
            None
        },
    );
    assert_eq!(got, Ok("me@x.y".to_string()));
}

#[test]
fn check_without_a_saved_token_fails() {
    let got = connect::check(
        Service::Jira,
        None,
        Some("x.atlassian.net"),
        |_| None,
        |_, _, _, _| panic!("nothing to validate"),
    );
    assert_eq!(got, Err("✗ Chưa có token đã lưu".to_string()));
}

#[test]
fn check_returns_the_failure_text() {
    let got = connect::check(
        Service::Jira,
        pair("a@b.c", "t"),
        Some("x.atlassian.net"),
        |_| None,
        |_, _, _, _| Some("✗ Lỗi HTTP 500".to_string()),
    );
    assert_eq!(got, Err("✗ Lỗi HTTP 500".to_string()));
}
