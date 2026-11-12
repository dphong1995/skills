use crate::support::{FakeMeasure, snapshot};
use claude_sessions::hits::{Click, ClickEnv, PanelChange, Target};
use claude_sessions::theme;
use claude_sessions::views::{Built, View, build};
use claude_sessions_core::feed::{BlockState, FeedItem, FeedOption, FeedQuestion};
use claude_sessions_core::layout::{self, expanded_layout, expanded_size};
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::scene::{Align, DrawCmd, Material, Node, Scene, ThemeColor};
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
        .chain(node.cmds.iter().filter_map(|c| match c {
            DrawCmd::Rich { runs, .. } => Some(runs[0].text.as_str()),
            _ => None,
        }))
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
    let title = |key: &str| {
        let t = texts(scene.nodes.iter().find(|n| n.key.0 == key).unwrap());
        (t[0].to_string(), t[1].to_string())
    };
    assert_eq!(title("section-needs"), ("Cần bạn".into(), "2".into()));
    assert_eq!(title("section-done"), ("Xong".into(), "1".into()));
    assert_eq!(title("section-running"), ("Đang chạy".into(), "2".into()));
}

#[test]
fn state_text_sits_beside_the_title_and_every_line_is_kept() {
    let (snap, panel) = mixed();
    let scene = render(&snap, &panel).scene;
    assert!(texts(card(&scene, "appr")).contains(&"chờ duyệt · Bash"));
    assert!(texts(card(&scene, "ask")).contains(&"chờ trả lời · 1 câu hỏi"));
    assert!(texts(card(&scene, "bg")).contains(&"chờ 2 agent nền"));
    let run = card(&scene, "run");
    // The section already says so: the generic labels are not repeated on the card.
    assert!(!texts(run).contains(&"đang chạy"));
    assert!(texts(run).contains(&"Bash npm test"));
    assert!(texts(run).contains(&"3/5 · auth"));
    assert!(run.spinner.is_some(), "a running card spins");
    let done = card(&scene, "done");
    assert!(!texts(done).contains(&"xong"));
    assert!(texts(done).contains(&"Đã sửa xong"));
    assert!(texts(done).contains(&"front · main"));
    assert!(card(&scene, "bg").spinner.is_none());
}

#[test]
fn cards_jump_and_quick_buttons_come_first_in_hit_order() {
    let (snap, mut panel) = mixed();
    // The quick buttons are drawn only on the hovered row.
    panel.hovered_row = Some("done".into());
    let built = render(&snap, &panel);
    assert_eq!(
        targets(&built, card(&built.scene, "done")),
        [Target::Reply("done".into()), Target::Jump("done".into())]
    );
    panel.hovered_row = Some("run".into());
    let built = render(&snap, &panel);
    assert_eq!(
        targets(&built, card(&built.scene, "run")),
        [Target::Stop("run".into()), Target::Jump("run".into())]
    );
    assert_eq!(
        targets(&built, card(&built.scene, "appr")),
        [
            Target::Approve {
                session: "appr".into(),
                tool_id: "t1".into()
            },
            Target::Deny {
                session: "appr".into(),
                tool_id: "t1".into()
            },
            Target::Jump("appr".into())
        ]
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
fn pill_draws_no_quota_ring() {
    let (mut snap, _) = mixed();
    snap.quota = Some(claude_sessions_core::quota::Quota {
        five_hour: 97,
        five_hour_reset: SystemTime::now(),
        weekly: 10,
        weekly_reset: SystemTime::now(),
        forecast: None,
    });
    let built = render(&snap, &PanelState::default());
    let node = &built.scene.nodes[0];
    assert!(!node.cmds.iter().any(|c| matches!(c, DrawCmd::Arc { .. })));
    assert!(texts(node).iter().all(|t| !t.ends_with('%')));
    assert_eq!(built.scene.size.w, layout::collapsed_width(&snap));
    let mut empty = snapshot(vec![]);
    empty.quota = snap.quota.clone();
    let built = render(&empty, &PanelState::default());
    let node = &built.scene.nodes[0];
    assert!(!node.cmds.iter().any(|c| matches!(c, DrawCmd::Arc { .. })));
    assert_eq!(texts(node), ["–"]);
    assert_eq!(built.scene.size.w, layout::COLLAPSED.w);
}

#[test]
fn the_quota_ring_is_an_arc_swept_by_the_percent() {
    for (percent, sweep) in [(85, 306.0), (100, 360.0), (130, 360.0)] {
        let built = fixture_built_with("quota-1-normal", |snap| {
            snap.quota
                .get_or_insert(claude_sessions_core::quota::Quota {
                    five_hour: 0,
                    five_hour_reset: now(),
                    weekly: 40,
                    weekly_reset: now(),
                    forecast: None,
                })
                .five_hour = percent;
        });
        let header = built
            .scene
            .nodes
            .iter()
            .find(|n| n.key.0 == "header")
            .unwrap();
        let arcs: Vec<(f64, f64)> = header
            .cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Arc { start, end, .. } => Some((*start, *end)),
                _ => None,
            })
            .collect();
        assert!(arcs.contains(&(0.0, 360.0)), "{percent}: track in {arcs:?}");
        assert!(arcs.contains(&(0.0, sweep)), "{percent}: sweep in {arcs:?}");
    }
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

// ---- 6b: the blocks ----

use claude_sessions_core::closed_sessions::ClosedSession;
use claude_sessions_core::code_rabbit::Rabbit;
use claude_sessions_core::layout::{Block, TextMeasure, card_layout};
use claude_sessions_core::panel_state::{EditKind, Editing};
use claude_sessions_core::pull_requests::{PRRef, PRStatus};
use claude_sessions_core::scene::Rect as GRect;
use claude_sessions_core::scene::Run;
use claude_sessions_core::store::{ItemTone, LinkItem, LinkKind, LinkSection, SessionLink};
use url::Url;

const NOW: Duration = Duration::from_secs(1_000_000);

fn open() -> PanelState {
    let mut p = PanelState::new();
    p.hovering = true;
    p
}

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW
}

fn bottom(cmd: &DrawCmd) -> f64 {
    match cmd {
        DrawCmd::RoundedRect { rect, .. }
        | DrawCmd::Text { rect, .. }
        | DrawCmd::Rich { rect, .. }
        | DrawCmd::Symbol { rect, .. }
        | DrawCmd::Image { rect, .. } => rect.y + rect.h,
        DrawCmd::Line { from, to, .. } => from.y.max(to.y),
        DrawCmd::Arc { center, radius, .. } => center.y + radius,
    }
}

/// The only card of `snap`: its node and the layout the size model gave it.
fn only_card(snap: &Snapshot, panel: &PanelState) -> (Built, layout::CardLayout) {
    let built = render(snap, panel);
    let l = card_layout(&snap.sessions[0], snap, panel, &FakeMeasure);
    (built, l)
}

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

fn pr(id: i64, state: &str) -> PRStatus {
    PRStatus {
        ref_: PRRef {
            workspace: "w".into(),
            repo: "r".into(),
            id,
        },
        url: url(&format!("https://bitbucket.org/w/r/pull-requests/{id}")),
        state: state.into(),
        reviewers: 5,
        approved: 0,
        changes_requested: 1,
        comments: 23,
        commit: None,
        source_repo: None,
        branch: None,
        title: None,
        last_other: None,
        approvers: Default::default(),
        author: None,
        rabbit: None,
        rabbit_status: None,
        rabbit_since: None,
        rabbit_threads: None,
        rabbit_others_pending: false,
    }
}

fn two_questions() -> Session {
    let mut s = question("ask");
    let q = s.question.as_mut().unwrap();
    let opt = |l: &str, d: Option<&str>| FeedOption {
        id: l.into(),
        label: l.into(),
        description: d.map(Into::into),
    };
    q.questions = vec![
        FeedItem {
            prompt: "Cache?".into(),
            header: Some("Cache".into()),
            multi_select: false,
            options: vec![opt("Redis", Some("nhanh")), opt("Memory", None)],
        },
        FeedItem {
            prompt: "TTL?".into(),
            header: None,
            multi_select: true,
            options: vec![opt("1h", None), opt("1d", None), opt("never", None)],
        },
    ];
    s
}

#[test]
fn approval_buttons_resolve_to_approve_and_deny() {
    let snap = snapshot(vec![approval("appr")]);
    let panel = open();
    let built = render(&snap, &panel);
    let t = targets(&built, card(&built.scene, "appr"));
    let approve = Target::Approve {
        session: "appr".into(),
        tool_id: "t1".into(),
    };
    let deny = Target::Deny {
        session: "appr".into(),
        tool_id: "t1".into(),
    };
    assert_eq!(
        t,
        [approve.clone(), deny.clone(), Target::Jump("appr".into())]
    );
    let env = |text| ClickEnv {
        snap: &snap,
        panel: &panel,
        now: now(),
        text,
        pr_prompt: "",
    };
    assert_eq!(
        approve.click(&env("")),
        Click::Action(Action::Approve {
            session: "appr".into(),
            tool_id: "t1".into()
        })
    );
    assert!(matches!(
        deny.click(&env("")),
        Click::Panel(PanelChange::OpenEditor(_))
    ));
    let texts = texts(card(&built.scene, "appr"));
    assert!(texts.contains(&"git status"));
    assert!(texts.contains(&"Cho phép") && texts.contains(&"Từ chối…"));
}

fn approval_with(command: &str) -> Session {
    let mut s = approval("appr");
    if let Some(PendingState::One(p)) = &mut s.pending {
        p.text = command.into();
    }
    s
}

fn has_approve_or_deny(t: &[Target]) -> bool {
    t.iter()
        .any(|t| matches!(t, Target::Approve { .. } | Target::Deny { .. }))
}

#[test]
fn a_long_command_has_no_buttons_until_xem_du_is_clicked() {
    let long = "x".repeat(400);
    let mut snap = snapshot(vec![approval_with(&long)]);
    let panel = open();
    let built = render(&snap, &panel);
    let node = card(&built.scene, "appr");
    let t = targets(&built, node);
    assert!(texts(node).contains(&"Xem đủ"));
    assert!(!texts(node).contains(&"Cho phép"));
    assert!(!has_approve_or_deny(&t));
    let show = Target::ShowCommand {
        session: "appr".into(),
        tool_id: "t1".into(),
    };
    assert!(t.contains(&show));
    let env = ClickEnv {
        snap: &snap,
        panel: &panel,
        now: now(),
        text: "",
        pr_prompt: "",
    };
    assert_eq!(
        show.click(&env),
        Click::Action(Action::ShowCommand {
            session: "appr".into(),
            tool_id: "t1".into()
        })
    );

    snap.shown_commands.insert("t1".into());
    let built = render(&snap, &panel);
    let node = card(&built.scene, "appr");
    let t = targets(&built, node);
    assert!(texts(node).contains(&"Cho phép") && !texts(node).contains(&"Xem đủ"));
    assert!(has_approve_or_deny(&t) && !t.contains(&show));
}

#[test]
fn a_too_long_command_and_an_unclear_pending_only_jump_to_cmux() {
    let snap = snapshot(vec![approval_with(&"x".repeat(1300))]);
    let built = render(&snap, &open());
    let node = card(&built.scene, "appr");
    assert!(texts(node).contains(&"Lệnh dài, bấm để duyệt trong cmux"));
    let t = targets(&built, node);
    assert!(!has_approve_or_deny(&t));
    assert!(t.contains(&Target::Jump("appr".into())));

    let mut unclear = approval("appr");
    unclear.pending = Some(PendingState::Unclear { count: 2 });
    let snap = snapshot(vec![unclear]);
    let built = render(&snap, &open());
    let node = card(&built.scene, "appr");
    assert!(texts(node).contains(&"Cần duyệt trong cmux"));
    assert!(!has_approve_or_deny(&targets(&built, node)));
    // The card body and the line both jump.
    assert!(targets(&built, node).contains(&Target::Jump("appr".into())));
    let l = card_layout(&snap.sessions[0], &snap, &open(), &FakeMeasure);
    assert!(l.find(Block::Approval).is_some() && l.find(Block::Reply).is_none());
}

#[test]
fn the_jump_line_shows_its_send_state_and_takes_no_click_while_sending() {
    let mut unclear = approval("appr");
    unclear.pending = Some(PendingState::Unclear { count: 2 });
    let mut snap = snapshot(vec![unclear]);
    let key = SendKey::new("appr", SendKind::Jump);
    snap.sends.insert(key.clone(), (SendState::Sending, now()));
    let built = render(&snap, &open());
    let node = card(&built.scene, "appr");
    assert!(texts(node).contains(&"Đang gửi…") && !texts(node).contains(&"Cần duyệt trong cmux"));
    // Only the card body jumps; the line does not take a second click.
    let jumps = |t: Vec<Target>| t.iter().filter(|t| matches!(t, Target::Jump(_))).count();
    assert_eq!(jumps(targets(&built, node)), 1);

    snap.sends.insert(key, (SendState::Failed, now()));
    let built = render(&snap, &open());
    let node = card(&built.scene, "appr");
    assert!(texts(node).contains(&"Lỗi — bấm thử lại"));
    assert_eq!(jumps(targets(&built, node)), 2);
}

#[test]
fn denying_adds_the_reason_field_and_a_send_that_resolves_to_deny() {
    let snap = snapshot(vec![approval("appr")]);
    let mut panel = open();
    panel.editing = Some(Editing {
        id: "appr".into(),
        kind: EditKind::Deny,
        tool_id: Some("t1".into()),
    });
    let (built, l) = only_card(&snap, &panel);
    let node = card(&built.scene, "appr");
    assert_eq!(node.fields.len(), 1);
    let send = Target::DenySend {
        session: "appr".into(),
        tool_id: "t1".into(),
    };
    assert!(targets(&built, node).contains(&send));
    let field = node.fields[0].rect;
    assert!(field.y + field.h <= l.height, "the field fits the card");
    let click = send.click(&ClickEnv {
        snap: &snap,
        panel: &panel,
        now: now(),
        text: "no",
        pr_prompt: "",
    });
    assert_eq!(
        click,
        Click::Action(Action::Deny {
            session: "appr".into(),
            tool_id: "t1".into(),
            reason: "no".into()
        })
    );
}

