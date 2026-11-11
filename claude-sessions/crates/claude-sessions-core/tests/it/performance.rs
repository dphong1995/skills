//! Ported from Swift `PerformanceTests.swift`. Slow and timing-bound, so each test is `#[ignore]` and also returns at once
//! unless `CLAUDE_SESSIONS_PERF_TESTS=1`. Run them in release:
//! `CLAUDE_SESSIONS_PERF_TESTS=1 cargo test --release -p claude-sessions-core --test it performance:: -- --ignored`

use crate::support::*;
use claude_sessions_core::store::{Paths, Store, StoreOptions};
use claude_sessions_core::transcript_scanner::TranscriptScanner;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

const CHUNK_BYTES: usize = 1 << 20;

fn perf_enabled() -> bool {
    std::env::var("CLAUDE_SESSIONS_PERF_TESTS").is_ok_and(|v| v == "1")
}

#[derive(Clone, Copy, Default)]
struct Counters {
    bytes: usize,
    parses: usize,
    heads: usize,
    listings: usize,
    parsed: usize,
}

impl Counters {
    fn since(self, b: Counters) -> Counters {
        Counters {
            bytes: self.bytes - b.bytes,
            parses: self.parses - b.parses,
            heads: self.heads - b.heads,
            listings: self.listings - b.listings,
            parsed: self.parsed - b.parsed,
        }
    }
}

/// Builds sessions on temp dirs: transcripts, task files and real `.git/HEAD` files, read by a `Store` that uses the real branch reader.
struct Fixture {
    dir: tempfile::TempDir,
    projects: tempfile::TempDir,
    tasks: tempfile::TempDir,
    store: Store,
    ids: Vec<String>,
    created: usize,
    transcripts: HashMap<String, PathBuf>,
    largest: usize,
    /// Every temp dir the fixture made; removed when the fixture goes away.
    temps: Vec<tempfile::TempDir>,
}

/// One repeatable block of realistic transcript lines: a prompt with a Jira link, a reply, a tool call and its result, a PR link.
fn block() -> String {
    let mut s = [
        user_line("Please fix the failing test in the parser module. See https://acme.atlassian.net/browse/ABC-1 for the details.", false, None),
        assistant_line(Some(&format!("I will look at the parser first and then run the tests. {}", "Reading the code. ".repeat(12))), false),
        tool_use_line("toolu_01", "Bash", serde_json::json!({"command": "swift test --filter ParserTests"}), false),
        tool_result_line("toolu_01"),
        user_line("The PR is https://bitbucket.org/t6engineering/talo6-dct-backend/pull-requests/12 now.", false, None),
    ]
    .join("\n");
    s.push('\n');
    s
}

fn real(bytes: usize) -> String {
    let b = block();
    b.repeat((bytes / b.len()).max(1))
}

/// Lines without a `"type":"user"` or `"type":"assistant"` marker: the scanner skips them without a JSON parse.
fn filler_mb() -> String {
    let filler = format!(
        "{{\"type\":\"progress\",\"data\":\"{}\"}}\n",
        "x".repeat(900)
    );
    filler.repeat((1 << 20) / filler.len())
}

#[derive(Default)]
struct Add {
    status: &'static str,
    waiting_for: Option<&'static str>,
    transcript_bytes: usize,
    filler_mb: usize,
    worktree: bool,
    pending_tool: bool,
}

impl Fixture {
    fn new() -> Self {
        let dir = make_temp_dir();
        let projects = make_temp_dir();
        let tasks = make_temp_dir();
        let store = Store::new(
            Paths {
                sessions: dir.path().to_path_buf(),
                projects: projects.path().to_path_buf(),
                tasks: tasks.path().to_path_buf(),
            },
            StoreOptions {
                scanner: TranscriptScanner::new(false),
                branch: None,
                ..StoreOptions::default()
            },
        );
        Fixture {
            dir,
            projects,
            tasks,
            store,
            ids: vec![],
            created: 0,
            transcripts: HashMap::new(),
            largest: 0,
            temps: vec![],
        }
    }

