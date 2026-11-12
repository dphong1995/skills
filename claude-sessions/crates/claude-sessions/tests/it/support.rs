//! A snapshot built through the core `Store` in a temp dir (as the core tests do), and a fixed-width `TextMeasure`.

use claude_sessions_core::bitbucket::{BitbucketApi, BitbucketError};
use claude_sessions_core::geometry::Size;
use claude_sessions_core::layout::TextMeasure;
use claude_sessions_core::pr_monitor::PRMonitor;
use claude_sessions_core::scene::FontSpec;
use claude_sessions_core::store::{Paths, Session, Snapshot, Store, StoreOptions};
use claude_sessions_core::transcript_scanner::TranscriptScanner;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

/// 7 pt per char, 16 pt per line.
pub struct FakeMeasure;

impl TextMeasure for FakeMeasure {
    fn measure(&self, text: &str, _font: &FontSpec, max_width: f64, max_lines: u32) -> Size {
        let per_line = ((max_width / 7.0).floor() as usize).max(1);
        let mut lines = 0usize;
        let mut widest = 0usize;
        for line in text.split('\n') {
            let n = line.chars().count();
            lines += n.div_ceil(per_line).max(1);
            widest = widest.max(n.min(per_line));
        }
        if max_lines > 0 {
            lines = lines.min(max_lines as usize);
        }
        Size::new(widest as f64 * 7.0, lines as f64 * 16.0)
    }
}

fn make_store(dir: &Path) -> Store {
    let paths = Paths {
        sessions: dir.to_path_buf(),
        projects: dir.join("_projects"),
        tasks: dir.join("_tasks"),
    };
    Store::new(
        paths,
        StoreOptions {
            scanner: TranscriptScanner::new(false),
            branch: Some(Box::new(|_| None)),
            ..StoreOptions::default()
        },
    )
}

/// The snapshot of a store holding `sessions`; the temp dir is removed before returning.
pub fn snapshot(sessions: Vec<Session>) -> Snapshot {
    let dir = tempfile::Builder::new()
        .prefix("claude-sessions-view-tests-")
        .tempdir()
        .unwrap();
    let mut store = make_store(dir.path());
    store.sessions = sessions;
    store.snapshot()
}

const PR_REPO: &str = "t6engineering/talo6-dct-backend";

/// A Bitbucket that lists the given open PRs and answers nothing else.
struct OpenPrs(Vec<i64>);

impl BitbucketApi for OpenPrs {
    fn get(&self, path: &str) -> Result<Vec<u8>, BitbucketError> {
        let body = if path.contains("/2.0/user") {
            json!({"uuid": "{me}"})
        } else if path.contains("/statuses") || path.contains("/comments") {
            json!({"values": []})
        } else if path.contains("/workspaces/") {
            let prs: Vec<Value> = self
                .0
                .iter()
                .map(|id| {
                    json!({"id": id, "title": "t", "state": "OPEN", "comment_count": 0,
                        "source": {"branch": {"name": "b"}, "repository": {"full_name": PR_REPO},
                            "commit": {"hash": "abc"}},
                        "links": {"html": {"href": format!("https://bitbucket.org/{PR_REPO}/pull-requests/{id}")}},
                        "participants": []})
                })
                .collect();
            json!({"values": prs})
        } else {
            return Err(BitbucketError::Http(404));
        };
        Ok(serde_json::to_vec(&body).unwrap())
    }

    fn post(&self, _path: &str, _json: &Value) -> Result<Vec<u8>, BitbucketError> {
        Ok(b"{}".to_vec())
    }
}

/// A store with one live session `a` whose transcript links the PRs `ids`, refreshed once against a fake Bitbucket.
pub fn pr_store(ids: &[i64]) -> (Store, tempfile::TempDir) {
    let dir = tempfile::Builder::new()
        .prefix("claude-sessions-menu-tests-")
        .tempdir()
        .unwrap();
    let cwd = "/Users/phong/talo6-dct-backend";
    let projects = dir.path().join("_projects");
    let project = projects.join(claude_sessions_core::transcript::project_dir_name(cwd));
    std::fs::create_dir_all(&project).unwrap();
    let links: Vec<String> = ids
        .iter()
        .map(|i| format!("https://bitbucket.org/{PR_REPO}/pull-requests/{i}"))
        .collect();
    let line = json!({"type": "assistant", "isSidechain": false,
        "message": {"content": [{"type": "text", "text": links.join(" ")}]}});
    std::fs::write(project.join("a.jsonl"), format!("{line}\n")).unwrap();
    let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let updated = claude_sessions_core::time::unix_secs(std::time::SystemTime::now());
    std::fs::write(
        dir.path().join("a.json"),
        json!({"pid": std::process::id(), "sessionId": "a", "name": "a", "cwd": cwd,
            "status": "idle", "statusUpdatedAt": updated * 1000.0})
        .to_string(),
    )
    .unwrap();
    let monitor = PRMonitor::with_options(
        Arc::new(OpenPrs(ids.to_vec())),
        false,
        Arc::new(|_| None),
        45.0,
    );
    let mut store = Store::new(
        Paths {
            sessions: dir.path().to_path_buf(),
            projects,
            tasks: dir.path().join("_tasks"),
        },
        StoreOptions {
            prs: Some(monitor),
            scanner: TranscriptScanner::new(false),
            branch: Some(Box::new(|_| None)),
            ..StoreOptions::default()
        },
    );
    store.refresh(now);
    (store, dir)
}
