//! Process scanner and killer for the per-session process list: reads the system through `libproc` and `sysctl`
//! (never forks `ps` or `lsof`), caches argv and env per `(pid, start)`, and kills `(pid, start)` targets.
use claude_sessions_core::procs::{KILL_GRACE_SECS, RawProc, Scan};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::mem::{MaybeUninit, size_of};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// `PROC_UID_ONLY` of `<sys/proc_info.h>`; the `libc` crate lacks it.
const PROC_UID_ONLY: u32 = 4;
/// `PROC_PIDFDSOCKETINFO` of `<sys/proc_info.h>`; the `libc` crate lacks it.
const PROC_PIDFDSOCKETINFO: libc::c_int = 3;
/// `SOCKINFO_TCP` and `TSI_S_LISTEN` of `<sys/proc_info.h>`.
const SOCKINFO_TCP: i32 = 2;
const TSI_S_LISTEN: i32 = 1;
/// `SZOMB` of `<sys/proc.h>`.
const SZOMB: u32 = 5;
/// `kern.argmax` is 1 MiB on macOS; the buffer for `KERN_PROCARGS2`.
const ARGS_BUFFER: usize = 1 << 20;
/// A process younger than this may still be between `fork` and `exec` (same pid and start, other env): not cached yet.
const SETTLE_US: u64 = 1_000_000;
const KILL_FINAL_WAIT: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(25);

/// `struct proc_fileinfo` of `<sys/proc_info.h>`.
#[repr(C)]
struct ProcFileInfo {
    fi_openflags: u32,
    fi_status: u32,
    fi_offset: i64,
    fi_type: i32,
    fi_guardflags: u32,
}

/// `struct vinfo_stat` of `<sys/proc_info.h>`, only its size matters.
#[repr(C)]
struct VinfoStat([u8; 136]);

/// `struct sockbuf_info` of `<sys/proc_info.h>`.
#[repr(C)]
struct SockbufInfo([u8; 24]);

/// `struct in_sockinfo` of `<sys/proc_info.h>`: `insi_lport` (network byte order) is at offset 4.
#[repr(C)]
struct InSockInfo {
    insi_fport: i32,
    insi_lport: i32,
    rest: [u8; 72],
}

/// `struct tcp_sockinfo` of `<sys/proc_info.h>` (the union member is padded to the union size).
#[repr(C)]
struct TcpSockInfo {
    tcpsi_ini: InSockInfo,
    tcpsi_state: i32,
    rest: [u8; 444],
}

/// `struct socket_info` of `<sys/proc_info.h>`.
#[repr(C)]
struct SocketInfo {
    soi_stat: VinfoStat,
    soi_so: u64,
    soi_pcb: u64,
    soi_type: i32,
    soi_protocol: i32,
    soi_family: i32,
    soi_shorts: [i16; 7],
    soi_error: u16,
    soi_oobmark: u32,
    soi_rcv: SockbufInfo,
    soi_snd: SockbufInfo,
    soi_kind: i32,
    rfu_1: u32,
    soi_proto: TcpSockInfo,
}

/// `struct socket_fdinfo` of `<sys/proc_info.h>`; 792 bytes on arm64 and x86_64.
#[repr(C)]
struct SocketFdInfo {
    pfi: ProcFileInfo,
    psi: SocketInfo,
}

#[derive(Clone)]
struct EnvInfo {
    argv: Vec<String>,
    session: Option<String>,
    claude_pid: Option<i32>,
    pwd: Option<String>,
}

