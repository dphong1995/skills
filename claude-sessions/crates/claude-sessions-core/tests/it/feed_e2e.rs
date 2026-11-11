//! Real cmux, real Claude (haiku, short prompts): by hand only, `CLAUDE_SESSIONS_E2E=1 cargo test -p claude-sessions-core -- --ignored feed_e2e`.
//! Each shape opens its own throwaway workspace and closes it again; only items of that workspace's own session id are touched.
use claude_sessions_core::feed::{self, FeedMonitor, UnixFeedTransport};
use claude_sessions_core::shell::run_process;
use claude_sessions_core::transcript;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const CMUX: &str = "/Applications/cmux.app/Contents/Resources/bin/cmux";

struct Shape {
    name: &'static str,
    prompt: &'static str,
    answers: Vec<Vec<String>>,
    other: HashMap<usize, String>,
    /// What the AskUserQuestion `tool_result` must contain: each answer value, not the prompt text (haiku rewords prompts).
    expected: Vec<&'static str>,
}

fn shapes() -> Vec<Shape> {
    let a = |rows: &[&[&str]]| -> Vec<Vec<String>> {
        rows.iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect()
    };
    vec![
        Shape {
            name: "single",
            prompt: "Call AskUserQuestion once with exactly one question 'Pick a color' and options Red, Blue, Green, then reply with the answer only.",
            answers: a(&[&["Blue"]]),
            other: HashMap::new(),
            expected: vec!["=\"Blue\""],
        },
        Shape {
            name: "multi",
            prompt: "Call AskUserQuestion once with exactly one question 'Pick colors' with multiSelect true and options Red, Blue, Green, then reply with the answer only.",
            answers: a(&[&["Red", "Blue"]]),
            other: HashMap::new(),
            expected: vec!["=\"Red, Blue\""],
        },
        Shape {
            name: "two",
            prompt: "Call AskUserQuestion once with exactly two questions: 'Pick a color' (options Red, Blue) and 'Pick a size' (options Small, Large), then reply with both answers only.",
            answers: a(&[&["Red"], &["Large"]]),
            other: HashMap::new(),
            expected: vec!["=\"Red\"", "=\"Large\""],
        },
        Shape {
            name: "other",
            prompt: "Call AskUserQuestion once with exactly one question 'Pick a color' and options Red, Blue, Green, then reply with the answer only.",
            answers: a(&[&[]]),
            other: HashMap::from([(0, "Purple".to_string())]),
            expected: vec!["=\"Purple\""],
        },
    ]
}

/// The text of the AskUserQuestion `tool_result` line (found by its `tool_use_id`); empty until Claude has it.
fn ask_result(id: &str, cwd: &str, projects: &Path) -> String {
    let Some(url) = transcript::find(id, cwd, projects) else {
        return String::new();
    };
    let Ok(text) = std::fs::read_to_string(url) else {
        return String::new();
    };
    let re = regex::Regex::new(r#""id":"(toolu_[A-Za-z0-9]+)","name":"AskUserQuestion""#).unwrap();
    let Some(tool_id) = text
        .lines()
        .find_map(|l| re.captures(l).map(|c| c[1].to_string()))
    else {
        return String::new();
    };
    // Raw JSONL escapes the quotes inside the tool_result text.
    let needle = format!("\"tool_use_id\":\"{tool_id}\"");
    text.lines()
        .find(|l| l.contains(&needle))
        .map(|l| l.replace("\\\"", "\""))
        .unwrap_or_default()
}

fn wait(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    done()
}

#[test]
#[ignore = "real cmux and Claude; set CLAUDE_SESSIONS_E2E=1"]
fn feed_e2e_answer_reaches_claude() {
    if std::env::var("CLAUDE_SESSIONS_E2E").as_deref() != Ok("1") {
        return;
    }
    let home = PathBuf::from(std::env::var("HOME").unwrap());
    let cmux = Path::new(CMUX);
    for shape in shapes() {
        let id = uuid::Uuid::new_v4().to_string();
        let cwd =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/e2e/{}", shape.name));
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd = cwd.canonicalize().unwrap();
        let cwd_s = cwd.to_str().unwrap();
        let command = format!(
            "claude --model haiku --session-id {id} \"{}\"",
            shape.prompt
        );
        let name = format!("e2e-{}", shape.name);
        let made = run_process(
            cmux,
            &[
                "new-workspace",
                "--name",
                &name,
                "--cwd",
                cwd_s,
                "--focus",
                "false",
                "--command",
                &command,
            ],
            Duration::from_secs(20),
            true,
            None,
        )
        .unwrap_or_default();
        let workspace = made
            .lines()
            .find_map(|l| {
                l.strip_prefix("OK ")
                    .and_then(|r| r.split(' ').next())
                    .map(String::from)
            })
            .unwrap_or_else(|| panic!("cmux new-workspace gave: {made}"));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let socket = home.join(".local/state/cmux/last-socket-path");
            let transport = UnixFeedTransport::new(&socket).expect("no cmux socket path file");
            let monitor = FeedMonitor::without_worker(Arc::new(transport));
            let mut question = None;
            assert!(
                wait(Duration::from_secs(60), || {
                    monitor.update(true, SystemTime::now());
                    question = monitor
                        .take_snapshot()
                        .into_iter()
                        .find(|q| q.session_id.as_deref() == Some(id.as_str()));
                    question.is_some()
                }),
                "no Feed question for session {id}"
            );
            let question = question.unwrap();
            assert!(feed::supports(&question));
            let sent = Arc::new(Mutex::new(None));
            let s = sent.clone();
            monitor.reply(&question, &shape.answers, &shape.other, move |ok| {
                *s.lock().unwrap() = Some(ok)
            });
            assert_eq!(*sent.lock().unwrap(), Some(true));
            let projects = home.join(".claude/projects");
            assert!(
                wait(Duration::from_secs(60), || {
                    let r = ask_result(&id, cwd_s, &projects);
                    shape.expected.iter().all(|e| r.contains(e))
                }),
                "Claude never saw {:?}",
                shape.expected
            );
        }));
        let _ = run_process(
            cmux,
            &["close-workspace", "--workspace", &workspace, "--force"],
            Duration::from_secs(20),
            true,
            None,
        );
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }
}
