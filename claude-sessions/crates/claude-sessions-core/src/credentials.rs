//! Keychain access through `/usr/bin/security` (accepted risk, spec `## Rủi ro`) and the connection state of a service.
//! Tokens travel only on stdin to `security -i`: never in an argument list, a log or a panic message.

use crate::shell::run_process;
use std::path::Path;
use std::time::Duration;

const SECURITY: &str = "/usr/bin/security";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Service {
    Bitbucket,
    Jira,
}

impl Service {
    pub const ALL: [Service; 2] = [Service::Bitbucket, Service::Jira];
    pub const TOKEN_PAGE: &'static str =
        "https://id.atlassian.com/manage-profile/security/api-tokens";

    pub fn keychain_service(self) -> &'static str {
        match self {
            Service::Bitbucket => "bitbucket-api-token",
            Service::Jira => "jira-api-token",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Service::Bitbucket => "Bitbucket",
            Service::Jira => "Jira",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Checking,
    Ok,
    Rejected,
    Unreachable,
}

/// The error of a Bitbucket or Jira call, as far as the connection state is concerned.
pub trait ClientError {
    /// The HTTP status carried by a client error; `None` for a transport error.
    fn http_code(&self) -> Option<u16>;
}

impl ConnectionState {
    /// The state after one call; `error` is `None` on success. Errors that say nothing about the connection keep `previous`.
    pub fn after(error: Option<&dyn ClientError>, previous: ConnectionState) -> ConnectionState {
        let Some(error) = error else {
            return ConnectionState::Ok;
        };
        match error.http_code() {
            None => ConnectionState::Unreachable,
            Some(401) => ConnectionState::Rejected,
            Some(c) if c == 429 || c >= 500 => ConnectionState::Unreachable,
            Some(_) => previous,
        }
    }
}

/// How the Keychain code starts `security`: `(path, args, timeout, require_success, stdin)`, as `shell::run_process`.
pub type Runner<'a> = &'a dyn Fn(&Path, &[&str], Duration, bool, Option<&[u8]>) -> Option<String>;

pub fn read(s: Service) -> Option<(String, String)> {
    read_with(&run_process, s)
}

pub fn read_with(run: Runner, s: Service) -> Option<(String, String)> {
    let user = account_with(run, s)?;
    let pw = run(
        Path::new(SECURITY),
        &["find-generic-password", "-s", s.keychain_service(), "-w"],
        Duration::from_secs(2),
        true,
        None,
    )?;
    let token = pw.trim();
    (!token.is_empty()).then(|| (user, token.to_string()))
}

/// The account of the Keychain item, never the token.
pub fn account(s: Service) -> Option<String> {
    account_with(&run_process, s)
}

pub fn account_with(run: Runner, s: Service) -> Option<String> {
    run(
        Path::new(SECURITY),
        &["find-generic-password", "-s", s.keychain_service()],
        Duration::from_secs(2),
        true,
        None,
    )
    .and_then(|out| account_from_attributes(&out))
}

/// The value of the `"acct"` attribute line of `security find-generic-password` output.
pub fn account_from_attributes(out: &str) -> Option<String> {
    let raw = out.split('\n').find(|l| l.contains("\"acct\"<blob>="))?;
    let line = raw.trim_matches([' ', '\t']);
    if !line.ends_with('"') {
        return None;
    }
    let marker = "<blob>=\"";
    let start = line.find(marker)? + marker.len();
    line.get(start..line.len() - 1).map(str::to_string)
}

/// The characters Swift's `Character.isNewline` accepts.
pub(crate) fn is_newline(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// False when a value holds a quote, backslash or newline, which `security -i` cannot take on one line.
pub fn is_safe(v: &str) -> bool {
    !v.chars().any(|c| c == '"' || c == '\\' || is_newline(c))
}

/// A `security -i` command line, or `None` when a value is not safe. `-U` updates an item of the same account in place.
pub fn add_script(service: &str, account: &str, token: &str) -> Option<String> {
    [service, account, token]
        .iter()
        .all(|v| is_safe(v))
        .then(|| {
            format!("add-generic-password -U -s \"{service}\" -a \"{account}\" -w \"{token}\"\n")
        })
}

pub fn write(s: Service, account: &str, token: &str) -> bool {
    write_with(&run_process, s, account, token)
}

/// Adds or updates the item first, checks it reads back with the same account and token, and only then removes items
/// of other accounts, so a failed add keeps the working token. The token travels on stdin only, never in an argument list.
pub fn write_with(run: Runner, s: Service, account: &str, token: &str) -> bool {
    let service = s.keychain_service();
    let Some(script) = add_script(service, account, token) else {
        return false;
    };
    let security = Path::new(SECURITY);
    if run(
        security,
        &["-i"],
        Duration::from_secs(5),
        true,
        Some(script.as_bytes()),
    )
    .is_none()
    {
        return false;
    }
    let back = run(
        security,
        &["find-generic-password", "-s", service, "-a", account, "-w"],
        Duration::from_secs(2),
        true,
        None,
    );
    if back.as_deref().map(str::trim) != Some(token) {
        return false;
    }
    for _ in 0..10 {
        let Some(other) = account_with(run, s) else {
            break;
        };
        if other == account
            || run(
                security,
                &["delete-generic-password", "-s", service, "-a", &other],
                Duration::from_secs(2),
                true,
                None,
            )
            .is_none()
        {
            break;
        }
    }
    account_with(run, s).as_deref() == Some(account)
}
