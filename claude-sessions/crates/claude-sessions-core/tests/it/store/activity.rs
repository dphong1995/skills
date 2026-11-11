//! Swift `StoreTests.swift` from `onlyJiraAndPRButtons` through `activityReadsAtMostWindow`.

use super::*;
use claude_sessions_core::text::status_text;
use serde_json::json;
use std::path::PathBuf;

#[test]
fn only_jira_and_pr_buttons() {
    let lines = [assistant_line(
        Some("https://example.com/x https://docs.coderabbit.ai https://github.com/a/b/pull/1"),
        false,
    )];
    let (store, _d, _p) = scan_store(&lines, "idle");
    assert_eq!(labels(&store), Some(vec!["PR #1".to_string()]));
}

#[test]
fn unknown_prefix_dropped() {
    let lines = [user_line(
        &format!("AC-1 UTF-8 https://{JIRA_HOST}/browse/T6IR-1"),
        false,
        None,
    )];
    let (store, _d, _p) = scan_store(&lines, "idle");
    assert_eq!(labels(&store), Some(vec!["T6IR-1".to_string()]));
}

#[test]
fn bare_key_shown_once_project_learned() {
    let lines = [
        user_line("T6IR-5", false, None),
        assistant_line(
            Some(&format!("see https://{JIRA_HOST}/browse/T6IR-1")),
            false,
        ),
    ];
    let (store, _d, _p) = scan_store(&lines, "idle");
    assert_eq!(
        labels(&store),
        Some(vec!["T6IR-5".to_string(), "T6IR-1".to_string()])
    );
    assert_eq!(
        store.jira_projects.get("T6IR").map(String::as_str),
        Some(JIRA_HOST)
    );
    assert_eq!(
        store.sessions[0].links[0].url.as_str(),
        format!("https://{JIRA_HOST}/browse/T6IR-5")
    );
}

#[test]
fn ticket_button_cache_invalidates() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[user_line("T6IR-5", false, None), String::new()],
    );
    write_session(dir.path(), "a", me(), "idle", 0.0, "a", FRONTEND);
    let links = |store: &Store, id: &str| -> Option<Vec<String>> {
        store
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.links.iter().map(|l| l.label.clone()).collect())
    };
    store.refresh(SystemTime::now());
    store.refresh(SystemTime::now());
    assert_eq!(links(&store, "a"), Some(vec![]));
    // Another session teaches the T6IR prefix: the bare key appears.
    write_transcript(
        projects.path(),
        BACKEND,
        "b",
        &[
            user_line(&format!("https://{JIRA_HOST}/browse/T6IR-1"), false, None),
            String::new(),
        ],
    );
    write_session(dir.path(), "b", me(), "idle", 0.0, "b", BACKEND);
    store.refresh(SystemTime::now());
    store.refresh(SystemTime::now());
    assert_eq!(links(&store, "a"), Some(vec!["T6IR-5".to_string()]));
    assert_eq!(links(&store, "b"), Some(vec!["T6IR-1".to_string()]));
}

#[test]
fn ticket_buttons_survive_busy() {
    let lines = [
        user_line(
            &format!("/cook https://{JIRA_HOST}/browse/T6IR-14728"),
            false,
            None,
        ),
        assistant_line(Some("PR https://github.com/a/b/pull/1"), false),
    ];
    let (mut store, dir, _p) = scan_store(&lines, "idle");
    let idle = labels(&store);
    assert_eq!(
        idle,
        Some(vec!["T6IR-14728".to_string(), "PR #1".to_string()])
    );
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    assert_eq!(labels(&store), idle);
}

#[test]
fn no_button_cap() {
    let keys: Vec<String> = (1..=6).map(|i| format!("T6IR-{i}")).collect();
    let lines = [user_line(
        &format!("https://{JIRA_HOST}/browse/T6IR-1 {}", keys.join(" ")),
        false,
        None,
    )];
    let (store, _d, _p) = scan_store(&lines, "idle");
    assert_eq!(labels(&store), Some(keys));
}

