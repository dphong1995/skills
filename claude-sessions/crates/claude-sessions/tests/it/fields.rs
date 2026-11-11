use crate::support::{FakeMeasure, snapshot};
use claude_sessions::fields;
use claude_sessions::hits::{Click, ClickEnv, Target};
use claude_sessions::views::{View, build};
use claude_sessions_core::feed::{BlockState, FeedItem, FeedOption, FeedQuestion};
use claude_sessions_core::panel_state::{EditKind, Editing, PanelState};
use claude_sessions_core::store::{Action, Session, Snapshot};
use claude_sessions_core::transcript::{Pending, PendingState};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

fn finished(id: &str) -> Session {
    let mut s = Session::new(id, id, "/work/front", "idle");
    s.last_reply = Some("done".into());
    s
}

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

fn click(
    target: &Target,
    snap: &Snapshot,
    panel: &PanelState,
    drafts: &HashMap<String, String>,
) -> Click {
    target.click(&ClickEnv {
        snap,
        panel,
        now: SystemTime::UNIX_EPOCH,
        text: fields::text_for(target, drafts),
        pr_prompt: "",
    })
}

#[test]
fn typed_reply_reaches_the_reply_action() {
    let snap = snapshot(vec![finished("a")]);
    let (mut panel, mut drafts) = (PanelState::new(), HashMap::new());
    fields::changed("reply-a", "hello", &mut drafts, &mut panel, &snap);
    let target = fields::submit_target("reply-a", &drafts, &panel, &snap).unwrap();
    assert_eq!(target, Target::ReplySend("a".into()));
    assert_eq!(
        click(&target, &snap, &panel, &drafts),
        Click::Action(Action::Reply {
            session: "a".into(),
            text: "hello".into()
        })
    );
}

#[test]
fn empty_reply_does_not_submit() {
    let snap = snapshot(vec![finished("a")]);
    let (mut panel, mut drafts) = (PanelState::new(), HashMap::new());
    assert_eq!(
        fields::submit_target("reply-a", &drafts, &panel, &snap),
        None
    );
    fields::changed("reply-a", "  ", &mut drafts, &mut panel, &snap);
    assert_eq!(
        fields::submit_target("reply-a", &drafts, &panel, &snap),
        None
    );
}

#[test]
fn typed_deny_reason_reaches_the_deny_action() {
    let snap = snapshot(vec![approval("a")]);
    let (mut panel, mut drafts) = (PanelState::new(), HashMap::new());
    fields::changed("deny-a", "no thanks", &mut drafts, &mut panel, &snap);
    let target = fields::submit_target("deny-a", &drafts, &panel, &snap).unwrap();
    assert_eq!(
        click(&target, &snap, &panel, &drafts),
        Click::Action(Action::Deny {
            session: "a".into(),
            tool_id: "t1".into(),
            reason: "no thanks".into()
        })
    );
}

#[test]
fn reply_and_deny_drafts_of_one_session_stay_apart() {
    let snap = snapshot(vec![approval("a")]);
    let (mut panel, mut drafts) = (PanelState::new(), HashMap::new());
    fields::changed("reply-a", "r", &mut drafts, &mut panel, &snap);
    fields::changed("deny-a", "d", &mut drafts, &mut panel, &snap);
    assert_eq!(drafts.len(), 2);
}

#[test]
fn typed_other_text_reaches_the_answer_action() {
    let snap = snapshot(vec![question("a")]);
    let (mut panel, mut drafts) = (PanelState::new(), HashMap::new());
    assert_eq!(
        fields::submit_target("other-a#0", &drafts, &panel, &snap),
        None
    );
    fields::changed("other-a#0", "Memcached", &mut drafts, &mut panel, &snap);
    let target = fields::submit_target("other-a#0", &drafts, &panel, &snap).unwrap();
    let Click::Action(Action::Answer { other, .. }) = click(&target, &snap, &panel, &drafts) else {
        panic!("not an answer");
    };
    assert_eq!(other[&0], "Memcached");
}

