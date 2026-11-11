//! The pure parts of the token and prompt dialogs. No dialog is shown and no Keychain is touched: the writer is injected.

use std::cell::Cell;

use claude_sessions::connect;
use claude_sessions_core::bitbucket::BitbucketError;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::jira::JiraError;
use claude_sessions_macos::dialogs::{PromptAnswer, TokenAnswer};

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

#[test]
fn the_dialog_text_names_the_site() {
    assert!(connect::info(Service::Bitbucket, None).contains("read:pullrequest"));
    assert!(
        connect::info(Service::Jira, Some("acme.atlassian.net"))
            .contains("site acme.atlassian.net.")
    );
}

#[test]
fn a_token_answer_never_prints_the_token() {
    let a = TokenAnswer::Save {
        account: "a@b.c".into(),
        token: "SECRET-TOKEN".into(),
    };
    let shown = format!("{a:?}");
    assert!(!shown.contains("SECRET") && !shown.contains("a@b.c"));
}

#[test]
fn prompt_answers_become_the_saved_prompt() {
    let d = "default {url}";
    let v = |a: PromptAnswer| connect::prompt_value(&a, d);
    assert_eq!(
        v(PromptAnswer::Save("  mine {id} \n".into())).as_deref(),
        Some("mine {id}")
    );
    assert_eq!(v(PromptAnswer::Save("   ".into())).as_deref(), Some(d));
    assert_eq!(v(PromptAnswer::Default).as_deref(), Some(d));
    assert_eq!(v(PromptAnswer::Cancel), None);
}
