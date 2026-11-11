use crate::support::*;
use claude_sessions_core::transcript::*;
use serde_json::{Value, json};
use std::fs;
use std::time::{Duration, UNIX_EPOCH};
use url::Url;

const PLAIN_USER: &str = r#"{"type":"user","message":{"content":"hi"}}"#;

fn data(lines: &[String]) -> Vec<u8> {
    lines.join("\n").into_bytes()
}

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

fn assistant(text: &str) -> String {
    assistant_line(Some(text), false)
}

fn user(text: &str) -> String {
    user_line(text, false, None)
}

#[test]
fn project_dir_name_replaces_non_alphanumerics() {
    assert_eq!(
        project_dir_name("/Users/phong/.claude/skills-repo"),
        "-Users-phong--claude-skills-repo"
    );
}

#[test]
fn last_reply_picks_last_text() {
    assert_eq!(
        last_reply(&data(&[assistant("one"), assistant("two")])).as_deref(),
        Some("two")
    );
}

#[test]
fn last_reply_skips_tool_only_entries() {
    let tool_only =
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}"#
            .to_string();
    assert_eq!(
        last_reply(&data(&[
            assistant("the text"),
            tool_only,
            PLAIN_USER.to_string()
        ]))
        .as_deref(),
        Some("the text")
    );
}

#[test]
fn last_reply_ignores_cut_first_line() {
    let cut = assistant("x")[20..].to_string();
    assert_eq!(
        last_reply(&data(&[cut.clone(), assistant("kept")])).as_deref(),
        Some("kept")
    );
    assert_eq!(last_reply(&data(&[cut])), None);
}

#[test]
fn last_reply_skips_sidechain() {
    assert_eq!(
        last_reply(&data(&[
            assistant("main"),
            assistant_line(Some("side"), true)
        ]))
        .as_deref(),
        Some("main")
    );
}

#[test]
fn last_reply_collapses_without_cap() {
    assert_eq!(
        last_reply(&data(&[assistant("a\n\nb   c\n")])).as_deref(),
        Some("a b c")
    );
    assert_eq!(
        last_reply(&data(&[assistant(&"x".repeat(400))])).map(|s| s.chars().count()),
        Some(400)
    );
}

#[test]
fn last_reply_none() {
    assert_eq!(
        last_reply(&data(&[
            PLAIN_USER.to_string(),
            assistant_line(None, false)
        ])),
        None
    );
}

#[test]
fn find_falls_back_to_scan() {
    let projects = make_temp_dir();
    let other = projects.path().join("some-other-dir");
    fs::create_dir_all(&other).unwrap();
    let file = other.join("sid.jsonl");
    fs::write(&file, b"").unwrap();
    let found = find("sid", "/nope", projects.path()).unwrap();
    assert_eq!(found.canonicalize().unwrap(), file.canonicalize().unwrap());
    assert_eq!(find("missing", "/nope", projects.path()), None);
}

fn bash(id: &str) -> String {
    tool_use_line(id, "Bash", json!({"command": "npm test"}), false)
}

fn tool(id: &str, name: &str, input: Value) -> String {
    tool_use_line(id, name, input, false)
}

#[test]
fn pending_one_bash() {
    assert_eq!(
        pending(&data(&[bash("t1")])),
        Some(PendingState::One(Pending {
            tool_id: "t1".into(),
            tool: "Bash".into(),
            text: "npm test".into()
        }))
    );
}

#[test]
fn pending_cleared_by_tool_result() {
    assert_eq!(pending(&data(&[bash("t1"), tool_result_line("t1")])), None);
}

#[test]
fn pending_parallel_is_unclear() {
    assert_eq!(
        pending(&data(&[bash("t1"), bash("t2")])),
        Some(PendingState::Unclear { count: 2 })
    );
}

#[test]
fn pending_agent_only_is_unclear() {
    assert_eq!(
        pending(&data(&[tool("t1", "Agent", json!({}))])),
        Some(PendingState::Unclear { count: 1 })
    );
}

