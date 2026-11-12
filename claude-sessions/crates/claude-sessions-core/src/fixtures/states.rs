//! One function per Swift snapshot state (`SnapshotTests.swift`), named `state_<swift name with _ for ->`.
//! States that Swift builds one after another in a single store replay their steps up to their own here.

use super::*;
use crate::bitbucket::BitbucketError;
use crate::closed_sessions::{ClosedSession, ClosedSessions};
use crate::feed::FeedMonitor;
use crate::jira_monitor::JiraMonitor;
use crate::layout::{Anchor, HAnchor};
use crate::panel_state::{EditKind, Editing};
use crate::pr_monitor::PRMonitor;
use crate::pull_requests::PRRef;
use crate::quota::Quota;
use crate::store::{ItemTone, LinkItem, LinkKind, LinkSection, NewMark, SendKey, SendKind};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use url::Url;

type State = (Snapshot, PanelState);

const DEAD_PID: i32 = 999_999;
const FRONT_NAME: &str = "talo6-dct-frontend-d7";
const BACK_NAME: &str = "talo6-dct-backend-3f";
const REPO: &str = "t6engineering/talo6-dct-backend";
const C1: &str = "2026-11-03T03:40:00.000000+00:00";
const C2: &str = "2026-11-03T03:41:00.000000+00:00";
const C3: &str = "2026-11-03T03:42:00.000000+00:00";

impl Fx {
    fn alert(&mut self, secs: f64) {
        let transitions = self.store.transitions.clone();
        let chime = self.store.pr_chime;
        let at = self.at(secs);
        self.store
            .apply_alert(&transitions, chime, &HashSet::new(), at);
    }
}

fn pr_ref(id: i64) -> PRRef {
    PRRef {
        workspace: "t6engineering".into(),
        repo: "talo6-dct-backend".into(),
        id,
    }
}

fn pr_url(id: i64) -> String {
    format!("https://bitbucket.org/{REPO}/pull-requests/{id}")
}

fn pr_monitor(api: &Arc<FakeBitbucket>) -> PRMonitor {
    PRMonitor::with_options(api.clone(), false, Arc::new(|_| None), 45.0)
}

// ---- 01-09: one store through its life ----

fn lifecycle(dir: &Path, now: SystemTime, stage: u32) -> State {
    let mut f = Fx::new(dir, now);
    f.refresh(0.0);
    if stage == 1 {
        return f.done();
    }
    f.session("a", "busy", 42.0, FRONT_NAME, FRONTEND);
    f.session("b", "busy", 300.0, BACK_NAME, BACKEND);
    f.session_with("dead", DEAD_PID, "idle", None, 10.0, "dead", "/tmp", None);
    f.refresh(0.0);
    if stage == 2 {
        return f.done();
    }
    f.session("a", "idle", 0.0, FRONT_NAME, FRONTEND);
    f.refresh(1.0);
    f.alert(1.0);
    if stage == 3 {
        return f.done();
    }
    f.refresh(7.0);
    if stage == 4 {
        return f.done();
    }
    f.waiting("b", "permission prompt", 0.0, BACK_NAME, BACKEND);
    f.waiting("c", "input needed", 75.0, "ocr-service-a1", OCR);
    f.session("d", "busy", 3700.0, "talo-v2-9c", TALOS);
    f.refresh(8.0);
    f.alert(8.0);
    if stage == 5 {
        return f.done();
    }
    f.refresh(14.0);
    if stage == 6 {
        return f.done();
    }
    if stage == 7 {
        f.hover();
        return f.done();
    }
    f.refresh(30.0);
    f.session("a", "busy", 0.0, FRONT_NAME, FRONTEND);
    f.refresh(31.0);
    f.session("a", "idle", 0.0, FRONT_NAME, FRONTEND);
    f.refresh(32.0);
    f.session("c", "busy", 5.0, "ocr-service-a1", OCR);
    f.refresh(40.0);
    f.panel.anchor = Anchor {
        h: HAnchor::Leading,
        up: true,
    };
    if stage == 8 {
        f.hover();
    }
    f.done()
}

pub fn state_01_empty_collapsed(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 1)
}
pub fn state_02_all_busy_collapsed(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 2)
}
pub fn state_03_just_finished_peek(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 3)
}
pub fn state_04_after_peek_collapsed(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 4)
}
pub fn state_05_mixed_peek(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 5)
}
pub fn state_06_mixed_collapsed(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 6)
}
pub fn state_07_mixed_hover(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 7)
}
/// The data of `07-mixed-hover`; the harness renders it in the light appearance.
pub fn state_light_07_mixed_hover(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 7)
}
pub fn state_08_bottom_left_hover_expands_up(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 8)
}
pub fn state_09_bottom_left_collapsed(dir: &Path, now: SystemTime) -> State {
    lifecycle(dir, now, 9)
}

// ---- 10-19: card content ----

pub fn state_15_titles_branch(dir: &Path, now: SystemTime) -> State {
    let mut options = options();
    options.branch = Some(Box::new(|cwd| {
        (cwd == FRONTEND).then(|| "T6IR-14737".to_string())
    }));
    let mut f = Fx::with(dir, now, options);
    f.transcript(
        FRONTEND,
        "b",
        &[
            command_line("/clear"),
            user_line(
                "sửa lỗi đăng nhập khi token hết hạn và thêm test cho trường hợp refresh thất bại",
            ),
        ],
    );
    f.session_with(
        "a",
        me(),
        "idle",
        None,
        30.0,
        "widget",
        BACKEND,
        Some("user"),
    );
    f.session("b", "busy", 90.0, FRONT_NAME, FRONTEND);
    f.session("c", "busy", 3700.0, "talo-v2-9c", TALOS);
    f.refresh(0.0);
    f.hover();
    f.done()
}