#[test]
fn an_approval_in_flight_shows_it_and_has_no_active_button() {
    let mut snap = snapshot(vec![approval("appr")]);
    let mut key = SendKey::new("appr", SendKind::Approve);
    key.detail = Some("t1".into());
    snap.sends.insert(key.clone(), (SendState::Sending, now()));
    let built = render(&snap, &open());
    let node = card(&built.scene, "appr");
    assert!(texts(node).contains(&"Đang gửi…"));
    assert!(node.spinner.is_some());
    let t = targets(&built, node);
    assert!(!t.iter().any(|t| matches!(t, Target::Approve { .. })));
    assert!(t.iter().any(|t| matches!(t, Target::Deny { .. })));

    snap.sends.insert(key.clone(), (SendState::Failed, now()));
    let failed = render(&snap, &open());
    assert!(texts(card(&failed.scene, "appr")).contains(&"Lỗi — bấm thử lại"));
    assert!(
        targets(&failed, card(&failed.scene, "appr"))
            .iter()
            .any(|t| matches!(t, Target::Approve { .. })),
        "a failed send can be retried"
    );
}

#[test]
fn two_questions_have_one_option_hit_per_option() {
    let snap = snapshot(vec![two_questions()]);
    let (built, l) = only_card(&snap, &open());
    let t = targets(&built, card(&built.scene, "ask"));
    let picks: Vec<_> = t
        .iter()
        .filter_map(|t| match t {
            Target::PickOption {
                question, label, ..
            } => Some((*question, label.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        picks,
        [
            (0, "Redis"),
            (0, "Memory"),
            (1, "1h"),
            (1, "1d"),
            (1, "never")
        ]
    );
    let others = t
        .iter()
        .filter(|t| matches!(t, Target::OpenOther { .. }))
        .count();
    assert_eq!(others, 2, "one `Khác…` row per question");
    let node = card(&built.scene, "ask");
    assert!(texts(node).contains(&"còn 1:59") || texts(node).iter().any(|t| t.starts_with("còn ")));
    assert!(node.cmds.iter().all(|c| bottom(c) <= l.height));
    // `Gửi` stays inactive until every question has an answer.
    assert!(!t.contains(&Target::Answer("ask".into())));
}

/// Option rows drawn as picked: the radio is an `Accent` disc.
fn picked_rows(node: &Node) -> usize {
    node.cmds
        .iter()
        .filter(|c| {
            matches!(c, DrawCmd::RoundedRect { rect, fill: Some(f), .. }
                if f.token == ThemeColor::Accent && rect.w == 14.0)
        })
        .count()
}

#[test]
fn options_are_numbered_badge_rows_and_a_multi_select_marks_several() {
    let snap = snapshot(vec![two_questions()]);
    let mut panel = open();
    let q = snap.sessions[0].question.clone().unwrap();
    let node = |panel: &PanelState| card(&render(&snap, panel).scene, "ask").clone();
    assert_eq!(picked_rows(&node(&panel)), 0);
    assert!(!symbols(&node(&panel)).contains(&"circle"));
    assert!(texts(&node(&panel)).contains(&"1") && texts(&node(&panel)).contains(&"5"));
    panel.pick(&q, 1, "1h");
    panel.pick(&q, 1, "1d");
    assert_eq!(picked_rows(&node(&panel)), 2, "a multi-select keeps both");
    panel.pick(&q, 0, "Redis");
    panel.pick(&q, 0, "Memory");
    assert_eq!(picked_rows(&node(&panel)), 3, "a single-select keeps one");
    // The header chip stays.
    assert!(texts(&node(&panel)).contains(&"Cache"));
}

#[test]
fn picking_marks_the_option_and_activates_send() {
    let snap = snapshot(vec![question("ask")]);
    let mut panel = open();
    let q = snap.sessions[0].question.clone().unwrap();
    panel.pick(&q, 0, "Redis");
    let built = render(&snap, &panel);
    let node = card(&built.scene, "ask");
    assert_eq!(
        picked_rows(node),
        1,
        "the picked option has an Accent radio"
    );
    assert!(targets(&built, node).contains(&Target::Answer("ask".into())));
    let sending = {
        let mut snap = snap.clone();
        snap.sends.insert(
            SendKey::new("ask", SendKind::Answer),
            (SendState::Sending, now()),
        );
        render(&snap, &panel)
    };
    assert!(
        !targets(&sending, card(&sending.scene, "ask")).contains(&Target::Answer("ask".into()))
    );
}

#[test]
fn the_other_field_opens_under_its_question() {
    let snap = snapshot(vec![two_questions()]);
    let mut panel = open();
    panel.other_key = Some("ask#1".into());
    panel.editing = Some(Editing {
        id: "ask".into(),
        kind: EditKind::Answer,
        tool_id: None,
    });
    let (built, l) = only_card(&snap, &panel);
    let node = card(&built.scene, "ask");
    assert_eq!(node.fields.len(), 1);
    assert_eq!(node.fields[0].key, "other-ask#1");
    assert!(node.fields[0].rect.y + node.fields[0].rect.h <= l.height);
}

#[test]
fn the_other_field_needs_an_answer_edit() {
    let snap = snapshot(vec![two_questions()]);
    let reply = Editing {
        id: "ask".into(),
        kind: EditKind::Reply,
        tool_id: None,
    };
    for editing in [None, Some(reply)] {
        // The same panel without `other_key`: a reply edit has its own field, so the baseline is per case.
        let mut plain = open();
        plain.editing = editing.clone();
        let (_, base) = only_card(&snap, &plain);
        let mut panel = open();
        panel.other_key = Some("ask#1".into());
        panel.editing = editing;
        let (built, l) = only_card(&snap, &panel);
        assert!(
            card(&built.scene, "ask")
                .fields
                .iter()
                .all(|f| f.key != "other-ask#1"),
            "no `Khác…` field without an answer edit"
        );
        assert_eq!(l.height, base.height, "no FIELD_H in the height");
    }
}

#[test]
fn sent_and_fallback_questions_are_one_note() {
    let mut s = question("ask");
    s.block = Some(BlockState::Sent);
    let sent = render(&snapshot(vec![s.clone()]), &open());
    assert!(texts(card(&sent.scene, "ask")).contains(&"Đã gửi"));
    assert!(
        !targets(&sent, card(&sent.scene, "ask"))
            .iter()
            .any(|t| matches!(t, Target::PickOption { .. }))
    );
    s.block = Some(BlockState::Fallback);
    let fb = render(&snapshot(vec![s]), &open());
    assert!(texts(card(&fb.scene, "ask")).contains(&"Trả lời trong cmux"));
}

fn pr_session() -> Session {
    let mut s = finished("done");
    let mut open = pr(7571, "OPEN");
    open.rabbit = Some(Rabbit::Reviewed(Some(2)));
    let mut waiting = pr(7580, "OPEN");
    waiting.rabbit = Some(Rabbit::AwaitingApproval);
    waiting.changes_requested = 0;
    s.prs = vec![open, waiting, pr(7567, "MERGED")];
    s
}

#[test]
fn pr_rows_show_counts_the_rabbit_chip_and_their_buttons() {
    let mut snap = snapshot(vec![pr_session()]);
    snap.pr_stale = true;
    snap.pr_stale_since = Some(now() - Duration::from_secs(7 * 60));
    let built = render(&snap, &open());
    let node = card(&built.scene, "done");
    let t = texts(node);
    for want in [
        "#7571",
        "#7580",
        "#7567",
        "0/5",
        "23",
        "cũ 7m",
        "Nhờ Claude",
        "Nhắc duyệt",
    ] {
        assert!(t.contains(&want), "{want} in {t:?}");
    }
    assert!(symbols(node).contains(&"hare"));
    let hits = targets(&built, node);
    let ref_ = |id| PRRef {
        workspace: "w".into(),
        repo: "r".into(),
        id,
    };
    assert!(hits.contains(&Target::AskClaude {
        session: "done".into(),
        pr: ref_(7571)
    }));
    assert!(hits.contains(&Target::Nudge {
        session: "done".into(),
        pr: ref_(7580)
    }));
    // The merged PR has neither button; the awaiting one hides `Nhờ Claude` only once its threads are answered.
    assert!(!hits.contains(&Target::Nudge {
        session: "done".into(),
        pr: ref_(7571)
    }));
}

#[test]
fn a_pr_button_in_flight_shows_it_and_has_no_hit() {
    let mut snap = snapshot(vec![pr_session()]);
    let mut key = SendKey::new("done", SendKind::AskClaude);
    key.pr = Some(PRRef {
        workspace: "w".into(),
        repo: "r".into(),
        id: 7571,
    });
    snap.sends.insert(key, (SendState::Sending, now()));
    let built = render(&snap, &open());
    let node = card(&built.scene, "done");
    assert!(texts(node).contains(&"Đang gửi…"));
    assert!(node.spinner.is_some());
    assert!(!targets(&built, node).iter().any(|t| matches!(
        t,
        Target::AskClaude { pr, .. } if pr.id == 7571
    )));
}

fn tickets() -> Session {
    let mut s = finished("done");
    let item = |code: &str, icon: bool| LinkItem {
        text: code.into(),
        url: url(&format!("https://x.atlassian.net/browse/{code}")),
        kind: LinkKind::Ticket,
        code: code.into(),
        title: Some("Fix parser".into()),
        status: Some("In Progress".into()),
        tone: Some(ItemTone::Doing),
        icon_url: icon.then(|| url("https://x.atlassian.net/icon.png")),
        help: None,
    };
    s.links = vec![SessionLink {
        label: "ABC-12 In Progress".into(),
        url: url("https://x.atlassian.net/browse/ABC-12"),
        help: None,
        icon_url: Some(url("https://x.atlassian.net/icon.png")),
        tone: Some(ItemTone::Doing),
    }];
    s.menu = vec![LinkSection {
        title: "Ticket".into(),
        items: vec![item("ABC-12", true), item("ABC-9", false)],
    }];
    s.menu_needs_jira = true;
    s
}

#[test]
fn ticket_chips_and_the_menu_count_have_hits_and_an_icon_placeholder() {
    let snap = snapshot(vec![tickets()]);
    let built = render(&snap, &open());
    let node = card(&built.scene, "done");
    let t = texts(node);
    assert!(t.contains(&"ABC-12 In Progress") && t.contains(&"2"));
    let hits = targets(&built, node);
    assert!(hits.contains(&Target::Menu("done".into())));
    assert!(
        hits.iter()
            .any(|t| matches!(t, Target::Link { url, .. } if url.path() == "/browse/ABC-12"))
    );
    // The icon is a tinted square until `DrawCmd::Image` exists.
    let squares = node
        .cmds
        .iter()
        .filter(
            |c| matches!(c, DrawCmd::RoundedRect { rect, .. } if rect.w == 12.0 && rect.h == 12.0),
        )
        .count();
    assert_eq!(squares, 1);
}

#[test]
fn the_open_menu_lists_links_and_the_jira_connection() {
    let snap = snapshot(vec![tickets()]);
    let mut panel = open();
    panel.open_menus.insert("done".into());
    let (built, l) = only_card(&snap, &panel);
    let node = card(&built.scene, "done");
    let t = targets(&built, node);
    let links = t
        .iter()
        .filter(|t| matches!(t, Target::Link { url, .. } if url.path().starts_with("/browse/") ))
        .count();
    assert_eq!(links, 2, "the two items; the chips are hidden while open");
    assert!(t.contains(&Target::Connect(
        claude_sessions_core::credentials::Service::Jira
    )));
    assert!(texts(node).contains(&"Kết nối Jira…"));
    assert!(node.cmds.iter().all(|c| bottom(c) <= l.height));
}

/// The hits of a card, with their targets.
fn hits_of<'a>(b: &'a Built, node: &'a Node) -> Vec<(GRect, &'a Target)> {
    node.hits
        .iter()
        .map(|h| (h.rect, b.targets.get(h.id).unwrap()))
        .collect()
}

/// The item hits of an open list (the chips are hidden while it is open).
fn item_hits(b: &Built, node: &Node) -> Vec<GRect> {
    hits_of(b, node)
        .into_iter()
        .filter(|(_, t)| matches!(t, Target::Link { .. }))
        .map(|(r, _)| r)
        .collect()
}

fn link_item(code: &str, kind: LinkKind, title: Option<&str>, status: Option<&str>) -> LinkItem {
    LinkItem {
        text: code.into(),
        url: url(&format!("https://x.atlassian.net/browse/{code}")),
        kind,
        code: code.into(),
        title: title.map(Into::into),
        status: status.map(Into::into),
        tone: None,
        icon_url: None,
        help: None,
    }
}

/// An open session whose menu is `sections`, drawn.
fn open_list(sections: Vec<LinkSection>) -> Built {
    let mut s = finished("done");
    s.menu = sections;
    let snap = snapshot(vec![s]);
    let mut panel = open();
    panel.open_menus.insert("done".into());
    render(&snap, &panel)
}

fn section(title: &str, items: Vec<LinkItem>) -> LinkSection {
    LinkSection {
        title: title.into(),
        items,
    }
}

/// The rect of the text `want` drawn by the card.
fn text_rect(node: &Node, want: &str) -> GRect {
    node.cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::Text { rect, text, .. } if text == want => Some(*rect),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{want:?} is not drawn"))
}

fn mid_y(r: GRect) -> f64 {
    r.y + r.h / 2.0
}

#[test]
fn open_list_hides_the_ticket_chips() {
    let snap = snapshot(vec![tickets()]);
    let mut panel = open();
    // Closed: the chips as before.
    let built = render(&snap, &panel);
    let node = card(&built.scene, "done");
    assert!(texts(node).contains(&"ABC-12 In Progress"));
    assert!(!texts(node).iter().any(|t| t.starts_with("Ẩn danh sách")));
    // Open: only the toggle, and no item is in the chip row.
    panel.open_menus.insert("done".into());
    let (built, l) = only_card(&snap, &panel);
    let node = card(&built.scene, "done");
    let chips = l.find(Block::Chips).expect("the chip row");
    let row: Vec<_> = hits_of(&built, node)
        .into_iter()
        .filter(|(r, _)| r.y >= chips.y && r.y < chips.y + chips.h)
        .collect();
    assert_eq!(row.len(), 1, "{row:?}");
    assert_eq!(row[0].1, &Target::Menu("done".into()));
    assert!(texts(node).contains(&"Ẩn danh sách · 2"));
    assert!(!texts(node).contains(&"ABC-12 In Progress"));
    let toggle = row[0].0;
    let label = text_rect(node, "Ẩn danh sách · 2");
    assert!(label.x >= toggle.x && label.x + label.w <= toggle.x + toggle.w);
    assert!(
        node.cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::Symbol { name, .. } if name == "list.bullet"))
    );
}

