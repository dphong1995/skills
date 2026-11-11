use claude_sessions_core::credentials::ConnectionState;
use claude_sessions_core::jira::{self, JiraClient, JiraError, JiraIssue, JiraTone};
use serde_json::Value;
use url::Url;

fn issues(json: &str, requested: &[&str]) -> Option<std::collections::HashMap<String, JiraIssue>> {
    let requested: Vec<String> = requested.iter().map(|s| s.to_string()).collect();
    jira::issues(json.as_bytes(), &requested)
}

fn basic(creds: &str) -> String {
    use base64::Engine;
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(creds)
    )
}

#[test]
fn parses_bulk_fetch() {
    let json = r#"{"issues":[{"key":"T6IR-104","fields":{"summary":"Fix login","status":{"name":"Code Review"}}}],"issueErrors":[]}"#;
    let r = issues(json, &["T6IR-104", "T6IR-999"]).unwrap();
    assert_eq!(
        r["T6IR-104"],
        JiraIssue {
            title: Some("Fix login".into()),
            status: Some("Code Review".into()),
            ..Default::default()
        }
    );
    assert_eq!(r["T6IR-999"], JiraIssue::default());
}

#[test]
fn parse_rejects_non_json() {
    assert!(issues("nope", &["A-1"]).is_none());
}

#[test]
fn bulk_fetch_request_shape() {
    let auth = basic("a@b.c:tok");
    let req =
        JiraClient::request("talosix.atlassian.net", &["T6IR-104".to_string()], &auth).unwrap();
    assert_eq!(
        req.method,
        claude_sessions_core::basic_auth_http::Method::Post
    );
    assert_eq!(
        req.url.as_str(),
        "https://talosix.atlassian.net/rest/api/3/issue/bulkfetch"
    );
    assert_eq!(req.header("Authorization"), Some(auth.as_str()));
    assert_eq!(req.header("Content-Type"), Some("application/json"));
    let body: Value = serde_json::from_slice(req.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["issueIdsOrKeys"], serde_json::json!(["T6IR-104"]));
    assert_eq!(
        body["fields"],
        serde_json::json!(["summary", "status", "issuetype"])
    );
}

#[test]
fn request_refuses_foreign_hosts() {
    let keys = ["T6IR-1".to_string()];
    assert!(JiraClient::request("evil.example", &keys, "x").is_none());
    assert!(JiraClient::request("atlassian.net.evil.example", &keys, "x").is_none());
    assert!(JiraClient::request("Talosix.Atlassian.NET", &keys, "x").is_some());
}

#[test]
fn connection_maps_jira_errors() {
    use ConnectionState::*;
    let after = |e: JiraError, previous| ConnectionState::after(Some(&e), previous);
    assert_eq!(after(JiraError::Http(401), Ok), Rejected);
    assert_eq!(after(JiraError::Transport, Ok), Unreachable);
    assert_eq!(after(JiraError::Http(503), Ok), Unreachable);
    assert_eq!(after(JiraError::Http(429), Ok), Unreachable);
    assert_eq!(after(JiraError::Http(404), Rejected), Rejected);
}

const ICON_URL: &str = "https://talosix.atlassian.net/rest/api/2/universal_avatar/view/type/issuetype/avatar/10315?size=medium";

fn bulk(category: Option<&str>) -> Option<std::collections::HashMap<String, JiraIssue>> {
    let cat = category
        .map(|c| format!(r#","statusCategory":{{"key":"{c}","colorName":"yellow"}}"#))
        .unwrap_or_default();
    let json = format!(
        r#"{{"issues":[{{"key":"T6IR-13681","fields":{{"summary":"S","issuetype":{{"name":"Story/New Feature Request","iconUrl":"{ICON_URL}"}},"status":{{"name":"IMPLEMENTED"{cat}}}}}}}]}}"#
    );
    issues(&json, &["T6IR-13681"])
}

#[test]
fn parses_type_icon_and_tone() {
    assert_eq!(
        bulk(Some("indeterminate")).unwrap()["T6IR-13681"],
        JiraIssue {
            title: Some("S".into()),
            status: Some("IMPLEMENTED".into()),
            issue_type: Some("Story/New Feature Request".into()),
            icon_url: Url::parse(ICON_URL).ok(),
            tone: Some(JiraTone::Doing),
        }
    );
    assert_eq!(
        bulk(Some("new")).unwrap()["T6IR-13681"].tone,
        Some(JiraTone::Todo)
    );
    assert_eq!(
        bulk(Some("done")).unwrap()["T6IR-13681"].tone,
        Some(JiraTone::Done)
    );
}

#[test]
fn parses_unknown_category_as_nil_tone() {
    assert_eq!(bulk(Some("weird")).unwrap()["T6IR-13681"].tone, None);
    assert_eq!(bulk(None).unwrap()["T6IR-13681"].tone, None);
    let json = r#"{"issues":[{"key":"A-1","fields":{"summary":"S","status":{"name":"X"}}}]}"#;
    let i = issues(json, &["A-1"]).unwrap().remove("A-1").unwrap();
    assert!(i.issue_type.is_none() && i.icon_url.is_none());
}

#[test]
fn icon_request_shape() {
    let req = JiraClient::icon_request(&Url::parse(ICON_URL).unwrap(), "Basic x").unwrap();
    assert_eq!(
        req.method,
        claude_sessions_core::basic_auth_http::Method::Get
    );
    let items: Vec<(String, String)> = req
        .url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert!(items.contains(&("size".into(), "medium".into())));
    assert!(items.contains(&("format".into(), "png".into())));
    assert_eq!(req.header("Authorization"), Some("Basic x"));
    let icon = |u: &str| JiraClient::icon_request(&Url::parse(u).unwrap(), "x");
    assert!(icon("http://talosix.atlassian.net/x.png").is_none());
    assert!(icon("https://evil.example/x.png").is_none());
    assert!(icon("https://atlassian.net.evil.example/x").is_none());
    assert!(icon("https://a@talosix.atlassian.net/x.png").is_none());
    assert!(icon("https://talosix.atlassian.net:8443/x.png").is_none());
}