pub fn state_16_todo(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.transcript(
        BACKEND,
        "b",
        &[assistant_line(
            "Đã sửa xong, bạn xem lại diff giúp mình nhé.",
        )],
    );
    f.session("a", "busy", 90.0, FRONT_NAME, FRONTEND);
    f.session("b", "idle", 30.0, BACK_NAME, BACKEND);
    f.session("c", "busy", 3700.0, "talo-v2-9c", TALOS);
    f.task("a", 1, "completed", "Phase 1", None);
    f.task(
        "a",
        2,
        "in_progress",
        "Phase 2",
        Some("Implementing phase 2"),
    );
    f.task("a", 3, "pending", "Phase 3", None);
    f.task("a", 4, "pending", "Phase 4", None);
    f.task(
        "b",
        1,
        "in_progress",
        "Phase 1",
        Some("Implementing the deadlock retry in the transaction owner and adding a regression test for the lock timeout path"),
    );
    f.refresh(0.0);
    f.hover();
    f.done()
}

pub fn state_10_last_reply_hover(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.transcript(
        FRONTEND,
        "a",
        &[assistant_line("Đã sửa xong lỗi đăng nhập và chạy lại toàn bộ test, tất cả đều qua. Bạn xem lại diff giúp mình rồi mình sẽ mở PR nhé, còn một chỗ nhỏ cần bạn quyết định.")],
    );
    f.transcript(
        OCR,
        "c",
        &[assistant_line(
            "Mình cần bạn chọn giữa hai hướng: giữ API cũ hay đổi sang bản mới?",
        )],
    );
    f.session("a", "idle", 30.0, FRONT_NAME, FRONTEND);
    f.waiting("c", "input needed", 75.0, "ocr-service-a1", OCR);
    f.session("d", "busy", 3700.0, "talo-v2-9c", TALOS);
    f.refresh(0.0);
    // The finished row under the mouse shows its `Trả lời` button.
    f.hover_row("a");
    f.done()
}

pub fn state_17_long_content(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    let reply = format!(
        "Đã xong. **bold** và `code iiWW` xem https://talosix.atlassian.net/browse/T6IR-14500 {} https://bitbucket.org/t/r/pull-requests/6867 https://github.com/x/y/pull/12",
        "nội dung dài ".repeat(70)
    );
    f.transcript(
        FRONTEND,
        "a",
        &[
            user_line(&format!(
                "/cook https://talosix.atlassian.net/browse/T6IR-14500 {}",
                "Hãy sửa lỗi đăng nhập ".repeat(6)
            )),
            assistant_line(&reply),
        ],
    );
    f.session("a", "idle", 30.0, FRONT_NAME, FRONTEND);
    f.session_with(
        "b",
        me(),
        "busy",
        None,
        90.0,
        "short-title",
        BACKEND,
        Some("user"),
    );
    f.refresh(0.0);
    f.hover();
    f.done()
}

pub fn state_18_activity(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.transcript(
        FRONTEND,
        "a",
        &[
            tool_use_line(
                "t1",
                "Bash",
                json!({"command": "swift test --filter StoreTests"}),
            ),
            String::new(),
        ],
    );
    f.transcript(
        BACKEND,
        "b",
        &[
            tool_use_line(
                "t2",
                "Bash",
                json!({"command": "echo long-command ".repeat(12)}),
            ),
            String::new(),
        ],
    );
    f.session("a", "busy", 90.0, FRONT_NAME, FRONTEND);
    f.session("b", "busy", 40.0, BACK_NAME, BACKEND);
    f.session("c", "idle", 30.0, "ocr-service-a1", OCR);
    f.task(
        "a",
        1,
        "in_progress",
        "Phase 4",
        Some("Implementing phase 4"),
    );
    f.refresh(0.0);
    f.hover();
    f.done()
}