#[test]
fn link_items_are_two_line_items_in_inset_lists() {
    let mut a = link_item("ABC-1", LinkKind::Ticket, Some("First"), Some("Confirming"));
    a.tone = Some(ItemTone::Doing);
    let b = link_item("ABC-2", LinkKind::Ticket, Some("Second"), Some("Done"));
    let c = link_item("#41", LinkKind::Pr, Some("A pull request"), Some("mở"));
    let built = open_list(vec![
        section("Bạn nhắc", vec![a, b]),
        section("PR", vec![c]),
    ]);
    let node = card(&built.scene, "done");
    let lists: Vec<GRect> = node
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                radius,
                fill: Some(f),
                ..
            } if f.token == ThemeColor::Inset && *radius == layout::PR_LIST_RADIUS => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(lists.len(), 2, "one inset list per section");
    assert_eq!(lists[0].h, 2.0 * layout::LINK_ITEM_H);
    assert_eq!(lists[1].h, layout::LINK_ITEM_H);
    assert_eq!(
        lists[1].y - (lists[0].y + lists[0].h),
        layout::LINK_SECTION_H
    );
    let items = item_hits(&built, node);
    assert_eq!(items.len(), 3);
    for r in &items {
        assert_eq!(r.h, layout::LINK_ITEM_H);
        assert!(
            lists.iter().any(|l| r.x >= l.x
                && r.x + r.w <= l.x + l.w
                && r.y >= l.y
                && r.y + r.h <= l.y + l.h),
            "{r:?} sits in a list"
        );
    }
    // Line 1: the title; line 2: the code and the status.
    let first = items[0];
    let title = text_rect(node, "First");
    assert!(title.y >= first.y + 4.0 && title.y + title.h <= first.y + 22.0);
    for word in ["ABC-1", "Confirming"] {
        let r = text_rect(node, word);
        assert!(
            mid_y(r) > first.y + 20.0 && mid_y(r) < first.y + 36.0,
            "{word}"
        );
    }
    assert!(text_rect(node, "Confirming").x > text_rect(node, "ABC-1").x + 20.0);
    // No status pill, no monospace.
    assert!(node.cmds.iter().all(|c| match c {
        DrawCmd::RoundedRect { fill: Some(f), .. } => f.alpha != theme::tag_fill(f.token).alpha,
        DrawCmd::Text { font, .. } => !font.mono,
        _ => true,
    }));
    // A hairline between the two items of the first list, none above a first item.
    let hairlines = node
        .cmds
        .iter()
        .filter(|c| {
            matches!(c, DrawCmd::RoundedRect { rect, .. }
                if rect.h == 0.5 && rect.x == layout::CARD_INSET + 38.0)
        })
        .count();
    assert_eq!(hairlines, 1);
}

#[test]
fn pr_item_dot_uses_the_badge_colour() {
    let pr = |id: &str, tone: ItemTone, status: &str| {
        let mut i = link_item(id, LinkKind::Pr, Some("T"), Some(status));
        i.tone = Some(tone);
        i
    };
    let mut ticket = link_item("K-1", LinkKind::Ticket, Some("T"), Some("Doing"));
    ticket.tone = Some(ItemTone::Doing);
    let plain = link_item("K-2", LinkKind::Ticket, Some("T"), Some("Unknown"));
    let built = open_list(vec![section(
        "X",
        vec![
            pr("#1", ItemTone::Open, "mở"),
            pr("#2", ItemTone::Merged, "đã merge"),
            pr("#3", ItemTone::Closed, "đã đóng"),
            ticket,
            plain,
        ],
    )]);
    let node = card(&built.scene, "done");
    let dots: Vec<ThemeColor> = node
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                radius,
                fill: Some(f),
                ..
            } if rect.w == 6.0 && rect.h == 6.0 && *radius == 3.0 => Some(f.token),
            _ => None,
        })
        .collect();
    assert_eq!(
        dots,
        vec![
            ThemeColor::Green,
            ThemeColor::Indigo,
            ThemeColor::Red,
            ThemeColor::Blue,
            ThemeColor::TextFaint
        ]
    );
}

#[test]
fn link_item_draws_one_centred_line_with_the_link_badge() {
    // An untracked PR link: no title, no status, so only the label, centred, beside a `link` badge.
    let item = link_item("PR #12", LinkKind::Link, None, None);
    let built = open_list(vec![section("PR", vec![item])]);
    let node = card(&built.scene, "done");
    let it = item_hits(&built, node)[0];
    assert!((mid_y(text_rect(node, "PR #12")) - mid_y(it)).abs() < 0.5);
    let badge = node
        .cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                radius,
                fill: Some(f),
                ..
            } if rect.w == 20.0 && rect.h == 20.0 && *radius == 6.0 => Some((*rect, f.token)),
            _ => None,
        })
        .expect("a 20 pt badge");
    assert_eq!(badge.1, ThemeColor::Fill);
    assert_eq!(badge.0.x, it.x + 10.0);
    assert!((mid_y(badge.0) - mid_y(it)).abs() < 0.5);
    let symbols: Vec<(&str, f64)> = node
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Symbol { name, rect, .. } if name == "link" => Some((name.as_str(), rect.w)),
            _ => None,
        })
        .collect();
    assert_eq!(symbols, vec![("link", 12.0)]);
    // No status dot on a one-line item.
    assert!(
        !node.cmds.iter().any(
            |c| matches!(c, DrawCmd::RoundedRect { rect, .. } if rect.w == 6.0 && rect.h == 6.0)
        )
    );
}

#[test]
fn pr_items_use_the_state_badge_symbols_of_the_pr_list() {
    let pr = |id: &str, tone: ItemTone| {
        let mut i = link_item(id, LinkKind::Pr, Some("T"), Some("x"));
        i.tone = Some(tone);
        i
    };
    let built = open_list(vec![section(
        "PR",
        vec![
            pr("#1", ItemTone::Open),
            pr("#2", ItemTone::Merged),
            pr("#3", ItemTone::Closed),
        ],
    )]);
    let node = card(&built.scene, "done");
    let symbols: Vec<&str> = node
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Symbol { name, rect, .. } if rect.w == 12.0 && name != "list.bullet" => {
                Some(name.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        symbols,
        vec!["arrow.triangle.branch", "arrow.triangle.merge", "xmark"]
    );
}

#[test]
fn untitled_ticket_shows_code_then_status() {
    let both = link_item("K-1", LinkKind::Ticket, None, Some("Confirming"));
    let bare = link_item("K-2", LinkKind::Ticket, None, None);
    let built = open_list(vec![section("X", vec![both, bare])]);
    let node = card(&built.scene, "done");
    let items = item_hits(&built, node);
    let (one, two) = (items[0], items[1]);
    // The code on line 1, the dot and the status on line 2.
    let code = text_rect(node, "K-1");
    assert!(code.y >= one.y + 4.0 && mid_y(code) < one.y + 22.0);
    let status = text_rect(node, "Confirming");
    assert!(mid_y(status) > one.y + 20.0);
    assert_eq!(status.x, code.x + 11.0, "the dot (6 pt) and a 5 pt gap");
    // Neither title nor status: the code alone, centred on the item.
    let alone = text_rect(node, "K-2");
    assert!((mid_y(alone) - mid_y(two)).abs() < 0.5);
}

// A status too long for the item is cut with an ellipsis inside it instead of running out of the card.
#[test]
fn a_long_status_stays_inside_the_item() {
    let status = "x".repeat(120);
    let item = link_item(
        "ABC-12",
        LinkKind::Ticket,
        Some("Fix parser"),
        Some(&status),
    );
    let built = open_list(vec![section("X", vec![item])]);
    let node = card(&built.scene, "done");
    let tag = text_rect(node, &status);
    let code = text_rect(node, "ABC-12");
    assert!(tag.x >= code.x + code.w, "{tag:?} starts right of {code:?}");
    assert!(
        tag.x + tag.w <= layout::CARD_INSET + layout::TEXT_W,
        "{tag:?} ends inside the item"
    );
}

#[test]
fn collapsing_drops_the_open_link_list() {
    // Re-homes the phase 5 `on_collapse` assertion: the view after the collapse has no list.
    let snap = snapshot(vec![tickets()]);
    let mut panel = open();
    panel.hovering = true;
    panel.open_menus.insert("done".into());
    let list_rows = |b: &Built| {
        targets(b, card(&b.scene, "done"))
            .iter()
            .filter(|t| matches!(t, Target::Link { .. }))
            .count()
    };
    assert_eq!(list_rows(&render(&snap, &panel)), 2);
    panel.hovering = false;
    panel.on_collapse();
    assert_eq!(render(&snap, &panel).scene.nodes.len(), 1, "just the pill");
    panel.hovering = true;
    assert_eq!(
        list_rows(&render(&snap, &panel)),
        1,
        "only the chip is left"
    );
}

fn closed(id: &str) -> ClosedSession {
    ClosedSession {
        id: id.into(),
        title: format!("title {id}"),
        cwd: "/work/repo".into(),
        branch: Some("main".into()),
        closed_at: now() - Duration::from_secs(720),
    }
}

#[test]
fn the_closed_list_opens_rows_that_resume_and_show_their_send_state() {
    let mut snap = snapshot(vec![finished("done")]);
    snap.closed = vec![closed("c1"), closed("c2")];
    let mut panel = open();
    panel.hovering = true;
    let shut = render(&snap, &panel);
    let node = |b: &Built| {
        b.scene
            .nodes
            .iter()
            .find(|n| n.key.0 == "closed")
            .unwrap()
            .clone()
    };
    assert_eq!(node(&shut).frame.h, layout::closed_height(2, false));
    panel.closed_open = true;
    let open = render(&snap, &panel);
    let n = node(&open);
    assert_eq!(n.frame.h, layout::closed_height(2, true));
    assert_eq!(
        targets(&open, &n),
        [
            Target::ToggleClosed,
            Target::Resume("c1".into()),
            Target::Resume("c2".into())
        ]
    );
    assert!(texts(&n).contains(&"đóng 12p"));
    assert_eq!(open.scene.size, expanded_size(&snap, &panel, &FakeMeasure));

    snap.sends.insert(
        SendKey::new("c1", SendKind::Resume),
        (SendState::Sending, now()),
    );
    snap.sends.insert(
        SendKey::new("c2", SendKind::Resume),
        (SendState::Failed, now()),
    );
    let busy = render(&snap, &panel);
    let n = node(&busy);
    assert!(texts(&n).contains(&"Đang mở…") && texts(&n).contains(&"Lỗi — đã copy lệnh"));
    assert_eq!(
        targets(&busy, &n),
        [Target::ToggleClosed, Target::Resume("c2".into())],
        "a row being opened has no hit"
    );
}

fn reply_runs(reply: &str) -> Vec<Run> {
    let mut s = finished("done");
    s.last_reply = Some(reply.into());
    let built = render(&snapshot(vec![s]), &open());
    card(&built.scene, "done")
        .cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::Rich { runs, .. } => Some(runs.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn markdown_styles_without_links() {
    // Swift `TranscriptTests.markdownStylesWithoutLinks`.
    let runs = reply_runs("a **b** `c` [d](https://x.y)");
    let all: String = runs.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(all, "a b c d");
    assert!(!all.contains("x.y"), "the link is dropped, its text kept");
    let d = runs.last().unwrap();
    assert_eq!(
        (d.bold, d.italic, d.mono),
        (false, false, false),
        "{runs:?}"
    );
    let open: String = reply_runs("**open")
        .iter()
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(open, "**open");
}

#[test]
fn markdown_code_runs_get_a_monospaced_font() {
    // Swift `TranscriptTests.markdownCodeRunsGetMonospacedFont`.
    let runs = reply_runs("plain **bold** `code`");
    let styled: Vec<_> = runs
        .iter()
        .filter(|r| r.mono)
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(styled, ["code"]);
    assert!(runs.iter().any(|r| r.bold && r.text == "bold" && !r.mono));
    let italic = reply_runs("an *emphasis* here");
    assert!(italic.iter().any(|r| r.italic && r.text == "emphasis"));
}

#[test]
fn a_shell_waiting_on_a_background_command_shows_the_hourglass() {
    // Re-homes the phase 5 `Theme.Symbols.status(s) == "hourglass"` assertion.
    let s = Session::new("sh", "sh", "/work/skills", "shell");
    assert_eq!(
        claude_sessions::theme::status_symbol(&s),
        "hourglass.circle.fill"
    );
    let built = render(&snapshot(vec![s]), &open());
    let _ = built;
    let mut panel = open();
    panel.hovering = true;
    let built = render(
        &snapshot(vec![Session::new("sh", "sh", "/w", "shell")]),
        &panel,
    );
    assert!(symbols(card(&built.scene, "sh")).contains(&"hourglass"));
}

#[test]
fn every_block_takes_the_height_the_size_model_gives_it() {
    use layout::{editor_height, link_list_height, question_height, reply_height};
    let mut panel = open();
    panel.hovering = true;
    panel.open_menus.insert("done".into());
    let mut multi = approval("appr");
    if let Some(PendingState::One(p)) = &mut multi.pending {
        p.text = "a\nb\nc".into();
    }
    let mut sessions = vec![multi, two_questions(), pr_session(), tickets(), busy("run")];
    sessions[3].id = "done".into();
    sessions[2].id = "pr".into();
    let snap = snapshot(sessions);
    let built = render(&snap, &panel);
    for s in &snap.sessions {
        let l = card_layout(s, &snap, &panel, &FakeMeasure);
        let node = card(&built.scene, &s.id);
        assert_eq!(node.frame.h, l.height, "{}", s.id);
        for p in &l.blocks {
            let want = match p.block {
                Block::Approval => l.approval.unwrap().height(false),
                Block::Question => {
                    let q = s.question.as_ref().unwrap();
                    question_height(q, layout::question_state(s, q), None)
                }
                Block::Reply => reply_height(
                    FakeMeasure
                        .measure(
                            s.last_reply.as_deref().unwrap(),
                            &layout::REPLY_FONT,
                            layout::TEXT_W,
                            2,
                        )
                        .h,
                ),
                Block::Prs => layout::prs_height(s.prs.len()),
                Block::Chips => layout::CHIPS_H,
                Block::LinkList => link_list_height(s),
                Block::Editor => editor_height(),
                Block::SentLine => layout::sent_line_height(),
                Block::Head | Block::Todo | Block::Activity => p.h,
            };
            assert_eq!(p.h, want, "{} {:?}", s.id, p.block);
        }
        let last = l.blocks.last().unwrap();
        assert!(last.y + last.h <= l.height);
        // Nothing the blocks draw reaches past the card.
        assert!(
            node.cmds.iter().all(|c| {
                // The row shape of a group's first / last row is pushed past the frame on purpose (the node clips it).
                let row_shape = matches!(c, DrawCmd::RoundedRect { rect, .. }
                    if rect.x == 0.0 && rect.w == layout::CARD_W);
                row_shape || bottom(c) <= l.height + 0.001
            }),
            "{} draws past its card",
            s.id
        );
        for f in &node.fields {
            assert!(f.rect.y + f.rect.h <= l.height);
        }
    }
}

fn approval_send(session: &str, state: SendState) -> (SendKey, (SendState, SystemTime)) {
    let mut key = SendKey::new(session, SendKind::Approve);
    key.detail = Some("t1".into());
    (key, (state, now()))
}

#[test]
fn the_approval_result_stays_as_a_sent_line_after_the_command_is_gone() {
    // Swift state `53-approval-sent-line`: a busy session, no pending command, an approve that was just sent.
    let plain = snapshot(vec![busy("run")]);
    let base = card_layout(&plain.sessions[0], &plain, &open(), &FakeMeasure).height;
    for (state, text) in [
        (SendState::Sending, "Đang gửi…"),
        (SendState::Sent, "Đã gửi ✓"),
        (SendState::Failed, "Lỗi — bấm thử lại"),
    ] {
        let mut snap = snapshot(vec![busy("run")]);
        let (k, v) = approval_send("run", state);
        snap.sends.insert(k, v);
        let (built, l) = only_card(&snap, &open());
        assert!(l.find(Block::SentLine).is_some());
        assert_eq!(l.height - base, layout::SENT_LINE_H);
        assert!(texts(card(&built.scene, "run")).contains(&text), "{text}");
    }
    // While the command is still pending, the approval block shows and no line.
    let mut snap = snapshot(vec![approval("appr")]);
    let (k, v) = approval_send("appr", SendState::Sent);
    snap.sends.insert(k, v);
    let (_, l) = only_card(&snap, &open());
    assert!(l.find(Block::SentLine).is_none() && l.find(Block::Approval).is_some());
}

fn long_list(n: usize) -> Snapshot {
    snapshot((0..n).map(|i| finished(&format!("s{i}"))).collect())
}

#[test]
fn a_long_list_scrolls_under_the_header_with_the_content_height_of_all_cards() {
    let snap = long_list(30);
    let panel = open();
    let built = render(&snap, &panel);
    let l = expanded_layout(&snap, &panel, &FakeMeasure);
    assert_eq!(built.scene.size.h, layout::MAX_EXPANDED_HEIGHT);
    let scrolls: Vec<_> = built
        .scene
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.scroll.is_some())
        .collect();
    assert_eq!(scrolls.len(), 1);
    let (at, node) = scrolls[0];
    let top = layout::PANEL_PAD + layout::header_height();
    assert_eq!(node.frame.y, top);
    assert_eq!(node.frame.h, built.scene.size.h - top);
    let cards_h: f64 = l
        .items
        .iter()
        .filter_map(|i| match i {
            layout::Item::Card { layout, .. } => Some(layout.height + layout::CARD_GAP),
            _ => None,
        })
        .sum();
    let content = node.scroll.unwrap();
    assert_eq!(
        content.h,
        layout::section_title_height() + cards_h + layout::PANEL_PAD
    );
    // The panel and header stay outside; every card and section is inside.
    let keys = |r: std::ops::Range<usize>| -> Vec<&str> {
        built.scene.nodes[r]
            .iter()
            .map(|n| n.key.0.as_str())
            .collect()
    };
    assert_eq!(keys(0..at), ["panel", "header"]);
    assert_eq!(card_keys(&built.scene).len(), 30);
    assert!(
        built.scene.nodes[at + 1..]
            .iter()
            .all(|n| n.scroll.is_none())
    );

    let short = render(&long_list(2), &panel);
    assert!(short.scene.nodes.iter().all(|n| n.scroll.is_none()));
}

#[test]
fn tooltips_list_the_agents_and_describe_the_ticket_chips() {
    let mut run = busy("run");
    run.agents = vec!["explore src".into(), "review diff".into()];
    let mut panel = open();
    panel.hovered_row = Some("run".into());
    let built = render(&snapshot(vec![run]), &panel);
    let node = card(&built.scene, "run");
    let tip = node
        .tips
        .iter()
        .find(|t| t.text.contains("explore src"))
        .expect("agent tip");
    assert_eq!(tip.text, "explore src\nreview diff");
    assert!(
        node.tips.iter().any(|t| t.text == "Dừng (bấm hai lần)"),
        "the stop button explains itself"
    );

    let mut s = tickets();
    s.links[0].help = Some("ABC-12 · Fix parser · In Progress".into());
    // The open list shows the items, not the chips: the item's tip is its help text, else its own text.
    s.menu[0].items[0].help = s.links[0].help.clone();
    let mut panel = open();
    panel.open_menus.insert("done".into());
    let built = render(&snapshot(vec![s]), &panel);
    let tips: Vec<&str> = card(&built.scene, "done")
        .tips
        .iter()
        .map(|t| t.text.as_str())
        .collect();
    assert!(
        tips.contains(&"ABC-12 · Fix parser · In Progress"),
        "{tips:?}"
    );
    // A menu row without help text shows its own text.
    assert!(tips.contains(&"ABC-9"), "{tips:?}");
}

#[test]
fn pr_tags_carry_their_help_and_the_disabled_ask_button_says_why() {
    let mut s = pr_session();
    s.prs[0].title = Some("Fix the parser".into());
    s.status = "busy".into();
    let built = render(&snapshot(vec![s]), &open());
    let tips: Vec<&str> = card(&built.scene, "done")
        .tips
        .iter()
        .map(|t| t.text.as_str())
        .collect();
    assert!(tips.contains(&"Fix the parser"), "{tips:?}");
    assert!(
        tips.contains(&"Chỉ gửi được khi Claude đã xong lượt"),
        "{tips:?}"
    );
}

#[test]
fn the_pr_number_opens_its_pull_request() {
    let mut s = pr_session();
    for (i, p) in s.prs.iter_mut().enumerate() {
        p.title = Some(format!("Title {i}"));
    }
    let prs = s.prs.clone();
    let built = render(&snapshot(vec![s]), &open());
    let node = card(&built.scene, "done");
    for (i, p) in prs.iter().enumerate() {
        let tip = node
            .tips
            .iter()
            .find(|t| t.text == format!("Title {i}"))
            .expect("the PR number has its title tip");
        let at = claude_sessions_core::scene::Point {
            x: tip.rect.x + tip.rect.w / 2.0,
            y: tip.rect.y + tip.rect.h / 2.0,
        };
        let hit = node.hits.iter().find(|h| h.rect.contains(at)).unwrap();
        assert_eq!(
            built.targets.get(hit.id).unwrap(),
            &Target::Link {
                session: "done".into(),
                url: p.url.clone()
            }
        );
    }
}

#[test]
fn a_ticket_icon_is_an_image_once_its_bytes_are_in_the_snapshot() {
    let mut snap = snapshot(vec![tickets()]);
    let icon = url("https://x.atlassian.net/icon.png");
    let built = render(&snap, &open());
    assert!(built.scene.images.is_empty());
    assert!(
        !card(&built.scene, "done")
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::Image { .. }))
    );
    snap.issue_icons.insert(icon.clone(), vec![1, 2, 3]);
    let built = render(&snap, &open());
    let images: Vec<_> = card(&built.scene, "done")
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Image { key, rect } => Some((key.clone(), *rect)),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].0, icon.as_str());
    assert_eq!(
        built.scene.images.get(icon.as_str()).map(|b| b.as_slice()),
        Some(&[1u8, 2, 3][..])
    );
    // The placeholder square is gone.
    assert!(!card(&built.scene, "done").cmds.iter().any(
        |c| matches!(c, DrawCmd::RoundedRect { rect, .. } if rect.w == 12.0 && rect.h == 12.0)
    ));
}