pub struct Scanner {
    env: HashMap<(i32, u64), Option<EnvInfo>>,
    args_buffer: Vec<u8>,
    timebase: (u64, u64),
    #[cfg(test)]
    args_reads: usize,
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner {
    pub fn new() -> Self {
        let timebase = timebase();
        Self {
            env: HashMap::new(),
            args_buffer: vec![0; ARGS_BUFFER],
            timebase,
            #[cfg(test)]
            args_reads: 0,
        }
    }

    /// Every process of the current uid; env/argv read once per (pid, start); rss, cpu_ns and ports only for processes
    /// with a session id. A process that exits mid-scan is skipped.
    pub fn scan(&mut self) -> Scan {
        let own_pid = std::process::id() as i32;
        let at_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_micros() as u64);
        let mut procs = Vec::new();
        let mut seen = HashSet::new();
        for pid in list_pids() {
            let Some(bsd) = bsd_info(pid) else { continue };
            if bsd.pbi_status == SZOMB {
                continue;
            }
            let start_us = start_of(&bsd);
            seen.insert((pid, start_us));
            let info = self.env_of(pid, start_us, at_us);
            let mut proc = RawProc {
                pid,
                ppid: bsd.pbi_ppid as i32,
                pgid: bsd.pbi_pgid as i32,
                start_us,
                comm: c_chars(&bsd.pbi_comm),
                ..RawProc::default()
            };
            if let Some(info) = info {
                proc.argv = info.argv;
                proc.session = info.session;
                proc.claude_pid = info.claude_pid;
                proc.pwd = info.pwd;
            }
            if proc.session.is_some() {
                if let Some(task) = task_info(pid) {
                    proc.rss = task.pti_resident_size;
                    let ticks = task.pti_total_user.saturating_add(task.pti_total_system);
                    proc.cpu_ns = (u128::from(ticks) * u128::from(self.timebase.0)
                        / u128::from(self.timebase.1)) as u64;
                }
                proc.ports = listen_ports(pid);
            }
            procs.push(proc);
        }
        self.env.retain(|key, _| seen.contains(key));
        Scan {
            at_us,
            own_pid,
            procs,
        }
    }

    fn env_of(&mut self, pid: i32, start_us: u64, at_us: u64) -> Option<EnvInfo> {
        if let Some(cached) = self.env.get(&(pid, start_us)) {
            return cached.clone();
        }
        #[cfg(test)]
        {
            self.args_reads += 1;
        }
        let info = read_procargs(pid, &mut self.args_buffer);
        if at_us.saturating_sub(start_us) >= SETTLE_US {
            self.env.insert((pid, start_us), info.clone());
        }
        info
    }
}

/// `(numer, denom)` converting Mach absolute time to ns. `libc` marks the fields deprecated in favour of `mach2`.
#[allow(deprecated)]
fn timebase() -> (u64, u64) {
    let mut info = libc::mach_timebase_info { numer: 1, denom: 1 };
    // SAFETY: `info` is a valid, writable `mach_timebase_info`.
    if unsafe { libc::mach_timebase_info(&mut info) } != 0 || info.denom == 0 {
        return (1, 1);
    }
    (u64::from(info.numer), u64::from(info.denom))
}

fn list_pids() -> Vec<i32> {
    // SAFETY: `getuid` has no preconditions.
    let uid = unsafe { libc::getuid() };
    // SAFETY: a null buffer with size 0 only asks for the byte count.
    let bytes = unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, std::ptr::null_mut(), 0) };
    if bytes <= 0 {
        return Vec::new();
    }
    // Room for processes started between the two calls.
    let mut pids = vec![0i32; bytes as usize / size_of::<i32>() + 64];
    // SAFETY: `pids` is writable for the byte length passed.
    let got = unsafe {
        libc::proc_listpids(
            PROC_UID_ONLY,
            uid,
            pids.as_mut_ptr().cast(),
            (pids.len() * size_of::<i32>()) as libc::c_int,
        )
    };
    if got <= 0 {
        return Vec::new();
    }
    pids.truncate(got as usize / size_of::<i32>());
    pids.retain(|&p| p > 0);
    pids
}

fn bsd_info(pid: i32) -> Option<libc::proc_bsdinfo> {
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    // SAFETY: the buffer is writable for `size_of::<proc_bsdinfo>()` bytes.
    let got = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size_of::<libc::proc_bsdinfo>() as libc::c_int,
        )
    };
    if got != size_of::<libc::proc_bsdinfo>() as libc::c_int {
        return None;
    }
    // SAFETY: the kernel filled the whole struct (checked above); all fields are plain integers.
    Some(unsafe { info.assume_init() })
}