    /// Adds a session whose cwd is a temp dir with a git repo (`worktree`: a `.git` file pointing at a git dir elsewhere).
    fn add(&mut self, a: Add, task_files: i64) -> String {
        let id = format!("perf-{}", self.created);
        self.created += 1;
        let cwd = if a.worktree {
            let cwd = make_temp_dir();
            let gitdir_root = make_temp_dir();
            let gitdir = gitdir_root.path().join("wt");
            write(&gitdir.join("HEAD"), "ref: refs/heads/feature/wt\n");
            write(
                &cwd.path().join(".git"),
                &format!("gitdir: {}\n", gitdir.display()),
            );
            self.temps.push(gitdir_root);
            cwd
        } else {
            repo("ref: refs/heads/main\n")
        };
        let cwd_path = cwd.path().to_string_lossy().to_string();
        let file = write_transcript(self.projects.path(), &cwd_path, &id, &[]);
        self.transcripts.insert(id.clone(), file.clone());
        self.largest = self.largest.max(a.transcript_bytes + (a.filler_mb << 20));
        append(&file, &real(a.transcript_bytes));
        for _ in 0..a.filler_mb {
            append(&file, &filler_mb());
        }
        if a.pending_tool {
            let l = tool_use_line(
                "toolu_perm",
                "Bash",
                serde_json::json!({"command": "rm -rf build"}),
                false,
            );
            append(&file, &format!("{l}\n"));
        }
        for t in 1..=task_files {
            write_task(
                self.tasks.path(),
                Some(&id),
                t,
                if t == 1 { "in_progress" } else { "pending" },
                &format!("Task {t}"),
                Some(&format!("Doing task {t}")),
            );
        }
        write_session_with(
            self.dir.path(),
            &id,
            me(),
            a.status,
            60.0,
            &id,
            &cwd_path,
            SessionOpts {
                waiting_for: a.waiting_for,
                ..SessionOpts::default()
            },
        );
        self.temps.push(cwd);
        self.ids.push(id.clone());
        id
    }

    fn remove(&mut self, id: &str) {
        let _ = fs::remove_file(self.dir.path().join(format!("{id}.json")));
        self.ids.retain(|i| i != id);
    }

    /// 10 sessions: 3 busy, 4 idle, 3 waiting (one on a permission prompt); ~2 MB transcripts, the first one 64 MB.
    fn standard() -> Self {
        let mut f = Fixture::new();
        let mb2 = 2 << 20;
        for i in 0..3 {
            let a = Add {
                status: "busy",
                transcript_bytes: mb2,
                filler_mb: if i == 0 { 62 } else { 0 },
                ..Add::default()
            };
            f.add(a, 5);
        }
        for i in 0..4 {
            let a = Add {
                status: "idle",
                transcript_bytes: mb2,
                worktree: i == 0,
                ..Add::default()
            };
            f.add(a, 5);
        }
        for i in 0..3 {
            let a = Add {
                status: "waiting",
                waiting_for: (i == 2).then_some("permission prompt"),
                transcript_bytes: mb2,
                pending_tool: i == 2,
                ..Add::default()
            };
            f.add(a, 5);
        }
        f.warm_up();
        f
    }

    fn refresh(&mut self) {
        self.store.refresh(SystemTime::now());
    }

    /// Refreshes until the scanner has parsed everything: `parsed_lines` flat for 3 refreshes, after at least as many
    /// refreshes as the largest transcript has chunks (the filler part parses nothing, so it looks flat too).
    fn warm_up(&mut self) {
        let (mut flat, mut last) = (0, usize::MAX);
        let floor = self.largest / CHUNK_BYTES + 3;
        for n in 1..=200 {
            self.refresh();
            let parsed = self.store.parsed_lines();
            flat = if parsed == last { flat + 1 } else { 0 };
            last = parsed;
            if flat >= 3 && n >= floor {
                return;
            }
        }
    }

    fn counters(&self) -> Counters {
        Counters {
            bytes: self.store.transcript_bytes_read,
            parses: self.store.session_parses,
            heads: self.store.head_reads(),
            listings: self.store.task_listings,
            parsed: self.store.parsed_lines(),
        }
    }
}

#[test]
#[ignore = "slow; set CLAUDE_SESSIONS_PERF_TESTS=1 and pass --ignored"]
fn steady_refresh_reads_nothing() {
    if !perf_enabled() {
        return;
    }
    let mut f = Fixture::standard();
    let before = f.counters();
    for _ in 0..20 {
        f.refresh();
    }
    let d = f.counters().since(before);
    println!(
        "steady x20 deltas: bytes={} sessionParses={} headReads={} taskListings={} parsedLines={}",
        d.bytes, d.parses, d.heads, d.listings, d.parsed
    );
    assert_eq!(d.bytes, 0);
    assert_eq!(d.parsed, 0);
    assert_eq!(d.parses, 0);
    assert_eq!(d.heads, 0);
    assert_eq!(d.listings, 0);
}