#[test]
fn keyboard_mode_rings_the_selected_card_and_shows_the_key_hint() {
    let snap = snapshot(vec![question("ask"), finished("done")]);
    let mut panel = open();
    let none = render(&snap, &panel);
    assert!(
        none.scene.nodes.iter().all(|n| n.key.0 != "key-hint"),
        "no hint outside keyboard mode"
    );
    panel.keyboard = true;
    panel.selected = Some("ask".into());
    let built = render(&snap, &panel);
    let ring = |id: &str| {
        card(&built.scene, id).cmds.iter().any(|c| {
            matches!(c, DrawCmd::RoundedRect { stroke: Some(s), .. } if s.token == ThemeColor::Accent)
        })
    };
    assert!(ring("ask"), "the selected card has the Accent border");
    assert!(!ring("done"));
    let hint = built
        .scene
        .nodes
        .iter()
        .find(|n| n.key.0 == "key-hint")
        .expect("keyboard mode shows the hint line");
    assert_eq!(
        texts(hint),
        ["↑↓ chọn thẻ · ↩ nhảy tới pane · R trả lời · Esc đóng"]
    );
    assert_eq!(
        built.scene.size.h - none.scene.size.h,
        layout::KEY_HINT_H,
        "the hint line is part of the size model"
    );
}

#[test]
fn ticket_chips_that_do_not_fit_collapse_into_a_plus_chip_inside_the_card() {
    let mut s = tickets();
    s.links = (101..=112)
        .map(|n| SessionLink {
            label: format!("T6IR-{n} In Progress"),
            url: url(&format!("https://x.atlassian.net/browse/T6IR-{n}")),
            help: None,
            icon_url: None,
            tone: None,
        })
        .collect();
    let snap = snapshot(vec![s]);
    let built = render(&snap, &open());
    let node = card(&built.scene, "done");
    let more = texts(node)
        .into_iter()
        .find(|t| t.starts_with('+'))
        .expect("a +N chip");
    let shown = texts(node)
        .iter()
        .filter(|t| t.starts_with("T6IR-"))
        .count();
    assert_eq!(more, format!("+{}", 12 - shown));
    assert!(shown > 0 && shown < 12);
    let edge = layout::CARD_W - layout::CARD_PAD_RIGHT;
    for c in &node.cmds {
        if let DrawCmd::RoundedRect { rect, .. } = c
            && rect.x > 0.0
        {
            assert!(
                rect.x + rect.w <= edge + 0.01,
                "{rect:?} runs past the card"
            );
        }
    }
    // Every chip is still reachable: the `+N` chip opens the list.
    let t = targets(&built, node);
    assert!(t.iter().filter(|t| matches!(t, Target::Menu(_))).count() >= 2);
}

#[test]
fn the_waiting_badge_is_a_circle_for_one_digit_and_a_capsule_for_two() {
    let badge = |n: usize| {
        let snap = snapshot((0..n).map(|i| approval(&format!("a{i}"))).collect());
        let built = render(&snap, &PanelState::new());
        let pill = &built.scene.nodes[0];
        pill.cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::RoundedRect {
                    rect,
                    fill: Some(f),
                    ..
                } if f.token == ThemeColor::Red => Some(*rect),
                _ => None,
            })
            .unwrap()
    };
    let one = badge(1);
    assert_eq!(one.w, one.h);
    let many = badge(12);
    assert!(many.w > many.h);
}

#[test]
fn a_status_icon_is_a_solid_circle_with_a_plain_glyph() {
    let snap = snapshot(vec![finished("done")]);
    let built = render(&snap, &open());
    let node = card(&built.scene, "done");
    assert!(symbols(node).contains(&"checkmark"));
    assert!(node.cmds.iter().any(|c| matches!(c,
        DrawCmd::RoundedRect { rect, fill: Some(f), .. }
            if rect.w == rect.h && f.token == ThemeColor::Orange && f.alpha == 1.0)));
}

#[test]
fn panel_node_has_shadow() {
    let (snap, _) = mixed();
    let built = render(&snap, &open());
    let panel = built.scene.nodes.iter().find(|n| n.key.0 == "panel");
    assert_eq!(panel.and_then(|n| n.shadow), Some(theme::PANEL_SHADOW));
}

#[test]
fn pill_node_has_shadow() {
    let (mut snap, _) = mixed();
    snap.bg_opacity = *claude_sessions::settings::BG_OPACITIES.last().unwrap();
    let built = render(&snap, &PanelState::default());
    assert_eq!(built.scene.nodes[0].shadow, Some(theme::PILL_SHADOW));
}

