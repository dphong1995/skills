//! A snapshot built through the core `Store` in a temp dir (as the core tests do), and a fixed-width `TextMeasure`.

use claude_sessions_core::geometry::Size;
use claude_sessions_core::layout::TextMeasure;
use claude_sessions_core::scene::FontSpec;
use claude_sessions_core::store::{Paths, Session, Snapshot, Store, StoreOptions};
use claude_sessions_core::transcript_scanner::TranscriptScanner;
use std::path::Path;

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
