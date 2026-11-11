//! The one host guard for every request that carries a token (Jira API, Jira icons, Bitbucket). Hosts come from
//! transcript text, which is attacker-influenced: only `token_url` may build a token-bearing URL.

use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenHost {
    /// `*.atlassian.net`
    Jira,
    /// `api.bitbucket.org`
    Bitbucket,
}

pub const BITBUCKET_HOST: &str = "api.bitbucket.org";

/// `https://<host><path>` where `path` begins with `/` and may carry a query; `None` unless `host` is a plain ASCII
/// host name of the allowed family, with no userinfo, no port and no trailing dot.
pub fn token_url(host: &str, path: &str, kind: TokenHost) -> Option<Url> {
    let host = host.to_ascii_lowercase();
    if !is_plain_host(&host) {
        return None;
    }
    let allowed = match kind {
        TokenHost::Jira => host.ends_with(".atlassian.net"),
        TokenHost::Bitbucket => host == BITBUCKET_HOST,
    };
    if !allowed || !path.starts_with('/') {
        return None;
    }
    let mut url = Url::parse(&format!("https://{host}/")).ok()?;
    let (path, query) = match path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (path, None),
    };
    url.set_path(path);
    url.set_query(query);
    // Rebuild and reparse: the parsed host must still be the one asked for.
    let again = Url::parse(url.as_str()).ok()?;
    let same = again.scheme() == "https"
        && again.username().is_empty()
        && again.password().is_none()
        && again.port().is_none()
        && again.host_str() == Some(host.as_str());
    same.then_some(again)
}

/// Letters, digits, `-` and `.` only, no empty label: no character that could end the host early or hide another one.
fn is_plain_host(host: &str) -> bool {
    !host.is_empty()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