#[test]
fn pill_follows_background_opacity() {
    let full = claude_sessions::theme::CARD_OPACITY;
    assert_eq!(
        *claude_sessions::settings::BG_OPACITIES.last().unwrap(),
        full
    );
    let mut hit_rects = Vec::new();
    for o in [0.0, 0.25, full] {
        let (mut snap, _) = mixed();
        snap.bg_opacity = o;
        let built = render(&snap, &PanelState::default());
        let node = &built.scene.nodes[0];
        let Some(DrawCmd::RoundedRect { fill, stroke, .. }) = node.cmds.first() else {
            panic!("the pill starts with its background");
        };
        let fill = fill.unwrap();
        let stroke = stroke.unwrap();
        assert_eq!(
            (fill.token, stroke.token),
            (ThemeColor::Glass, ThemeColor::GlassEdge)
        );
        // `Glass` carries the alpha of the designed look: at `full` the fill is exactly `Glass`.
        assert!((fill.alpha - o / full).abs() < 1e-9, "fill {o}");
        assert!((stroke.alpha - o / full).abs() < 1e-9, "stroke {o}");
        let shadow = node.shadow.expect("the pill keeps its shadow");
        assert!(
            (shadow.color.alpha - theme::PILL_SHADOW.color.alpha * o / full).abs() < 1e-9,
            "shadow {o}"
        );
        hit_rects.push((node.frame, node.hits.clone()));
    }
    assert!(hit_rects.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn background_opacity_change_rebuilds_the_collapsed_pill() {
    let (mut snap, _) = mixed();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    let mut gate = claude_sessions::rebuild::RebuildGate::default();
    gate.built(&snap, &PanelState::default(), false, now);
    assert!(!gate.needs_rebuild(&snap, false, now));
    snap.bg_opacity = 0.0;
    assert!(gate.needs_rebuild(&snap, false, now));
}

#[test]
fn shadows_fit_the_margin() {
    for s in [theme::PANEL_SHADOW, theme::PILL_SHADOW] {
        assert!(s.radius + s.offset.y.abs() <= layout::MARGIN, "{s:?}");
    }
}

#[test]
fn other_nodes_have_no_shadow() {
    let (snap, _) = mixed();
    let built = render(&snap, &open());
    for node in built.scene.nodes.iter().filter(|n| n.key.0 != "panel") {
        assert!(node.shadow.is_none(), "{}", node.key.0);
    }
}

fn fixture_built(name: &str) -> Built {
    fixture_built_with(name, |_| {})
}

fn fixture_built_with(name: &str, tweak: impl Fn(&mut Snapshot)) -> Built {
    let (mut snap, panel) = fixture_state(name);
    tweak(&mut snap);
    let clock = |_: SystemTime| "14:30".to_string();
    build(&View {
        snap: &snap,
        panel: &panel,
        measure: &FakeMeasure,
        now: claude_sessions_core::fixtures::view_now(
            name,
            claude_sessions_core::fixtures::fixed_now(),
        ),
        clock: &clock,
    })
}

fn option_hit_rects(b: &Built, node: &Node) -> Vec<claude_sessions_core::scene::Rect> {
    node.hits
        .iter()
        .filter(|h| matches!(b.targets.get(h.id), Some(Target::PickOption { .. })))
        .map(|h| h.rect)
        .collect()
}

#[test]
fn question_rows_are_one_line() {
    let built = fixture_built("question-4-two-questions");
    let node = built
        .scene
        .nodes
        .iter()
        .find(|n| {
            n.hits
                .iter()
                .any(|h| matches!(built.targets.get(h.id), Some(Target::PickOption { .. })))
        })
        .unwrap();
    let rows = option_hit_rects(&built, node);
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| r.h == layout::OPTION_ROW_H), "{rows:?}");
    let numbers: Vec<&str> = texts(node)
        .into_iter()
        .filter(|t| t.len() == 1 && t.chars().all(|c| c.is_ascii_digit()))
        .collect();
    let want: Vec<String> = (1..=rows.len()).map(|n| n.to_string()).collect();
    assert_eq!(numbers, want, "option numbers 1..n in order");
    // Only a picked row has an `Accent` radio disc; no option row has a row-wide fill.
    let fills = |node: &Node| {
        node.cmds
            .iter()
            .filter(|c| {
                matches!(c, DrawCmd::RoundedRect { rect, fill: Some(f), .. }
                    if f.token == ThemeColor::Accent && rect.w == 14.0 && rect.h == 14.0)
            })
            .count()
    };
    let (_, panel) = fixture_state("question-4-two-questions");
    let picked: usize = panel.answers.values().flatten().map(Vec::len).sum();
    assert_eq!(fills(node), picked, "only picked rows have a fill");
    // No row-wide filled rect of any colour sits at an unpicked row.
    let filled_at_unpicked = node
        .cmds
        .iter()
        .filter(|c| {
            matches!(c, DrawCmd::RoundedRect { rect, fill: Some(f), .. }
                if rect.w == rows[0].w && f.alpha > 0.0
                    && rows.iter().any(|r| r.y == rect.y))
        })
        .count();
    assert_eq!(filled_at_unpicked, 0, "unpicked rows have no fill");
}

fn fixture_state(name: &str) -> (Snapshot, PanelState) {
    let (_, f) = claude_sessions_core::fixtures::STATES
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    f(dir.path(), claude_sessions_core::fixtures::fixed_now())
}

#[test]
fn long_option_description_stays_one_line() {
    let mut s = two_questions();
    let long = "x".repeat(300);
    s.question.as_mut().unwrap().questions[0].options[0].description = Some(long.clone());
    let snap = snapshot(vec![s]);
    let (built, _) = only_card(&snap, &open());
    let node = card(&built.scene, "ask");
    let rows = option_hit_rects(&built, node);
    assert!(rows.iter().all(|r| r.h == layout::OPTION_ROW_H));
    let desc = node
        .cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::Text {
                text, max_lines, ..
            } if *text == long => Some(*max_lines),
            _ => None,
        })
        .expect("the description is drawn");
    assert_eq!(desc, 1);
    assert!(
        node.tips
            .iter()
            .any(|t| t.text == long && t.rect.h == layout::OPTION_ROW_H)
    );
}

#[test]
fn countdown_is_not_mono() {
    let (built, _) = only_card(&snapshot(vec![two_questions()]), &open());
    let node = card(&built.scene, "ask");
    let font = node
        .cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::Text { text, font, .. } if text.starts_with("còn ") => Some(*font),
            _ => None,
        })
        .unwrap();
    assert!(!font.mono);
}

#[test]
fn the_expanded_header_has_no_claude_title() {
    let built = fixture_built("07-mixed-hover");
    let header = built
        .scene
        .nodes
        .iter()
        .find(|n| n.key.0 == "header")
        .unwrap();
    assert!(!texts(header).contains(&"Claude"));
}

#[test]
fn pill_group_gaps_are_equal() {
    let built = fixture_built("06-mixed-collapsed");
    let node = &built.scene.nodes[0];
    // Group content in draw order: [x0, x1] per group; a group starts at the first cmd after a text cmd that is a count.
    let mut spans: Vec<(f64, f64)> = Vec::new();
    let mut start: Option<f64> = None;
    for c in &node.cmds {
        match c {
            DrawCmd::Symbol { rect, .. } => start = Some(rect.x),
            DrawCmd::RoundedRect {
                rect,
                fill: Some(f),
                ..
            } if f.token != ThemeColor::Glass && rect.h <= 18.0 => start = Some(rect.x),
            DrawCmd::Text {
                rect, text, align, ..
            } if text.chars().all(|c| c.is_ascii_digit()) => {
                let s = start.take().unwrap_or(rect.x);
                // The badge is a capsule that holds its digit: it ends where its own rect ends. Left-aligned digits end
                // after their ink (`PILL_DIGIT_W` per digit), not at the end of their rect.
                let end = if *align == Align::Left {
                    rect.x + layout::PILL_DIGIT_W * text.len() as f64
                } else {
                    rect.x + rect.w
                };
                spans.push((s, end));
            }
            _ => {}
        }
    }
    assert!(spans.len() >= 3, "{spans:?}");
    let gaps: Vec<f64> = spans.windows(2).map(|w| w[1].0 - w[0].1).collect();
    assert!(
        gaps.iter().all(|g| (g - gaps[0]).abs() <= 0.5),
        "{gaps:?} {spans:?}"
    );
}

/// `FakeMeasure` with a narrow `1`, like a proportional font.
struct NarrowOne;

impl TextMeasure for NarrowOne {
    fn measure(
        &self,
        text: &str,
        font: &claude_sessions_core::scene::FontSpec,
        max_width: f64,
        max_lines: u32,
    ) -> claude_sessions_core::geometry::Size {
        let mut size = FakeMeasure.measure(text, font, max_width, max_lines);
        size.w -= 3.0 * text.matches('1').count() as f64;
        size
    }
}

#[test]
fn shortcut_label_stays_put_while_the_countdown_ticks() {
    let shortcut_x = |elapsed: u64| {
        let mut s = two_questions();
        let created = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 - elapsed);
        s.question.as_mut().unwrap().created_at = created;
        let snap = snapshot(vec![s]);
        let clock = |_: SystemTime| "14:30".to_string();
        let built = build(&View {
            snap: &snap,
            panel: &open(),
            measure: &NarrowOne,
            now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
            clock: &clock,
        });
        let node = card(&built.scene, "ask");
        let countdown = texts(node)
            .into_iter()
            .find(|t| t.starts_with("còn "))
            .map(str::to_string)
            .unwrap();
        let x = node
            .cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::Text { rect, text, .. } if text == "⌘↩" => Some(rect.x),
                _ => None,
            })
            .unwrap();
        (countdown, x)
    };
    let (a, ax) = shortcut_x(9);
    let (b, bx) = shortcut_x(112);
    assert_eq!((a.as_str(), b.as_str()), ("còn 1:51", "còn 0:08"));
    assert_eq!(ax, bx);
}

#[test]
fn panel_node_is_glass() {
    let built = fixture_built("07-mixed-hover");
    let panel = built
        .scene
        .nodes
        .iter()
        .find(|n| n.key.0 == "panel")
        .unwrap();
    assert_eq!(
        panel.material,
        Some(Material::Glass {
            corner: 20.0,
            alpha: 1.0
        })
    );
    let Some(DrawCmd::RoundedRect {
        fill: Some(fill),
        stroke: Some(stroke),
        radius,
        ..
    }) = panel.cmds.first()
    else {
        panic!("the panel starts with its background");
    };
    assert_eq!(fill.token, ThemeColor::Glass);
    assert_eq!(stroke.token, ThemeColor::GlassEdge);
    // The edge is inset by half its 1 pt stroke.
    assert_eq!(*radius, 19.5);
}

#[test]
fn pill_glass_follows_opacity() {
    let full = theme::CARD_OPACITY;
    for (o, want) in [(0.0, 0.0), (0.25, 0.25 / full), (full, 1.0)] {
        let built = fixture_built_with("06-mixed-collapsed", |s| s.bg_opacity = o);
        let pill = &built.scene.nodes[0];
        assert_eq!(pill.key.0, "pill");
        let Some(Material::Glass { corner, alpha }) = pill.material else {
            panic!("the pill is glass");
        };
        assert_eq!(corner, 15.0);
        assert!((alpha - want).abs() < 1e-9, "alpha {alpha} at {o}");
        let Some(DrawCmd::RoundedRect {
            fill: Some(fill), ..
        }) = pill.cmds.first()
        else {
            panic!("the pill starts with its tint");
        };
        assert_eq!(fill.token, ThemeColor::Glass);
        assert!((fill.alpha - want).abs() < 1e-9, "tint {o}");
    }
}

#[test]
fn only_pill_and_panel_have_material() {
    for name in ["06-mixed-collapsed", "07-mixed-hover", "closed-2-open"] {
        let built = fixture_built(name);
        for node in &built.scene.nodes {
            let glass = node.key.0 == "pill" || node.key.0 == "panel";
            assert_eq!(node.material.is_some(), glass, "{name}: {}", node.key.0);
        }
    }
}

#[test]
fn section_labels_sentence_case() {
    let built = fixture_built("07-mixed-hover");
    let all: Vec<&str> = built.scene.nodes.iter().flat_map(texts).collect();
    for label in ["Cần bạn", "Xong", "Đang chạy"] {
        assert!(all.contains(&label), "{label} in {all:?}");
    }
    for label in ["CẦN BẠN", "XONG", "ĐANG CHẠY"] {
        assert!(!all.iter().any(|t| t.contains(label)), "{label} in {all:?}");
    }
}

#[test]
fn header_groups_quota_and_counts_on_the_right() {
    let (mut snap, panel) = mixed();
    snap.quota = Some(claude_sessions_core::quota::Quota {
        five_hour: 78,
        five_hour_reset: now(),
        weekly: 40,
        weekly_reset: now(),
        forecast: None,
    });
    let built = render(&snap, &panel);
    let header = built
        .scene
        .nodes
        .iter()
        .find(|n| n.key.0 == "header")
        .unwrap();
    assert_eq!(header.frame.h, layout::HEADER_HEIGHT);
    // The whole group stays in the right half of the header; the reset note drops when it would not fit there.
    let left_edge = header.frame.w / 2.0;
    let rect_of = |want: &str| {
        header.cmds.iter().find_map(|c| match c {
            DrawCmd::Text { rect, text, .. } if text == want => Some(*rect),
            _ => None,
        })
    };
    let quota: Vec<Rect> = ["5h", "78%", "Tuần", "40%"]
        .iter()
        .map(|t| rect_of(t).unwrap_or_else(|| panic!("no {t}")))
        .collect();
    assert!(
        !header
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::Text { text, .. } if text.starts_with("· reset"))),
        "the reset note gives way first"
    );
    let rings = header
        .cmds
        .iter()
        .filter(|c| matches!(c, DrawCmd::Arc { .. }))
        .count();
    assert_eq!(rings, 4, "two rings, a track and a sweep each");
    assert!(quota[0].x < quota[1].x && quota[1].x < quota[2].x);
    let counts: Vec<Rect> = header
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text { rect, text, .. } if text.chars().all(|c| c.is_ascii_digit()) => {
                Some(*rect)
            }
            _ => None,
        })
        .collect();
    assert!(!counts.is_empty());
    for r in quota.iter().chain(&counts) {
        assert!(r.x >= left_edge, "{r:?} is left of {left_edge}");
    }
    // The hairline between the two quotas is the other 0.5 pt one: the divider to the counts is the right-most.
    let divider = header
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                fill: Some(f),
                ..
            } if rect.w == 0.5 && f.token == ThemeColor::Separator => Some(*rect),
            _ => None,
        })
        .max_by(|a, b| a.x.total_cmp(&b.x))
        .expect("a 0.5 pt divider");
    let quota_max = quota.iter().map(|r| r.x + r.w).fold(f64::MIN, f64::max);
    let counts_min = counts.iter().map(|r| r.x).fold(f64::MAX, f64::min);
    assert!(
        quota_max < divider.x && divider.x < counts_min,
        "{quota_max} < {} < {counts_min}",
        divider.x
    );
}

