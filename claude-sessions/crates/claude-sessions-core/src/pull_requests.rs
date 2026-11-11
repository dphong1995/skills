//! Bitbucket pull-request links. Phase 2 holds what the transcript scanner needs; the rest comes with the network modules.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PRRef {
    pub workspace: String,
    pub repo: String,
    pub id: i64,
}

static LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https://bitbucket\.org/([\w.-]+)/([\w.-]+)/pull-requests/(\d+)").unwrap()
});

/// The Bitbucket PRs linked in `text`, first appearance order, one per PR.
pub fn refs(text: &str) -> Vec<PRRef> {
    let mut out: Vec<PRRef> = Vec::new();
    for m in LINK_REGEX.captures_iter(text) {
        let Ok(id) = m[3].parse::<i64>() else {
            continue;
        };
        let r = PRRef {
            workspace: m[1].to_string(),
            repo: m[2].to_string(),
            id,
        };
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

/// `!` bash-mode and local-command output, which Claude Code stores as plain user lines.
pub fn is_shell_output(s: &str) -> bool {
    let t = s.trim();
    [
        "<bash-stdout>",
        "<bash-stderr>",
        "<local-command-stdout>",
        "<local-command-stderr>",
    ]
    .iter()
    .any(|p| t.starts_with(p))
}
