//! Process model: turns one raw process scan into per-session process groups (session matching, filtering, grouping, naming, memory, CPU flag). Pure: no syscalls, no clock reads.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

pub const SCAN_EVERY_TICKS: u64 = 5;
pub const MIN_AGE_SECS: u64 = 5;
pub const HEAVY_BYTES: u64 = 2048 * 1_048_576;
pub const CPU_HIGH_PERCENT: f64 = 50.0;
pub const CPU_HIGH_SECS: f64 = 60.0;
pub const KILL_GRACE_SECS: f64 = 3.0;

const MB: u64 = 1_048_576;
const GB: u64 = 1_073_741_824;
const SNAPSHOT_MARK: &str = "/.claude/shell-snapshots/";

/// One process of the user. Every process has the first five fields; the rest are filled only when `session` is Some.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RawProc {
    pub pid: i32,
    pub ppid: i32,
    pub pgid: i32,
    /// Start time, µs since the Unix epoch.
    pub start_us: u64,
    pub comm: String,
    pub argv: Vec<String>,
    /// `CLAUDE_CODE_SESSION_ID`.
    pub session: Option<String>,
    /// `CLAUDE_PID`.
    pub claude_pid: Option<i32>,
    /// `PWD`.
    pub pwd: Option<String>,
    /// Bytes.
    pub rss: u64,
    /// User + system CPU time.
    pub cpu_ns: u64,
    /// TCP ports in LISTEN.
    pub ports: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Scan {
    pub at_us: u64,
    pub own_pid: i32,
    pub procs: Vec<RawProc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    Command,
    ClaudeAndMcp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcGroup {
    /// `<session>:<pgid>` for Command, `<session>:claude` for ClaudeAndMcp.
    pub key: String,
    pub kind: GroupKind,
    pub name: String,
    /// Tooltip.
    pub command: String,
    /// Sorted, deduplicated.
    pub ports: Vec<u16>,
    pub rss: u64,
    /// Some only while the CPU flag is on.
    pub cpu_percent: Option<f64>,
    /// Earliest member start.
    pub started_us: u64,
    /// `(pid, start_us)`, the kill targets.
    pub members: Vec<(i32, u64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionProcs {
    pub session: String,
    /// Oldest first.
    pub commands: Vec<ProcGroup>,
    pub claude_and_mcp: Option<ProcGroup>,
    pub pwd: Option<String>,
    /// Earliest start of any member.
    pub started_us: u64,
}

impl SessionProcs {
    pub fn groups(&self) -> impl Iterator<Item = &ProcGroup> {
        self.commands.iter().chain(self.claude_and_mcp.iter())
    }

    /// Commands plus claude and MCP.
    pub fn rss(&self) -> u64 {
        self.groups().map(|g| g.rss).sum()
    }

    pub fn cpu_high(&self) -> bool {
        self.groups().any(|g| g.cpu_percent.is_some())
    }

    pub fn row_count(&self) -> usize {
        self.commands.len() + usize::from(self.claude_and_mcp.is_some())
    }

    /// Every member of every group.
    pub fn targets(&self) -> Vec<(i32, u64)> {
        self.groups()
            .flat_map(|g| g.members.iter().copied())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ProcTable {
    /// Session id with a live session file.
    pub live: HashMap<String, SessionProcs>,
    /// Claude proven dead; newest `started_us` first.
    pub closed: Vec<SessionProcs>,
}

struct CpuHist {
    at_us: u64,
    cpu_ns: u64,
    /// `(start_us, end_us, percent)`, every one above the threshold.
    samples: VecDeque<(u64, u64, f64)>,
}

/// Holds the CPU history between scans.
#[derive(Default)]
pub struct ProcModel {
    cpu: HashMap<String, CpuHist>,
    /// The `rss` last emitted per group key (hysteresis: see `settle_rss`).
    rss: HashMap<String, u64>,
}

impl ProcModel {
    /// `live` maps the ids of sessions whose `~/.claude/sessions/*.json` pid is running to that claude pid.
    pub fn update<'a>(&mut self, scan: &'a Scan, live: &HashMap<String, i32>) -> ProcTable {
        let by_pid: HashMap<i32, &RawProc> = scan.procs.iter().map(|p| (p.pid, p)).collect();
        let skipped = |p: &RawProc| reaches(p, scan.own_pid, &by_pid, scan.procs.len());

        let mut by_session: BTreeMap<&str, Vec<&RawProc>> = BTreeMap::new();
        // Processes without a session id (platform binaries hide their env), by group: they join a Command group.
        let mut untagged: HashMap<i32, Vec<&RawProc>> = HashMap::new();
        for p in &scan.procs {
            if skipped(p) {
                continue;
            }
            match &p.session {
                Some(s) => by_session.entry(s).or_default().push(p),
                None => untagged.entry(p.pgid).or_default().push(p),
            }
        }
        for id in live.keys() {
            by_session.entry(id).or_default();
        }

        let mut sessions: Vec<(SessionProcs, bool)> = Vec::new();
        let mut cpu_sums: HashMap<String, u64> = HashMap::new();
        for (id, all) in &by_session {
            let live_pid = live.get(*id).copied();
            let procs: Vec<&RawProc> = all
                .iter()
                .copied()
                .filter(|p| Some(p.pid) != live_pid)
                .collect();
            let claude = live_pid
                .and_then(|pid| by_pid.get(&pid).copied())
                .filter(|c| !skipped(c));

            let mut groups: BTreeMap<i32, Vec<&RawProc>> = BTreeMap::new();
            for p in &procs {
                groups.entry(p.pgid).or_default().push(p);
            }

            // Every member of a Bash-tool group: tagged processes plus the untagged ones sharing the group.
            let mates =
                |pgid: i32, ms: &[&'a RawProc]| with_mates(ms, untagged.get(&pgid), live_pid);

            if live_pid.is_none() {
                let earliest = procs.iter().map(|p| p.start_us).min().unwrap_or(0);
                let mut member_pids: HashSet<i32> = procs.iter().map(|p| p.pid).collect();
                for (pgid, ms) in &groups {
                    if ms.iter().any(|m| m.claude_pid.is_some()) {
                        member_pids.extend(mates(*pgid, ms).iter().map(|m| m.pid));
                    }
                }
                let mut candidates: HashSet<i32> =
                    procs.iter().filter_map(|p| p.claude_pid).collect();
                candidates.extend(procs.iter().map(|p| p.ppid).filter(|&pp| pp != 1));
                for (pgid, ms) in &groups {
                    if ms.iter().all(|m| m.claude_pid.is_none()) {
                        candidates.insert(*pgid);
                    }
                }
                let alive = candidates.iter().any(|c| {
                    !member_pids.contains(c)
                        && by_pid.get(c).is_some_and(|q| q.start_us <= earliest)
                });
                if alive {
                    continue;
                }
            }

            let mut commands = Vec::new();
            let mut mcp: Vec<&RawProc> = Vec::new();
            for (pgid, ms) in &groups {
                // The leader is looked up in the whole scan: an untagged one is the Bash wrapper (or a user shell).
                let leader = by_pid.get(pgid).copied().filter(|l| l.pgid == *pgid);
                let leader_ok = leader.is_none_or(is_wrapper);
                let ours = ms.iter().all(|m| {
                    m.claude_pid.is_some() && live_pid.is_none_or(|l| m.claude_pid == Some(l))
                });
                if leader_ok && ours {
                    let wrapper = leader.filter(|l| is_wrapper(l));
                    let (g, cpu) = command_group(id, *pgid, &mates(*pgid, ms), wrapper);
                    cpu_sums.insert(g.key.clone(), cpu);
                    commands.push(g);
                } else {
                    mcp.extend(ms.iter().copied());
                }
            }
            commands.sort_by(|a, b| (a.started_us, &a.key).cmp(&(b.started_us, &b.key)));

            let claude_and_mcp = (!mcp.is_empty() || claude.is_some()).then(|| {
                let (g, cpu) = mcp_group(id, &mcp, claude, live_pid.is_some());
                cpu_sums.insert(g.key.clone(), cpu);
                g
            });

            let started_us = procs
                .iter()
                .map(|p| p.start_us)
                .min()
                .or(claude.map(|c| c.start_us))
                .unwrap_or(0);
            let pwd = mcp
                .iter()
                .chain(procs.iter())
                .filter(|p| p.pwd.is_some())
                .min_by_key(|p| p.start_us)
                .and_then(|p| p.pwd.clone());
            sessions.push((
                SessionProcs {
                    session: (*id).to_string(),
                    commands,
                    claude_and_mcp,
                    pwd,
                    started_us,
                },
                live_pid.is_some(),
            ));
        }

        let flags = self.sample(scan.at_us, &cpu_sums);
        let min_age_us = MIN_AGE_SECS * 1_000_000;
        let mut table = ProcTable::default();
        let mut emitted: HashMap<String, u64> = HashMap::new();
        for (mut s, is_live) in sessions {
            for g in s.commands.iter_mut().chain(s.claude_and_mcp.iter_mut()) {
                g.cpu_percent = flags.get(&g.key).map(|p| p.round());
                g.rss = settle_rss(self.rss.get(&g.key).copied(), g.rss);
                emitted.insert(g.key.clone(), g.rss);
            }
            s.commands
                .retain(|g| g.started_us + min_age_us <= scan.at_us);
            if s.row_count() == 0 {
                continue;
            }
            if is_live {
                table.live.insert(s.session.clone(), s);
            } else {
                table.closed.push(s);
            }
        }
        self.rss = emitted;
        table.closed.sort_by(|a, b| {
            b.started_us
                .cmp(&a.started_us)
                .then_with(|| a.session.cmp(&b.session))
        });
        table
    }

    /// Records one CPU sample per key; returns the percent of every key whose flag is on. Keys missing from `sums` lose their history.
    fn sample(&mut self, at_us: u64, sums: &HashMap<String, u64>) -> HashMap<String, f64> {
        let mut old = std::mem::take(&mut self.cpu);
        let window = (CPU_HIGH_SECS * 1_000_000.0) as u64;
        let mut on = HashMap::new();
        for (key, &cpu_ns) in sums {
            let Some(mut h) = old.remove(key) else {
                let h = CpuHist {
                    at_us,
                    cpu_ns,
                    samples: VecDeque::new(),
                };
                self.cpu.insert(key.clone(), h);
                continue;
            };
            if at_us < h.at_us {
                // The clock went backwards: the history no longer lines up.
                h.samples.clear();
                h.at_us = at_us;
                h.cpu_ns = cpu_ns;
            } else if at_us > h.at_us {
                let pct = cpu_ns.saturating_sub(h.cpu_ns) as f64
                    / ((at_us - h.at_us) as f64 * 1000.0)
                    * 100.0;
                if pct > CPU_HIGH_PERCENT {
                    h.samples.push_back((h.at_us, at_us, pct));
                } else {
                    h.samples.clear();
                }
                while h.samples.front().is_some_and(|s| s.1 + window <= at_us) {
                    h.samples.pop_front();
                }
                h.at_us = at_us;
                h.cpu_ns = cpu_ns;
            }
            if let (Some(first), Some(last)) = (h.samples.front(), h.samples.back())
                && at_us.saturating_sub(first.0) >= window
            {
                on.insert(key.clone(), last.2);
            }
            self.cpu.insert(key.clone(), h);
        }
        on
    }
}

fn with_mates<'a>(
    ms: &[&'a RawProc],
    untagged: Option<&Vec<&'a RawProc>>,
    live_pid: Option<i32>,
) -> Vec<&'a RawProc> {
    let mut all = ms.to_vec();
    all.extend(
        untagged
            .into_iter()
            .flatten()
            .copied()
            .filter(|u| Some(u.pid) != live_pid),
    );
    all
}

/// Keeps the previously emitted `rss` unless the new one moved by 5 % or 16 MB, so RSS jitter does not change the table every scan.
fn settle_rss(prev: Option<u64>, new: u64) -> u64 {
    match prev {
        Some(p) if new.abs_diff(p) < 16 * MB && (new.abs_diff(p) as f64) < p as f64 * 0.05 => p,
        _ => new,
    }
}

/// True when `p` is `own` or its ppid chain (within the scan) reaches `own`.
fn reaches(p: &RawProc, own: i32, by_pid: &HashMap<i32, &RawProc>, limit: usize) -> bool {
    let mut cur = p;
    for _ in 0..=limit {
        if cur.pid == own || cur.ppid == own {
            return true;
        }
        match by_pid.get(&cur.ppid) {
            Some(next) => cur = next,
            None => return false,
        }
    }
    false
}

fn is_wrapper(p: &RawProc) -> bool {
    p.argv.iter().any(|a| a.contains(SNAPSHOT_MARK))
}

fn command_group(
    session: &str,
    pgid: i32,
    ms: &[&RawProc],
    wrapper: Option<&RawProc>,
) -> (ProcGroup, u64) {
    let others: Vec<&RawProc> = ms
        .iter()
        .copied()
        .filter(|m| Some(m.pid) != wrapper.map(|w| w.pid))
        .collect();
    // Untagged helpers (`sleep`, `sh`) never name a group while a tagged process does.
    let tagged: Vec<&RawProc> = others
        .iter()
        .copied()
        .filter(|m| m.session.is_some())
        .collect();
    let named = pick_named(if !tagged.is_empty() {
        &tagged
    } else if !others.is_empty() {
        &others
    } else {
        ms
    });
    let (name, command) = match wrapper {
        Some(w) => {
            let text = eval_text(&w.argv.join(" "));
            let name = if others.is_empty() {
                text.as_deref()
                    .and_then(|t| t.split_whitespace().next())
                    .map(|t| t.rsplit('/').next().unwrap_or(t).to_string())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| display_name(&named.argv, &named.comm))
            } else {
                display_name(&named.argv, &named.comm)
            };
            (name, text.unwrap_or_else(|| w.argv.join(" ")))
        }
        None => {
            let command = ms
                .iter()
                .find(|m| m.pid == pgid)
                .map_or_else(|| named.argv.join(" "), |l| l.argv.join(" "));
            (display_name(&named.argv, &named.comm), command)
        }
    };
    aggregate(
        format!("{session}:{pgid}"),
        GroupKind::Command,
        name,
        command,
        ms,
        None,
    )
}

fn mcp_group(
    session: &str,
    ms: &[&RawProc],
    claude: Option<&RawProc>,
    live: bool,
) -> (ProcGroup, u64) {
    let name = if live {
        "Claude và MCP".to_string()
    } else {
        format!("{} MCP và agent", ms.len())
    };
    let command = if ms.is_empty() {
        claude.map(|c| c.argv.join(" ")).unwrap_or_default()
    } else {
        pick_named(ms).argv.join(" ")
    };
    aggregate(
        format!("{session}:claude"),
        GroupKind::ClaudeAndMcp,
        name,
        command,
        ms,
        claude,
    )
}

/// The member that gives the group its name: a port listener, else any member; the oldest first, then the lowest pid (stable between scans).
fn pick_named<'a>(ms: &[&'a RawProc]) -> &'a RawProc {
    let best = |listening: bool| {
        ms.iter()
            .copied()
            .filter(|m| !listening || !m.ports.is_empty())
            .min_by_key(|m| (m.start_us, m.pid))
    };
    best(true).or_else(|| best(false)).unwrap_or(ms[0])
}