/// 10 rounds of K bytes of `tool_use` lines appended to the 3 busy transcripts, each followed by a refresh.
#[test]
#[ignore = "slow; set CLAUDE_SESSIONS_PERF_TESTS=1 and pass --ignored"]
fn append_reads_only_new_bytes() {
    if !perf_enabled() {
        return;
    }
    let mut f = Fixture::standard();
    let busy: Vec<PathBuf> = f.ids[..3]
        .iter()
        .map(|i| f.transcripts[i].clone())
        .collect();
    let line = |n: usize| {
        let l = tool_use_line(
            &format!("toolu_a{n}"),
            "Bash",
            serde_json::json!({"command": format!("echo {n}")}),
            false,
        );
        format!("{l}\n")
    };
    let longest = line(99999).len();
    let mut n = 0;
    for round in 1..=10 {
        let before = f.counters();
        let mut appended = 0;
        for url in &busy {
            let mut c = String::new();
            while c.len() < 4096 {
                n += 1;
                c += &line(n);
            }
            appended += c.len();
            append(url, &c);
        }
        f.refresh();
        let d = f.counters().since(before);
        println!(
            "append round {round}: appended={appended} bytesRead={} sessionParses={}",
            d.bytes, d.parses
        );
        assert!(d.bytes <= appended + 3 * longest);
        assert_eq!(d.parses, 0);
    }
}

#[test]
#[ignore = "slow; set CLAUDE_SESSIONS_PERF_TESTS=1 and pass --ignored"]
fn refresh_median_under_10ms() {
    if !perf_enabled() {
        return;
    }
    let mut f = Fixture::standard();
    let mut times: Vec<Duration> = (0..50)
        .map(|_| {
            let t = Instant::now();
            f.refresh();
            t.elapsed()
        })
        .collect();
    times.sort();
    let median = times[times.len() / 2];
    let p95 = times[(times.len() as f64 * 0.95) as usize];
    println!("refresh median={median:?} p95={p95:?}");
    assert!(median < Duration::from_millis(10));
}

#[test]
#[ignore = "slow; set CLAUDE_SESSIONS_PERF_TESTS=1 and pass --ignored"]
fn caches_follow_live_sessions() {
    if !perf_enabled() {
        return;
    }
    let mut f = Fixture::new();
    for i in 0..10 {
        let a = Add {
            status: ["busy", "idle", "waiting"][i % 3],
            transcript_bytes: 16 << 10,
            ..Add::default()
        };
        f.add(a, 2);
    }
    f.refresh();
    let mut worst = 0;
    for _ in 0..25 {
        for _ in 0..2 {
            let a = Add {
                status: "idle",
                transcript_bytes: 16 << 10,
                ..Add::default()
            };
            let id = f.add(a, 2);
            f.store.surfaces.insert(id.clone(), format!("surface-{id}"));
        }
        for _ in 0..2 {
            let id = f.ids[0].clone();
            f.remove(&id);
        }
        f.refresh();
        let live = f.ids.len();
        for (name, count) in f.store.cache_counts() {
            worst = worst.max(count as i64 - live as i64);
            assert!(
                count <= live,
                "{name} holds {count} entries for {live} live sessions"
            );
        }
        // "branches" is per cwd, so it is bounded by the distinct cwds of the live sessions.
        let live_cwds = f
            .store
            .sessions
            .iter()
            .map(|s| s.cwd.as_str())
            .collect::<HashSet<_>>()
            .len();
        let branches = f.store.cache_counts()["branches"];
        assert!(
            branches > 0,
            "branches cache is empty while {live} sessions are live"
        );
        assert!(
            branches <= live_cwds,
            "branches holds {branches} entries for {live_cwds} live cwds"
        );
    }
    println!("caches: worst excess over live sessions = {worst}");
    for id in f.ids.clone() {
        f.remove(&id);
    }
    f.refresh();
    for (name, count) in f.store.cache_counts() {
        assert_eq!(
            count, 0,
            "{name} holds {count} entries with no session left"
        );
    }
}