#[test]
fn title_cut_inside_url_shows_label_and_keeps_link() {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    let pr = format!(
        "https://bitbucket.org/talosix/very-long-repository-name/pull-requests/6867/overview?tab=activity&filter={}",
        "z".repeat(80)
    );
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[
            user_line(&format!("{} {pr}", "a".repeat(120)), false, None),
            String::new(),
        ],
    );
    write_session(dir.path(), "a", me(), "busy", 0.0, "a", FRONTEND);
    store.refresh(SystemTime::now());
    let s = &store.sessions[0];
    assert_eq!(s.title(), format!("{} PR #6867", "a".repeat(120)));
    assert_eq!(
        s.links.iter().map(|l| l.label.clone()).collect::<Vec<_>>(),
        vec!["PR #6867".to_string()]
    );
}

// ---- Running-tool line ----

struct ActivityRig {
    dir: TempDir,
    _projects: TempDir,
    store: Store,
    file: PathBuf,
}

impl ActivityRig {
    fn new(lines: &[String]) -> Self {
        let (dir, projects) = (make_temp_dir(), make_temp_dir());
        let store = with_projects(dir.path(), projects.path());
        let mut all = lines.to_vec();
        all.push(String::new());
        let file = write_transcript(projects.path(), FRONTEND, "a", &all);
        let rig = ActivityRig {
            dir,
            _projects: projects,
            store,
            file,
        };
        rig.busy();
        rig
    }

    fn busy(&self) {
        write_session(self.dir.path(), "a", me(), "busy", 5.0, "n", FRONTEND);
    }

    fn idle(&self) {
        write_session(self.dir.path(), "a", me(), "idle", 5.0, "n", FRONTEND);
    }

    fn refresh(&mut self) {
        self.store.refresh(SystemTime::now());
    }

    fn activity(&self) -> Option<&str> {
        self.store.sessions.first()?.activity.as_deref()
    }

    fn session(&self) -> &claude_sessions_core::store::Session {
        self.store.sessions.first().unwrap()
    }

    fn append(&self, s: &str) {
        append(&self.file, s);
    }
}

fn bash_tool(c: &str) -> String {
    bash(&format!("id-{c}"), c)
}

fn ask_x() -> Action {
    ask("a")
}

fn turn_end(background: Option<i64>, timestamp: Option<SystemTime>) -> String {
    turn_duration_line(background, None, false, timestamp)
}

#[test]
fn activity_from_existing_transcript() {
    let mut r = ActivityRig::new(&[user_line("hi", false, None), bash_tool("swift test")]);
    r.refresh();
    assert_eq!(r.activity(), Some("Bash: swift test"));
}

/// Two processes with one sessionId: the idle one must not clear the busy row's cached activity, whatever order the files are read in.
fn activity_survives_duplicate_session_id_order(busy_file: &str, idle_file: &str) {
    let (dir, projects) = (make_temp_dir(), make_temp_dir());
    let mut store = with_projects(dir.path(), projects.path());
    write_transcript(
        projects.path(),
        FRONTEND,
        "a",
        &[bash_tool("swift test"), String::new()],
    );
    let write_dup = |file: &str, pid: i32, status: &str, age: f64| {
        let o = json!({"pid": pid, "sessionId": "a", "name": file, "cwd": FRONTEND, "status": status,
            "statusUpdatedAt": (claude_sessions_core::time::unix_secs(SystemTime::now()) - age) * 1000.0});
        std::fs::write(
            dir.path().join(format!("{file}.json")),
            serde_json::to_vec(&o).unwrap(),
        )
        .unwrap();
    };
    write_dup(busy_file, me(), "busy", 1.0);
    write_dup(idle_file, parent_pid(), "idle", 600.0);
    for _ in 0..2 {
        store.refresh(SystemTime::now());
        let names: Vec<&str> = store.sessions.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec![busy_file]);
        assert_eq!(
            store.sessions[0].activity.as_deref(),
            Some("Bash: swift test")
        );
    }
}

#[test]
fn activity_survives_duplicate_session_id_order_busy_first() {
    activity_survives_duplicate_session_id_order("a-busy", "z-idle");
}

#[test]
fn activity_survives_duplicate_session_id_order_idle_first() {
    activity_survives_duplicate_session_id_order("z-busy", "a-idle");
}

