//! Claude Sessions core: no UI and no `objc2`. Phase 1 holds only the scene description the AppKit layer renders.
#![forbid(unsafe_code)]

pub mod closed_sessions;
pub mod file_stamp;
pub mod git_branch;
pub mod jira;
pub mod kv;
pub mod layout;
pub mod pull_requests;
pub mod quota;
pub mod scene;
pub mod shell;
pub mod tasks;
pub mod time;
pub mod transcript;
pub mod transcript_scanner;
