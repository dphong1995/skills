use claude_sessions_core::bitbucket::BitbucketError;
use claude_sessions_core::credentials::{
    self, ConnectionState, Service, account_from_attributes, add_script, is_safe,
};
use std::cell::RefCell;
use std::path::Path;
use std::time::Duration;

#[test]
fn add_script_quotes_values() {
    assert_eq!(
        add_script("jira-api-token", "a@b.com", "AbC_12-=").as_deref(),
        Some("add-generic-password -U -s \"jira-api-token\" -a \"a@b.com\" -w \"AbC_12-=\"\n")
    );
}

#[test]
fn add_script_rejects_unsafe_input() {
    assert_eq!(add_script("s", "a\"b", "t"), None);
    assert_eq!(add_script("s", "a", "t\\u"), None);
    assert_eq!(add_script("s", "a", "t\nu"), None);
}

#[test]
fn is_safe_refuses_quote_backslash_newline() {
    assert!(is_safe("a@b.com"));
    assert!(is_safe("AbC_12-= ?"));
    assert!(!is_safe("a\"b"));
    assert!(!is_safe("a\\b"));
    assert!(!is_safe("a\nb"));
    assert!(!is_safe("a\r\nb"));
}

#[test]
fn connection_mapping() {
    use ConnectionState::*;
    assert_eq!(ConnectionState::after(None, Rejected), Ok);
    let after = |e: BitbucketError, previous| ConnectionState::after(Some(&e), previous);
    assert_eq!(after(BitbucketError::Http(401), Ok), Rejected);
    assert_eq!(after(BitbucketError::Transport, Ok), Unreachable);
    assert_eq!(after(BitbucketError::Http(429), Ok), Unreachable);
    assert_eq!(after(BitbucketError::Http(503), Ok), Unreachable);
    assert_eq!(after(BitbucketError::Http(404), Rejected), Rejected);
}

#[test]
fn account_parsed_from_keychain_attributes() {
    let out = "keychain: \"/x/login.keychain-db\"\nattributes:\n    \"acct\"<blob>=\"phong@example.com\"\n    \"svce\"<blob>=\"bitbucket-api-token\"\n";
    assert_eq!(
        account_from_attributes(out).as_deref(),
        Some("phong@example.com")
    );
    assert_eq!(account_from_attributes("    \"acct\"<blob>=<NULL>\n"), None);
}

#[test]
fn is_safe_rejects_all_newlines() {
    for c in [
        '\n', '\r', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}',
    ] {
        assert!(!is_safe(&format!("a{c}b")), "U+{:04X} accepted", c as u32);
        assert_eq!(add_script("s", "a", &format!("t{c}u")), None);
    }
}

#[test]
fn write_sends_token_on_stdin_only() {
    type Call = (String, Vec<String>, Option<Vec<u8>>);
    let token = "s3cr3t-T0KEN";
    let calls: RefCell<Vec<Call>> = RefCell::new(Vec::new());
    let runner = |path: &Path,
                  args: &[&str],
                  _timeout: Duration,
                  _require_success: bool,
                  input: Option<&[u8]>| {
        calls.borrow_mut().push((
            path.to_string_lossy().into_owned(),
            args.iter().map(|a| a.to_string()).collect(),
            input.map(<[u8]>::to_vec),
        ));
        Some(if args.contains(&"-w") {
            format!("{token}\n")
        } else if args.first() == Some(&"find-generic-password") {
            "attributes:\n    \"acct\"<blob>=\"me@x.com\"\n".to_string()
        } else {
            String::new()
        })
    };
    assert!(credentials::write_with(
        &runner,
        Service::Jira,
        "me@x.com",
        token
    ));
    let calls = calls.borrow();
    assert!(!calls.is_empty());
    for (path, args, _) in calls.iter() {
        assert_eq!(path, "/usr/bin/security");
        assert!(
            args.iter().all(|a| !a.contains(token)),
            "token in argv: {args:?}"
        );
    }
    let with_input: Vec<_> = calls.iter().filter(|c| c.2.is_some()).collect();
    assert_eq!(with_input.len(), 1);
    assert_eq!(with_input[0].1, ["-i"]);
    let input = String::from_utf8(with_input[0].2.clone().unwrap()).unwrap();
    assert!(input.contains(token) && input.ends_with('\n'));
}