fn header_texts(built: &Built) -> Vec<String> {
    let header = built
        .scene
        .nodes
        .iter()
        .find(|n| n.key.0 == "header")
        .unwrap();
    header
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

// These pin which part gives way at the 560 pt panel with FakeMeasure's 7 pt-per-char widths; a real font changes
// the widths, not the order.
#[test]
fn a_forecast_outlives_the_weekly_quota_when_the_block_must_shrink() {
    let texts = header_texts(&fixture_built("quota-3-high-forecast"));
    assert!(
        texts.iter().any(|t| t == "hết ~14:30"),
        "the forecast stays: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == "Tuần"),
        "the weekly quota gives way first: {texts:?}"
    );
}

#[test]
fn the_reset_note_stays_when_the_quota_fits_without_counts() {
    let mut snap = snapshot(vec![]);
    snap.quota = Some(claude_sessions_core::quota::Quota {
        five_hour: 40,
        five_hour_reset: now(),
        weekly: 20,
        weekly_reset: now(),
        forecast: None,
    });
    let texts = header_texts(&render(&snap, &open()));
    assert!(
        texts.iter().any(|t| t.starts_with("· reset")),
        "the reset note fits and stays: {texts:?}"
    );
    assert!(texts.iter().any(|t| t == "Tuần"), "{texts:?}");
}

/// Every quota and count element of the header lies in its right half (the notch sits over the left half); the reset
/// note, then the weekly part, are what give way.
#[test]
fn header_quota_and_counts_stay_in_the_right_half() {
    for name in [
        "quota-1-normal",
        "quota-2-warn",
        "quota-3-high-forecast",
        "07-mixed-hover",
        "11-pending-hover",
        "35-background-agents",
    ] {
        // The count-heavy fixtures get a quota with a forecast, the widest note.
        let built = fixture_built_with(name, |snap| {
            snap.quota
                .get_or_insert(claude_sessions_core::quota::Quota {
                    five_hour: 97,
                    five_hour_reset: now(),
                    weekly: 40,
                    weekly_reset: now(),
                    forecast: Some(now()),
                });
        });
        let header = built
            .scene
            .nodes
            .iter()
            .find(|n| n.key.0 == "header")
            .unwrap();
        let mid = header.frame.w / 2.0;
        let mut texts = Vec::new();
        for cmd in &header.cmds {
            match cmd {
                DrawCmd::Text { rect, text, .. } => {
                    texts.push(text.as_str());
                    assert!(
                        rect.x >= mid,
                        "{name}: {text:?} at {} is left of {mid}",
                        rect.x
                    );
                }
                DrawCmd::Symbol { rect, .. } => assert!(rect.x >= mid, "{name}: symbol {rect:?}"),
                DrawCmd::Arc { center, radius, .. } => {
                    assert!(center.x - radius >= mid, "{name}: ring at {center:?}")
                }
                _ => {}
            }
        }

        assert!(
            texts.contains(&"5h"),
            "{name}: the 5h quota stays: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.ends_with('%')),
            "{name}: its percent stays: {texts:?}"
        );
    }
}

// ---- phase 4: grouped rows, buttons and fields ----

use claude_sessions_core::scene::{Color, Point, Rect};

/// The first rounded rect of a card with a `Group` fill: the row's background, as `(rect, radius)`.
fn group_bg(node: &Node) -> (Rect, f64) {
    node.cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                radius,
                fill: Some(f),
                ..
            } if f.token == ThemeColor::Group => Some((*rect, *radius)),
            _ => None,
        })
        .expect("a row draws its Group background")
}

fn text_cmd<'a>(node: &'a Node, want: &str) -> Option<(&'a Rect, &'a Color)> {
    node.cmds.iter().find_map(|c| match c {
        DrawCmd::Text {
            rect, text, color, ..
        } if text == want => Some((rect, color)),
        _ => None,
    })
}

#[test]
fn rows_in_a_section_touch() {
    let (snap, panel) = mixed();
    let scene = render(&snap, &panel).scene;
    let mut checked = 0;
    for (_, ids) in snap.sections() {
        for pair in ids.windows(2) {
            let (a, b) = (card(&scene, &pair[0]).frame, card(&scene, &pair[1]).frame);
            assert_eq!(b.y, a.y + a.h, "{} touches {}", pair[1], pair[0]);
            checked += 1;
        }
    }
    assert_eq!(checked, 2, "two sections hold two rows");
    assert_eq!(layout::CARD_GAP, 0.0);
}

#[test]
fn group_corners_first_and_last() {
    let scene = render(&long_list(3), &open()).scene;
    let (first, mid, last) = (card(&scene, "s0"), card(&scene, "s1"), card(&scene, "s2"));
    // Nodes clip to their frame: a 13 pt rounded rect pushed past the edge that stays square leaves only the other corners round.
    let (r, radius) = group_bg(first);
    assert_eq!((radius, r.y), (13.0, 0.0));
    assert!(r.y + r.h > first.frame.h, "the bottom corners are clipped");
    let (r, radius) = group_bg(last);
    assert_eq!(radius, 13.0);
    assert!(r.y < 0.0 && r.y + r.h == last.frame.h, "{r:?}");
    let (r, radius) = group_bg(mid);
    assert_eq!(radius, 0.0);
    assert_eq!((r.y, r.h), (0.0, mid.frame.h));
    let one = render(&long_list(1), &open()).scene;
    let (r, radius) = group_bg(card(&one, "s0"));
    assert_eq!(radius, 13.0);
    assert_eq!((r.y, r.h), (0.0, card(&one, "s0").frame.h));
}

#[test]
fn separator_inset_to_text_column() {
    let scene = render(&long_list(3), &open()).scene;
    let seps = |node: &Node| -> Vec<Rect> {
        node.cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::RoundedRect {
                    rect,
                    fill: Some(f),
                    ..
                } if f.token == ThemeColor::Separator => Some(*rect),
                _ => None,
            })
            .collect()
    };
    assert!(
        seps(card(&scene, "s0")).is_empty(),
        "the first row has none"
    );
    for id in ["s1", "s2"] {
        let want = Rect::new(
            layout::CARD_INSET,
            0.0,
            layout::CARD_W - layout::CARD_INSET,
            0.5,
        );
        assert_eq!(seps(card(&scene, id)), [want], "{id}");
    }
    assert_eq!(layout::CARD_INSET, 40.0);
}

#[test]
fn hover_fixtures_show_their_quick_button() {
    for (name, id, target) in [
        ("10-last-reply-hover", "a", Target::Reply("a".into())),
        ("12-actions-hover", "f", Target::Stop("f".into())),
    ] {
        let (_, panel) = fixture_state(name);
        assert_eq!(panel.hovered_row.as_deref(), Some(id), "{name}");
        let built = fixture_built(name);
        let node = card(&built.scene, id);
        assert!(targets(&built, node).contains(&target), "{name}");
        assert!(
            built
                .scene
                .nodes
                .iter()
                .filter(|n| n.key.0.starts_with("card-") && n.key.0 != format!("card-{id}"))
                .all(|n| targets(&built, n)
                    .iter()
                    .all(|t| !matches!(t, Target::Reply(_) | Target::Stop(_)))),
            "{name}: only the hovered row shows a quick button"
        );
    }
}

#[test]
fn hovered_row_shows_actions() {
    let (snap, mut panel) = mixed();
    let quiet = render(&snap, &panel);
    for id in ["done", "run"] {
        let t = targets(&quiet, card(&quiet.scene, id));
        assert_eq!(t, [Target::Jump(id.into())], "{id} has no button at rest");
    }
    assert!(!symbols(card(&quiet.scene, "done")).contains(&"arrowshape.turn.up.left"));
    assert!(!symbols(card(&quiet.scene, "run")).contains(&"stop.fill"));
    panel.hovered_row = Some("done".into());
    let hover = render(&snap, &panel);
    let done = card(&hover.scene, "done");
    assert!(symbols(done).contains(&"arrowshape.turn.up.left"));
    assert_eq!(
        targets(&hover, done),
        [Target::Reply("done".into()), Target::Jump("done".into())]
    );
    assert_eq!(
        targets(&hover, card(&hover.scene, "run")),
        [Target::Jump("run".into())],
        "only the hovered row shows its button"
    );
    panel.hovered_row = Some("run".into());
    let hover = render(&snap, &panel);
    assert!(symbols(card(&hover.scene, "run")).contains(&"stop.fill"));
    assert_eq!(
        targets(&hover, card(&hover.scene, "run")),
        [Target::Stop("run".into()), Target::Jump("run".into())]
    );
}

#[test]
fn keyboard_selected_row_shows_actions() {
    let (snap, mut panel) = mixed();
    panel.keyboard = true;
    panel.selected = Some("run".into());
    let built = render(&snap, &panel);
    assert_eq!(
        targets(&built, card(&built.scene, "run")),
        [Target::Stop("run".into()), Target::Jump("run".into())]
    );
    assert_eq!(
        targets(&built, card(&built.scene, "done")),
        [Target::Jump("done".into())]
    );
    panel.selected = Some("done".into());
    let built = render(&snap, &panel);
    assert_eq!(
        targets(&built, card(&built.scene, "done")),
        [Target::Reply("done".into()), Target::Jump("done".into())]
    );
}

#[test]
fn stop_arms_to_red_capsule() {
    let (snap, mut panel) = mixed();
    let rest = render(&snap, &panel);
    assert!(text_cmd(card(&rest.scene, "run"), "Dừng?").is_none());
    panel.stop_armed.insert("run".into(), now());
    let built = render(&snap, &panel);
    let run = card(&built.scene, "run");
    let (label, ink) = text_cmd(run, "Dừng?").expect("the armed button says Dừng?");
    assert_eq!(ink.token, ThemeColor::White);
    let capsule = run
        .cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                radius,
                fill: Some(f),
                ..
            } if f.token == ThemeColor::Red => Some((*rect, *radius)),
            _ => None,
        })
        .expect("a red capsule");
    assert_eq!((capsule.0.h, capsule.1), (20.0, 10.0));
    assert!(capsule.0.contains(claude_sessions_core::scene::Point::new(
        label.x + 1.0,
        label.y + 1.0
    )));
    // Armed, the click is the second step: the button is in the hits without any hover.
    assert!(targets(&built, run).contains(&Target::Stop("run".into())));
}

/// The right-aligned time of the title line, when the row draws one.
fn time_cmd(node: &Node) -> Option<&DrawCmd> {
    node.cmds.iter().find(|c| {
        matches!(c, DrawCmd::Text { font, align: Align::Right, .. } if *font == theme::TIME_FONT)
    })
}

#[test]
fn armed_stop_stays_in_the_right_column() {
    let mut run = busy("run");
    run.since = Some(now() - Duration::from_secs(2 * 3600 + 30 * 60));
    let snap = snapshot(vec![run]);
    let mut panel = open();
    let rest = render(&snap, &panel);
    assert!(time_cmd(card(&rest.scene, "run")).is_some());
    panel.stop_armed.insert("run".into(), now());
    let built = render(&snap, &panel);
    let run = card(&built.scene, "run");
    let title = run
        .cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::Text { rect, text, .. } if text == "run" => Some(*rect),
            _ => None,
        })
        .expect("the title");
    let capsule = run
        .cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                fill: Some(f),
                ..
            } if f.token == ThemeColor::Red => Some(*rect),
            _ => None,
        })
        .expect("a red capsule");
    assert!(
        capsule.x >= title.x + title.w,
        "the capsule {capsule:?} covers the title {title:?}"
    );
    assert!(capsule.x + capsule.w <= layout::CARD_W - layout::CARD_PAD_RIGHT + 0.01);
    assert!(
        time_cmd(run).is_none(),
        "the time makes way for the armed button"
    );
}

#[test]
fn allow_is_primary_accent() {
    let snap = snapshot(vec![approval("appr")]);
    let built = render(&snap, &open());
    let node = card(&built.scene, "appr");
    let approve = node
        .hits
        .iter()
        .find(|h| matches!(built.targets.get(h.id), Some(Target::Approve { .. })))
        .unwrap()
        .rect;
    assert_eq!(approve.h, layout::BUTTON_H);
    let fill = node.cmds.iter().find_map(|c| match c {
        DrawCmd::RoundedRect {
            rect,
            fill: Some(f),
            ..
        } if *rect == approve => Some(f.token),
        _ => None,
    });
    assert_eq!(fill, Some(ThemeColor::Accent));
    let (label, ink) = text_cmd(node, "Cho phép").unwrap();
    assert_eq!(ink.token, ThemeColor::White);
    assert!(approve.contains(claude_sessions_core::scene::Point::new(label.x, label.y)));
    let (letter, faint) = text_cmd(node, "A").expect("the shortcut letter");
    assert_eq!((faint.token, faint.alpha), (ThemeColor::White, 0.5));
    assert!(approve.contains(claude_sessions_core::scene::Point::new(letter.x, letter.y)));
    // The deny button is a plain `Fill` capsule to the left of it.
    let deny = node
        .hits
        .iter()
        .find(|h| matches!(built.targets.get(h.id), Some(Target::Deny { .. })))
        .unwrap()
        .rect;
    assert!(deny.x + deny.w <= approve.x);
    let dfill = node.cmds.iter().find_map(|c| match c {
        DrawCmd::RoundedRect {
            rect,
            fill: Some(f),
            ..
        } if *rect == deny => Some(f.token),
        _ => None,
    });
    assert_eq!(dfill, Some(ThemeColor::Fill));
}

#[test]
fn round_send_faces_keep_frame() {
    let mut panel = open();
    panel.editing = Some(Editing {
        id: "done".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    let face = |state: Option<SendState>| {
        let mut snap = snapshot(vec![finished("done")]);
        if let Some(st) = state {
            snap.sends
                .insert(SendKey::new("done", SendKind::Reply), (st, now()));
        }
        let built = render(&snap, &panel);
        let node = card(&built.scene, "done");
        let round = node
            .cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::RoundedRect { rect, radius, .. } if rect.w == 22.0 && *radius == 11.0 => {
                    Some(*rect)
                }
                _ => None,
            })
            .expect("the round send button");
        let hit = node
            .hits
            .iter()
            .find(|h| matches!(built.targets.get(h.id), Some(Target::ReplySend(_))))
            .map(|h| h.rect);
        let syms: Vec<String> = symbols(node).into_iter().map(str::to_string).collect();
        let tip = node
            .tips
            .iter()
            .find(|t| t.rect == round)
            .map(|t| t.text.clone());
        (round, hit, syms, node.spinner.is_some(), tip)
    };
    let (idle, hit, syms, spinner, _) = face(None);
    assert_eq!(hit, Some(idle));
    assert!(syms.iter().any(|s| s == "arrow.up") && !spinner);
    let (sending, hit, _, spinner, _) = face(Some(SendState::Sending));
    assert!(spinner, "sending shows the spinner");
    assert_eq!(hit, None, "no second click while it is in flight");
    let (sent, hit, syms, spinner, _) = face(Some(SendState::Sent));
    assert!(syms.iter().any(|s| s == "checkmark") && !spinner);
    assert_eq!(hit, Some(sent));
    let (failed, hit, syms, _, tip) = face(Some(SendState::Failed));
    assert!(syms.iter().any(|s| s == "exclamationmark"));
    assert_eq!(hit, Some(failed));
    assert!(
        tip.is_some_and(|t| t.contains("Lỗi")),
        "the tooltip carries the error"
    );
    assert_eq!(
        (idle, sent, failed),
        (sending, idle, idle),
        "the frame never moves"
    );
}

