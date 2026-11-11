//! The `UserDefaults` settings: load into the store, write back what changed.

use claude_sessions::settings::{self, BG_OPACITIES};
use claude_sessions_core::kv::{KeyValueStore, MemoryStore};
use claude_sessions_core::pull_requests::DEFAULT_PROMPT;
use claude_sessions_core::store::{Paths, Store, StoreOptions};
use serde_json::{Map, Value};

fn store(dir: &std::path::Path) -> Store {
    Store::new(
        Paths {
            sessions: dir.join("sessions"),
            projects: dir.join("projects"),
            tasks: dir.join("tasks"),
        },
        StoreOptions::default(),
    )
}

#[test]
fn load_reads_the_swift_values() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = store(dir.path());
    let kv = MemoryStore::default();
    kv.set_bool("muted", true);
    kv.set_f64("bgOpacity", 0.6);
    kv.set_string("prPrompt", "x {url}");
    let mut projects = Map::new();
    projects.insert("ABC".into(), Value::String("acme.atlassian.net".into()));
    kv.set_dict("jiraProjects", &projects);
    let saved = settings::load(&kv, &mut store);
    assert!(store.muted);
    // The nearest of the offered opacities.
    assert_eq!(store.bg_opacity, 0.5);
    assert_eq!(saved.pr_prompt, "x {url}");
    assert_eq!(store.jira_projects["ABC"], "acme.atlassian.net");
    assert!(BG_OPACITIES.contains(&store.bg_opacity));
}

#[test]
fn absent_keys_keep_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = store(dir.path());
    let before = store.bg_opacity;
    let saved = settings::load(&MemoryStore::default(), &mut store);
    assert!(!store.muted);
    assert_eq!(store.bg_opacity, before);
    assert_eq!(saved.pr_prompt, DEFAULT_PROMPT);
}

#[test]
fn persist_writes_only_what_changed() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = store(dir.path());
    let kv = MemoryStore::default();
    let mut saved = settings::load(&kv, &mut store);
    let now = settings::current(&store, saved.pr_prompt.clone());
    settings::persist(&kv, &mut saved, now);
    assert_eq!(kv.get_bool("muted"), None);
    store.muted = true;
    store
        .jira_projects
        .insert("ABC".into(), "a.atlassian.net".into());
    let now = settings::current(&store, saved.pr_prompt.clone());
    settings::persist(&kv, &mut saved, now);
    assert_eq!(kv.get_bool("muted"), Some(true));
    assert_eq!(kv.get_f64("bgOpacity"), None);
    assert_eq!(
        kv.get_dict("jiraProjects").unwrap()["ABC"],
        "a.atlassian.net"
    );
}

#[test]
fn persist_store_writes_a_change_and_skips_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = store(dir.path());
    let kv = MemoryStore::default();
    let mut saved = settings::load(&kv, &mut store);
    let prompt = saved.pr_prompt.clone();
    settings::persist_store(&kv, &mut saved, &store, &prompt);
    assert_eq!(kv.get_bool("muted"), None);
    store.muted = true;
    settings::persist_store(&kv, &mut saved, &store, &prompt);
    assert_eq!(kv.get_bool("muted"), Some(true));
    assert!(saved.muted);
}