#[test]
fn pending_unknown_tool_is_unclear() {
    assert_eq!(
        pending(&data(&[tool("t1", "mcp__x__y", json!({}))])),
        Some(PendingState::Unclear { count: 1 })
    );
    assert_eq!(
        pending(&data(&[tool("t1", "ExitPlanMode", json!({}))])),
        Some(PendingState::Unclear { count: 1 })
    );
}

#[test]
fn pending_empty_command_is_unclear() {
    assert_eq!(
        pending(&data(&[tool("t1", "Bash", json!({}))])),
        Some(PendingState::Unclear { count: 1 })
    );
    assert_eq!(
        pending(&data(&[tool("t1", "Bash", json!({"command": ""}))])),
        Some(PendingState::Unclear { count: 1 })
    );
}

#[test]
fn pending_skips_sidechain() {
    assert_eq!(
        pending(&data(&[tool_use_line(
            "t1",
            "Bash",
            json!({"command": "x"}),
            true
        )])),
        None
    );
}

#[test]
fn pending_text_per_tool() {
    fn text(name: &str, input: Value) -> Option<String> {
        match pending(&data(&[tool_use_line("t1", name, input, false)])) {
            Some(PendingState::One(p)) => Some(p.text),
            _ => None,
        }
    }
    assert_eq!(
        text("Edit", json!({"file_path": "/a/b.swift"})).as_deref(),
        Some("/a/b.swift")
    );
    assert_eq!(
        text("WebFetch", json!({"url": "https://x.dev"})).as_deref(),
        Some("https://x.dev")
    );
    assert_eq!(
        text("WebSearch", json!({"query": "swift testing"})).as_deref(),
        Some("swift testing")
    );
    assert_eq!(
        text("Grep", json!({"pattern": "foo.*"})).as_deref(),
        Some("foo.*")
    );
}

#[test]
fn clean_prompt_rules() {
    assert_eq!(
        clean_prompt("<command-name>/cook</command-name><command-message>cook</command-message><command-args>plans/x/plan.md</command-args>").as_deref(),
        Some("/cook plans/x/plan.md")
    );
    assert_eq!(
        clean_prompt(
            "<command-name>/clear</command-name>\n<command-message>clear</command-message>\n<command-args></command-args>"
        ),
        None
    );
    assert_eq!(
        clean_prompt("<local-command-stdout>ok</local-command-stdout>"),
        None
    );
    assert_eq!(
        clean_prompt("Caveat: The messages below were generated"),
        None
    );
    assert_eq!(
        clean_prompt("sửa bug\n<system-reminder>x</system-reminder>  đăng nhập").as_deref(),
        Some("sửa bug đăng nhập")
    );
    assert_eq!(
        clean_prompt("xem log <command-name>/x</command-name>").as_deref(),
        Some("xem log <command-name>/x</command-name>")
    );
    let long = "a".repeat(70);
    assert_eq!(clean_prompt(&long).as_deref(), Some(long.as_str()));
}

#[test]
fn first_prompt_skips_clear_and_local_command() {
    let meta = r#"{"type":"user","isMeta":true,"message":{"content":"meta text"}}"#.to_string();
    let local = user("<local-command-stdout>ok</local-command-stdout>");
    let lines = [
        command_line("/clear", ""),
        local,
        meta,
        tool_result_line("t"),
        user("sửa widget"),
        user("câu sau"),
    ];
    assert_eq!(first_prompt(&data(&lines)).as_deref(), Some("sửa widget"));
}

#[test]
fn first_prompt_array_content() {
    let l =
        r#"{"type":"user","message":{"content":[{"type":"text","text":"xin chào"}]}}"#.to_string();
    assert_eq!(first_prompt(&data(&[l])).as_deref(), Some("xin chào"));
}

#[test]
fn first_prompt_none() {
    assert_eq!(
        first_prompt(&data(&[command_line("/clear", ""), assistant("x")])),
        None
    );
}