#[test]
fn busy_card_counts_agents() {
    let agent = |id: &str| {
        tool_use_line(
            id,
            "Agent",
            json!({"description": format!("review {id}")}),
            false,
        )
    };
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        agent("a1"),
        agent("a2"),
        agent("a3"),
        tool_result_line("a1"),
    ]);
    r.refresh();
    assert_eq!(r.session().agents, vec!["review a2", "review a3"]);
    r.append(&(tool_result_line("a2") + "\n"));
    r.refresh();
    assert_eq!(r.session().agents, vec!["review a3"]);
    r.idle();
    r.refresh();
    assert!(r.session().agents.is_empty());
}

#[test]
fn background_wait_shown() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        bash_tool("swift test"),
        assistant_line(Some("done"), false),
        turn_end(Some(3), None),
    ]);
    r.refresh();
    assert_eq!(r.session().background_agents, Some(3));
    assert_eq!(r.session().shown_status(), "idle");
    assert_eq!(r.session().color_status(), "busy");
    assert_eq!(r.activity(), None);
    assert!(r.store.still_valid(&ask_x()));
}

#[test]
fn ended_with_no_background_is_idle_color() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(Some(0), None),
    ]);
    r.refresh();
    assert_eq!(r.session().shown_status(), "idle");
    assert_eq!(r.session().color_status(), "idle");
}

#[test]
fn background_wait_ends_on_new_turn() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(Some(3), None),
    ]);
    r.refresh();
    assert_eq!(r.session().background_agents, Some(3));
    r.append(&(user_line("task-notification", false, None) + "\n" + &bash_tool("ls") + "\n"));
    r.refresh();
    assert_eq!(r.session().background_agents, None);
    assert_eq!(r.session().shown_status(), "busy");
    assert_eq!(r.activity(), Some("Bash: ls"));
    assert!(!r.store.still_valid(&ask_x()));
}

#[test]
fn ended_clears_old_tool_line_for_next_turn() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        bash_tool("swift test"),
        assistant_line(Some("done"), false),
        turn_end(Some(1), None),
    ]);
    r.refresh();
    r.append(&(user_line("next", false, None) + "\n"));
    r.refresh();
    assert_eq!(r.session().background_agents, None);
    assert_eq!(r.activity(), None);
}

#[test]
fn background_wait_shows_last_reply() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("all done"), false),
        turn_end(Some(2), None),
    ]);
    r.refresh();
    assert_eq!(r.session().background_agents, Some(2));
    assert_eq!(r.session().last_reply.as_deref(), Some("all done"));
    assert!(r.session().is_finished());
}

#[test]
fn background_wait_survives_metadata_only_append() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(Some(3), None),
    ]);
    r.refresh();
    r.append(&(json_line(json!({"type": "last-prompt", "sessionId": "a"})) + "\n"));
    r.refresh();
    assert_eq!(r.session().background_agents, Some(3));
}

#[test]
fn sidechain_lines_keep_background_wait() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(Some(2), None),
    ]);
    r.refresh();
    r.append(
        &(tool_use_line("s", "Bash", json!({"command": "ls"}), true)
            + "\n"
            + &assistant_line(Some("side"), true)
            + "\n"),
    );
    r.refresh();
    assert_eq!(r.session().background_agents, Some(2));
    assert_eq!(r.activity(), None);
    assert!(r.store.still_valid(&ask_x()));
}

#[test]
fn turn_ended_without_background_counts() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(None, None),
    ]);
    r.refresh();
    assert_eq!(r.session().background_agents, Some(0));
    assert_eq!(r.session().shown_status(), "idle");
    assert!(!r.session().busy_after_turn);
    assert_eq!(status_text(r.session()), "xong");
    assert!(r.store.still_valid(&ask_x()));
}

fn ago(s: u64) -> SystemTime {
    SystemTime::now() - Duration::from_secs(s)
}

#[test]
fn busy_after_turn_shows_background_wait() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(None, Some(ago(60))),
    ]);
    r.refresh();
    assert!(r.session().busy_after_turn);
    assert_eq!(r.session().color_status(), "busy");
    assert_eq!(r.session().shown_status(), "idle");
    assert!(r.session().waits_in_background());
    assert_eq!(status_text(r.session()), "chờ agent nền");
    assert!(r.store.still_valid(&ask_x()));
}

