//! Canned Bitbucket, Jira and cmux Feed, and the JSON builders the snapshot states feed them (Swift `FakeBitbucket`,
//! `FakeJira`, `FakeFeedTransport`, `prJSON`, `rabbitStatus`, ...).

use crate::bitbucket::{BitbucketApi, BitbucketError};
use crate::feed::{FeedTransport, TransportError};
use crate::jira::{JiraApi, JiraError};
use crate::shell::format_iso_date;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use url::Url;

/// A route matches when the requested path contains its key; the longest matching key wins.
#[derive(Default)]
pub struct FakeBitbucket {
    routes: Mutex<HashMap<String, Result<Vec<u8>, BitbucketError>>>,
    pub fail_all: Mutex<Option<BitbucketError>>,
}

impl FakeBitbucket {
    pub fn set(&self, key: &str, json: Value) {
        self.routes
            .lock()
            .unwrap()
            .insert(key.to_string(), Ok(serde_json::to_vec(&json).unwrap()));
    }

    pub fn set_error(&self, key: &str, e: BitbucketError) {
        self.routes.lock().unwrap().insert(key.to_string(), Err(e));
    }
}

impl BitbucketApi for FakeBitbucket {
    fn get(&self, path: &str) -> Result<Vec<u8>, BitbucketError> {
        if let Some(e) = *self.fail_all.lock().unwrap() {
            return Err(e);
        }
        let routes = self.routes.lock().unwrap();
        let key = routes
            .keys()
            .filter(|k| path.contains(k.as_str()))
            .max_by_key(|k| k.len())
            .ok_or(BitbucketError::Http(404))?;
        routes[key].clone()
    }

    fn post(&self, _path: &str, _json: &Value) -> Result<Vec<u8>, BitbucketError> {
        Ok(b"{}".to_vec())
    }
}

/// Routes for the user, the open list, and no statuses or comments.
pub fn fake_bitbucket(open: Vec<Value>) -> Arc<FakeBitbucket> {
    let api = Arc::new(FakeBitbucket::default());
    api.set("/2.0/user", json!({"uuid": "{me}"}));
    api.set(
        "/2.0/workspaces/t6engineering/pullrequests/",
        json!({"values": open}),
    );
    api.set("/statuses", json!({"values": []}));
    api.set("/comments", json!({"values": []}));
    api
}

/// A pull request as the Bitbucket list returns it. `reviewers` adds reviewers with changes requested, approved, none.
pub fn pr_json(
    id: i64,
    comments: i64,
    reviewers: usize,
    rabbit_approved: Option<bool>,
    state: &str,
) -> Value {
    let repo = "t6engineering/talo6-dct-backend";
    let mut participants = Vec::new();
    for i in 0..reviewers {
        let state = match i {
            0 => json!("changes_requested"),
            1 => json!("approved"),
            _ => Value::Null,
        };
        participants.push(
            json!({"role": "REVIEWER", "approved": i == 1, "state": state,
            "user": {"uuid": format!("{{r{i}}}"), "display_name": format!("Reviewer {i}")}}),
        );
    }
    if let Some(approved) = rabbit_approved {
        participants.push(json!({"role": "REVIEWER", "approved": approved,
            "state": if approved { json!("approved") } else { Value::Null },
            "user": {"uuid": "{tin}", "display_name": "Tin Tran"}}));
    }
    participants.push(
        json!({"role": "PARTICIPANT", "approved": false, "state": null,
        "user": {"uuid": "{author}", "display_name": "Author"}}),
    );
    json!({"id": id, "title": "t", "state": state, "comment_count": comments,
        "source": {"branch": {"name": "bugfix/T6IR-14744"}, "repository": {"full_name": repo}, "commit": {"hash": "abc123"}},
        "links": {"html": {"href": format!("https://bitbucket.org/{repo}/pull-requests/{id}")}},
        "participants": participants})
}

pub fn comment(uuid: &str, name: &str, at: SystemTime) -> Value {
    json!({"user": {"uuid": uuid, "display_name": name}, "created_on": format_iso_date(at), "deleted": false})
}

pub const RABBIT_CREATED: &str = "2026-11-03T03:33:15.111541+00:00";

/// CodeRabbit's build status, successful, on the default commit.
pub fn rabbit_status(description: &str, created: &str) -> Value {
    json!({"key": "coderabbitai/6867", "name": "CodeRabbit", "state": "SUCCESSFUL", "description": description,
        "created_on": created, "updated_on": created, "commit": {"hash": "abc123"}})
}

/// A comment carrying CodeRabbit's marker, posted by a human account like the real ones.
pub fn rabbit_comment(raw: &str, at: &str) -> Value {
    json!({"user": {"uuid": "{tin}", "display_name": "Tin Tran"}, "created_on": at, "updated_on": at, "deleted": false,
        "content": {"raw": format!("[coderabbit-html-comment-v1-0]: https://coderabbit.ai/#html-comment-v1:abc\n{raw}")}})
}

/// An inline CodeRabbit comment: a finding, or a reply when `parent` is given.
pub fn rabbit_inline(id: i64, raw: &str, at: &str, parent: Option<i64>) -> Value {
    let mut c = rabbit_comment(raw, at);
    c["id"] = json!(id);
    c["inline"] = json!({"path": "a.ts"});
    if let Some(p) = parent {
        c["parent"] = json!({"id": p});
    }
    c
}