fn aggregate(
    key: String,
    kind: GroupKind,
    name: String,
    command: String,
    ms: &[&RawProc],
    claude: Option<&RawProc>,
) -> (ProcGroup, u64) {
    let rss = ms.iter().map(|m| m.rss).sum::<u64>() + claude.map_or(0, |c| c.rss);
    let ports: BTreeSet<u16> = ms.iter().flat_map(|m| m.ports.iter().copied()).collect();
    let started_us = ms
        .iter()
        .map(|m| m.start_us)
        .min()
        .or(claude.map(|c| c.start_us))
        .unwrap_or(0);
    let mut members: Vec<(i32, u64)> = ms.iter().map(|m| (m.pid, m.start_us)).collect();
    members.sort_unstable();
    let cpu = ms.iter().map(|m| m.cpu_ns).sum();
    let group = ProcGroup {
        key,
        kind,
        name,
        command,
        ports: ports.into_iter().collect(),
        rss: round_bytes(rss),
        cpu_percent: None,
        started_us,
        members,
    };
    (group, cpu)
}

/// The text between `eval '` and its matching `'`, with `'\''` unescaped.
fn eval_text(s: &str) -> Option<String> {
    let rest = &s[s.find("eval '")? + "eval '".len()..];
    let mut out = String::new();
    let mut i = 0;
    while i < rest.len() {
        if rest[i..].starts_with("'\\''") {
            out.push('\'');
            i += 4;
        } else if rest[i..].starts_with('\'') {
            return Some(out);
        } else {
            let c = rest[i..].chars().next()?;
            out.push(c);
            i += c.len_utf8();
        }
    }
    None
}