#[test]
fn last_ai_title_picks_newest() {
    let d = data(&[
        ai_title_line("old"),
        user("a"),
        ai_title_line("new"),
        assistant("b"),
    ]);
    assert_eq!(last_ai_title(&d).as_deref(), Some("new"));
}

#[test]
fn head_drops_partial_line() {
    let dir = make_temp_dir();
    let f = dir.path().join("h.jsonl");
    let (l1, l2, l3) = (user("one"), user("two is a longer line"), user("three"));
    fs::write(&f, [l1.as_str(), l2.as_str(), l3.as_str()].join("\n")).unwrap();
    let bytes = l1.len() + 1 + 10;
    let h = head(&f, bytes);
    assert_eq!(first_prompt(&h).as_deref(), Some("one"));
    assert_eq!(String::from_utf8(h).unwrap(), format!("{l1}\n"));
}

#[test]
fn label_rules() {
    let cases = [
        (
            "https://bitbucket.org/t6engineering/talo6-dct-backend/pull-requests/7562",
            "PR #7562",
        ),
        (
            "https://bitbucket.org/t6engineering/talo6-dct-frontend/pull-requests/6867/_/diff",
            "PR #6867",
        ),
        ("https://github.com/formio/formio.js/pull/3759", "PR #3759"),
        ("https://gitlab.com/a/b/-/merge_requests/12", "PR #12"),
        (
            "https://talosix.atlassian.net/browse/T6IR-14728",
            "T6IR-14728",
        ),
        (
            "https://talosix.atlassian.net/browse/T6IR-13103?focusedCommentId=88378",
            "T6IR-13103",
        ),
        ("https://github.com/a/b/issues/42", "#42"),
        (
            "https://www.anthropic.com/legal/consumer-terms",
            "anthropic.com",
        ),
        (
            "https://bitbucket.org/t6engineering/talo6-dct-frontend/pull-requests/new?source=x",
            "bitbucket.org",
        ),
        ("http://localhost:8080", "localhost"),
    ];
    for (u, l) in cases {
        assert_eq!(label_for(&url(u)), l, "{u}");
    }
}

#[test]
fn display_text_replaces_urls_and_cuts() {
    assert_eq!(
        display_text("/cook https://talosix.atlassian.net/browse/T6IR-14728", 140),
        "/cook T6IR-14728"
    );
    assert_eq!(
        display_text("xem **PR** [đây](https://github.com/a/b/pull/9).", 140),
        "xem **PR** đây."
    );
    assert_eq!(
        display_text("done: https://bitbucket.org/x/y/pull-requests/7562**", 140),
        "done: PR #7562**"
    );
    assert_eq!(
        display_text(&"a".repeat(700), 600),
        format!("{}…", "a".repeat(600))
    );
    let exact = "a".repeat(600);
    assert_eq!(display_text(&exact, 600), exact);
}

#[test]
fn ticket_keys_bare() {
    assert_eq!(
        ticket_keys("fix T6IR-14725, AC-1 và UTF-8 trên bugfix/T6IR-14726"),
        ["T6IR-14725", "AC-1", "UTF-8", "T6IR-14726"]
    );
    assert!(ticket_keys("T6IR-1a").is_empty());
}

#[test]
fn jira_ticket_from_browse_and_selected_issue() {
    let a = jira_ticket(&url(
        "https://talosix.atlassian.net/browse/T6IR-13204?focusedCommentId=1",
    ))
    .unwrap();
    assert_eq!(
        (a.0.as_str(), a.1.as_str()),
        ("T6IR-13204", "talosix.atlassian.net")
    );
    let b = jira_ticket(&url(
        "https://talosix.atlassian.net/jira/software/projects/T6IR/boards/1?selectedIssue=T6IR-9",
    ))
    .unwrap();
    assert_eq!(
        (b.0.as_str(), b.1.as_str()),
        ("T6IR-9", "talosix.atlassian.net")
    );
    assert_eq!(jira_ticket(&url("https://example.com/x")), None);
    assert_eq!(
        jira_ticket(&url("https://talosix.atlassian.net/browse/ABCDEFGHIJK-1")),
        None
    );
    assert_eq!(
        jira_ticket(&url("https://talosix.atlassian.net/browse/ABCDEFGHIJ-1"))
            .map(|t| t.0)
            .as_deref(),
        Some("ABCDEFGHIJ-1")
    );
}

