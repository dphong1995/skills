//! Clicks on the process controls: opening a list and the two-step kill.

use std::time::{Duration, SystemTime};

use claude_sessions::hits::{Click, ClickEnv, PanelChange, Target};
use claude_sessions_core::action::Action;
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::procs::{GroupKind, ProcGroup, SessionProcs};
use claude_sessions_core::store::Snapshot;

use crate::support::snapshot;

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

fn snap_with_group() -> Snapshot {
    let mut snap = snapshot(vec![]);
    snap.procs.insert(
        "a".into(),
        SessionProcs {
            session: "a".into(),
            commands: vec![group("g", &[(10, 1), (11, 2)])],
            claude_and_mcp: None,
            pwd: None,
            started_us: 0,
        },
    );
    snap
}

fn click(target: &Target, snap: &Snapshot, panel: &PanelState, now: SystemTime) -> Click {
    target.click(&ClickEnv {
        snap,
        panel,
        now,
        text: "",
        pr_prompt: "",
    })
}

fn kill() -> Target {
    Target::Kill {
        session: "a".into(),
        key: "g".into(),
    }
}

#[test]
fn kill_needs_two_clicks_within_three_seconds() {
    let snap = snap_with_group();
    let mut panel = PanelState::new();
    let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let first = click(&kill(), &snap, &panel, t0);
    assert_eq!(first, Click::Panel(PanelChange::ArmKill("g".into())));
    let Click::Panel(change) = first else {
        unreachable!()
    };
    change.apply(&mut panel, &snap, t0);
    // Too late: arms again instead of killing.
    let late = t0 + Duration::from_secs(4);
    assert_eq!(
        click(&kill(), &snap, &panel, late),
        Click::Panel(PanelChange::ArmKill("g".into()))
    );
    assert_eq!(
        click(&kill(), &snap, &panel, t0 + Duration::from_secs(2)),
        Click::Action(Action::KillProcs {
            session: "a".into(),
            key: "g".into(),
            targets: vec![(10, 1), (11, 2)],
        })
    );
}

#[test]
fn kill_of_a_group_that_vanished_is_stale() {
    let snap = snapshot(vec![]);
    let mut panel = PanelState::new();
    let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    panel.kill_armed.insert("g".into(), t0);
    assert_eq!(click(&kill(), &snap, &panel, t0), Click::Stale);
}

#[test]
fn toggle_procs_opens_and_closes() {
    let snap = snapshot(vec![]);
    let mut panel = PanelState::new();
    let now = SystemTime::UNIX_EPOCH;
    let toggle = || {
        click(
            &Target::ToggleProcs("a".into()),
            &snap,
            &PanelState::new(),
            now,
        )
    };
    assert_eq!(toggle(), Click::Panel(PanelChange::ToggleProcs("a".into())));
    PanelChange::ToggleProcs("a".into()).apply(&mut panel, &snap, now);
    assert!(panel.open_procs.contains("a"));
    PanelChange::ToggleProcs("a".into()).apply(&mut panel, &snap, now);
    assert!(panel.open_procs.is_empty());
    panel.open_procs.insert("a".into());
    panel.kill_armed.insert("g".into(), now);
    panel.on_collapse();
    assert!(panel.open_procs.is_empty() && panel.kill_armed.is_empty());
}
