//! Claude Sessions core: no UI and no `objc2`. Phase 1 holds only the scene description the AppKit layer renders.
#![forbid(unsafe_code)]

pub mod action;
pub mod basic_auth_http;
pub mod bitbucket;
pub mod closed_sessions;
pub mod cmux;
pub mod code_rabbit;
pub mod credentials;
pub mod feed;
pub mod file_stamp;
#[cfg(feature = "fixtures")]
pub mod fixtures;
pub mod geometry;
pub mod git_branch;
pub mod hotkeys;
pub mod jira;
pub mod jira_monitor;
pub mod keyboard;
pub mod kv;
pub mod layout;
pub mod panel_state;
pub mod pr_monitor;
pub mod pull_requests;
pub mod quota;
pub mod scene;
pub mod shell;
pub mod store;
pub mod tasks;
pub mod text;
pub mod time;
pub mod transcript;
pub mod transcript_scanner;
pub mod url_guard;
mod worker;
