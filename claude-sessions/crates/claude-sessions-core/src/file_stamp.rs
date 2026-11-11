//! What identifies a file's content between two refreshes: inode (rename-based writes), size and nanosecond mtime.

use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStamp {
    pub inode: u64,
    pub size: u64,
    pub sec: i64,
    pub nsec: i64,
}

impl FileStamp {
    /// `None` when the file cannot be stat'ed.
    pub fn new(path: &Path) -> Option<Self> {
        let m = std::fs::metadata(path).ok()?;
        Some(Self {
            inode: m.ino(),
            size: m.size(),
            sec: m.mtime(),
            nsec: m.mtime_nsec(),
        })
    }
}
