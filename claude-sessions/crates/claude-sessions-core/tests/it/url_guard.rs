use crate::support::TestHttpServer;
use claude_sessions_core::basic_auth_http::{BasicAuthHttp, HttpRequest};
use claude_sessions_core::credentials::Service;
use claude_sessions_core::url_guard::{TokenHost, token_url};

#[test]
fn rejects_host_confusion() {
    for host in [
        "evil.com#.atlassian.net",
        "evil.com?.atlassian.net",
        "evil.com/.atlassian.net",
        "evil.com\\.atlassian.net",
        "a@evil.com",
        "a@x.atlassian.net",
        "x.atlassian.net:8443",
        "x.atlassian.net.",
        "evil.example",
        "atlassian.net.evil.example",
        "atlassian.net",
        ".atlassian.net",
        "a..atlassian.net",
        "",
        "évil.atlassian.net",
        "evil.com\u{3002}atlassian.net",
        "x.atlassian.net\n",
        "x.atlassian.net ",
        "x.atlassian.net%2f.evil.com",
    ] {
        assert!(
            token_url(host, "/rest/api/3/myself", TokenHost::Jira).is_none(),
            "accepted Jira host {host:?}"
        );
    }
    for host in [
        "evil.com",
        "api.bitbucket.org.evil.com",
        "api.bitbucket.org:8443",
        "a@api.bitbucket.org",
        "x.atlassian.net",
        "bitbucket.org",
    ] {
        assert!(
            token_url(host, "/2.0/user", TokenHost::Bitbucket).is_none(),
            "accepted Bitbucket host {host:?}"
        );
    }
    // A path cannot move the request to another host.
    let u = token_url("x.atlassian.net", "//evil.com/a", TokenHost::Jira).unwrap();
    assert_eq!(u.host_str(), Some("x.atlassian.net"));
    let u = token_url(
        "api.bitbucket.org",
        "/@evil.com/x?a=b#c",
        TokenHost::Bitbucket,
    )
    .unwrap();
    assert_eq!(u.host_str(), Some("api.bitbucket.org"));
    assert_eq!(u.username(), "");
}

#[test]
fn accepts_atlassian_hosts() {
    for host in ["acme.atlassian.net", "ACME.atlassian.net"] {
        let u = token_url(host, "/rest/api/3/myself", TokenHost::Jira).unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host_str(), Some(host.to_lowercase().as_str()));
        assert_eq!(u.port(), None);
        assert_eq!(u.path(), "/rest/api/3/myself");
    }
    let u = token_url(
        "api.bitbucket.org",
        "/2.0/user?fields=uuid",
        TokenHost::Bitbucket,
    )
    .unwrap();
    assert_eq!(u.as_str(), "https://api.bitbucket.org/2.0/user?fields=uuid");
}

#[test]
fn no_redirects_for_basic_auth() {
    let second = TestHttpServer::start(|_| "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".into());
    let target = format!("http://localhost:{}/stolen", second.port);
    let first = TestHttpServer::start(move |_| {
        format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\n\r\n")
    });
    let http = BasicAuthHttp::new(Service::Jira, "a@b.c", "tok");
    let url = url::Url::parse(&format!("http://127.0.0.1:{}/start", first.port)).unwrap();
    let auth = http.current_auth().unwrap();
    let result = http.send(&HttpRequest::get(url, &auth));
    assert!(result.is_err(), "a redirect was followed: {result:?}");
    assert_eq!(first.request_count(), 1);
    assert_eq!(
        second.request_count(),
        0,
        "the redirect target got a request"
    );
}

#[test]
fn basic_auth_refuses_other_hosts_and_plain_http() {
    let http = BasicAuthHttp::new(Service::Jira, "a@b.c", "tok");
    let auth = http.current_auth().unwrap();
    // Neither request leaves the process: the guard refuses before any connection.
    for url in [
        "http://acme.atlassian.net/rest/api/3/myself",
        "https://evil.example/rest/api/3/myself",
        "https://api.bitbucket.org/2.0/user",
        "https://acme.atlassian.net:8443/x",
        "http://localhost:1/x",
    ] {
        let url = url::Url::parse(url).unwrap();
        let result = http.send(&HttpRequest::get(url.clone(), &auth));
        assert!(result.is_err(), "{url} was sent");
    }
}
