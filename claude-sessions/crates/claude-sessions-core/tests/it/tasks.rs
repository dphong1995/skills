use crate::support::{make_temp_dir, write_task};
use claude_sessions_core::tasks::{Todo, read};
use std::fs;

fn todo(done: usize, total: usize, current: Option<&str>) -> Option<Todo> {
    Some(Todo {
        done,
        total,
        current: current.map(String::from),
    })
}

#[test]
fn tasks_missing_dir_is_nil() {
    assert_eq!(read(&make_temp_dir().path().join("nope")), None);
}

#[test]
fn tasks_empty_dir_is_nil() {
    let dir = make_temp_dir();
    fs::write(dir.path().join(".highwatermark"), b"").unwrap();
    fs::write(dir.path().join(".lock"), b"").unwrap();
    assert_eq!(read(dir.path()), None);
}

#[test]
fn tasks_all_done_is_nil() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "completed", "A", None);
    write_task(dir.path(), None, 2, "completed", "B", None);
    assert_eq!(read(dir.path()), None);
}

#[test]
fn tasks_prefers_in_progress() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "completed", "Phase 1", None);
    write_task(
        dir.path(),
        None,
        2,
        "in_progress",
        "Phase 2",
        Some("Implementing phase 2"),
    );
    write_task(dir.path(), None, 3, "pending", "Phase 3", None);
    write_task(
        dir.path(),
        None,
        10,
        "in_progress",
        "Phase 10",
        Some("Implementing phase 10"),
    );
    assert_eq!(read(dir.path()), todo(1, 4, Some("Implementing phase 2")));
}

#[test]
fn tasks_in_progress_without_active_form_uses_subject() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "in_progress", "Phase 1", None);
    assert_eq!(read(dir.path()), todo(0, 1, Some("Phase 1")));
}

#[test]
fn tasks_falls_back_to_first_pending() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "completed", "Phase 1", None);
    write_task(dir.path(), None, 2, "pending", "Phase 2", None);
    write_task(dir.path(), None, 3, "pending", "Phase 3", None);
    assert_eq!(read(dir.path()), todo(1, 3, Some("Phase 2")));
}

#[test]
fn tasks_skips_broken_file() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "pending", "Phase 1", None);
    fs::write(dir.path().join("2.json"), br#"{"id":"2","sub"#).unwrap();
    assert_eq!(read(dir.path()), todo(0, 1, Some("Phase 1")));
}

#[test]
fn tasks_ignores_unknown_status() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "pending", "Phase 1", None);
    write_task(dir.path(), None, 2, "deleted", "Phase 2", None);
    assert_eq!(read(dir.path()), todo(0, 1, Some("Phase 1")));
}

#[test]
fn tasks_empty_text_is_not_current() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "in_progress", "Phase 1", Some(""));
    assert_eq!(read(dir.path()), todo(0, 1, Some("Phase 1")));
    let dir2 = make_temp_dir();
    write_task(dir2.path(), None, 1, "pending", "", None);
    assert_eq!(read(dir2.path()), todo(0, 1, None));
}

#[test]
fn tasks_keeps_non_numeric_id() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "completed", "A", None);
    fs::write(
        dir.path().join("b.json"),
        br#"{"id":"abc","subject":"B","status":"pending"}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("c.json"),
        br#"{"subject":"C","status":"pending"}"#,
    )
    .unwrap();
    assert_eq!(read(dir.path()), todo(1, 3, Some("B")));
}

#[test]
fn tasks_in_progress_without_text_does_not_show_pending() {
    let dir = make_temp_dir();
    write_task(dir.path(), None, 1, "in_progress", "", None);
    write_task(dir.path(), None, 2, "pending", "Phase 2", None);
    assert_eq!(read(dir.path()), todo(0, 2, None));
}

#[test]
fn todo_label() {
    assert_eq!(
        Todo {
            done: 1,
            total: 3,
            current: Some("Phase 2".into())
        }
        .label(),
        "1/3 · Phase 2"
    );
    assert_eq!(
        Todo {
            done: 1,
            total: 3,
            current: None
        }
        .label(),
        "1/3"
    );
}
