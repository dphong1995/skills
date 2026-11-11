//! The Jira Cloud client and the parsing of its answers. Every token-bearing URL comes from `url_guard::token_url`.

use crate::basic_auth_http::{BasicAuthHttp, Failure, HttpRequest, Method};
use crate::bitbucket::CredentialReloading;
use crate::credentials::{ClientError, Service};
use crate::url_guard::{TokenHost, token_url};
use serde_json::{Value, json};
use std::collections::HashMap;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JiraError {
    Http(u16),
    Transport,
}

impl ClientError for JiraError {
    fn http_code(&self) -> Option<u16> {
        match self {
            JiraError::Http(c) => Some(*c),
            JiraError::Transport => None,
        }
    }
}

pub trait JiraApi: Send + Sync {
    /// Blocking: called only on the monitor's worker thread or a background thread.
    fn bulk_fetch(&self, host: &str, keys: &[String]) -> Result<Vec<u8>, JiraError>;
    /// Blocking: called only on a background thread.
    fn myself(&self, host: &str) -> Result<Vec<u8>, JiraError>;
    /// Blocking: called only on the monitor's worker thread. `url` is an issue type icon.
    fn icon(&self, url: &Url) -> Result<Vec<u8>, JiraError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JiraTone {
    Todo,
    Doing,
    Done,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JiraIssue {
    pub title: Option<String>,
    pub status: Option<String>,
    pub issue_type: Option<String>,
    pub icon_url: Option<Url>,
    pub tone: Option<JiraTone>,
}

/// The only hosts that may receive the Jira token.
pub fn is_atlassian_host(host: &str) -> bool {
    host.to_lowercase().ends_with(".atlassian.net")
}

/// Whether a newly seen host replaces the stored one: first seen wins, except an Atlassian host replaces a foreign one.
pub fn replaces(stored: Option<&str>, host: &str) -> bool {
    match stored {
        None => true,
        Some(stored) => is_atlassian_host(host) && !is_atlassian_host(stored),
    }
}

fn str_of<'a>(v: Option<&'a Value>, key: &str) -> Option<&'a str> {
    v?.get(key)?.as_str()
}

/// Every requested key is in the result; one `bulkfetch` did not return gets no title and no status. `None` when `data` is not JSON.
pub fn issues(data: &[u8], requested: &[String]) -> Option<HashMap<String, JiraIssue>> {
    let o: Value = serde_json::from_slice(data).ok()?;
    o.as_object()?;
    let mut out: HashMap<String, JiraIssue> = requested
        .iter()
        .map(|k| (k.clone(), JiraIssue::default()))
        .collect();
    for issue in o
        .get("issues")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(key) = issue.get("key").and_then(Value::as_str) else {
            continue;
        };
        if !out.contains_key(key) {
            continue;
        }
        let fields = issue.get("fields");
        let status = fields.and_then(|f| f.get("status"));
        let kind = fields.and_then(|f| f.get("issuetype"));
        let tone = match str_of(status.and_then(|s| s.get("statusCategory")), "key") {
            Some("new") => Some(JiraTone::Todo),
            Some("indeterminate") => Some(JiraTone::Doing),
            Some("done") => Some(JiraTone::Done),
            _ => None,
        };
        out.insert(
            key.to_string(),
            JiraIssue {
                title: str_of(fields, "summary").map(str::to_string),
                status: str_of(status, "name").map(str::to_string),
                issue_type: str_of(kind, "name").map(str::to_string),
                icon_url: str_of(kind, "iconUrl").and_then(|u| Url::parse(u).ok()),
                tone,
            },
        );
    }
    Some(out)
}

pub struct JiraClient {
    http: BasicAuthHttp,
}

impl JiraClient {
    /// Uses the given credentials until `reload()`; no Keychain read.
    pub fn new(account: &str, token: &str) -> Self {
        JiraClient {
            http: BasicAuthHttp::new(Service::Jira, account, token),
        }
    }

    /// `None` for a host outside `*.atlassian.net`: the token must never go anywhere else.
    fn request_to(host: &str, path: &str, auth: &str) -> Option<HttpRequest> {
        let url = token_url(host, path, TokenHost::Jira)?;
        Some(HttpRequest::get(url, auth).with_header("Accept", "application/json"))
    }

    pub fn request(host: &str, keys: &[String], auth: &str) -> Option<HttpRequest> {
        let mut req = Self::request_to(host, "/rest/api/3/issue/bulkfetch", auth)?;
        let body = json!({"issueIdsOrKeys": keys, "fields": ["summary", "status", "issuetype"]});
        req.method = Method::Post;
        req.body = Some(serde_json::to_vec(&body).ok()?);
        Some(req.with_header("Content-Type", "application/json"))
    }

    /// `None` for a non-https URL, userinfo, a port, or a host outside `*.atlassian.net`: the token must never go elsewhere.
    /// Asks Jira for a PNG rendering.
    pub fn icon_request(url: &Url, auth: &str) -> Option<HttpRequest> {
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            return None;
        }
        let mut guarded = token_url(url.host_str()?, url.path(), TokenHost::Jira)?;
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| k != "format")
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        guarded
            .query_pairs_mut()
            .clear()
            .extend_pairs(kept)
            .append_pair("format", "png");
        Some(HttpRequest::get(guarded, auth))
    }

    fn send(&self, req: Option<HttpRequest>) -> Result<Vec<u8>, JiraError> {
        let req = req.ok_or(JiraError::Transport)?;
        self.http.send(&req).map_err(|e| match e {
            Failure::Http(code) => JiraError::Http(code),
            Failure::Transport => JiraError::Transport,
        })
    }

    fn auth(&self) -> Result<String, JiraError> {
        self.http.current_auth().ok_or(JiraError::Transport)
    }
}

impl CredentialReloading for JiraClient {
    /// Re-reads the Keychain now, replacing any cached or explicit credentials.
    fn reload(&self) {
        self.http.reload();
    }
}

impl JiraApi for JiraClient {
    fn bulk_fetch(&self, host: &str, keys: &[String]) -> Result<Vec<u8>, JiraError> {
        let auth = self.auth()?;
        self.send(Self::request(host, keys, &auth))
    }

    fn myself(&self, host: &str) -> Result<Vec<u8>, JiraError> {
        let auth = self.auth()?;
        self.send(Self::request_to(host, "/rest/api/3/myself", &auth))
    }

    fn icon(&self, url: &Url) -> Result<Vec<u8>, JiraError> {
        let auth = self.auth()?;
        self.send(Self::icon_request(url, &auth))
    }
}