#[test]
fn busy_before_turn_end_stays_done() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(None, Some(ago(1))),
    ]);
    r.refresh();
    assert!(!r.session().busy_after_turn);
    assert_eq!(r.session().color_status(), "idle");
    assert_eq!(status_text(r.session()), "xong");
}

#[test]
fn busy_after_turn_keeps_count() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(Some(2), Some(ago(60))),
    ]);
    r.refresh();
    assert_eq!(status_text(r.session()), "chờ 2 agent nền");
}

#[test]
fn busy_after_turn_ends_on_new_turn() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(None, Some(ago(60))),
    ]);
    r.refresh();
    assert!(r.session().busy_after_turn);
    r.append(&(user_line("again", false, None) + "\n" + &bash_tool("ls") + "\n"));
    r.refresh();
    assert!(!r.session().busy_after_turn);
    assert_eq!(r.session().shown_status(), "busy");
    assert_eq!(r.activity(), Some("Bash: ls"));
}

#[test]
fn background_wait_cleared_when_session_not_busy() {
    let mut r = ActivityRig::new(&[
        user_line("hi", false, None),
        assistant_line(Some("done"), false),
        turn_end(Some(1), None),
    ]);
    r.refresh();
    r.idle();
    r.refresh();
    assert_eq!(r.session().background_agents, None);
}

#[test]
fn activity_no_read_when_unchanged() {
    let mut r = ActivityRig::new(&[bash_tool("swift test")]);
    r.refresh();
    let reads = r.store.activity_reads;
    assert_eq!(reads, 1);
    r.refresh();
    assert_eq!(r.store.activity_reads, reads);
    assert_eq!(r.activity(), Some("Bash: swift test"));
}

fn edit_line() -> String {
    tool_use_line("e", "Edit", json!({"file_path": "/a/File.swift"}), false)
}

#[test]
fn activity_updates_on_append() {
    let mut r = ActivityRig::new(&[bash_tool("swift test")]);
    r.refresh();
    let reads = r.store.activity_reads;
    r.append(&(edit_line() + "\n"));
    r.refresh();
    assert_eq!(r.activity(), Some("Edit File.swift"));
    assert_eq!(r.store.activity_reads, reads + 1);
}

#[test]
fn activity_waits_for_complete_line() {
    let mut r = ActivityRig::new(&[bash_tool("swift test")]);
    r.refresh();
    let line = edit_line();
    let half = line.len() / 2;
    r.append(&line[..half]);
    r.refresh();
    assert_eq!(r.activity(), Some("Bash: swift test"));
    r.append(&(line[half..].to_string() + "\n"));
    r.refresh();
    assert_eq!(r.activity(), Some("Edit File.swift"));
}

#[test]
fn activity_resets_when_file_shrinks() {
    let mut r = ActivityRig::new(&[bash_tool(&"x".repeat(100)), bash_tool("swift test")]);
    r.refresh();
    std::fs::write(
        &r.file,
        tool_use_line("g", "Grep", json!({"pattern": "foo"}), false) + "\n",
    )
    .unwrap();
    r.refresh();
    assert_eq!(r.activity(), Some("Grep: foo"));
}

#[test]
fn activity_cleared_when_not_busy() {
    let mut r = ActivityRig::new(&[bash_tool("swift test")]);
    r.refresh();
    assert_eq!(r.activity(), Some("Bash: swift test"));
    r.idle();
    r.refresh();
    assert_eq!(r.activity(), None);
    r.busy();
    r.refresh();
    assert_eq!(r.activity(), None);
}

#[test]
fn activity_reads_at_most_window() {
    let filler = user_line(&"x".repeat(1_000), false, None);
    let mut lines = vec![bash_tool("old")];
    lines.extend(std::iter::repeat_n(filler, 400));
    let mut r = ActivityRig::new(&lines);
    r.refresh();
    assert_eq!(r.activity(), None);
    assert_eq!(r.store.activity_reads, 1);
}