/// Rounds to what `format_bytes` shows, so an unchanged display gives an equal table.
fn round_bytes(bytes: u64) -> u64 {
    let mb = bytes as f64 / MB as f64;
    if mb < 999.5 {
        mb.round() as u64 * MB
    } else {
        (mb / 1024.0 * 10.0).round() as u64 * GB / 10
    }
}

/// `412 MB`, or `2,6 GB` from 1000 MB.
pub fn format_bytes(bytes: u64) -> String {
    let mb = bytes as f64 / MB as f64;
    if mb < 999.5 {
        format!("{} MB", mb.round() as u64)
    } else {
        format!("{:.1} GB", mb / 1024.0).replace('.', ",")
    }
}

fn basename(s: &str) -> &str {
    s.rsplit('/').next().unwrap_or(s)
}

fn is_runner(b: &str) -> bool {
    // The framework python of Homebrew is named `Python`.
    let lower = b.to_ascii_lowercase();
    lower.starts_with("python3.")
        || matches!(
            lower.as_str(),
            "node"
                | "python"
                | "python3"
                | "ruby"
                | "java"
                | "sh"
                | "bash"
                | "zsh"
                | "npm"
                | "npx"
                | "pnpm"
                | "yarn"
                | "bun"
                | "deno"
        )
}

pub fn display_name(argv: &[String], comm: &str) -> String {
    let base = argv.first().map_or("", |a| basename(a));
    let base = if base.is_empty() {
        basename(comm)
    } else {
        base
    };
    let args = argv.get(1..).unwrap_or(&[]);
    if base == "gradlew" {
        return match args.iter().find(|a| !a.starts_with('-')) {
            Some(a) => format!("gradle {a}"),
            None => base.to_string(),
        };
    }
    if !is_runner(base) {
        return base.to_string();
    }
    for (i, a) in args.iter().enumerate() {
        if a == "-m" {
            if let Some(m) = args.get(i + 1) {
                return m.clone();
            }
            break;
        }
        if !a.starts_with('-') {
            let mut name = basename(a);
            if a.contains("node_modules/.bin") {
                for ext in [".js", ".mjs", ".cjs", ".py"] {
                    if let Some(stem) = name.strip_suffix(ext) {
                        name = stem;
                        break;
                    }
                }
            }
            return if name.is_empty() {
                base.to_string()
            } else {
                name.to_string()
            };
        }
    }
    base.to_string()
}
