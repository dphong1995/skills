//! The Bitbucket Cloud API client. Every request URL comes from `url_guard::token_url`.

use crate::basic_auth_http::{BasicAuthHttp, Failure, HttpRequest, Method};
use crate::credentials::{ClientError, Service};
use crate::url_guard::{BITBUCKET_HOST, TokenHost, token_url};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitbucketError {
    Http(u16),
    Transport,
}

impl ClientError for BitbucketError {
    fn http_code(&self) -> Option<u16> {
        match self {
            BitbucketError::Http(c) => Some(*c),
            BitbucketError::Transport => None,
        }
    }
}

pub trait BitbucketApi: Send + Sync {
    /// `path` begins with `/2.0/` and includes the query. Blocking: called only on the monitor's worker thread.
    fn get(&self, path: &str) -> Result<Vec<u8>, BitbucketError>;
    /// Blocking like `get`; `json` is sent as the request body.
    fn post(&self, path: &str, json: &Value) -> Result<Vec<u8>, BitbucketError>;
}

/// Something that can drop its cached credentials and read the Keychain again.
pub trait CredentialReloading: Send + Sync {
    fn reload(&self);
}

pub struct BitbucketClient {
    http: BasicAuthHttp,
}

impl BitbucketClient {
    /// Uses the given credentials until `reload()`; no Keychain read.
    pub fn new(account: &str, token: &str) -> Self {
        BitbucketClient {
            http: BasicAuthHttp::new(Service::Bitbucket, account, token),
        }
    }

    fn send(
        &self,
        path: &str,
        method: Method,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, BitbucketError> {
        let header = self.http.current_auth().ok_or(BitbucketError::Transport)?;
        let url = token_url(BITBUCKET_HOST, path, TokenHost::Bitbucket)
            .ok_or(BitbucketError::Transport)?;
        let mut req = HttpRequest::get(url, &header).with_header("Accept", "application/json");
        req.method = method;
        if body.is_some() {
            req = req.with_header("Content-Type", "application/json");
        }
        req.body = body;
        self.http.send(&req).map_err(|e| match e {
            Failure::Http(code) => BitbucketError::Http(code),
            Failure::Transport => BitbucketError::Transport,
        })
    }
}

impl CredentialReloading for BitbucketClient {
    /// Re-reads the Keychain now, replacing any cached or explicit credentials.
    fn reload(&self) {
        self.http.reload();
    }
}

impl BitbucketApi for BitbucketClient {
    fn get(&self, path: &str) -> Result<Vec<u8>, BitbucketError> {
        self.send(path, Method::Get, None)
    }

    fn post(&self, path: &str, json: &Value) -> Result<Vec<u8>, BitbucketError> {
        let body = serde_json::to_vec(json).map_err(|_| BitbucketError::Transport)?;
        self.send(path, Method::Post, Some(body))
    }
}