#[test]
fn is_pull_request_shapes() {
    assert!(is_pull_request(&url(
        "https://bitbucket.org/a/b/pull-requests/1"
    )));
    assert!(is_pull_request(&url("https://github.com/a/b/pull/2")));
    assert!(is_pull_request(&url(
        "https://gitlab.com/a/b/-/merge_requests/3"
    )));
    assert!(!is_pull_request(&url(
        "https://talosix.atlassian.net/browse/T6IR-1"
    )));
}

#[test]
fn tool_summary_rules() {
    let s = |name: &str, input: Value| tool_summary(name, &input);
    assert_eq!(
        s("Bash", json!({"command": "swift test"})),
        "Bash: swift test"
    );
    assert_eq!(
        s("Edit", json!({"file_path": "/a/b/Session.swift"})),
        "Edit Session.swift"
    );
    assert_eq!(s("Read", json!({"file_path": "/a/b/c.md"})), "Read c.md");
    assert_eq!(s("Write", json!({"file_path": "/a/b/c.md"})), "Write c.md");
    assert_eq!(
        s("MultiEdit", json!({"file_path": "/a/b/c.md"})),
        "MultiEdit c.md"
    );
    assert_eq!(
        s("NotebookEdit", json!({"file_path": "/a/b/c.ipynb"})),
        "NotebookEdit c.ipynb"
    );
    assert_eq!(
        s("Glob", json!({"pattern": "**/*.swift"})),
        "Glob: **/*.swift"
    );
    assert_eq!(s("Grep", json!({"pattern": "foo"})), "Grep: foo");
    assert_eq!(
        s("WebFetch", json!({"url": "https://x.y"})),
        "WebFetch: https://x.y"
    );
    assert_eq!(
        s("WebSearch", json!({"query": "swift"})),
        "WebSearch: swift"
    );
    assert_eq!(
        s("Agent", json!({"description": "find it"})),
        "Agent: find it"
    );
    assert_eq!(
        s("Task", json!({"description": "find it"})),
        "Agent: find it"
    );
    assert_eq!(s("Skill", json!({"skill": "cook"})), "Skill: cook");
    assert_eq!(s("mcp__cmux-cua__click", json!({})), "click");
    assert_eq!(s("TodoWrite", json!({"todos": []})), "TodoWrite");
    assert_eq!(s("Bash", json!({})), "Bash");
    assert_eq!(s("Bash", json!({"command": "a\n  b"})), "Bash: a b");
    let long = s("Bash", json!({"command": "x".repeat(200)}));
    let expected: String = format!("Bash: {}", "x".repeat(200))
        .chars()
        .take(120)
        .collect();
    assert_eq!(long, format!("{expected}…"));
}

#[test]
fn last_tool_use_picks_last() {
    let d = data(&[
        tool_use_line("1", "Bash", json!({"command": "one"}), false),
        tool_use_line("2", "Edit", json!({"file_path": "/a/two.swift"}), false),
        tool_result_line("2"),
    ]);
    assert_eq!(last_tool_use(&d).as_deref(), Some("Edit two.swift"));
}

#[test]
fn last_tool_use_skips_sidechain() {
    let d = data(&[
        tool_use_line("1", "Bash", json!({"command": "main"}), false),
        tool_use_line("2", "Read", json!({"file_path": "/a/b"}), true),
    ]);
    assert_eq!(last_tool_use(&d).as_deref(), Some("Bash: main"));
}

#[test]
fn last_tool_use_before_long_line() {
    let long = user(&"x".repeat(300_000));
    assert_eq!(
        last_tool_use(&data(&[
            tool_use_line("1", "Bash", json!({"command": "go"}), false),
            long
        ]))
        .as_deref(),
        Some("Bash: go")
    );
}

