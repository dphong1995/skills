//! The branch checked out in a directory, read from `.git/HEAD` without spawning git.

use crate::file_stamp::FileStamp;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Walks up from `cwd` to the first `.git` and returns its `HEAD` file; `None` for no repo or an unreadable `.git` file.
pub fn head_file(cwd: &str) -> Option<PathBuf> {
    if !cwd.starts_with('/') {
        return None;
    }
    let mut dir = PathBuf::from(cwd);
    loop {
        let git = dir.join(".git");
        if let Ok(meta) = std::fs::metadata(&git) {
            let mut head = git.clone();
            if !meta.is_dir() {
                let text = std::fs::read_to_string(&git).ok()?;
                let rest = text.strip_prefix("gitdir:")?;
                // `join` keeps an absolute path as it is and resolves a relative one against the worktree dir.
                head = dir.join(rest.trim());
            }
            return Some(head.join("HEAD"));
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// The branch named by a `HEAD` file; `None` for a detached HEAD or an unreadable file.
pub fn branch_in_head(head: &Path) -> Option<String> {
    let content = std::fs::read_to_string(head).ok()?;
    let name = content.trim().strip_prefix("ref: refs/heads/")?;
    (!name.is_empty()).then(|| name.to_string())
}

/// Branch of the repo containing `cwd`; `None` for a detached HEAD, no repo or an unreadable file.
pub fn current(cwd: &str) -> Option<String> {
    head_file(cwd).and_then(|h| branch_in_head(&h))
}

struct Cached {
    head: PathBuf,
    stamp: FileStamp,
    branch: Option<String>,
}

/// Reads branches for a `Store`: per cwd it remembers the `HEAD` file and re-reads it only when its `FileStamp` changes.
/// A cwd with no repo is not cached, so a later `git init` is seen. Counts `HEAD` file reads for tests.
#[derive(Default)]
pub struct GitBranchReader {
    head_reads: usize,
    cache: HashMap<String, Cached>,
}

impl GitBranchReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn head_reads(&self) -> usize {
        self.head_reads
    }

    pub fn cached_cwds(&self) -> usize {
        self.cache.len()
    }

    pub fn current(&mut self, cwd: &str) -> Option<String> {
        // A nearer `.git` created under a cached repo is not seen while the cached HEAD still exists (accepted trade-off).
        if let Some(c) = self.cache.get(cwd)
            && FileStamp::new(&c.head) == Some(c.stamp)
        {
            return c.branch.clone();
        }
        self.cache.remove(cwd);
        let head = head_file(cwd)?;
        let stamp = FileStamp::new(&head);
        self.head_reads += 1;
        let branch = branch_in_head(&head);
        if let Some(stamp) = stamp {
            self.cache.insert(
                cwd.to_string(),
                Cached {
                    head,
                    stamp,
                    branch: branch.clone(),
                },
            );
        }
        branch
    }

    pub fn prune(&mut self, keeping: &HashSet<String>) {
        self.cache.retain(|k, _| keeping.contains(k));
    }
}
