use claude_sessions_core::procs::{
    GroupKind, HEAVY_BYTES, ProcModel, ProcTable, RawProc, Scan, display_name, format_bytes,
};
use std::collections::HashMap;

const S: u64 = 1_000_000;
const MB: u64 = 1_048_576;
const WIDGET: i32 = 999;

fn base(pid: i32, pgid: i32, start_s: u64, argv: &[&str]) -> RawProc {
    RawProc {
        pid,
        ppid: 1,
        pgid,
        start_us: start_s * S,
        comm: argv[0].rsplit('/').next().unwrap().to_string(),
        argv: argv.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

/// A process that carries the session id only (an MCP server).
fn mcp(pid: i32, pgid: i32, start_s: u64, argv: &[&str]) -> RawProc {
    RawProc {
        session: Some("s1".into()),
        ..base(pid, pgid, start_s, argv)
    }
}

/// A Bash-tool descendant: session id and `CLAUDE_PID`.
fn cmd(pid: i32, pgid: i32, start_s: u64, argv: &[&str]) -> RawProc {
    RawProc {
        claude_pid: Some(100),
        ..mcp(pid, pgid, start_s, argv)
    }
}

fn wrapper(pid: i32, start_s: u64, text: &str) -> RawProc {
    let script = format!(
        "source /Users/u/.claude/shell-snapshots/snapshot-zsh-1.sh && eval '{text}' < /dev/null"
    );
    cmd(pid, pid, start_s, &["/bin/zsh", "-c", &script])
}

fn claude(rss_mb: u64) -> RawProc {
    RawProc {
        rss: rss_mb * MB,
        ..base(100, 100, 0, &["claude"])
    }
}

fn scan(at_s: u64, procs: Vec<RawProc>) -> Scan {
    Scan {
        at_us: at_s * S,
        own_pid: WIDGET,
        procs,
    }
}

fn live() -> HashMap<String, i32> {
    HashMap::from([("s1".to_string(), 100)])
}

fn update(procs: Vec<RawProc>, live: &HashMap<String, i32>) -> ProcTable {
    ProcModel::default().update(&scan(100, procs), live)
}

fn vite(pid: i32, pgid: i32, start_s: u64) -> RawProc {
    RawProc {
        ports: vec![5173],
        rss: 300 * MB,
        ..cmd(pid, pgid, start_s, &["node", "/p/node_modules/.bin/vite"])
    }
}

#[test]
fn bash_command_is_its_own_group() {
    let t = update(
        vec![
            claude(500),
            wrapper(200, 10, "npm run dev"),
            vite(201, 200, 10),
        ],
        &live(),
    );
    let s = &t.live["s1"];
    assert_eq!(s.commands.len(), 1);
    let c = &s.commands[0];
    assert_eq!(
        (c.key.as_str(), c.kind, c.name.as_str()),
        ("s1:200", GroupKind::Command, "vite")
    );
    assert_eq!(c.command, "npm run dev");
    assert_eq!(c.ports, [5173]);
    assert_eq!(c.members, [(200, 10 * S), (201, 10 * S)]);
    let m = s.claude_and_mcp.as_ref().unwrap();
    assert_eq!(
        (m.key.as_str(), m.name.as_str()),
        ("s1:claude", "Claude và MCP")
    );
    assert_eq!(m.rss, 500 * MB);
    assert!(
        m.members.is_empty(),
        "the claude process is never a kill target"
    );
    assert!(t.closed.is_empty());
}

#[test]
fn nohup_leftover_with_dead_leader_is_command() {
    let p = cmd(301, 300, 10, &["node", "server.js"]);
    let t = update(vec![claude(1), p], &live());
    let s = &t.live["s1"];
    assert_eq!(s.commands.len(), 1);
    assert_eq!(s.commands[0].key, "s1:300");
    assert_eq!(s.commands[0].name, "server.js");
}

#[test]
fn leader_without_session_id_is_command() {
    let shell = base(400, 400, 5, &["-zsh"]);
    let t = update(
        vec![claude(1), shell, cmd(401, 400, 10, &["sleep", "999"])],
        &live(),
    );
    let s = &t.live["s1"];
    assert_eq!(s.commands.len(), 1);
    assert_eq!(s.commands[0].members, [(401, 10 * S)]);
}

#[test]
fn mcp_without_claude_pid_is_claude_and_mcp() {
    let p = mcp(500, 100, 3, &["node", "/x/playwright-mcp"]);
    let t = update(vec![claude(1), p], &live());
    let s = &t.live["s1"];
    assert!(s.commands.is_empty());
    assert_eq!(s.claude_and_mcp.as_ref().unwrap().members, [(500, 3 * S)]);
}

#[test]
fn cmux_hook_is_claude_and_mcp() {
    let p = cmd(600, 600, 3, &["cmux", "hooks", "claude", "inbox-wait"]);
    let t = update(vec![claude(1), p], &live());
    let s = &t.live["s1"];
    assert!(s.commands.is_empty());
    assert_eq!(s.claude_and_mcp.as_ref().unwrap().members, [(600, 3 * S)]);
}

#[test]
fn closed_session_mcp_and_command_are_separate_rows() {
    let procs = vec![
        wrapper(700, 20, "npm run dev"),
        vite(701, 700, 20),
        mcp(710, 100, 3, &["node", "/x/a-mcp"]),
        mcp(711, 100, 3, &["node", "/x/b-mcp"]),
        mcp(712, 100, 4, &["node", "/x/c-mcp"]),
    ];
    let t = update(procs, &HashMap::new());
    assert!(t.live.is_empty());
    assert_eq!(t.closed.len(), 1);
    let s = &t.closed[0];
    assert_eq!(s.session, "s1");
    assert_eq!(s.commands.len(), 1);
    assert_eq!(s.commands[0].name, "vite");
    assert_eq!(s.claude_and_mcp.as_ref().unwrap().name, "3 MCP và agent");
    assert_eq!(s.row_count(), 2);
    assert_eq!(s.started_us, 3 * S);
    assert_eq!(s.targets().len(), 5);
}

#[test]
fn session_with_live_claude_but_no_file_is_hidden() {
    let procs = vec![
        claude(1),
        wrapper(200, 10, "x"),
        mcp(500, 100, 3, &["node", "m"]),
    ];
    let t = update(procs, &HashMap::new());
    assert!(t.live.is_empty() && t.closed.is_empty());
}

#[test]
fn reused_claude_pid_with_later_start_counts_as_dead() {
    let reused = base(100, 100, 50, &["something-else"]);
    let t = update(
        vec![reused, mcp(500, 100, 3, &["node", "m"])],
        &HashMap::new(),
    );
    assert_eq!(t.closed.len(), 1);
}

#[test]
fn widget_descendants_are_skipped() {
    let own = RawProc {
        session: Some("s1".into()),
        ..base(WIDGET, WIDGET, 1, &["ClaudeSessions"])
    };
    let child = RawProc {
        ppid: WIDGET,
        ..mcp(800, 800, 2, &["node", "x"])
    };
    let grandchild = RawProc {
        ppid: 800,
        ..cmd(801, 801, 2, &["sleep", "9"])
    };
    let t = update(vec![own, child, grandchild], &HashMap::new());
    assert!(t.live.is_empty() && t.closed.is_empty());
}

#[test]
fn young_command_is_hidden_until_five_seconds() {
    let procs = |start| {
        vec![
            claude(1),
            wrapper(200, start, "x"),
            cmd(201, 200, start, &["sleep", "9"]),
        ]
    };
    let hidden = update(procs(97), &live());
    assert!(hidden.live["s1"].commands.is_empty());
    let shown = update(procs(95), &live());
    assert_eq!(shown.live["s1"].commands.len(), 1);
}

#[test]
fn rss_and_ports_sum_per_group() {
    let a = RawProc {
        rss: 100 * MB,
        ports: vec![5173],
        ..cmd(201, 200, 10, &["node", "a.js"])
    };
    let b = RawProc {
        rss: 200 * MB,
        ports: vec![8080, 5173],
        ..cmd(202, 200, 10, &["node", "b.js"])
    };
    let t = update(vec![claude(1), wrapper(200, 10, "x"), a, b], &live());
    let c = &t.live["s1"].commands[0];
    assert_eq!(c.rss, 300 * MB);
    assert_eq!(c.ports, [5173, 8080]);
    assert_eq!(c.name, "b.js", "the largest listener is named");
}

fn cpu_scans(pct_of_5s: u64, n: u64) -> Vec<Option<f64>> {
    let mut model = ProcModel::default();
    (0..n)
        .map(|i| {
            let mut leaf = cmd(201, 200, 0, &["node", "busy.js"]);
            leaf.cpu_ns = i * pct_of_5s * 50_000_000;
            let t = model.update(
                &scan(10 + 5 * i, vec![claude(1), wrapper(200, 0, "x"), leaf]),
                &live(),
            );
            t.live["s1"].commands[0].cpu_percent
        })
        .collect()
}

#[test]
fn cpu_flag_needs_sixty_seconds_above_fifty() {
    let high = cpu_scans(60, 13);
    assert!(high[..12].iter().all(Option::is_none));
    assert_eq!(high[12], Some(60.0));
    assert!(cpu_scans(40, 13).iter().all(Option::is_none));
}

#[test]
fn cpu_flag_drops_at_first_low_sample() {
    let mut model = ProcModel::default();
    let mut last = None;
    for i in 0..15u64 {
        let mut leaf = cmd(201, 200, 0, &["node", "busy.js"]);
        leaf.cpu_ns = if i == 14 {
            13 * 3_000_000_000 + 500_000_000
        } else {
            i * 3_000_000_000
        };
        let t = model.update(
            &scan(10 + 5 * i, vec![claude(1), wrapper(200, 0, "x"), leaf]),
            &live(),
        );
        last = t.live["s1"].commands[0].cpu_percent;
        if i == 13 {
            assert_eq!(last, Some(60.0));
        }
    }
    assert_eq!(last, None);
}

#[test]
fn heavy_threshold_is_2048_mb() {
    let heavy = update(
        vec![
            claude(1024),
            wrapper(200, 10, "x"),
            RawProc {
                rss: 1024 * MB,
                ..cmd(201, 200, 10, &["a"])
            },
        ],
        &live(),
    );
    assert!(heavy.live["s1"].rss() >= HEAVY_BYTES);
    let light = update(
        vec![
            claude(1024),
            wrapper(200, 10, "x"),
            RawProc {
                rss: 500 * MB,
                ..cmd(201, 200, 10, &["a"])
            },
        ],
        &live(),
    );
    assert!(light.live["s1"].rss() < HEAVY_BYTES);
}

#[test]
fn rss_rounded_to_display_precision() {
    let at = |bytes: u64| {
        update(
            vec![
                claude(1),
                wrapper(200, 10, "x"),
                RawProc {
                    rss: bytes,
                    ..cmd(201, 200, 10, &["a"])
                },
            ],
            &live(),
        )
    };
    assert_eq!(at(412 * MB + 100_000), at(412 * MB + 300_000));
    assert_eq!(at(2_600 * MB), at(2_610 * MB));
    assert_ne!(at(412 * MB), at(413 * MB));
}

#[test]
fn display_name_rules() {
    let name = |argv: &[&str]| {
        let v: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        display_name(&v, "comm")
    };
    assert_eq!(name(&["node", "/p/node_modules/.bin/vite"]), "vite");
    assert_eq!(name(&["node", "/p/node_modules/.bin/vite.js"]), "vite");
    assert_eq!(
        name(&["python3", "-m", "http.server", "8000"]),
        "http.server"
    );
    assert_eq!(
        name(&["./gradlew", "bootRun", "--args=x"]),
        "gradle bootRun"
    );
    assert_eq!(
        name(&["node", "scripts/mock-auth-server.js"]),
        "mock-auth-server.js"
    );
    assert_eq!(name(&["ugrep", "-rn", "TODO", "/"]), "ugrep");
    assert_eq!(name(&["docker", "compose", "up", "kafka"]), "docker");
    assert_eq!(name(&["/usr/bin/python3.12", "app.py"]), "app.py");
    assert_eq!(display_name(&[], "fallback"), "fallback");
}

#[test]
fn tooltip_unwraps_eval() {
    let t = update(
        vec![claude(1), wrapper(200, 10, r"echo '\''hi'\''")],
        &live(),
    );
    let c = &t.live["s1"].commands[0];
    assert_eq!(c.command, "echo 'hi'");
    assert_eq!(c.name, "echo");
}

#[test]
fn format_bytes_rules() {
    assert_eq!(format_bytes(412 * MB), "412 MB");
    assert_eq!(format_bytes(999 * MB), "999 MB");
    assert_eq!(format_bytes(1000 * MB), "1,0 GB");
    assert_eq!(format_bytes(2662 * MB + MB * 2 / 5), "2,6 GB");
}
