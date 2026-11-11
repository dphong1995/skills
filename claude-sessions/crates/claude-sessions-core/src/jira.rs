//! Jira host rules. Phase 2 holds what the transcript scanner needs; the rest comes with the network modules.

/// The only hosts that may receive the Jira token.
pub fn is_atlassian_host(host: &str) -> bool {
    host.to_lowercase().ends_with(".atlassian.net")
}

/// Whether a newly seen host replaces the stored one: first seen wins, except an Atlassian host replaces a foreign one.
pub fn replaces(stored: Option<&str>, host: &str) -> bool {
    match stored {
        None => true,
        Some(stored) => is_atlassian_host(host) && !is_atlassian_host(stored),
    }
}
