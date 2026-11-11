//! The settings the Swift widget keeps in `UserDefaults` (`muted`, `bgOpacity`, `prPrompt`, `jiraProjects`): loaded into the
//! store at start, written back when a value changed (Swift's `$muted.dropFirst().sink`).

use std::collections::HashMap;

use crate::theme;
use claude_sessions_core::kv::KeyValueStore;
use claude_sessions_core::pull_requests::DEFAULT_PROMPT;
use claude_sessions_core::store::Store;
use serde_json::{Map, Value};

/// Background opacities of the collapsed pill (Swift `Layout.bgOpacities`).
pub const BG_OPACITIES: [f64; 5] = [0.0, 0.25, 0.5, 0.75, theme::CARD_OPACITY];

/// The values last loaded or saved, to tell what changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub muted: bool,
    pub bg_opacity: f64,
    pub pr_prompt: String,
    pub jira_projects: HashMap<String, String>,
}

/// Reads the saved values into `store` and returns the prompt template (the default when none is saved).
pub fn load(kv: &dyn KeyValueStore, store: &mut Store) -> Settings {
    store.muted = kv.get_bool("muted").unwrap_or(false);
    if let Some(v) = kv.get_f64("bgOpacity") {
        store.bg_opacity = BG_OPACITIES
            .iter()
            .copied()
            .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
            .unwrap_or(v);
    }
    let pr_prompt = kv
        .get_string("prPrompt")
        .unwrap_or_else(|| DEFAULT_PROMPT.to_string());
    // `[String: String]` as a whole, as Swift's cast: one other value type drops the dictionary.
    if let Some(dict) = kv.get_dict("jiraProjects")
        && let Some(map) = dict
            .iter()
            .map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
            .collect::<Option<HashMap<_, _>>>()
    {
        store.jira_projects = map;
    }
    current(store, pr_prompt)
}

pub fn current(store: &Store, pr_prompt: String) -> Settings {
    Settings {
        muted: store.muted,
        bg_opacity: store.bg_opacity,
        pr_prompt,
        jira_projects: store.jira_projects.clone(),
    }
}

/// `persist` of the store's current values, built only when one of them differs from `saved` (called every tick).
pub fn persist_store(kv: &dyn KeyValueStore, saved: &mut Settings, store: &Store, pr_prompt: &str) {
    if store.muted == saved.muted
        && store.bg_opacity == saved.bg_opacity
        && pr_prompt == saved.pr_prompt
        && store.jira_projects == saved.jira_projects
    {
        return;
    }
    persist(kv, saved, current(store, pr_prompt.to_string()));
}

/// Writes what differs between `saved` and `now`; `saved` becomes `now`.
pub fn persist(kv: &dyn KeyValueStore, saved: &mut Settings, now: Settings) {
    if now.muted != saved.muted {
        kv.set_bool("muted", now.muted);
    }
    if now.bg_opacity != saved.bg_opacity {
        kv.set_f64("bgOpacity", now.bg_opacity);
    }
    if now.pr_prompt != saved.pr_prompt {
        kv.set_string("prPrompt", &now.pr_prompt);
    }
    if now.jira_projects != saved.jira_projects {
        let dict: Map<String, Value> = now
            .jira_projects
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        kv.set_dict("jiraProjects", &dict);
    }
    *saved = now;
}