#[test]
fn drafts_fill_the_fields_of_a_rebuilt_scene() {
    let snap = snapshot(vec![finished("a")]);
    let mut panel = PanelState::new();
    panel.hovering = true;
    panel.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    let clock = |_: SystemTime| String::new();
    let mut built = build(&View {
        snap: &snap,
        panel: &panel,
        measure: &FakeMeasure,
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
        clock: &clock,
    });
    let drafts = HashMap::from([("a#reply".to_string(), "kept".to_string())]);
    fields::fill(&mut built.scene, &drafts);
    let field = built
        .scene
        .nodes
        .iter()
        .flat_map(|n| &n.fields)
        .find(|f| f.key == "reply-a")
        .unwrap();
    assert_eq!(field.text, "kept");
}

#[test]
fn cancel_ends_only_the_field_that_is_open() {
    let editing = |id: &str, kind| Editing {
        id: id.into(),
        kind,
        tool_id: None,
    };
    let mut panel = PanelState::default();
    panel.editing = Some(editing("s1", EditKind::Reply));
    panel.keyboard = true;
    // Another session's or another kind's field is not the open one.
    assert!(!fields::cancel("reply-s2", &mut panel));
    assert!(!fields::cancel("deny-s1", &mut panel));
    assert!(panel.editing.is_some());
    assert!(fields::cancel("reply-s1", &mut panel));
    assert!(panel.editing.is_none());
    assert!(panel.keyboard, "keyboard mode stays");
    assert!(!fields::cancel("reply-s1", &mut panel));
    panel.editing = Some(editing("s1", EditKind::Deny));
    assert!(fields::cancel("deny-s1", &mut panel));
    assert!(panel.editing.is_none());
}

#[test]
fn cancel_in_other_field_closes_that_field_only() {
    let answer = |id: &str| Editing {
        id: id.into(),
        kind: EditKind::Answer,
        tool_id: None,
    };
    let mut panel = PanelState::default();
    panel.other_key = Some("s1#0".into());
    panel.editing = Some(answer("s1"));
    assert!(!fields::cancel("other-s2#0", &mut panel));
    assert!(panel.other_key.is_some() && panel.editing.is_some());
    assert!(fields::cancel("other-s1#0", &mut panel));
    assert!(panel.other_key.is_none());
    assert!(panel.editing.is_none(), "Esc ends the answer editing");
    assert!(!fields::cancel("bogus", &mut panel));

    panel.other_key = Some("s1#0".into());
    panel.editing = Some(Editing {
        id: "s1".into(),
        kind: EditKind::Reply,
        tool_id: None,
    });
    assert!(fields::cancel("other-s1#0", &mut panel));
    assert!(panel.editing.is_some(), "an unrelated editor is untouched");
}

#[test]
fn opening_the_other_field_starts_answer_editing_and_keeps_keys_for_the_field() {
    use claude_sessions::hits::PanelChange;
    use claude_sessions_core::keyboard::{KeyCommand, KeyEvent, Mods, key_code};
    let snap = snapshot(vec![question("s1")]);
    let mut panel = PanelState::default();
    panel.keyboard = true;
    PanelChange::OpenOther {
        session: "s1".into(),
        question: 0,
    }
    .apply(&mut panel, &snap, SystemTime::UNIX_EPOCH);
    assert_eq!(panel.other_key.as_deref(), Some("s1#0"));
    assert_eq!(
        panel.editing,
        Some(Editing {
            id: "s1".into(),
            kind: EditKind::Answer,
            tool_id: None
        })
    );
    assert!(panel.wants_key());
    for (code, chars) in [
        (key_code::ESCAPE, ""),
        (key_code::RETURN, ""),
        (0, "1"),
        (0, "r"),
    ] {
        let event = KeyEvent {
            key_code: code,
            chars: chars.into(),
            mods: Mods::default(),
        };
        assert_eq!(
            KeyCommand::resolve(&event, &snap, &panel),
            KeyCommand::Pass,
            "{chars:?} {code}"
        );
    }
    assert!(fields::cancel("other-s1#0", &mut panel));
    assert!(panel.editing.is_none() && panel.keyboard);
}