#[test]
fn chips_overflow_at_panel_width() {
    let built = fixture_built("23-ticket-row-overflow");
    let (snap, _) = fixture_state("23-ticket-row-overflow");
    let node = built
        .scene
        .nodes
        .iter()
        .find(|n| {
            n.hits
                .iter()
                .any(|h| matches!(built.targets.get(h.id), Some(Target::Menu(_))))
        })
        .expect("the card with the chips");
    let session = snap
        .sessions
        .iter()
        .find(|s| node.key.0 == format!("card-{}", s.id))
        .expect("the session of the card");
    let is_fill_chip = |c: &DrawCmd| {
        matches!(c, DrawCmd::RoundedRect { rect, radius, fill: Some(f), .. }
            if rect.h == layout::CHIP_H && *radius == 6.0 && f.token == ThemeColor::Fill)
    };
    // The link chips are the chips with a link hit to one of the session's links (the PR numbers link to their PR too); the
    // `+M` chip and the menu chip are the two Menu hits.
    let links = node
        .hits
        .iter()
        .filter(|h| {
            matches!(built.targets.get(h.id), Some(Target::Link { url, .. })
                if session.links.iter().any(|l| &l.url == url))
        })
        .count();
    let more = texts(node)
        .into_iter()
        .find(|t| t.starts_with('+'))
        .expect("a +M chip");
    let more: usize = more[1..].parse().unwrap();
    assert!(links >= 2 && more >= 1, "{links} shown, +{more}");
    assert_eq!(
        links + more,
        session.links.len(),
        "every link is a chip or counted in the +M chip"
    );
    // Fill chips: the links, the `+M` chip; the menu chip (`list.bullet`) is a Fill chip too, since no menu is open.
    assert!(symbols(node).contains(&"list.bullet"));
    assert_eq!(
        node.cmds.iter().filter(|c| is_fill_chip(c)).count(),
        links + 2
    );
    let edge = layout::CARD_W - layout::CARD_PAD_RIGHT;
    for c in &node.cmds {
        if let DrawCmd::RoundedRect {
            rect,
            fill: Some(f),
            ..
        } = c
            && rect.h == layout::CHIP_H
            && f.token == ThemeColor::Fill
        {
            assert!(rect.x >= layout::CARD_INSET);
            assert!(
                rect.x + rect.w <= edge + 0.01,
                "{rect:?} past the text column"
            );
        }
    }
}

#[test]
fn link_status_label_stays_inside_its_item() {
    let built = fixture_built("29-link-list-open");
    let node = built
        .scene
        .nodes
        .iter()
        .find(|n| texts(n).contains(&"Ready for Grooming with Engineering"))
        .expect("the card with the long status");
    let status = text_rect(node, "Ready for Grooming with Engineering");
    let item = item_hits(&built, node)
        .into_iter()
        .find(|r| status.y >= r.y && status.y < r.y + r.h)
        .expect("the item of the status");
    assert!(
        status.x + status.w <= item.x + item.w - 10.0 + 0.01,
        "{status:?} ends inside {item:?}"
    );
    assert!(status.x > item.x + 38.0, "right of the badge");
}

#[test]
fn question_option_row_24() {
    let built = fixture_built("question-4-two-questions");
    let node = built
        .scene
        .nodes
        .iter()
        .find(|n| {
            n.hits
                .iter()
                .any(|h| matches!(built.targets.get(h.id), Some(Target::PickOption { .. })))
        })
        .unwrap();
    let rows = option_hit_rects(&built, node);
    assert!(rows.iter().all(|r| r.h == 24.0));
    let (_, panel) = fixture_state("question-4-two-questions");
    let picked: usize = panel.answers.values().flatten().map(Vec::len).sum();
    for r in &rows {
        let radio = node.cmds.iter().find_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                radius,
                fill,
                stroke,
            } if rect.w == 14.0
                && *radius == 7.0
                && rect.y >= r.y
                && rect.y + rect.h <= r.y + r.h =>
            {
                Some((fill.map(|f| f.token), stroke.is_some()))
            }
            _ => None,
        });
        assert!(radio.is_some(), "a 14 pt radio in {r:?}");
    }
    let on = node
        .cmds
        .iter()
        .filter(|c| {
            matches!(c, DrawCmd::RoundedRect { rect, radius, fill: Some(f), .. }
                if rect.w == 14.0 && *radius == 7.0 && f.token == ThemeColor::Accent)
        })
        .count();
    assert_eq!(on, picked, "a picked option has the Accent radio");
}

#[test]
fn every_fixture_draws_the_height_the_size_model_predicts() {
    let mut seen = 0;
    for (name, _) in claude_sessions_core::fixtures::STATES {
        let built = fixture_built(name);
        if built.scene.nodes.iter().all(|n| n.key.0 != "panel") {
            continue;
        }
        let (snap, panel) = fixture_state(name);
        let l = expanded_layout(&snap, &panel, &FakeMeasure);
        for item in &l.items {
            if let layout::Item::Card { id, y, layout } = item {
                let node = card(&built.scene, id);
                assert_eq!(
                    (node.frame.y, node.frame.h),
                    (*y, layout.height),
                    "{name} {id}"
                );
                assert!(
                    node.cmds.iter().all(|c| {
                        // The row shape of a group's first / last row is pushed past the frame on purpose (the node clips it).
                        let row_shape = matches!(c, DrawCmd::RoundedRect { rect, .. }
                            if rect.x == 0.0 && rect.w == layout::CARD_W);
                        row_shape || bottom(c) <= layout.height + 0.001
                    }),
                    "{name} {id} draws past its row"
                );
                seen += 1;
            }
        }
    }
    assert!(seen > 20, "{seen} cards checked");
}

// ---- The PR inset list ----

const ITEM_H: f64 = layout::PR_ITEM_H;
/// Where an open PR's buttons stop short of the list's right edge.
const BTN_EDGE: f64 = 10.0;

/// A one-PR session whose PR has the title and no checks, comments or rabbit.
fn bare_pr(id: i64, state: &str, title: Option<&str>) -> PRStatus {
    let mut p = pr(id, state);
    p.reviewers = 0;
    p.changes_requested = 0;
    p.comments = 0;
    p.title = title.map(Into::into);
    p
}

fn session_with(prs: Vec<PRStatus>) -> Session {
    let mut s = finished("done");
    s.prs = prs;
    s
}

/// The inset list of the only card: (list rect, node), card-relative.
fn pr_list(snap: &Snapshot) -> (Rect, Built, layout::CardLayout) {
    let (built, l) = only_card(snap, &open());
    let block = l.find(Block::Prs).expect("the PR block is placed");
    let n = snap.sessions[0].prs.len() as f64;
    let list = Rect::new(
        layout::CARD_INSET,
        block.y + layout::BLOCK_GAP,
        layout::TEXT_W,
        n * ITEM_H,
    );
    (list, built, l)
}

fn item_rect(list: Rect, i: usize) -> Rect {
    Rect::new(list.x, list.y + i as f64 * ITEM_H, list.w, ITEM_H)
}

fn cy(r: &Rect) -> f64 {
    r.y + r.h / 2.0
}

fn text_in<'a>(node: &'a Node, want: &str, within: Rect) -> Option<&'a Rect> {
    node.cmds.iter().find_map(|c| match c {
        DrawCmd::Text { rect, text, .. }
            if text == want && rect.y >= within.y && rect.y < within.y + within.h =>
        {
            Some(rect)
        }
        _ => None,
    })
}

/// The hit under `at`, first registered first like the real view.
fn hit_under(b: &Built, node: &Node, at: Point) -> Option<Target> {
    node.hits
        .iter()
        .find(|h| h.rect.contains(at))
        .map(|h| b.targets.get(h.id).unwrap().clone())
}

fn hit_rect(b: &Built, node: &Node, want: impl Fn(&Target) -> bool) -> Rect {
    node.hits
        .iter()
        .find(|h| want(b.targets.get(h.id).unwrap()))
        .expect("the hit exists")
        .rect
}

#[test]
fn prs_block_height_is_fixed_per_pr() {
    let mut with_buttons = pr(1, "OPEN");
    with_buttons.rabbit = Some(Rabbit::AwaitingApproval);
    let merged = pr(2, "MERGED");
    let closed = pr(3, "CLOSED");
    let bare = bare_pr(4, "OPEN", None);
    for prs in [
        vec![],
        vec![bare.clone()],
        vec![with_buttons.clone()],
        vec![merged.clone()],
        vec![with_buttons.clone(), merged.clone(), closed.clone()],
        vec![bare, closed, with_buttons, merged],
    ] {
        let n = prs.len();
        let snap = snapshot(vec![session_with(prs)]);
        let l = card_layout(&snap.sessions[0], &snap, &open(), &FakeMeasure);
        match l.find(Block::Prs) {
            None => assert_eq!(n, 0),
            Some(p) => {
                assert_eq!(p.h, layout::BLOCK_GAP + n as f64 * 42.0, "{n} PRs");
                assert_eq!(p.h, layout::prs_height(n));
            }
        }
    }
}