fn task_info(pid: i32) -> Option<libc::proc_taskinfo> {
    let mut info = MaybeUninit::<libc::proc_taskinfo>::zeroed();
    // SAFETY: the buffer is writable for `size_of::<proc_taskinfo>()` bytes.
    let got = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size_of::<libc::proc_taskinfo>() as libc::c_int,
        )
    };
    if got != size_of::<libc::proc_taskinfo>() as libc::c_int {
        return None;
    }
    // SAFETY: the kernel filled the whole struct (checked above); all fields are plain integers.
    Some(unsafe { info.assume_init() })
}

fn start_of(bsd: &libc::proc_bsdinfo) -> u64 {
    bsd.pbi_start_tvsec * 1_000_000 + bsd.pbi_start_tvusec
}

fn c_chars(chars: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn listen_ports(pid: i32) -> Vec<u16> {
    // SAFETY: a null buffer with size 0 only asks for the byte count.
    let bytes =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
    if bytes <= 0 {
        return Vec::new();
    }
    let entry = size_of::<libc::proc_fdinfo>();
    let mut fds = vec![
        libc::proc_fdinfo {
            proc_fd: 0,
            proc_fdtype: 0
        };
        bytes as usize / entry + 16
    ];
    // SAFETY: `fds` is writable for the byte length passed.
    let got = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDLISTFDS,
            0,
            fds.as_mut_ptr().cast::<c_void>(),
            (fds.len() * entry) as libc::c_int,
        )
    };
    if got <= 0 {
        return Vec::new();
    }
    fds.truncate(got as usize / entry);
    let mut ports = Vec::new();
    for fd in fds
        .iter()
        .filter(|f| f.proc_fdtype == libc::PROX_FDTYPE_SOCKET as u32)
    {
        let mut info = MaybeUninit::<SocketFdInfo>::zeroed();
        // SAFETY: the buffer is writable for `size_of::<SocketFdInfo>()` bytes.
        let got = unsafe {
            libc::proc_pidfdinfo(
                pid,
                fd.proc_fd,
                PROC_PIDFDSOCKETINFO,
                info.as_mut_ptr().cast(),
                size_of::<SocketFdInfo>() as libc::c_int,
            )
        };
        if got != size_of::<SocketFdInfo>() as libc::c_int {
            continue;
        }
        // SAFETY: the kernel filled the whole struct (checked above); all fields are plain integers.
        let info = unsafe { info.assume_init() };
        let tcp = &info.psi.soi_proto;
        if info.psi.soi_kind == SOCKINFO_TCP && tcp.tcpsi_state == TSI_S_LISTEN {
            ports.push(u16::from_be(tcp.tcpsi_ini.insi_lport as u16));
        }
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// `KERN_PROCARGS2`: `argc: i32`, exec path, NULs, `argc` argv strings, then `KEY=VALUE` strings.
fn read_procargs(pid: i32, buffer: &mut [u8]) -> Option<EnvInfo> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size = buffer.len();
    // SAFETY: `mib` holds 3 ints, `buffer` is writable for `size` bytes; no new value is set.
    let status = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 || size < 4 {
        return None;
    }
    let data = &buffer[..size];
    let argc = i32::from_ne_bytes([data[0], data[1], data[2], data[3]]).max(0) as usize;
    let mut rest = data[4..].split(|&b| b == 0);
    rest.next()?; // exec path
    let mut strings = rest.skip_while(|s| s.is_empty());
    let argv: Vec<String> = strings
        .by_ref()
        .take(argc)
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    let mut info = EnvInfo {
        argv,
        session: None,
        claude_pid: None,
        pwd: None,
    };
    for entry in strings.take_while(|s| !s.is_empty()) {
        let entry = String::from_utf8_lossy(entry);
        if let Some(v) = entry.strip_prefix("CLAUDE_CODE_SESSION_ID=") {
            info.session = Some(v.to_string());
        } else if let Some(v) = entry.strip_prefix("CLAUDE_PID=") {
            info.claude_pid = v.parse().ok();
        } else if let Some(v) = entry.strip_prefix("PWD=") {
            info.pwd = Some(v.to_string());
        }
    }
    Some(info)
}

