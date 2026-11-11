use crate::support::{FakeMeasure, snapshot};
use claude_sessions::hits::{Click, ClickEnv, PanelChange, Target};
use claude_sessions::views::{Built, View, build};
use claude_sessions_core::feed::{BlockState, FeedItem, FeedOption, FeedQuestion};
use claude_sessions_core::layout::{self, expanded_layout, expanded_size};
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::scene::{DrawCmd, Node, Scene};
use claude_sessions_core::store::{Action, SendKey, SendKind, SendState, Session, Snapshot};
use claude_sessions_core::transcript::{Pending, PendingState};
use std::time::{Duration, SystemTime};

fn approval(id: &str) -> Session {
    let mut s = Session::new(id, id, "/work/repo", "waiting");
    s.waiting_for = Some("permission prompt".into());
    s.pending = Some(PendingState::One(Pending {
        tool_id: "t1".into(),
        tool: "Bash".into(),
        text: "git status".into(),
    }));
    s
}

fn question(id: &str) -> Session {
    let mut s = Session::new(id, id, "/work/ocr", "waiting");
    s.question = Some(FeedQuestion {
        request_id: "R1".into(),
        created_at: SystemTime::now(),
        session_id: Some(id.into()),
        questions: vec![FeedItem {
            prompt: "Cache?".into(),
            header: None,
            multi_select: false,
            options: vec![FeedOption {
                id: "o1".into(),
                label: "Redis".into(),
                description: None,
            }],
        }],
    });
    s.question_live = true;
    s.block = Some(BlockState::Form);
    s
}

fn finished(id: &str) -> Session {
    let mut s = Session::new(id, id, "/work/front", "idle");
    s.last_reply = Some("Đã sửa xong".into());
    s.branch = Some("main".into());
    s
}

fn busy(id: &str) -> Session {
    let mut s = Session::new(id, id, "/work/v2", "busy");
    s.activity = Some("Bash npm test".into());
    s.todo = Some(claude_sessions_core::tasks::Todo {
        done: 3,
        total: 5,
        current: Some("auth".into()),
    });
    s
}

fn background(id: &str) -> Session {
    let mut s = Session::new(id, id, "/work/skills", "busy");
    s.background_agents = Some(2);
    s
}

/// One of each: needs-you (approval, question), done, running (busy, waiting on background agents).
fn mixed() -> (Snapshot, PanelState) {
    let snap = snapshot(vec![
        busy("run"),
        finished("done"),
        approval("appr"),
        background("bg"),
        question("ask"),
    ]);
    let mut panel = PanelState::new();
    panel.hovering = true;
    (snap, panel)
}

fn render(snap: &Snapshot, panel: &PanelState) -> Built {
    let clock = |_: SystemTime| "14:30".to_string();
    build(&View {
        snap,
        panel,
        measure: &FakeMeasure,
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
        clock: &clock,
    })
}

fn card_keys(scene: &Scene) -> Vec<&str> {
    scene
        .nodes
        .iter()
        .filter(|n| n.key.0.starts_with("card-"))
        .map(|n| n.key.0.as_str())
        .collect()
}