#[test]
fn pr_item_draws_title_and_meta_on_two_lines() {
    let mut p = bare_pr(7571, "OPEN", Some("Fix the parser"));
    p.reviewers = 5;
    p.comments = 3;
    let snap = snapshot(vec![session_with(vec![p])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    let num = text_in(node, "#7571", it).expect("the number");
    let title = text_in(node, "Fix the parser", it).expect("the title");
    assert_eq!(num.y, it.y + 7.0);
    assert_eq!(num.h, 16.0);
    assert_eq!(title.y, it.y + 7.0);
    assert_eq!(num.x, it.x + 38.0);
    assert_eq!(title.x, num.x + 7.0 * 5.0 + 6.0, "6 pt after the number");
    let checks = text_in(node, "0/5", it).expect("the checks");
    let comments = text_in(node, "3", it).expect("the comments");
    assert_eq!(cy(checks), it.y + 24.0 + 7.0);
    assert_eq!(cy(comments), cy(checks));
    assert!(checks.x >= it.x + 38.0);
    // The badge: 20 x 20 at x0 + 10, centred, the branch symbol.
    assert!(symbols(node).contains(&"arrow.triangle.branch"));
    let badge = node.cmds.iter().find_map(|c| match c {
        DrawCmd::RoundedRect {
            rect,
            radius,
            fill: Some(f),
            ..
        } if f.token == ThemeColor::Green && *radius == 6.0 => Some(*rect),
        _ => None,
    });
    let badge = badge.expect("a green badge");
    assert_eq!(
        (badge.x, badge.w, badge.h),
        (it.x + 10.0, 20.0, 20.0),
        "{badge:?}"
    );
    assert_eq!(cy(&badge), cy(&it));
}

#[test]
fn pr_item_without_title_shows_only_the_number() {
    let mut p = bare_pr(7571, "OPEN", None);
    p.comments = 3;
    let snap = snapshot(vec![session_with(vec![p])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    let line_one: Vec<&str> = node
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text { rect, text, .. } if rect.y == it.y + 7.0 && rect.x >= it.x => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(line_one, ["#7571"]);
    // The hover text of an untitled item is `PR #n`.
    assert!(node.tips.iter().any(|t| t.text == "PR #7571"));
}

#[test]
fn open_pr_without_meta_centres_line_one() {
    let snap = snapshot(vec![session_with(vec![bare_pr(
        7571,
        "OPEN",
        Some("Alone"),
    )])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    let num = text_in(node, "#7571", it).unwrap();
    assert_eq!(cy(num), cy(&it));
}

#[test]
fn merged_pr_item_has_state_word_and_no_button() {
    let snap = snapshot(vec![session_with(vec![
        bare_pr(7567, "MERGED", Some("Old fix")),
        bare_pr(7568, "CLOSED", Some("Dropped")),
    ])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let merged = item_rect(list, 0);
    let (word, color) = {
        let r = text_in(node, "đã merge", merged).expect("the state word");
        let (_, c) = text_cmd(node, "đã merge").unwrap();
        (*r, *c)
    };
    assert_eq!(color.token, ThemeColor::TextSecondary);
    assert_eq!(cy(&word), merged.y + 24.0 + 7.0);
    let (_, title) = text_cmd(node, "Old fix").unwrap();
    assert_eq!(title.token, ThemeColor::TextSecondary);
    let (_, closed) = text_cmd(node, "đã đóng").unwrap();
    assert_eq!(closed.token, ThemeColor::TextSecondary);
    let syms = symbols(node);
    assert!(syms.contains(&"arrow.triangle.merge") && syms.contains(&"xmark"));
    let badge = |name: &str| {
        node.cmds.iter().find_map(|c| match c {
            DrawCmd::Symbol { name: n, color, .. } if n == name => Some(color.token),
            _ => None,
        })
    };
    assert_eq!(badge("arrow.triangle.merge"), Some(ThemeColor::Indigo));
    assert_eq!(badge("xmark"), Some(ThemeColor::Red));
    let t = texts(node);
    assert!(!t.contains(&"Nhờ Claude") && !t.contains(&"Nhắc duyệt"));
    assert!(
        !targets(&built, node)
            .iter()
            .any(|t| matches!(t, Target::AskClaude { .. } | Target::Nudge { .. }))
    );
}

fn awaiting() -> PRStatus {
    let mut p = bare_pr(7580, "OPEN", Some("Show thumbnails"));
    p.reviewers = 5;
    p.rabbit = Some(Rabbit::AwaitingApproval);
    p
}

#[test]
fn nudge_is_the_primary_button_left_of_ask_claude() {
    let snap = snapshot(vec![session_with(vec![awaiting()])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    let nudge = hit_rect(&built, node, |t| matches!(t, Target::Nudge { .. }));
    let ask = hit_rect(&built, node, |t| matches!(t, Target::AskClaude { .. }));
    assert!(nudge.x + nudge.w < ask.x, "{nudge:?} {ask:?}");
    assert_eq!(ask.x - (nudge.x + nudge.w), 6.0);
    assert_eq!(ask.x + ask.w, list.x + list.w - BTN_EDGE);
    for r in [nudge, ask] {
        assert_eq!(r.h, layout::PR_BUTTON_H);
        assert_eq!(cy(&r), cy(&it));
    }
    let fill_of = |r: Rect| {
        node.cmds.iter().find_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                fill: Some(f),
                ..
            } if *rect == r => Some(f.token),
            _ => None,
        })
    };
    assert_eq!(fill_of(nudge), Some(ThemeColor::Accent));
    assert_eq!(fill_of(ask), Some(ThemeColor::Fill));
    let (_, white) = text_cmd(node, "Nhắc duyệt").unwrap();
    assert_eq!(white.token, ThemeColor::White);
}

#[test]
fn whole_pr_item_opens_the_pr_except_its_buttons() {
    let mut second = awaiting();
    second.ref_.id = 7581;
    second.url = url("https://bitbucket.org/w/r/pull-requests/7581");
    let snap = snapshot(vec![session_with(vec![
        bare_pr(7567, "MERGED", Some("Old")),
        awaiting(),
        second,
    ])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let want_url = |id: i64| url(&format!("https://bitbucket.org/w/r/pull-requests/{id}"));
    // Merged item: all of it.
    let it0 = item_rect(list, 0);
    for at in [
        Point::new(it0.x + 2.0, it0.y + 2.0),
        Point::new(cy_x(&it0), cy(&it0)),
        Point::new(it0.x + it0.w - 2.0, it0.y + it0.h - 2.0),
    ] {
        assert_eq!(
            hit_under(&built, node, at),
            Some(Target::Link {
                session: "done".into(),
                url: want_url(7567)
            }),
            "{at:?}"
        );
    }
    // Open item: the link everywhere left of the buttons, the buttons win over it.
    let it1 = item_rect(list, 1);
    let nudge = hit_rect(
        &built,
        node,
        |t| matches!(t, Target::Nudge { pr, .. } if pr.id == 7580),
    );
    let ask = hit_rect(
        &built,
        node,
        |t| matches!(t, Target::AskClaude { pr, .. } if pr.id == 7580),
    );
    assert_eq!(
        hit_under(&built, node, Point::new(it1.x + 40.0, it1.y + 4.0)),
        Some(Target::Link {
            session: "done".into(),
            url: want_url(7580)
        })
    );
    assert!(matches!(
        hit_under(&built, node, Point::new(cy_x(&nudge), cy(&nudge))),
        Some(Target::Nudge { .. })
    ));
    assert!(matches!(
        hit_under(&built, node, Point::new(cy_x(&ask), cy(&ask))),
        Some(Target::AskClaude { .. })
    ));
    // The third item is its own link.
    let it2 = item_rect(list, 2);
    assert_eq!(
        hit_under(&built, node, Point::new(it2.x + 40.0, it2.y + 4.0)),
        Some(Target::Link {
            session: "done".into(),
            url: want_url(7581)
        })
    );
}

fn cy_x(r: &Rect) -> f64 {
    r.x + r.w / 2.0
}

#[test]
fn crowded_meta_line_drops_parts_before_the_buttons() {
    use claude_sessions_core::pull_requests::PRComment;
    let mut p = awaiting();
    p.title = Some("Show PDF thumbnails".into());
    p.changes_requested = 1;
    p.comments = 23;
    p.last_other = Some(PRComment {
        name: "Linh".into(),
        at: now() - Duration::from_secs(120),
    });
    let mut snap = snapshot(vec![session_with(vec![p])]);
    snap.pr_stale_since = Some(now() - Duration::from_secs(7 * 60));
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    let nudge = hit_rect(&built, node, |t| matches!(t, Target::Nudge { .. }));
    let limit = nudge.x - 8.0;
    let t = texts(node);
    // The clock goes first, then the person, then the word `CodeRabbit `.
    assert!(!t.contains(&"cũ 7m"), "{t:?}");
    assert!(!t.iter().any(|s| s.starts_with("Linh")), "{t:?}");
    assert!(t.contains(&"chờ duyệt"), "{t:?}");
    // What stays keeps its place and ends before the buttons.
    for c in &node.cmds {
        if let DrawCmd::Text { rect, .. } | DrawCmd::Symbol { rect, .. } = c
            && rect.y >= it.y
            && rect.y < it.y + it.h
            && rect.x < nudge.x
            && rect.x >= it.x
        {
            assert!(
                rect.x + rect.w <= limit + 0.001,
                "{rect:?} under the buttons"
            );
        }
    }
    // With room all of it shows, the word included.
    let mut roomy = awaiting();
    roomy.rabbit = Some(Rabbit::Approved);
    roomy.rabbit_threads = Some(claude_sessions_core::pull_requests::RabbitThreads {
        pending: 0,
        total: 1,
    });
    let snap = snapshot(vec![session_with(vec![roomy])]);
    let (_, built, _) = pr_list(&snap);
    assert!(texts(card(&built.scene, "done")).contains(&"CodeRabbit đã duyệt"));
}

#[test]
fn stale_clock_comes_after_the_rabbit_without_a_reviewer() {
    let mut p = awaiting();
    p.reviewers = 0;
    let mut snap = snapshot(vec![session_with(vec![p])]);
    snap.pr_stale_since = Some(now() - Duration::from_secs(7 * 60));
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    let clock = text_in(node, "cũ 7m", it).expect("the clock");
    let hare = node
        .cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::Symbol { name, rect, .. } if name == "hare" => Some(*rect),
            _ => None,
        })
        .expect("the hare");
    assert!(hare.x < clock.x, "{hare:?} {clock:?}");
}

#[test]
fn a_crowded_meta_line_never_draws_past_the_buttons() {
    let mut checked = 0;
    for comments_digits in 1..=15u32 {
        for changes_digits in 0..=15u32 {
            for reviewers in [0, 5, 123456] {
                let mut p = awaiting();
                p.reviewers = reviewers;
                p.comments = 10_i64.pow(comments_digits - 1);
                p.changes_requested = if changes_digits == 0 {
                    0
                } else {
                    10_i64.pow(changes_digits - 1)
                };
                let snap = snapshot(vec![session_with(vec![p])]);
                let (list, built, _) = pr_list(&snap);
                let node = card(&built.scene, "done");
                let it = item_rect(list, 0);
                let nudge = hit_rect(&built, node, |t| matches!(t, Target::Nudge { .. }));
                let limit = nudge.x - 8.0;
                for c in &node.cmds {
                    if let DrawCmd::Text { rect, .. } | DrawCmd::Symbol { rect, .. } = c
                        && rect.y >= it.y + 24.0
                        && rect.y < it.y + it.h
                        && rect.x < nudge.x
                        && rect.x >= it.x
                    {
                        assert!(
                            rect.x + rect.w <= limit + 0.001,
                            "{rect:?} past {limit} ({comments_digits} {changes_digits} {reviewers})"
                        );
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 0);
}

#[test]
fn long_title_truncates_before_the_buttons() {
    let long = "x".repeat(100);
    let mut p = awaiting();
    p.title = Some(long.clone());
    let snap = snapshot(vec![session_with(vec![p])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    let title = text_in(node, &long, it)
        .expect("the title is one text")
        .to_owned();
    let nudge = hit_rect(&built, node, |t| matches!(t, Target::Nudge { .. }));
    let ask = hit_rect(&built, node, |t| matches!(t, Target::AskClaude { .. }));
    assert!(title.w < 100.0 * 7.0, "the rect is narrower than the text");
    assert!(
        title.x + title.w <= nudge.x - 8.0 + 0.001,
        "{title:?} {nudge:?}"
    );
    assert!(ask.x + ask.w <= list.x + list.w - BTN_EDGE + 0.001);
    assert!(nudge.x > list.x + 38.0);
    let max_lines = node.cmds.iter().find_map(|c| match c {
        DrawCmd::Text {
            text, max_lines, ..
        } if *text == long => Some(*max_lines),
        _ => None,
    });
    assert_eq!(max_lines, Some(1), "one line, cut with an ellipsis");
}

#[test]
fn approval_failed_reason_is_the_rabbit_tooltip() {
    let mut p = bare_pr(7580, "OPEN", Some("Failing"));
    p.rabbit = Some(Rabbit::ApprovalFailed("Bitbucket 403".into()));
    let snap = snapshot(vec![session_with(vec![p])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    let tip = node
        .tips
        .iter()
        .find(|t| t.text == "Bitbucket 403")
        .expect("the reason is a tooltip");
    assert!(tip.rect.y >= it.y + 20.0 && tip.rect.y + tip.rect.h <= it.y + it.h);
    let (label, color) = text_cmd(node, "CodeRabbit duyệt lỗi").expect("the label");
    assert_eq!(color.token, ThemeColor::Red);
    assert!(tip.rect.contains(Point::new(label.x + 4.0, cy(label))));
    // The tip sits on the label alone: no other tip overlaps it.
    for other in node.tips.iter().filter(|t| t.text != "Bitbucket 403") {
        let (a, b) = (other.rect, tip.rect);
        let overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
        assert!(!overlap, "{other:?} overlaps the reason tip");
    }
}

#[test]
fn pr_send_result_without_button_stays_in_the_item() {
    use claude_sessions_core::pull_requests::RabbitThreads;
    let mut p = bare_pr(7631, "OPEN", Some("Done review"));
    p.reviewers = 5;
    p.rabbit = Some(Rabbit::Approved);
    p.rabbit_threads = Some(RabbitThreads {
        pending: 0,
        total: 1,
    });
    let mut key = SendKey::new("done", SendKind::AskClaude);
    key.pr = Some(p.ref_.clone());
    let mut snap = snapshot(vec![session_with(vec![p])]);
    snap.sends.insert(key, (SendState::Sent, now()));
    let (list, built, l) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let it = item_rect(list, 0);
    assert_eq!(l.find(Block::Prs).unwrap().h, layout::prs_height(1));
    let sent = text_in(node, "Đã gửi ✓", it).expect("the result is drawn in the item");
    assert!(sent.x + sent.w <= list.x + list.w - BTN_EDGE + 0.001);
    assert_eq!(cy(sent), cy(&it));
    // No capsule: the button is gone, so is its hit.
    assert!(
        !targets(&built, node)
            .iter()
            .any(|t| matches!(t, Target::AskClaude { .. }))
    );
    // The title and the meta line stop before it.
    let title = text_in(node, "Done review", it).unwrap();
    assert!(title.x + title.w <= sent.x - 8.0 + 0.001);
    let rabbit = text_in(node, "CodeRabbit đã duyệt", it)
        .or_else(|| text_in(node, "đã duyệt", it))
        .expect("the label");
    assert!(rabbit.x + rabbit.w <= sent.x - 8.0 + 0.001);
}

#[test]
fn pr_list_uses_appearance_tokens() {
    use claude_sessions_core::scene::Appearance;
    assert_eq!(
        ThemeColor::Inset.rgba(Appearance::Dark),
        [0.0, 0.0, 0.0, 0.16]
    );
    assert_eq!(
        ThemeColor::Inset.rgba(Appearance::Light),
        [0.0, 0.0, 0.0, 0.035]
    );
    assert_eq!(
        ThemeColor::InsetEdge.rgba(Appearance::Dark),
        [1.0, 1.0, 1.0, 0.06]
    );
    assert_eq!(
        ThemeColor::InsetEdge.rgba(Appearance::Light),
        [0.0, 0.0, 0.0, 0.06]
    );
    // The scene names the tokens, not colours: both appearances resolve them.
    let snap = snapshot(vec![session_with(vec![awaiting()])]);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let rect = node.cmds.iter().find_map(|c| match c {
        DrawCmd::RoundedRect {
            rect,
            radius,
            fill: Some(f),
            ..
        } if f.token == ThemeColor::Inset && *rect == list => Some((*radius, f.alpha)),
        _ => None,
    });
    assert_eq!(rect, Some((layout::PR_LIST_RADIUS, 1.0)));
    let edge = node.cmds.iter().any(|c| {
        matches!(
            c,
            DrawCmd::RoundedRect { stroke: Some(s), radius, .. }
                if s.token == ThemeColor::InsetEdge
                    && s.alpha == 0.5
                    && *radius == layout::PR_LIST_RADIUS - 0.5
        )
    });
    assert!(
        edge,
        "the 1 pt inner edge is drawn with its token at half alpha"
    );
    // Text colours are tokens too, so they follow the appearance.
    let (_, c) = text_cmd(node, "Show thumbnails").unwrap();
    assert_eq!(c.token, ThemeColor::Text);
}

#[test]
fn fresh_first_item_tint_follows_the_list_corners() {
    use claude_sessions_core::store::NewMark;
    let a = bare_pr(1, "OPEN", Some("A"));
    let b = bare_pr(2, "OPEN", Some("B"));
    let c = bare_pr(3, "OPEN", Some("C"));
    let mut snap = snapshot(vec![session_with(vec![a.clone(), b, c])]);
    let mut mark = NewMark::default();
    mark.prs.insert(a.ref_.clone());
    snap.unseen.insert("done".into(), mark);
    let (list, built, _) = pr_list(&snap);
    let node = card(&built.scene, "done");
    let tints: Vec<(Rect, f64)> = node
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                radius,
                fill: Some(f),
                ..
            } if f.token == ThemeColor::Indigo && f.alpha == 0.10 => Some((*rect, *radius)),
            _ => None,
        })
        .collect();
    assert_eq!(tints.len(), 1, "only the fresh item is tinted: {tints:?}");
    let (rect, radius) = tints[0];
    assert_eq!((rect.x, rect.y, rect.w), (list.x, list.y, list.w));
    assert_eq!(radius, layout::PR_LIST_RADIUS, "the list's corner radius");
    assert!(rect.h <= ITEM_H, "the tint stays inside its own item");
    // The sparkle comes before the number.
    let it = item_rect(list, 0);
    let (num, _) = text_cmd(node, "#1").unwrap();
    assert_eq!(num.x, it.x + 38.0 + 18.0);
    assert!(symbols(node).contains(&"sparkle"));
    // Separators sit between the items only.
    let seps: Vec<Rect> = node
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::RoundedRect {
                rect,
                fill: Some(f),
                ..
            } if f.token == ThemeColor::Separator
                && rect.h == 0.5
                && rect.y >= list.y
                && rect.y < list.y + list.h =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .collect();
    assert_eq!(seps.len(), 2);
    for (i, s) in seps.iter().enumerate() {
        assert_eq!(s.y, list.y + (i + 1) as f64 * ITEM_H);
        assert_eq!(s.x, list.x + 38.0);
        assert_eq!(s.x + s.w, list.x + list.w);
    }
}
