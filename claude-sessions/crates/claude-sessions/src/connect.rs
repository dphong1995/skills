//! The pure parts of the connect and prompt dialogs: what is checked before a token is stored, the error lines and the saved
//! prompt. The dialogs themselves are `claude-sessions-macos::dialogs`; the Keychain writer and the service check are injected.

use claude_sessions_core::bitbucket::{BitbucketApi, BitbucketClient};
use claude_sessions_core::credentials::{ClientError, Service, is_safe};
use claude_sessions_core::jira::{JiraApi, JiraClient};
use claude_sessions_macos::dialogs::PromptAnswer;

/// The dialog text under the title (Swift `showDialog`'s `how`).
pub fn info(service: Service, host: Option<&str>) -> String {
    match service {
        Service::Bitbucket => "Tạo token bằng nút \"Create API token with scopes\": app Bitbucket, quyền read:user, read:repository, read:pullrequest.".to_string(),
        Service::Jira => format!(
            "Tạo token bằng nút \"Create API token\" (loại classic) cho site {}.",
            host.unwrap_or("")
        ),
    }
}

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

/// The new prompt template for a dialog answer, or `None` to keep the current one (Swift `editPrompt`): an empty `Lưu` falls
/// back to `default`.
pub fn prompt_value(answer: &PromptAnswer, default: &str) -> Option<String> {
    match answer {
        PromptAnswer::Save(text) => {
            let t = text.trim();
            Some(if t.is_empty() { default } else { t }.to_string())
        }
        PromptAnswer::Default => Some(default.to_string()),
        PromptAnswer::Cancel => None,
    }
}