fn texts(node: &Node) -> Vec<&str> {
    node.cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn symbols(node: &Node) -> Vec<&str> {
    node.cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Symbol { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

fn card<'a>(scene: &'a Scene, id: &str) -> &'a Node {
    scene
        .nodes
        .iter()
        .find(|n| n.key.0 == format!("card-{id}"))
        .unwrap()
}

fn targets(b: &Built, node: &Node) -> Vec<Target> {
    node.hits
        .iter()
        .map(|h| b.targets.get(h.id).unwrap().clone())
        .collect()
}

#[test]
fn one_node_per_card_in_sections_order_with_stable_keys() {
    let (snap, panel) = mixed();
    let built = render(&snap, &panel);
    let want: Vec<String> = snap
        .sections()
        .into_iter()
        .flat_map(|(_, ids)| ids)
        .map(|id| format!("card-{id}"))
        .collect();
    assert_eq!(card_keys(&built.scene), want);
    assert_eq!(
        want,
        ["card-appr", "card-ask", "card-done", "card-run", "card-bg"]
    );
    let mut keys: Vec<_> = built.scene.nodes.iter().map(|n| &n.key).collect();
    keys.sort_by(|a, b| a.0.cmp(&b.0));
    keys.dedup();
    assert_eq!(keys.len(), built.scene.nodes.len(), "keys are unique");
    // Equal input, equal scene: nothing redraws on an idle tick.
    assert_eq!(render(&snap, &panel).scene, built.scene);
}

#[test]
fn nodes_use_the_size_model() {
    let (snap, panel) = mixed();
    let built = render(&snap, &panel);
    assert_eq!(built.scene.size, expanded_size(&snap, &panel, &FakeMeasure));
    let l = expanded_layout(&snap, &panel, &FakeMeasure);
    for item in &l.items {
        if let layout::Item::Card { id, y, layout } = item {
            let f = card(&built.scene, id).frame;
            assert_eq!((f.y, f.h), (*y, layout.height), "{id}");
        }
    }
}

#[test]
fn sections_have_titles_with_counts() {
    let (snap, panel) = mixed();
    let scene = render(&snap, &panel).scene;
    let title =
        |key: &str| texts(scene.nodes.iter().find(|n| n.key.0 == key).unwrap())[0].to_string();
    assert_eq!(title("section-needs"), "CẦN BẠN · 2");
    assert_eq!(title("section-done"), "XONG · 1");
    assert_eq!(title("section-running"), "ĐANG CHẠY · 2");
}

#[test]
fn state_text_sits_beside_the_title_and_every_line_is_kept() {
    let (snap, panel) = mixed();
    let scene = render(&snap, &panel).scene;
    assert!(texts(card(&scene, "appr")).contains(&"chờ duyệt · Bash"));
    assert!(texts(card(&scene, "ask")).contains(&"chờ trả lời · 1 câu hỏi"));
    assert!(texts(card(&scene, "bg")).contains(&"chờ 2 agent nền"));
    let run = card(&scene, "run");
    assert!(texts(run).contains(&"đang chạy"));
    assert!(texts(run).contains(&"Bash npm test"));
    assert!(texts(run).contains(&"3/5 · auth"));
    assert!(run.spinner.is_some(), "a running card spins");
    let done = card(&scene, "done");
    assert!(texts(done).contains(&"Đã sửa xong"));
    assert!(texts(done).contains(&"front · main"));
    assert!(card(&scene, "bg").spinner.is_none());
}

#[test]
fn cards_jump_and_quick_buttons_come_first_in_hit_order() {
    let (snap, panel) = mixed();
    let built = render(&snap, &panel);
    assert_eq!(
        targets(&built, card(&built.scene, "done")),
        [Target::Reply("done".into()), Target::Jump("done".into())]
    );
    assert_eq!(
        targets(&built, card(&built.scene, "run")),
        [Target::Stop("run".into()), Target::Jump("run".into())]
    );
    assert_eq!(
        targets(&built, card(&built.scene, "appr")),
        [Target::Jump("appr".into())]
    );
}

#[test]
fn a_card_in_flight_shows_it_and_has_no_active_button() {
    let (mut snap, panel) = mixed();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    snap.sends.insert(
        SendKey::new("done", SendKind::Reply),
        (SendState::Sending, now),
    );
    let built = render(&snap, &panel);
    let done = card(&built.scene, "done");
    assert!(done.spinner.is_some());
    assert_eq!(targets(&built, done), [Target::Jump("done".into())]);

    // A busy card already spins at its icon: the button falls back to a static glyph.
    snap.sends.insert(
        SendKey::new("run", SendKind::Stop),
        (SendState::Sending, now),
    );
    let run = render(&snap, &panel);
    assert!(symbols(card(&run.scene, "run")).contains(&"ellipsis"));

    snap.sends.insert(
        SendKey::new("done", SendKind::Reply),
        (SendState::Sent, now),
    );
    snap.sends.insert(
        SendKey::new("run", SendKind::Stop),
        (SendState::Failed, now),
    );
    let after = render(&snap, &panel);
    assert!(symbols(card(&after.scene, "done")).contains(&"checkmark"));
    assert!(symbols(card(&after.scene, "run")).contains(&"exclamationmark.triangle.fill"));
}

#[test]
fn collapsed_pill_is_one_node_sized_by_the_model() {
    let (snap, _) = mixed();
    let built = render(&snap, &PanelState::default());
    assert_eq!(built.scene.nodes.len(), 1);
    assert_eq!(built.scene.nodes[0].key.0, "pill");
    assert_eq!(built.scene.size.w, layout::collapsed_width(&snap));
    let counts = texts(&built.scene.nodes[0]);
    assert_eq!(counts, ["2", "1", "2"]);
    let empty = render(&snapshot(vec![]), &PanelState::default());
    assert_eq!(empty.scene.nodes.len(), 1);
}

#[test]
fn hit_ids_are_stable_and_resolve_to_actions_and_panel_changes() {
    let (snap, panel) = mixed();
    let a = render(&snap, &panel);
    let b = render(&snap, &panel);
    let ids = |x: &Built| -> Vec<_> { card(&x.scene, "done").hits.iter().map(|h| h.id).collect() };
    assert_eq!(ids(&a), ids(&b));
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    let env = |panel: &'_ PanelState| -> Click {
        Target::Stop("run".into()).click(&ClickEnv {
            snap: &snap,
            panel,
            now,
            text: "",
            pr_prompt: "",
        })
    };
    assert_eq!(
        env(&panel),
        Click::Panel(PanelChange::ArmStop("run".into()))
    );
    let mut armed = panel.clone();
    PanelChange::ArmStop("run".into()).apply(&mut armed, &snap, now);
    assert_eq!(env(&armed), Click::Action(Action::Stop("run".into())));
    assert_eq!(
        Target::Jump("x".into()).click(&ClickEnv {
            snap: &snap,
            panel: &panel,
            now,
            text: "",
            pr_prompt: ""
        }),
        Click::Action(Action::Jump("x".into()))
    );
}

#[test]
fn reply_editor_adds_a_field_and_a_send_button() {
    let (snap, mut panel) = mixed();
    panel.editing = Some(claude_sessions_core::panel_state::Editing {
        id: "done".into(),
        kind: claude_sessions_core::panel_state::EditKind::Reply,
        tool_id: None,
    });
    let built = render(&snap, &panel);
    let done = card(&built.scene, "done");
    assert_eq!(done.fields.len(), 1);
    assert!(targets(&built, done).contains(&Target::ReplySend("done".into())));
    assert_eq!(built.scene.size, expanded_size(&snap, &panel, &FakeMeasure));
}