#[derive(Debug, PartialEq)]
pub enum KillOutcome {
    AllGone,
    StillAlive(usize),
}

/// The process still has this start time and is not a zombie.
fn alive(pid: i32, start_us: u64) -> bool {
    bsd_info(pid).is_some_and(|b| b.pbi_status != SZOMB && start_of(&b) == start_us)
}

fn wait_gone(pending: &[(i32, u64)], limit: Duration) -> Vec<(i32, u64)> {
    let deadline = Instant::now() + limit;
    let mut left: Vec<(i32, u64)> = pending.to_vec();
    loop {
        left.retain(|&(pid, start)| alive(pid, start));
        if left.is_empty() || Instant::now() >= deadline {
            return left;
        }
        std::thread::sleep(POLL);
    }
}

/// Signals `pid` only when it still has the shown start time. True when the signal was sent.
fn signal(pid: i32, start_us: u64, sig: libc::c_int) -> bool {
    if !alive(pid, start_us) {
        return false;
    }
    // SAFETY: `pid` is a single positive pid other than 1 and our own (callers filter); no process group is signalled.
    unsafe { libc::kill(pid, sig) == 0 }
}

/// SIGTERM each target whose start still matches, wait up to `KILL_GRACE_SECS`, SIGKILL the survivors, wait up to 1 s.
/// Skips pid <= 1 and the caller's own pid. Blocks; call it off the main thread.
pub fn kill(targets: &[(i32, u64)]) -> KillOutcome {
    let own = std::process::id() as i32;
    let sent: Vec<(i32, u64)> = targets
        .iter()
        .copied()
        .filter(|&(pid, start)| pid > 1 && pid != own && signal(pid, start, libc::SIGTERM))
        .collect();
    let survivors = wait_gone(&sent, Duration::from_secs_f64(KILL_GRACE_SECS));
    for &(pid, start) in &survivors {
        signal(pid, start, libc::SIGKILL);
    }
    let left = wait_gone(&survivors, KILL_FINAL_WAIT);
    if left.is_empty() {
        KillOutcome::AllGone
    } else {
        KillOutcome::StillAlive(left.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    struct Guard(Child);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn unique() -> String {
        format!(
            "test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    fn tagged(program: &str, args: &[&str], session: &str) -> Command {
        let mut c = Command::new(program);
        c.args(args)
            .env("CLAUDE_CODE_SESSION_ID", session)
            .env("CLAUDE_PID", "1234")
            .env("PWD", "/tmp")
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        c
    }

    fn find(scan: &Scan, pid: i32) -> RawProc {
        scan.procs
            .iter()
            .find(|p| p.pid == pid)
            .unwrap_or_else(|| panic!("pid {pid} not in scan"))
            .clone()
    }

    fn start_of_pid(pid: i32) -> u64 {
        start_of(&bsd_info(pid).expect("process exists"))
    }

    #[test]
    fn socket_fdinfo_matches_the_header_size() {
        assert_eq!(size_of::<SocketFdInfo>(), 792);
    }

    #[test]
    fn scan_sees_child_with_session_env() {
        let session = unique();
        // Not `sleep`: the kernel hides the env of platform binaries (`/bin/*`) from `KERN_PROCARGS2`.
        let nap = "import time;time.sleep(30)";
        let child = Guard(tagged("python3", &["-c", nap], &session).spawn().unwrap());
        let pid = child.0.id() as i32;
        std::thread::sleep(Duration::from_millis(500)); // the `python3` shim execs the real binary
        let scan = Scanner::new().scan();
        let p = find(&scan, pid);
        assert_eq!(p.ppid, std::process::id() as i32);
        // SAFETY: `getpgid` only reads.
        assert_eq!(p.pgid, unsafe { libc::getpgid(pid) });
        assert_eq!(p.session.as_deref(), Some(session.as_str()));
        assert_eq!(p.claude_pid, Some(1234));
        assert_eq!(p.pwd.as_deref(), Some("/tmp"));
        // The shim re-execs the real binary, so argv[0] is its full path.
        assert_eq!(p.argv[1..], ["-c", nap]);
        assert!(p.comm.starts_with("Python") || p.comm.starts_with("python"));
        assert!(p.rss > 0);
        assert!(p.start_us > 0);
        assert_eq!(scan.own_pid, std::process::id() as i32);
    }

    #[test]
    fn scan_reads_listen_port() {
        let code = "import socket;s=socket.socket();s.bind(('127.0.0.1',0));s.listen(1);print(s.getsockname()[1],flush=True);input()";
        let mut cmd = tagged("python3", &["-c", code], &unique());
        let mut child = Guard(
            cmd.stdout(Stdio::piped())
                .stdin(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut line = String::new();
        BufReader::new(child.0.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port: u16 = line.trim().parse().unwrap();
        let scan = Scanner::new().scan();
        let p = find(&scan, child.0.id() as i32);
        assert!(p.ports.contains(&port), "{port} not in {:?}", p.ports);
    }

    #[test]
    fn scan_ignores_env_of_untagged() {
        let child = Guard(
            Command::new("sleep")
                .arg("30")
                .env_remove("CLAUDE_CODE_SESSION_ID")
                .spawn()
                .unwrap(),
        );
        let p = find(&Scanner::new().scan(), child.0.id() as i32);
        assert_eq!(p.session, None);
        assert_eq!(p.rss, 0);
        assert_eq!(p.argv, ["sleep", "30"]);
    }

    #[test]
    fn env_is_read_once_per_pid_start() {
        let child = Guard(tagged("sleep", &["30"], &unique()).spawn().unwrap());
        let pid = child.0.id() as i32;
        std::thread::sleep(Duration::from_millis(1200));
        let mut scanner = Scanner::new();
        scanner.scan();
        let after_first = scanner.args_reads;
        assert!(after_first > 0);
        scanner.scan();
        // Processes started between the scans add reads; the cached ones do not.
        assert!(
            scanner.args_reads < after_first + 10,
            "{} vs {after_first}",
            scanner.args_reads
        );
        assert!(scanner.env.contains_key(&(pid, start_of_pid(pid))));
    }

    #[test]
    fn kill_terminates_child() {
        let mut child = tagged("sleep", &["30"], &unique()).spawn().unwrap();
        let pid = child.id() as i32;
        let start = start_of_pid(pid);
        assert_eq!(kill(&[(pid, start)]), KillOutcome::AllGone);
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn kill_escalates_to_sigkill() {
        use std::os::unix::process::CommandExt;
        let mut child = tagged("sh", &["-c", "trap '' TERM; sleep 30"], &unique())
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        std::thread::sleep(Duration::from_millis(300));
        let start = start_of_pid(pid);
        let began = Instant::now();
        let outcome = kill(&[(pid, start)]);
        let elapsed = began.elapsed();
        // The shell leads its own group; the orphaned `sleep` keeps running after the shell is killed.
        // SAFETY: test cleanup of the group this test created.
        unsafe { libc::kill(-pid, libc::SIGKILL) };
        assert_eq!(outcome, KillOutcome::AllGone);
        assert!(elapsed >= Duration::from_secs(3), "{elapsed:?}");
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn kill_skips_pid_with_other_start() {
        let child = Guard(tagged("sleep", &["30"], &unique()).spawn().unwrap());
        let pid = child.0.id() as i32;
        let start = start_of_pid(pid);
        assert_eq!(kill(&[(pid, start + 1)]), KillOutcome::AllGone);
        assert!(alive(pid, start));
    }

    #[test]
    fn kill_never_signals_self_or_init() {
        let own = std::process::id() as i32;
        let start = start_of_pid(own);
        assert_eq!(kill(&[(own, start), (1, 0)]), KillOutcome::AllGone);
        assert!(alive(own, start));
    }

    #[test]
    #[ignore = "timing on the real machine"]
    fn scan_timing() {
        let mut scanner = Scanner::new();
        scanner.scan();
        let began = Instant::now();
        let scan = scanner.scan();
        println!(
            "second scan: {:?} for {} processes",
            began.elapsed(),
            scan.procs.len()
        );
    }
}
