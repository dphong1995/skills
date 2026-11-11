//! Shared by `BitbucketClient` and `JiraClient`: a cached Basic header for one Keychain service and a blocking request.
//! Redirects are disabled: a redirect would carry the `Authorization` header to another host.

use crate::credentials::{self, Service};
use crate::url_guard::{TokenHost, token_url};
use base64::Engine;
use std::fmt;
use std::sync::Mutex;
use std::time::Duration;
use url::Url;

const MAX_BODY: u64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Http(u16),
    Transport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One request, built only from a URL that passed `url_guard`. `Debug` hides the header values: they hold the token.
#[derive(Clone, PartialEq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: Url,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    pub fn get(url: Url, auth: &str) -> Self {
        HttpRequest {
            method: Method::Get,
            url,
            headers: vec![("Authorization".into(), auth.into())],
            body: None,
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url.as_str())
            .field(
                "headers",
                &self.headers.iter().map(|(k, _)| k).collect::<Vec<_>>(),
            )
            .field("body_len", &self.body.as_ref().map(Vec::len))
            .finish()
    }
}

pub struct BasicAuthHttp {
    service: Service,
    auth: Mutex<Option<String>>,
    agent: ureq::Agent,
}

fn header(account: &str, token: &str) -> String {
    let creds = format!("{account}:{token}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(creds)
    )
}

fn read_auth(service: Service) -> Option<String> {
    credentials::read(service).map(|(account, token)| header(&account, &token))
}

impl BasicAuthHttp {
    /// Uses the given credentials until `reload()`; no Keychain read.
    pub fn new(service: Service, account: &str, token: &str) -> Self {
        // Trust the macOS Keychain roots like URLSession does (corporate TLS inspection). The system proxy is not supported.
        let agent = ureq::Agent::config_builder()
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .into();
        BasicAuthHttp {
            service,
            auth: Mutex::new(Some(header(account, token))),
            agent,
        }
    }

    /// Re-reads the Keychain now, replacing any cached or explicit credentials.
    pub fn reload(&self) {
        let a = read_auth(self.service);
        *self.auth.lock().unwrap() = a;
    }

    /// The cached header; re-read from the Keychain after a 401 cleared it.
    pub fn current_auth(&self) -> Option<String> {
        let mut auth = self.auth.lock().unwrap();
        if auth.is_none() {
            *auth = read_auth(self.service);
        }
        auth.clone()
    }

    /// The Basic header goes only to https on this service's host, or to a loopback IP (it never leaves the machine).
    fn may_carry_auth(&self, url: &Url) -> bool {
        match url.scheme() {
            "https" => {
                let kind = match self.service {
                    Service::Jira => TokenHost::Jira,
                    Service::Bitbucket => TokenHost::Bitbucket,
                };
                url.host_str()
                    .and_then(|h| token_url(h, "/", kind))
                    .is_some_and(|_| url.port().is_none() && url.username().is_empty())
            }
            "http" => match url.host() {
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                _ => false,
            },
            _ => false,
        }
    }

    /// A 401 forgets the cached header so the next call reads the Keychain again.
    pub fn send(&self, req: &HttpRequest) -> Result<Vec<u8>, Failure> {
        if !self.may_carry_auth(&req.url) {
            return Err(Failure::Transport);
        }
        let url = req.url.as_str();
        let mut response = match req.method {
            Method::Get => {
                let mut b = self.agent.get(url);
                for (k, v) in &req.headers {
                    b = b.header(k, v);
                }
                b.call()
            }
            Method::Post => {
                let mut b = self.agent.post(url);
                for (k, v) in &req.headers {
                    b = b.header(k, v);
                }
                b.send(req.body.as_deref().unwrap_or_default())
            }
        }
        .map_err(|_| Failure::Transport)?;
        let status = response.status().as_u16();
        if status == 401 {
            *self.auth.lock().unwrap() = None;
        }
        if !(200..300).contains(&status) {
            return Err(Failure::Http(status));
        }
        response
            .body_mut()
            .with_config()
            .limit(MAX_BODY)
            .read_to_vec()
            .map_err(|_| Failure::Transport)
    }
}
