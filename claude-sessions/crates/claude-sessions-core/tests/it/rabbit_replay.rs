use claude_sessions_core::code_rabbit::{self as cr, RabbitInputs};
use claude_sessions_core::pull_requests;
use claude_sessions_core::shell::parse_iso_date;
use claude_sessions_core::time::{add_secs, from_unix_secs};
use serde_json::{Map, Value, json};

fn load(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v[key].as_array().unwrap()
}

/// Feeds one recorded PR through what `PRMonitor` does on first sight (statuses and comments fetched once, then `derive`)
/// and records what the card would show. The monitor itself comes in phase 4.
fn replay_outcome(pr: &Value) -> Value {
    let repo = pr["repo"].as_str().unwrap();
    let id = pr["id"].as_i64().unwrap();
    let comments = array(pr, "comments");
    let statuses = array(pr, "statuses");
    let newest = comments
        .iter()
        .chain(statuses)
        .flat_map(|v| [&v["created_on"], &v["updated_on"]])
        .filter_map(|s| s.as_str().and_then(parse_iso_date))
        .max()
        .unwrap_or_else(|| from_unix_secs(1_800_000_000.0));
    let now = add_secs(newest, 60.0);

    let open = json!({"id": id, "title": "t", "state": "OPEN", "comment_count": pr["commentCount"],
        "source": {"branch": {"name": "b"}, "repository": {"full_name": repo}, "commit": {"hash": pr["commit"]}},
        "links": {"html": {"href": format!("https://bitbucket.org/{repo}/pull-requests/{id}")}},
        "participants": pr["participants"], "author": {"uuid": pr["author"]}});
    let status = pull_requests::status_from(&open).unwrap();
    let summary = cr::newest_summary(comments).cloned();
    let view = cr::derive(
        &RabbitInputs {
            commit: status.commit.clone(),
            statuses: Some(statuses.to_vec()),
            comments: Some(comments.to_vec()),
            summary,
            approvers: status.approvers.clone(),
            me: pr["author"].as_str().map(str::to_string),
            author: status.author.clone(),
            nudged_at: None,
        },
        now,
    );
    let mut out = Map::new();
    out.insert(
        "label".into(),
        view.rabbit
            .as_ref()
            .map_or(Value::Null, |r| json!(cr::label(r, view.threads))),
    );
    out.insert(
        "pending".into(),
        view.threads.map_or(Value::Null, |t| json!(t.pending)),
    );
    out.insert(
        "total".into(),
        view.threads.map_or(Value::Null, |t| json!(t.total)),
    );
    out.insert(
        "hidesAskClaude".into(),
        json!(cr::hides_ask_claude(
            view.rabbit.as_ref(),
            view.threads,
            view.others_pending
        )),
    );
    Value::Object(out)
}

#[test]
fn replay_matches_expected() {
    let prs = load("rabbit-replay.json")["prs"]
        .as_array()
        .unwrap()
        .clone();
    let mut actual = Map::new();
    for pr in &prs {
        let key = pr["key"].as_str().unwrap();
        actual.insert(key.to_string(), replay_outcome(pr));
        // Same data with nobody approving, so the threads/summary/awaiting-approval paths are reached.
        let mut no_approvers = pr.clone();
        for p in no_approvers["participants"].as_array_mut().unwrap() {
            p["approved"] = json!(false);
            p["state"] = Value::Null;
        }
        actual.insert(format!("{key}/noApprovers"), replay_outcome(&no_approvers));
    }
    let expected = load("rabbit-replay-expected.json");
    let expected = expected.as_object().unwrap();
    let mut diffs = Vec::new();
    let keys: std::collections::BTreeSet<_> = actual.keys().chain(expected.keys()).collect();
    for key in keys {
        if actual.get(key) != expected.get(key) {
            diffs.push(format!(
                "{key}: {:?} -> {:?}",
                expected.get(key),
                actual.get(key)
            ));
        }
    }
    assert!(
        diffs.is_empty(),
        "{} PRs differ:\n{}",
        diffs.len(),
        diffs.join("\n")
    );
}