/// CodeRabbit's summary and one unanswered finding.
pub fn pending_finding(count: i64, at: &str) -> Vec<Value> {
    vec![
        rabbit_comment(&format!("**Actionable comments posted: {count}**"), at),
        rabbit_inline(1, "nit", at, None),
    ]
}

/// A human reply to a finding: no CodeRabbit marker.
pub fn human_reply(id: i64, parent: i64, raw: &str, at: &str) -> Value {
    json!({"id": id, "user": {"uuid": "{human}", "display_name": "Human"}, "created_on": at, "updated_on": at,
        "deleted": false, "content": {"raw": raw}, "inline": {"path": "a.ts"}, "parent": {"id": parent}})
}

/// Canned Jira: issues, their status categories and types, and the icon bytes by URL.
#[derive(Default)]
pub struct FakeJira {
    /// Key to `(title, status)`.
    pub issues: Mutex<HashMap<String, (String, Option<String>)>>,
    /// Key to `(issue type name, icon url)`.
    pub types: Mutex<HashMap<String, (String, String)>>,
    pub icon_data: Mutex<HashMap<Url, Vec<u8>>>,
    /// Key to `statusCategory.key`.
    pub categories: Mutex<HashMap<String, String>>,
}

impl JiraApi for FakeJira {
    fn bulk_fetch(&self, _host: &str, keys: &[String]) -> Result<Vec<u8>, JiraError> {
        let issues = self.issues.lock().unwrap();
        let types = self.types.lock().unwrap();
        let found: Vec<Value> = keys
            .iter()
            .filter_map(|k| {
                let (title, status) = issues.get(k)?;
                let mut fields = json!({"summary": title});
                if let Some(s) = status {
                    fields["status"] = json!({"name": s});
                    if let Some(c) = self.categories.lock().unwrap().get(k) {
                        fields["status"]["statusCategory"] = json!({"key": c});
                    }
                }
                if let Some((name, icon)) = types.get(k) {
                    fields["issuetype"] = json!({"name": name, "iconUrl": icon});
                }
                Some(json!({"key": k, "fields": fields}))
            })
            .collect();
        Ok(serde_json::to_vec(&json!({"issues": found})).unwrap())
    }

    fn myself(&self, _host: &str) -> Result<Vec<u8>, JiraError> {
        Ok(b"{}".to_vec())
    }

    fn icon(&self, url: &Url) -> Result<Vec<u8>, JiraError> {
        self.icon_data
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or(JiraError::Http(404))
    }
}

/// Canned `feed.list`; every reply succeeds.
#[derive(Default)]
pub struct FakeFeedTransport {
    pub items: Mutex<Vec<Value>>,
}

impl FeedTransport for FakeFeedTransport {
    fn request(
        &self,
        method: &str,
        _params: &Value,
        _timeout: Duration,
    ) -> Result<Vec<u8>, TransportError> {
        if method == "feed.list" {
            let items = self.items.lock().unwrap().clone();
            let reply = json!({"ok": true, "id": "x", "result": {"items": items}});
            return Ok(serde_json::to_vec(&reply).unwrap());
        }
        Ok(br#"{"ok":true,"result":{"delivered":true}}"#.to_vec())
    }
}

/// One `question` item as `feed.list` returns it (the session id is inside `workstream_id`).
pub fn feed_item(
    request_id: &str,
    session_id: &str,
    created: SystemTime,
    questions: Vec<Value>,
) -> Value {
    use base64::Engine;
    let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
    json!({"kind": "question", "id": format!("ID-{request_id}"), "request_id": request_id, "status": "pending",
        "source": "claude", "cwd": "/x", "created_at": format_iso_date(created),
        "workstream_id": format!("cmux-feed-v1:{}:{}", b64("claude"), b64(session_id)),
        "questions": questions})
}

/// A question whose options are described `Mô tả <label>`.
pub fn feed_question_json(
    id: &str,
    prompt: &str,
    header: &str,
    multi: bool,
    labels: &[&str],
) -> Value {
    let options: Vec<Value> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| json!({"id": format!("o{i}"), "label": l, "description": format!("Mô tả {l}")}))
        .collect();
    json!({"id": id, "prompt": prompt, "header": header, "multi_select": multi, "options": options})
}

/// A 16x16 PNG: a rounded square of `rgb` on a transparent background (stored deflate, no compression).
pub fn swatch_png(rgb: [u8; 3]) -> Vec<u8> {
    const N: usize = 16;
    let mut raw = Vec::with_capacity(N * (N * 4 + 1));
    for y in 0..N {
        raw.push(0);
        for x in 0..N {
            let (dx, dy) = (
                (x as f64 + 0.5 - 8.0).abs() - 5.0,
                (y as f64 + 0.5 - 8.0).abs() - 5.0,
            );
            let inside = dx.max(0.0).hypot(dy.max(0.0)) <= 3.0 && dx <= 3.0 && dy <= 3.0;
            let px = if inside {
                [rgb[0], rgb[1], rgb[2], 255]
            } else {
                [0; 4]
            };
            raw.extend_from_slice(&px);
        }
    }
    let mut z = vec![0x78, 0x01, 0x01];
    z.extend_from_slice(&(raw.len() as u16).to_le_bytes());
    z.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
    z.extend_from_slice(&raw);
    let (mut a, mut b) = (1u32, 0u32);
    for &v in &raw {
        a = (a + v as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in &out[start..] {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        out.extend_from_slice(&(!crc).to_be_bytes());
    }
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(N as u32).to_be_bytes());
    ihdr.extend_from_slice(&(N as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);
    png
}
