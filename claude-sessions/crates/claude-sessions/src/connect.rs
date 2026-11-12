//! The pure parts of the token check and save in the Settings window: what is checked before a token is stored and the error
//! lines. The Keychain reader and writer and the service check are injected.

use claude_sessions_core::bitbucket::{BitbucketApi, BitbucketClient};
use claude_sessions_core::credentials::{ClientError, Service, is_safe};
use claude_sessions_core::jira::{JiraApi, JiraClient};

/// The trimmed account and token, or the notice to show when they cannot be used. Nothing reaches the Keychain or a request
/// line unless it passes `is_safe`.
pub fn precheck(account: &str, token: &str) -> Result<(String, String), String> {
    let (account, token) = (account.trim(), token.trim());
    if account.is_empty() || token.is_empty() {
        return Err("✗ Nhập cả email và token".into());
    }
    if !is_safe(account) || !is_safe(token) {
        return Err("✗ Email hoặc token có ký tự không hợp lệ".into());
    }
    Ok((account.to_string(), token.to_string()))
}

/// The notice for a failed call: `error` is the client error (Swift `validate`).
pub fn failure_text(service: Service, site: &str, error: &dyn ClientError) -> String {
    match error.http_code() {
        None => format!("✗ Không kết nối được tới {site}"),
        Some(401) => format!(
            "✗ {} trả về 401: email hoặc token sai",
            service.display_name()
        ),
        Some(code) => format!("✗ Lỗi HTTP {code}"),
    }
}

/// Calls the service with the typed credentials; the notice, or `None` when they work. Blocks: background thread only.
pub fn validate(
    service: Service,
    account: &str,
    token: &str,
    host: Option<&str>,
) -> Option<String> {
    match service {
        Service::Bitbucket => BitbucketClient::new(account, token)
            .get("/2.0/user?fields=uuid")
            .err()
            .map(|e| failure_text(service, "api.bitbucket.org", &e)),
        Service::Jira => {
            let host = host.unwrap_or("");
            JiraClient::new(account, token)
                .myself(host)
                .err()
                .map(|e| failure_text(service, host, &e))
        }
    }
}

/// Checks the credentials, then writes them with `write` (the Keychain); the notice, or `None` when stored. `write` is not
/// called when the check fails.
pub fn store_credentials(
    service: Service,
    account: &str,
    token: &str,
    check: impl FnOnce() -> Option<String>,
    write: impl FnOnce(Service, &str, &str) -> bool,
) -> Option<String> {
    if let Some(failure) = check() {
        return Some(failure);
    }
    (!write(service, account, token)).then(|| "✗ Không lưu được vào Keychain".to_string())
}

/// The account a check confirmed, or the notice to show. `typed` is the email and token from the Settings fields; without it
/// the saved pair is read with `read`. `validate` calls the service (it blocks: background thread only).
pub fn check(
    service: Service,
    typed: Option<(String, String)>,
    site: Option<&str>,
    read: impl FnOnce(Service) -> Option<(String, String)>,
    validate: impl FnOnce(Service, &str, &str, Option<&str>) -> Option<String>,
) -> Result<String, String> {
    let (account, token) = match typed {
        Some(pair) => pair,
        None => read(service).ok_or_else(|| "✗ Chưa có token đã lưu".to_string())?,
    };
    match validate(service, &account, &token, site) {
        Some(failure) => Err(failure),
        None => Ok(account),
    }
}