#[test]
fn last_tool_use_none() {
    let text = json_line(
        json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "hello"}]}}),
    );
    assert_eq!(last_tool_use(&data(&[text.clone(), text])), None);
}

#[test]
fn agent_changes_tracks_opened_and_closed() {
    let d = data(&[
        tool_use_line("1", "Agent", json!({"description": "review A"}), false),
        tool_use_line("2", "Task", json!({"description": "review B"}), false),
        tool_use_line("3", "Agent", json!({"description": "side"}), true),
        tool_use_line("4", "Bash", json!({"command": "ls"}), false),
        tool_result_line("1"),
    ]);
    let c = agent_changes(&d, &[]);
    assert_eq!(
        c.opened.iter().map(|o| o.0.as_str()).collect::<Vec<_>>(),
        ["1", "2"]
    );
    assert_eq!(
        c.opened.iter().map(|o| o.1.as_str()).collect::<Vec<_>>(),
        ["review A", "review B"]
    );
    assert_eq!(c.closed, ["1"]);
}

#[test]
fn agent_changes_closes_agents_opened_in_an_earlier_chunk() {
    let d = data(&[tool_result_line("1"), tool_result_line("9")]);
    assert!(agent_changes(&d, &[]).closed.is_empty());
    assert_eq!(agent_changes(&d, &["1".to_string()]).closed, ["1"]);
}

const META_LINE: &str = r#"{"type":"last-prompt","lastPrompt":"x"}"#;

fn ended(background: i64) -> Option<MainTurn> {
    Some(MainTurn::Ended {
        background,
        at: None,
    })
}

#[test]
fn main_turn_ended_with_background_agents() {
    let d = data(&[
        assistant("done"),
        turn_duration_line(Some(3), Some(0), false, None),
        META_LINE.to_string(),
    ]);
    assert_eq!(main_turn(&d), ended(3));
    assert_eq!(
        main_turn(&data(&[turn_duration_line(Some(2), Some(1), false, None)])),
        ended(3)
    );
}

#[test]
fn main_turn_running_after_new_main_line() {
    let base = [
        assistant("done"),
        turn_duration_line(Some(3), None, false, None),
        META_LINE.to_string(),
    ];
    let with = |extra: String| data(&[base.as_slice(), &[extra]].concat());
    assert_eq!(main_turn(&with(user("go"))), Some(MainTurn::Running));
    assert_eq!(
        main_turn(&with(assistant_line(None, false))),
        Some(MainTurn::Running)
    );
}

#[test]
fn main_turn_ignores_sidechain() {
    let d = data(&[
        turn_duration_line(Some(3), None, false, None),
        assistant_line(Some("side"), true),
        turn_duration_line(Some(0), None, true, None),
    ]);
    assert_eq!(main_turn(&d), ended(3));
    let sc = data(&[
        turn_duration_line(Some(3), None, false, None),
        assistant_line(Some("side"), true),
        tool_use_line("x", "Bash", json!({}), true),
    ]);
    assert_eq!(main_turn(&sc), ended(3));
}

#[test]
fn main_turn_ended_without_counts() {
    assert_eq!(
        main_turn(&data(&[turn_duration_line(None, None, false, None)])),
        ended(0)
    );
}

#[test]
fn main_turn_carries_turn_end_time() {
    let t = UNIX_EPOCH + Duration::from_secs_f64(1_800_000_000.123);
    let Some(MainTurn::Ended { background, at }) =
        main_turn(&data(&[turn_duration_line(None, None, false, Some(t))]))
    else {
        panic!("not ended");
    };
    assert_eq!(background, 0);
    let at = at.unwrap();
    let diff = at.duration_since(t).unwrap_or_else(|e| e.duration());
    assert!(diff < Duration::from_millis(1));
}

#[test]
fn main_turn_nil_without_relevant_lines() {
    assert_eq!(
        main_turn(&data(&[META_LINE.to_string(), META_LINE.to_string()])),
        None
    );
    assert_eq!(main_turn(&[]), None);
}
