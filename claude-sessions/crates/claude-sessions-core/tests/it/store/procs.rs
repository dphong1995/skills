//! Leftover processes in the store: the closed rows, the held table and the kill send key.

use super::*;
use claude_sessions_core::action::{SendKey, SendKind};
use claude_sessions_core::closed_sessions::{ClosedSession, ClosedSessions, ClosedStore};
use claude_sessions_core::kv::MemoryStore;
use claude_sessions_core::procs::{GroupKind, ProcGroup, ProcTable, SessionProcs};

fn group(key: &str, members: &[(i32, u64)]) -> ProcGroup {
    ProcGroup {
        key: key.into(),
        kind: GroupKind::Command,
        name: key.into(),
        command: key.into(),
        ports: vec![],
        rss: 0,
        cpu_percent: None,
        started_us: 0,
        members: members.to_vec(),
    }
}

fn procs(
    session: &str,
    started_us: u64,
    pwd: Option<&str>,
    groups: Vec<ProcGroup>,
) -> SessionProcs {
    SessionProcs {
        session: session.into(),
        commands: groups,
        claude_and_mcp: None,
        pwd: pwd.map(String::from),
        started_us,
    }
}

fn closed(id: &str, ago_secs: u64) -> ClosedSession {
    ClosedSession {
        id: id.into(),
        title: format!("title {id}"),
        cwd: format!("/work/{id}"),
        branch: None,
        closed_at: SystemTime::now() - Duration::from_secs(ago_secs),
    }
}

/// A store that starts with `items` in its closed list.
fn store_closed(items: Vec<ClosedSession>) -> (Store, TempDir) {
    let kv = MemoryStore::default();
    ClosedStore::new(Box::new(kv.clone())).save(&ClosedSessions::new(items));
    let dir = make_temp_dir();
    let store = make_store_with(
        dir.path(),
        StoreSetup {
            closed_store: Some(ClosedStore::new(Box::new(kv))),
            ..Default::default()
        },
    );
    (store, dir)
}

fn row_ids(store: &Store) -> Vec<String> {
    store
        .snapshot()
        .closed_rows
        .iter()
        .map(|r| r.id.clone())
        .collect()
}

#[test]
fn snapshot_puts_leftover_rows_first() {
    let (mut store, _dir) = store_closed(vec![
        closed("new", 10),
        closed("mid", 20),
        closed("old", 30),
    ]);
    store.set_procs(ProcTable {
        live: HashMap::new(),
        closed: vec![
            procs("zz", 500, Some("/p/zz"), vec![group("g1", &[(10, 1)])]),
            procs("old", 100, None, vec![group("g2", &[(11, 1)])]),
            procs("mid", 100, None, vec![group("g3", &[(12, 1)])]),
            procs("aa", 900, Some("/p/aa"), vec![group("g4", &[(13, 1)])]),
        ],
    });
    // Leftovers of known sessions (newest closed first), synthesized rows (newest start first), then the rest.
    assert_eq!(row_ids(&store), ["mid", "old", "aa", "zz", "new"]);
    let snap = store.snapshot();
    assert_eq!(snap.closed.len(), 3);
    assert!(snap.closed_rows[4].procs.is_none());
    assert_eq!(snap.closed_rows[0].title, "title mid");
}

#[test]
fn synthesized_row_for_unknown_session() {
    let (mut store, _dir) = store_closed(vec![]);
    store.set_procs(ProcTable {
        live: HashMap::new(),
        closed: vec![procs(
            "u1",
            5,
            Some("/Users/x/my-app"),
            vec![group("g", &[(10, 1)])],
        )],
    });
    let row = &store.snapshot().closed_rows[0];
    assert_eq!(row.title, "my-app");
    assert_eq!(row.folder, "my-app");
    assert_eq!(row.closed_at, None);
    assert_eq!(row.session.cwd, "/Users/x/my-app");
    assert_eq!(row.session.id, "u1");
}

#[test]
fn leftover_count_sums_rows() {
    let (mut store, _dir) = store_closed(vec![closed("a", 10)]);
    store.set_procs(ProcTable {
        live: HashMap::new(),
        closed: vec![
            procs(
                "a",
                1,
                None,
                vec![group("g1", &[(10, 1)]), group("g2", &[(11, 1)])],
            ),
            procs("b", 2, Some("/p/b"), vec![group("g3", &[(12, 1)])]),
        ],
    });
    assert_eq!(store.snapshot().leftover_count(), 3);
}

