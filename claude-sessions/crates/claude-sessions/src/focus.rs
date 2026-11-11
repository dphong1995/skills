//! Focused-pane logic (Swift `silentIds` and `checkFocusedPane`): which sessions' cmux surface is the focused one. The cmux
//! calls are closures so tests inject them; the app runs `gather` on a worker thread, never on the main one.

use std::collections::{HashMap, HashSet};

use claude_sessions_core::store::Store;

/// A surface lookup that finished: `surface` is `None` when cmux does not know the session. `pid` is the session's process when
/// the lookup started; a result for another process is dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Located {
    pub id: String,
    pub pid: Option<i32>,
    pub surface: Option<String>,
}

/// Looks the surface of each `(id, pid)` target up, then asks for the focused surface, but only when at least one surface is
/// known (`known_surface`, or found now): cmux is not queried for nothing. `focused` is `None` on any cmux failure.
pub fn gather(
    targets: &[(String, Option<i32>)],
    known_surface: bool,
    lookup: impl Fn(&str) -> Option<String>,
    focused: impl FnOnce() -> Option<String>,
) -> (Vec<Located>, Option<String>) {
    let located: Vec<Located> = targets
        .iter()
        .map(|(id, pid)| Located {
            id: id.clone(),
            pid: *pid,
            surface: lookup(id),
        })
        .collect();
    let any = known_surface || located.iter().any(|l| l.surface.is_some());
    let focused = if any { focused() } else { None };
    (located, focused)
}

/// Caches the finished lookups, as Swift `resolveSurface` does (a session whose process changed meanwhile is skipped).
pub fn apply_located(store: &mut Store, located: &[Located]) {
    for l in located {
        if store.pid_of(&l.id) != l.pid {
            continue;
        }
        match &l.surface {
            Some(s) => {
                store.surfaces.insert(l.id.clone(), s.clone());
                store.surface_misses.remove(&l.id);
            }
            None => {
                store.surface_misses.insert(l.id.clone());
            }
        }
    }
}

/// The ids among `ids` whose surface is the focused one (compared case-insensitively): their alert must not chime.
pub fn silent_ids<'a>(
    ids: impl IntoIterator<Item = &'a String>,
    surfaces: &HashMap<String, String>,
    focused: Option<&str>,
) -> HashSet<String> {
    let Some(focused) = focused else {
        return HashSet::new();
    };
    ids.into_iter()
        .filter(|id| {
            surfaces
                .get(*id)
                .is_some_and(|s| s.to_lowercase() == focused.to_lowercase())
        })
        .cloned()
        .collect()
}

/// What the 2 s check starts.
#[derive(Debug, PartialEq, Eq)]
pub enum FocusPlan {
    /// No mark to clear or cmux is not frontmost: forget the pending check.
    Clear,
    /// Look these sessions up (their marks have no surface yet) and ask cmux for the focused surface when one is known.
    Check {
        lookups: Vec<(String, Option<i32>)>,
        known_surface: bool,
    },
}

/// `looking_up` holds the ids whose lookup is still running.
pub fn plan(store: &Store, cmux_frontmost: bool, looking_up: &HashSet<String>) -> FocusPlan {
    if store.unseen.is_empty() || !cmux_frontmost {
        return FocusPlan::Clear;
    }
    let mut lookups: Vec<(String, Option<i32>)> = store
        .unseen
        .keys()
        .filter(|id| {
            !store.surfaces.contains_key(*id)
                && !store.surface_misses.contains(*id)
                && !looking_up.contains(*id)
        })
        .map(|id| (id.clone(), store.pid_of(id)))
        .collect();
    lookups.sort();
    let known_surface = store
        .unseen
        .keys()
        .any(|id| store.surfaces.contains_key(id));
    if !known_surface {
        return FocusPlan::Clear;
    }
    FocusPlan::Check {
        lookups,
        known_surface,
    }
}