pub fn state_19_agents(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    let agent = |id: &str, tool: &str, d: &str| {
        format!("{}\n", tool_use_line(id, tool, json!({"description": d})))
    };
    f.transcript(
        FRONTEND,
        "a",
        &[
            agent("t1", "Agent", "review A"),
            agent("t2", "Agent", "review B"),
            agent("t3", "Task", "review C"),
        ],
    );
    f.session("a", "busy", 90.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    f.hover();
    f.done()
}

pub fn state_35_background_agents(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.transcript(
        FRONTEND,
        "a",
        &[
            user_line("hi"),
            assistant_line("Đã sửa xong, đang chờ agent nền chạy test."),
            turn_duration_line(2),
            String::new(),
        ],
    );
    f.session("a", "busy", 90.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    f.hover();
    f.done()
}

pub fn state_19_scroll(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    for i in 0..12 {
        f.transcript(
            FRONTEND,
            &format!("s{i}"),
            &[assistant_line(&format!("câu trả lời dài {i} ").repeat(40))],
        );
        f.session(
            &format!("s{i}"),
            "idle",
            30.0 + i as f64,
            &format!("session-{i}"),
            FRONTEND,
        );
    }
    f.refresh(0.0);
    f.hover();
    f.done()
}

pub fn state_11_pending_hover(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.transcript(
        FRONTEND,
        "a",
        &[assistant_line(
            "Đã sửa xong, bạn xem lại diff giúp mình nhé.",
        )],
    );
    f.transcript(
        BACKEND,
        "b",
        &[tool_use_line("t1", "Bash", json!({"command": "npm test"}))],
    );
    f.transcript(
        OCR,
        "c",
        &[tool_use_line(
            "t2",
            "Bash",
            json!({"command": "git add -A\ngit commit -m \"fix\"\ngit push origin main"}),
        )],
    );
    f.transcript(
        TALOS,
        "d",
        &[
            tool_use_line("t3", "Bash", json!({"command": "ls"})),
            tool_use_line("t4", "Edit", json!({"file_path": "/x"})),
        ],
    );
    f.session("a", "busy", 30.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    f.session("a", "idle", 0.0, FRONT_NAME, FRONTEND);
    f.waiting("b", "permission prompt", 5.0, BACK_NAME, BACKEND);
    f.waiting("c", "permission prompt", 9.0, "ocr-service-a1", OCR);
    f.waiting("d", "permission prompt", 12.0, "talo-v2-9c", TALOS);
    f.refresh(1.0);
    f.hover();
    f.done()
}

// ---- 20, 21, 23-26, 41: the collapsed pill ----

fn opacity(dir: &Path, now: SystemTime, bg: f64, hover: bool) -> State {
    let mut f = Fx::new(dir, now);
    f.waiting("a", "input needed", 5.0, "a", FRONTEND);
    f.session("b", "idle", 5.0, "b", BACKEND);
    f.session("c", "busy", 5.0, "c", BACKEND);
    f.refresh(0.0);
    f.store.bg_opacity = bg;
    if hover {
        f.hover();
    }
    f.done()
}

pub fn state_20_collapsed_opacity_0(dir: &Path, now: SystemTime) -> State {
    opacity(dir, now, 0.0, false)
}
pub fn state_20_collapsed_opacity_25(dir: &Path, now: SystemTime) -> State {
    opacity(dir, now, 0.25, false)
}
pub fn state_20_collapsed_opacity_92(dir: &Path, now: SystemTime) -> State {
    opacity(dir, now, 0.92, false)
}
pub fn state_20_expanded_at_0(dir: &Path, now: SystemTime) -> State {
    opacity(dir, now, 0.0, true)
}

/// `waiting`, `idle` and `busy` sessions named `w<i>`, `i<i>`, `b<i>` in `cwds` (waiting, idle, busy).
fn counts(f: &Fx, waiting: usize, idle: usize, busy: usize, cwds: [&str; 3]) {
    for i in 0..waiting {
        f.waiting(
            &format!("w{i}"),
            "input needed",
            5.0,
            &format!("w{i}"),
            cwds[0],
        );
    }
    for i in 0..idle {
        f.session(&format!("i{i}"), "idle", 5.0, &format!("i{i}"), cwds[1]);
    }
    for i in 0..busy {
        f.session(&format!("b{i}"), "busy", 5.0, &format!("b{i}"), cwds[2]);
    }
}

fn new_marks(ids: &[&str]) -> HashMap<String, NewMark> {
    ids.iter()
        .map(|i| {
            (
                i.to_string(),
                NewMark {
                    claude: true,
                    ..NewMark::default()
                },
            )
        })
        .collect()
}

fn indexed(prefix: &str, n: usize) -> HashMap<String, NewMark> {
    let ids: Vec<String> = (0..n).map(|i| format!("{prefix}{i}")).collect();
    new_marks(&ids.iter().map(String::as_str).collect::<Vec<_>>())
}

pub fn state_21_collapsed_two_digits(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    counts(&f, 10, 11, 12, [FRONTEND; 3]);
    f.refresh(0.0);
    f.done()
}

pub fn state_41_collapsed_badge(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    counts(&f, 12, 10, 11, [FRONTEND; 3]);
    f.refresh(0.0);
    f.store.unseen = indexed("i", 12);
    f.store.quota = Some(Quota {
        five_hour: 97,
        five_hour_reset: f.at(7200.0),
        weekly: 40,
        weekly_reset: f.at(5.0 * 86400.0),
        forecast: None,
    });
    f.done()
}

fn new_mark_store(dir: &Path, now: SystemTime, waiting: usize, idle: usize, busy: usize) -> Fx {
    let mut f = Fx::new(dir, now);
    counts(&f, waiting, idle, busy, [FRONTEND, BACKEND, BACKEND]);
    f.refresh(0.0);
    f
}

pub fn state_23_collapsed_new(dir: &Path, now: SystemTime) -> State {
    let mut f = new_mark_store(dir, now, 1, 2, 1);
    f.store.unseen = new_marks(&["w0", "i1"]);
    f.done()
}

pub fn state_24_collapsed_new_two_digits(dir: &Path, now: SystemTime) -> State {
    let mut f = new_mark_store(dir, now, 3, 12, 1);
    f.store.unseen = indexed("i", 12);
    f.done()
}

pub fn state_25_collapsed_no_new(dir: &Path, now: SystemTime) -> State {
    let mut f = new_mark_store(dir, now, 1, 2, 1);
    f.store.unseen = HashMap::new();
    f.done()
}

pub fn state_26_expanded_new(dir: &Path, now: SystemTime) -> State {
    let mut f = new_mark_store(dir, now, 1, 2, 1);
    f.store.unseen = new_marks(&["i1"]);
    f.hover();
    f.done()
}

pub fn state_14_finished_collapsed(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.session("a", "busy", 30.0, FRONT_NAME, FRONTEND);
    f.session("b", "busy", 30.0, BACK_NAME, BACKEND);
    f.session("c", "busy", 30.0, "ocr-service-a1", OCR);
    f.refresh(0.0);
    f.session("a", "idle", 0.0, FRONT_NAME, FRONTEND);
    f.waiting("b", "input needed", 0.0, BACK_NAME, BACKEND);
    f.refresh(1.0);
    f.refresh(10.0);
    f.done()
}

// ---- 12, 13: approvals ----

fn actions(dir: &Path, now: SystemTime, deny: bool) -> State {
    let mut f = Fx::new(dir, now);
    let six: Vec<String> = (1..=6).map(|i| format!("echo line {i}")).collect();
    let twenty: Vec<String> = (1..=20).map(|i| format!("echo line {i}")).collect();
    f.transcript(
        BACKEND,
        "b",
        &[tool_use_line("t1", "Bash", json!({"command": "npm test"}))],
    );
    f.transcript(
        OCR,
        "c",
        &[tool_use_line(
            "t2",
            "Bash",
            json!({"command": six.join("\n")}),
        )],
    );
    f.transcript(
        TALOS,
        "d",
        &[tool_use_line(
            "t3",
            "Bash",
            json!({"command": twenty.join("\n")}),
        )],
    );
    f.waiting("b", "permission prompt", 5.0, BACK_NAME, BACKEND);
    f.waiting("c", "permission prompt", 9.0, "ocr-service-a1", OCR);
    f.waiting("d", "permission prompt", 12.0, "talo-v2-9c", TALOS);
    if deny {
        f.transcript(
            FRONTEND,
            "a",
            &[assistant_line(
                "Đã sửa xong, bạn xem lại diff giúp mình nhé.",
            )],
        );
        f.transcript(
            "/Users/phong/api",
            "e",
            &[assistant_line("Bạn muốn mình làm tiếp phần nào?")],
        );
        f.session("a", "idle", 30.0, FRONT_NAME, FRONTEND);
        f.session("e", "idle", 40.0, "api-5e", "/Users/phong/api");
    } else {
        // The three approvals leave no room under them for the finished rows: a running row takes their place, and the mouse
        // over it shows its `Dừng` button.
        f.session("f", "busy", 60.0, "api-5e", "/Users/phong/api");
    }
    f.refresh(0.0);
    if deny {
        f.hover();
        f.panel.editing = Some(Editing {
            id: "b".into(),
            kind: EditKind::Deny,
            tool_id: Some("t1".into()),
        });
    } else {
        f.hover_row("f");
    }
    f.done()
}

pub fn state_12_actions_hover(dir: &Path, now: SystemTime) -> State {
    actions(dir, now, false)
}
pub fn state_13_deny_reason(dir: &Path, now: SystemTime) -> State {
    actions(dir, now, true)
}

// ---- 22, 23-ticket, 31, 32-35, 50-53: pull requests ----

pub fn state_22_pr_lines(dir: &Path, now: SystemTime) -> State {
    pr_lines(dir, now, false)
}
pub fn state_22_pr_lines_stale(dir: &Path, now: SystemTime) -> State {
    pr_lines(dir, now, true)
}

fn pr_lines(dir: &Path, now: SystemTime, stale: bool) -> State {
    let mut open = pr_json(7571, 23, 5, None, "OPEN");
    if let Some(ps) = open["participants"].as_array_mut() {
        for p in ps {
            if p["role"] == "REVIEWER" && p["approved"] == true {
                p["approved"] = json!(false);
            }
        }
    }
    let api = fake_bitbucket(vec![open.clone(), pr_json(7567, 0, 0, None, "OPEN")]);
    api.set(
        "/comments",
        json!({"values": [comment("{tin}", "Tin Tran", now - Duration::from_secs(180))]}),
    );
    let mut options = options();
    options.prs = Some(pr_monitor(&api));
    let mut f = Fx::with(dir, now, options);
    let links = [7571, 7567].map(pr_url).join(" ");
    f.transcript(
        BACKEND,
        "b",
        &[
            assistant_line(&format!("Đã push, xem {links}")),
            String::new(),
        ],
    );
    f.session("b", "idle", 30.0, BACK_NAME, BACKEND);
    f.hover();
    f.refresh(0.0);
    api.set(
        "/2.0/workspaces/t6engineering/pullrequests/",
        json!({"values": [open]}),
    );
    api.set(
        "/talo6-dct-backend/pullrequests/7567?",
        pr_json(7567, 0, 0, None, "MERGED"),
    );
    f.refresh(61.0);
    if stale {
        *api.fail_all.lock().unwrap() = Some(BitbucketError::Transport);
        f.refresh(122.0);
        f.refresh(420.0);
    }
    f.done()
}

pub fn state_23_ticket_row_overflow(dir: &Path, now: SystemTime) -> State {
    let api = fake_bitbucket(vec![pr_json(7571, 1, 0, None, "OPEN")]);
    api.set(
        "/statuses",
        json!({"values": [rabbit_status("Review completed", RABBIT_CREATED)]}),
    );
    api.set(
        "/comments",
        json!({"values": pending_finding(2, "2026-11-03T03:40:00.000000+00:00")}),
    );
    let urls: Vec<String> = (101..=108)
        .map(|i| format!("https://talo6.atlassian.net/browse/T6IR-{i}"))
        .collect();
    let mut options = options();
    options.prs = Some(pr_monitor(&api));
    let mut f = Fx::with(dir, now, options);
    f.transcript(
        BACKEND,
        "b",
        &[
            user_line(&urls.join(" ")),
            assistant_line(&format!("Đã push, xem {}", pr_url(7571))),
            String::new(),
        ],
    );
    f.session("b", "idle", 30.0, BACK_NAME, BACKEND);
    f.refresh(0.0);
    f.hover();
    f.done()
}

/// One idle session `b` with PR 7583. `reviewed`: CodeRabbit finished its review; `comments`: the PR's comments, `None`
/// when that fetch fails.
fn pr_row(dir: &Path, now: SystemTime, reviewed: bool, comments: Option<Vec<Value>>) -> Fx {
    let open = if reviewed {
        pr_json(7583, 3, 0, Some(false), "OPEN")
    } else {
        pr_json(7583, 0, 0, None, "OPEN")
    };
    let api = fake_bitbucket(vec![open]);
    let statuses = if reviewed {
        vec![rabbit_status("Review completed", RABBIT_CREATED)]
    } else {
        vec![]
    };
    api.set("/statuses", json!({ "values": statuses }));
    match comments {
        Some(c) => api.set("/comments", json!({ "values": c })),
        None => api.set_error("/comments", BitbucketError::Transport),
    }
    let mut options = options();
    options.prs = Some(pr_monitor(&api));
    let mut f = Fx::with(dir, now, options);
    let pr = pr_url(7583);
    f.transcript(
        BACKEND,
        "b",
        &[
            user_line(&format!("Xem {pr}")),
            assistant_line(&format!("Đã push, xem {pr}")),
            String::new(),
        ],
    );
    f.session("b", "idle", 30.0, BACK_NAME, BACKEND);
    f.hover();
    f
}

fn all_done_comments() -> Vec<Value> {
    vec![
        rabbit_comment("**Actionable comments posted: 1**", C1),
        rabbit_inline(1, "nit", C1, None),
        human_reply(2, 1, "won't fix", C2),
        rabbit_inline(3, "withdrawn", C3, Some(1)),
    ]
}

pub fn state_31_pr_awaiting_approval(dir: &Path, now: SystemTime) -> State {
    let mut f = pr_row(dir, now, true, Some(all_done_comments()));
    f.refresh(0.0);
    f.done()
}

pub fn state_51_pr_remaining(dir: &Path, now: SystemTime) -> State {
    let comments = vec![
        rabbit_comment("**Actionable comments posted: 1**", C1),
        rabbit_inline(1, "nit", C1, None),
        rabbit_inline(4, "nit 2", C2, None),
    ];
    let mut f = pr_row(dir, now, true, Some(comments));
    f.refresh(0.0);
    f.done()
}

pub fn state_52_pr_counts_unknown(dir: &Path, now: SystemTime) -> State {
    let mut f = pr_row(dir, now, true, None);
    f.refresh(0.0);
    f.done()
}

pub fn state_53_pr_all_done(dir: &Path, now: SystemTime) -> State {
    state_31_pr_awaiting_approval(dir, now)
}

fn send_state(dir: &Path, now: SystemTime, stage: u32) -> State {
    let mut f = pr_row(dir, now, false, Some(vec![]));
    f.refresh(0.0);
    let ask = SendKey {
        session: "b".into(),
        kind: SendKind::AskClaude,
        pr: Some(pr_ref(7583)),
        detail: None,
    };
    let (t0, t1, t2) = (f.at(0.0), f.at(1.0), f.at(2.0));
    f.store.begin_send(ask.clone(), t0);
    f.refresh(1.0);
    if stage == 50 {
        return f.done();
    }
    f.store.finish_send(&ask, true, t1);
    f.refresh(2.0);
    if stage == 51 {
        return f.done();
    }
    f.store.finish_send(&ask, false, t2);
    f.done()
}

pub fn state_50_send_sending(dir: &Path, now: SystemTime) -> State {
    send_state(dir, now, 50)
}
pub fn state_51_send_sent(dir: &Path, now: SystemTime) -> State {
    send_state(dir, now, 51)
}
pub fn state_52_send_failed(dir: &Path, now: SystemTime) -> State {
    send_state(dir, now, 52)
}

/// Two sessions: `b` with two open PRs (7571, 7567), `c` plain, so unmarked cards show next to marked ones.
fn marks(dir: &Path, now: SystemTime, marks: &[(&str, bool, &[i64])]) -> State {
    let api = fake_bitbucket(vec![
        pr_json(7571, 2, 0, None, "OPEN"),
        pr_json(7567, 0, 0, None, "OPEN"),
    ]);
    let mut options = options();
    options.prs = Some(pr_monitor(&api));
    let mut f = Fx::with(dir, now, options);
    let links = [7571, 7567].map(pr_url).join(" ");
    f.transcript(
        BACKEND,
        "b",
        &[
            assistant_line(&format!("Đã push, xem {links}")),
            String::new(),
        ],
    );
    f.session("b", "idle", 30.0, BACK_NAME, BACKEND);
    f.session("c", "idle", 40.0, "ocr-service-a1", OCR);
    f.refresh(0.0);
    f.store.unseen = marks
        .iter()
        .map(|(id, claude, prs)| {
            (
                id.to_string(),
                NewMark {
                    claude: *claude,
                    prs: prs.iter().map(|p| pr_ref(*p)).collect(),
                },
            )
        })
        .collect();
    f.hover();
    f.done()
}

pub fn state_32_mark_claude(dir: &Path, now: SystemTime) -> State {
    marks(dir, now, &[("b", true, &[])])
}
pub fn state_33_mark_pr(dir: &Path, now: SystemTime) -> State {
    marks(dir, now, &[("b", false, &[7571])])
}
pub fn state_34_mark_both(dir: &Path, now: SystemTime) -> State {
    marks(dir, now, &[("b", true, &[7571])])
}
pub fn state_35_mark_pr_hidden(dir: &Path, now: SystemTime) -> State {
    marks(dir, now, &[("b", false, &[9999])])
}

pub fn state_27_fresh_long_title(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.transcript(
        FRONTEND,
        "a",
        &[
            user_line("Hãy sửa lỗi đăng nhập khi token hết hạn, thêm test cho luồng làm mới token và cập nhật tài liệu hướng dẫn cho đội"),
            assistant_line("Đã sửa xong, test đều qua."),
        ],
    );
    f.session("a", "idle", 30.0, FRONT_NAME, FRONTEND);
    f.store.unseen = new_marks(&["a"]);
    f.refresh(0.0);
    f.hover();
    f.done()
}

// ---- 28-30: the link menu ----

type TicketSpec<'a> = (
    &'a str,
    &'a str,
    &'a str,
    Option<&'a str>,
    Option<(&'a str, String)>,
);

/// One session `b` mentioning eight tickets (the last has no type or icon data) and PRs; the icons come through the Jira fake.
fn link_store(dir: &Path, now: SystemTime, connected: bool) -> Fx {
    let urls: Vec<String> = (101..=108)
        .map(|i| format!("https://talo6.atlassian.net/browse/T6IR-{i}"))
        .collect();
    let prs = "https://github.com/a/b/pull/1 https://github.com/a/b/pull/2";
    let icon = |name: &str| format!("https://talo6.atlassian.net/icon/{name}");
    let api = Arc::new(FakeJira::default());
    let long = "Very long ticket title that overflows the row ".repeat(3);
    let spec: [TicketSpec; 8] = [
        (
            "T6IR-101",
            "Add retry to the sync worker",
            "To Do",
            Some("new"),
            Some(("Bug", icon("bug"))),
        ),
        (
            "T6IR-102",
            "Fix site scope filter on the dashboard",
            "In Progress",
            Some("indeterminate"),
            None,
        ),
        (
            "T6IR-103",
            "Remove the legacy export endpoint",
            "Done",
            Some("done"),
            Some(("Task", icon("task"))),
        ),
        (
            "T6IR-104",
            "Cache the permission lookups",
            "Ready for Grooming with Engineering",
            Some("new"),
            Some(("Bug", icon("bug"))),
        ),
        (
            "T6IR-105",
            "Odd workflow status",
            "Blocked",
            Some("weird"),
            Some(("Story", icon("story"))),
        ),
        (
            "T6IR-106",
            "Ticket with a status and no category",
            "Review",
            None,
            Some(("Task", icon("task"))),
        ),
        (
            "T6IR-107",
            "Ticket whose icon never arrived",
            "To Do",
            Some("new"),
            None,
        ),
        (
            "T6IR-108",
            &long,
            "To Do",
            Some("new"),
            Some(("Task", icon("task"))),
        ),
    ];
    for (k, title, status, cat, ty) in spec {
        api.issues
            .lock()
            .unwrap()
            .insert(k.into(), (title.into(), Some(status.into())));
        if let Some(c) = cat {
            api.categories.lock().unwrap().insert(k.into(), c.into());
        }
        if let Some((name, url)) = ty {
            api.types
                .lock()
                .unwrap()
                .insert(k.into(), (name.into(), url));
        }
    }
    for (name, rgb) in [
        ("bug", [255, 69, 58]),
        ("story", [48, 209, 88]),
        ("task", [10, 132, 255]),
    ] {
        api.icon_data
            .lock()
            .unwrap()
            .insert(Url::parse(&icon(name)).unwrap(), swatch_png(rgb));
    }
    let mut options = options();
    options.jira = connected.then(|| JiraMonitor::new(api, false));
    let mut f = Fx::with(dir, now, options);
    f.transcript(
        BACKEND,
        "b",
        &[
            user_line(&urls.join(" ")),
            assistant_line(prs),
            String::new(),
        ],
    );
    f.session("b", "idle", 30.0, BACK_NAME, BACKEND);
    f.store
        .jira_projects
        .insert("T6IR".into(), "talo6.atlassian.net".into());
    f.refresh(0.0);
    if connected {
        // Bitbucket is not faked here: swap the untracked PR links for one PR in each state.
        let pr = |id: i64, status: &str, tone: ItemTone| LinkItem {
            text: format!("PR #{id}"),
            url: Url::parse(&format!("https://github.com/a/b/pull/{id}")).unwrap(),
            kind: LinkKind::Pr,
            code: format!("#{id}"),
            title: Some(format!("Rework the {status} pull request")),
            status: Some(status.into()),
            tone: Some(tone),
            icon_url: None,
            help: None,
        };
        let menu = &mut f.store.sessions[0].menu;
        if let Some(i) = menu.iter().position(|s| s.title == "PR") {
            menu[i] = LinkSection {
                title: "PR".into(),
                items: vec![
                    pr(41, "mở", ItemTone::Open),
                    pr(42, "đã merge", ItemTone::Merged),
                    pr(43, "đã đóng", ItemTone::Closed),
                ],
            };
        }
    }
    f.hover();
    f
}

pub fn state_28_link_menu_row(dir: &Path, now: SystemTime) -> State {
    link_store(dir, now, true).done()
}

pub fn state_29_link_list_open(dir: &Path, now: SystemTime) -> State {
    let mut f = link_store(dir, now, true);
    let id = f.store.sessions[0].id.clone();
    f.panel.open_menus = HashSet::from([id]);
    f.done()
}

pub fn state_30_link_list_connect(dir: &Path, now: SystemTime) -> State {
    let mut f = link_store(dir, now, false);
    let id = f.store.sessions[0].id.clone();
    f.panel.open_menus = HashSet::from([id]);
    f.done()
}

// ---- closed list ----

fn closed_list(f: &mut Fx) {
    f.store.closed = ClosedSessions::new(vec![
        ClosedSession {
            id: "x".into(),
            title: "Sửa lỗi đăng nhập".into(),
            cwd: BACKEND.into(),
            branch: Some("feat/login".into()),
            closed_at: f.at(-12.0 * 60.0),
        },
        ClosedSession {
            id: "y".into(),
            title: "ocr-service-a1".into(),
            cwd: OCR.into(),
            branch: None,
            closed_at: f.at(-3.0 * 3600.0),
        },
    ]);
}

pub fn state_closed_1_collapsed(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.session("a", "busy", 42.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    closed_list(&mut f);
    f.hover();
    f.done()
}

pub fn state_closed_2_open(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.session("a", "busy", 42.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    closed_list(&mut f);
    f.hover();
    f.panel.closed_open = true;
    f.done()
}

pub fn state_54_closed_opening(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.refresh(0.0);
    closed_list(&mut f);
    let t0 = f.at(0.0);
    f.store.begin_send(SendKey::new("x", SendKind::Resume), t0);
    f.store
        .finish_send(&SendKey::new("y", SendKind::Resume), false, t0);
    f.hover();
    f.panel.closed_open = true;
    f.done()
}

pub fn state_53_approval_sent_line(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.session("a", "busy", 5.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    let t0 = f.at(0.0);
    f.store
        .finish_send(&SendKey::new("a", SendKind::Approve), true, t0);
    f.hover();
    f.done()
}

// ---- reply, card states, stop, quota ----

fn reply(dir: &Path, now: SystemTime, editor: bool) -> State {
    let mut f = Fx::new(dir, now);
    f.session("a", "idle", 30.0, BACK_NAME, BACKEND);
    f.session("b", "busy", 42.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    f.hover();
    if editor {
        f.store
            .drafts
            .insert(Store::draft_key("a", EditKind::Reply), "say hi".into());
        f.panel.editing = Some(Editing {
            id: "a".into(),
            kind: EditKind::Reply,
            tool_id: None,
        });
    }
    f.done()
}

pub fn state_reply_1_buttons(dir: &Path, now: SystemTime) -> State {
    reply(dir, now, false)
}
pub fn state_reply_2_editor(dir: &Path, now: SystemTime) -> State {
    reply(dir, now, true)
}

pub fn state_40_card_states(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.transcript(
        FRONTEND,
        "bg",
        &[
            user_line("hi"),
            assistant_line("Đang chờ agent nền."),
            turn_duration_line(2),
            String::new(),
        ],
    );
    f.waiting(
        "perm",
        "permission prompt",
        20.0,
        "card-permission",
        BACKEND,
    );
    f.session_with(
        "ask",
        me(),
        "waiting",
        None,
        40.0,
        "card-question",
        BACKEND,
        None,
    );
    f.session("done", "idle", 3700.0, "card-done", BACKEND);
    f.session("run", "busy", 90.0, "card-running", BACKEND);
    f.session("bg", "busy", 120.0, "card-bg-agents", FRONTEND);
    f.session("fresh", "idle", 5.0, "card-fresh", BACKEND);
    f.store.unseen = new_marks(&["fresh"]);
    f.refresh(0.0);
    f.hover();
    f.done()
}

pub fn state_stop_armed(dir: &Path, now: SystemTime) -> State {
    let mut f = Fx::new(dir, now);
    f.session("b", "busy", 42.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    f.hover();
    f.panel.stop_armed.insert("b".into(), now);
    f.done()
}

fn quota(
    dir: &Path,
    now: SystemTime,
    five: i64,
    weekly: i64,
    forecast: bool,
    hover: bool,
) -> State {
    let mut f = Fx::new(dir, now);
    f.session("a", "busy", 42.0, FRONT_NAME, FRONTEND);
    f.waiting("b", "input needed", 75.0, "ocr-service-a1", BACKEND);
    f.refresh(0.0);
    f.store.quota = Some(Quota {
        five_hour: five,
        five_hour_reset: f.at(2.0 * 3600.0),
        weekly,
        weekly_reset: f.at(5.0 * 86400.0),
        forecast: forecast.then(|| f.at(3600.0)),
    });
    if hover {
        f.hover();
    }
    f.done()
}

pub fn state_quota_1_normal(dir: &Path, now: SystemTime) -> State {
    quota(dir, now, 30, 5, false, true)
}
pub fn state_quota_2_warn(dir: &Path, now: SystemTime) -> State {
    quota(dir, now, 82, 96, false, true)
}
pub fn state_quota_3_high_forecast(dir: &Path, now: SystemTime) -> State {
    quota(dir, now, 97, 40, true, true)
}
pub fn state_quota_5_pill_85(dir: &Path, now: SystemTime) -> State {
    quota(dir, now, 85, 10, false, false)
}

// ---- questions ----

fn question(dir: &Path, now: SystemTime, stage: u32) -> State {
    let feed = Arc::new(FakeFeedTransport::default());
    let single = feed_question_json("q0", "Chọn màu nào?", "Màu", false, &["Đỏ", "Xanh", "Lá"]);
    let multi = feed_question_json(
        "q1",
        "Chọn các tính năng",
        "Tính năng",
        true,
        &["A", "B", "C"],
    );
    *feed.items.lock().unwrap() = vec![feed_item("R1", "a", now, vec![single.clone()])];
    let mut options = options();
    options.feed = Some(FeedMonitor::without_worker(feed.clone()));
    let mut f = Fx::with(dir, now, options);
    f.waiting("a", "input needed", 5.0, BACK_NAME, BACKEND);
    f.session("b", "busy", 42.0, FRONT_NAME, FRONTEND);
    f.refresh(0.0);
    f.hover();
    let ask = |f: &Fx| {
        f.store
            .sessions
            .iter()
            .find(|s| s.id == "a")
            .and_then(|s| s.question.clone())
    };
    let mut q = ask(&f).expect("session a asks a question");
    if stage == 1 {
        return f.done();
    }
    f.panel.pick(&q, 0, "Xanh");
    if stage == 2 {
        return f.done();
    }
    f.panel.set_other(&q, 0, "Tím than");
    f.panel.editing = Some(Editing {
        id: "a".into(),
        kind: EditKind::Answer,
        tool_id: None,
    });
    if stage == 3 {
        return f.done();
    }
    f.panel.editing = None;
    *feed.items.lock().unwrap() = vec![feed_item("R2", "a", now, vec![single, multi])];
    f.refresh(1.0);
    q = ask(&f).expect("session a asks two questions");
    f.panel.pick(&q, 0, "Đỏ");
    f.panel.pick(&q, 1, "A");
    f.panel.pick(&q, 1, "C");
    if stage == 4 {
        return f.done();
    }
    f.refresh(95.0);
    if stage == 5 {
        return f.done();
    }
    f.store.note_sent("a", "R2");
    if stage == 6 {
        return f.done();
    }
    feed.items.lock().unwrap().clear();
    f.store.sent.clear();
    f.refresh(100.0);
    f.done()
}

pub fn state_question_1_single(dir: &Path, now: SystemTime) -> State {
    question(dir, now, 1)
}
pub fn state_question_2_single_picked(dir: &Path, now: SystemTime) -> State {
    question(dir, now, 2)
}
pub fn state_question_3_other_typed(dir: &Path, now: SystemTime) -> State {
    question(dir, now, 3)
}
pub fn state_question_4_two_questions(dir: &Path, now: SystemTime) -> State {
    question(dir, now, 4)
}
pub fn state_question_5_countdown(dir: &Path, now: SystemTime) -> State {
    question(dir, now, 5)
}
pub fn state_question_6_sent(dir: &Path, now: SystemTime) -> State {
    question(dir, now, 6)
}
pub fn state_question_7_fallback(dir: &Path, now: SystemTime) -> State {
    question(dir, now, 7)
}

/// Not a Swift state: keyboard mode (mockup 3) over the picked single question card, which is the selected one.
pub fn state_keyboard_mode(dir: &Path, now: SystemTime) -> State {
    let (snap, mut panel) = question(dir, now, 2);
    panel.keyboard = true;
    panel.selected = Some("a".into());
    (snap, panel)
}

/// Every state under its Swift name, in the order of `SnapshotTests.swift`, then `keyboard-mode`.
pub const STATES: &[(&str, StateFn)] = &[
    ("01-empty-collapsed", state_01_empty_collapsed),
    ("02-all-busy-collapsed", state_02_all_busy_collapsed),
    ("03-just-finished-peek", state_03_just_finished_peek),
    ("04-after-peek-collapsed", state_04_after_peek_collapsed),
    ("05-mixed-peek", state_05_mixed_peek),
    ("06-mixed-collapsed", state_06_mixed_collapsed),
    ("07-mixed-hover", state_07_mixed_hover),
    ("light-07-mixed-hover", state_light_07_mixed_hover),
    (
        "08-bottom-left-hover-expands-up",
        state_08_bottom_left_hover_expands_up,
    ),
    ("09-bottom-left-collapsed", state_09_bottom_left_collapsed),
    ("15-titles-branch", state_15_titles_branch),
    ("16-todo", state_16_todo),
    ("10-last-reply-hover", state_10_last_reply_hover),
    ("17-long-content", state_17_long_content),
    ("18-activity", state_18_activity),
    ("19-agents", state_19_agents),
    ("35-background-agents", state_35_background_agents),
    ("19-scroll", state_19_scroll),
    ("11-pending-hover", state_11_pending_hover),
    ("20-collapsed-opacity-0", state_20_collapsed_opacity_0),
    ("20-collapsed-opacity-25", state_20_collapsed_opacity_25),
    ("20-collapsed-opacity-92", state_20_collapsed_opacity_92),
    ("20-expanded-at-0", state_20_expanded_at_0),
    ("21-collapsed-two-digits", state_21_collapsed_two_digits),
    ("41-collapsed-badge", state_41_collapsed_badge),
    ("23-collapsed-new", state_23_collapsed_new),
    (
        "24-collapsed-new-two-digits",
        state_24_collapsed_new_two_digits,
    ),
    ("25-collapsed-no-new", state_25_collapsed_no_new),
    ("26-expanded-new", state_26_expanded_new),
    ("14-finished-collapsed", state_14_finished_collapsed),
    ("12-actions-hover", state_12_actions_hover),
    ("13-deny-reason", state_13_deny_reason),
    ("22-pr-lines", state_22_pr_lines),
    ("22-pr-lines-stale", state_22_pr_lines_stale),
    ("23-ticket-row-overflow", state_23_ticket_row_overflow),
    ("31-pr-awaiting-approval", state_31_pr_awaiting_approval),
    ("51-pr-remaining", state_51_pr_remaining),
    ("52-pr-counts-unknown", state_52_pr_counts_unknown),
    ("53-pr-all-done", state_53_pr_all_done),
    ("32-mark-claude", state_32_mark_claude),
    ("33-mark-pr", state_33_mark_pr),
    ("34-mark-both", state_34_mark_both),
    ("35-mark-pr-hidden", state_35_mark_pr_hidden),
    ("27-fresh-long-title", state_27_fresh_long_title),
    ("28-link-menu-row", state_28_link_menu_row),
    ("29-link-list-open", state_29_link_list_open),
    ("30-link-list-connect", state_30_link_list_connect),
    ("closed-1-collapsed", state_closed_1_collapsed),
    ("closed-2-open", state_closed_2_open),
    ("50-send-sending", state_50_send_sending),
    ("51-send-sent", state_51_send_sent),
    ("52-send-failed", state_52_send_failed),
    ("53-approval-sent-line", state_53_approval_sent_line),
    ("54-closed-opening", state_54_closed_opening),
    ("reply-1-buttons", state_reply_1_buttons),
    ("reply-2-editor", state_reply_2_editor),
    ("40-card-states", state_40_card_states),
    ("stop-armed", state_stop_armed),
    ("quota-1-normal", state_quota_1_normal),
    ("quota-2-warn", state_quota_2_warn),
    ("quota-3-high-forecast", state_quota_3_high_forecast),
    ("quota-5-pill-85", state_quota_5_pill_85),
    ("question-1-single", state_question_1_single),
    ("question-2-single-picked", state_question_2_single_picked),
    ("question-3-other-typed", state_question_3_other_typed),
    ("question-4-two-questions", state_question_4_two_questions),
    ("question-5-countdown", state_question_5_countdown),
    ("question-6-sent", state_question_6_sent),
    ("question-7-fallback", state_question_7_fallback),
    ("keyboard-mode", state_keyboard_mode),
];