#[test]
fn set_procs_with_equal_table_keeps_snapshot_equal() {
    let (mut store, _dir) = store_closed(vec![]);
    let table = ProcTable {
        live: HashMap::new(),
        closed: vec![procs("b", 2, Some("/p/b"), vec![group("g", &[(12, 1)])])],
    };
    store.refresh(SystemTime::now());
    store.set_procs(table.clone());
    let before = store.snapshot();
    store.refresh(SystemTime::now() + Duration::from_secs(5));
    store.set_procs(table);
    assert_eq!(store.snapshot(), before);
}

#[test]
fn kill_send_key_tells_groups_apart() {
    let kill = |session: &str, key: &str| Action::KillProcs {
        session: session.into(),
        key: key.into(),
        targets: vec![(10, 1)],
    };
    let a = kill("s", "g1").send_key().unwrap();
    assert_eq!(a.kind, SendKind::Kill);
    assert_eq!(a.detail.as_deref(), Some("g1"));
    assert_ne!(Some(a.clone()), kill("s", "g2").send_key());
    assert_ne!(Some(a), kill("s", "all:s").send_key());
    assert_eq!(
        kill("", "all").send_key(),
        Some(SendKey {
            session: "".into(),
            kind: SendKind::Kill,
            pr: None,
            detail: Some("all".into())
        })
    );
}

#[test]
fn kill_still_valid_only_while_a_target_is_listed() {
    let (mut store, _dir) = store_closed(vec![]);
    let action = Action::KillProcs {
        session: "s".into(),
        key: "g".into(),
        targets: vec![(10, 1), (11, 1)],
    };
    assert!(!store.still_valid(&action));
    store.set_procs(ProcTable {
        live: [(
            "s".to_string(),
            procs("s", 1, None, vec![group("g", &[(11, 1)])]),
        )]
        .into(),
        closed: vec![],
    });
    assert!(store.still_valid(&action));
    // The same pid with another start time is another process.
    store.set_procs(ProcTable {
        live: [(
            "s".to_string(),
            procs("s", 1, None, vec![group("g", &[(11, 2)])]),
        )]
        .into(),
        closed: vec![],
    });
    assert!(!store.still_valid(&action));
}

fn leftovers() -> ProcTable {
    ProcTable {
        live: HashMap::new(),
        closed: vec![procs("b", 2, Some("/p/b"), vec![group("g", &[(12, 1)])])],
    }
}

#[test]
fn leftovers_keep_the_widget_visible_without_live_sessions() {
    let (mut store, _dir) = store_closed(vec![]);
    let t0 = SystemTime::now();
    store.refresh(t0);
    store.refresh(t0 + Duration::from_secs(60));
    assert!(store.hidden());
    store.set_procs(leftovers());
    assert!(
        !store.hidden(),
        "a scan that finds leftovers shows it again"
    );
    store.refresh(t0 + Duration::from_secs(120));
    assert!(!store.hidden());
    // The leftovers go: the 30 s timer starts over.
    store.set_procs(ProcTable::default());
    store.refresh(t0 + Duration::from_secs(130));
    assert!(!store.hidden());
    store.refresh(t0 + Duration::from_secs(170));
    assert!(store.hidden());
}

#[test]
fn old_leftover_session_keeps_its_closed_entry_across_restart_and_refresh() {
    let day = 24 * 3600;
    let (mut store, _dir) = store_closed(vec![closed("old", day + 3600), closed("new", 10)]);
    store.refresh(SystemTime::now());
    // No scan result yet: nothing is pruned.
    assert_eq!(store.snapshot().closed.len(), 2);
    store.set_procs(ProcTable {
        live: HashMap::new(),
        closed: vec![procs("old", 1, None, vec![group("g", &[(12, 1)])])],
    });
    store.refresh(SystemTime::now());
    let mut ids: Vec<String> = store
        .snapshot()
        .closed
        .iter()
        .map(|c| c.id.clone())
        .collect();
    ids.sort();
    assert_eq!(ids, ["new", "old"]);
    let old_row = store
        .snapshot()
        .closed_rows
        .into_iter()
        .find(|r| r.id == "old")
        .unwrap();
    assert_eq!(old_row.title, "title old");
    // Once its leftovers are gone, the age limit applies again.
    store.set_procs(ProcTable::default());
    store.refresh(SystemTime::now());
    assert_eq!(store.snapshot().closed.len(), 1);
}
