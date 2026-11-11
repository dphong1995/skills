//! Process liveness: `kill(pid, 0)` alone also passes a zombie or a process stuck in exit.

/// Byte offsets of `p_flag` (i32) and `p_stat` (u8) inside `kinfo_proc.kp_proc` (`struct extern_proc`: a 16 byte union,
/// `p_vmspace` and `p_sigacts` pointers, then `p_flag`, then `p_stat`). The `libc` crate has no `kinfo_proc`.
const P_FLAG_OFFSET: usize = 32;
const P_STAT_OFFSET: usize = 36;
/// `sizeof(struct kinfo_proc)` is 648 on arm64 and x86_64; the buffer leaves room.
const KINFO_PROC_BUFFER: usize = 1024;
/// `P_WEXIT` of `<sys/proc.h>`: the process is working on exiting.
const P_WEXIT: i32 = 0x0000_2000;

/// The pid exists and is not exiting or a zombie. A pid that cannot be inspected (permission, sysctl failure) counts as
/// running, as in the Swift widget's `Store.isRunning`.
pub fn is_running(pid: i32) -> bool {
    // `kill(0, ..)` and `kill(-n, ..)` signal a whole process group, so they must never pass for a pid.
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 only checks that the pid exists and may be signalled.
    let alive = unsafe { libc::kill(pid, 0) } == 0
        || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
    if !alive {
        return false;
    }
    let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
    let mut buffer = [0u8; KINFO_PROC_BUFFER];
    let mut size = buffer.len();
    // SAFETY: `mib` holds 4 ints, `buffer` is writable for `size` bytes and `size` is its length; no new value is set.
    let status = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            4,
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 || size <= P_STAT_OFFSET {
        return true;
    }
    let flag = i32::from_ne_bytes([
        buffer[P_FLAG_OFFSET],
        buffer[P_FLAG_OFFSET + 1],
        buffer[P_FLAG_OFFSET + 2],
        buffer[P_FLAG_OFFSET + 3],
    ]);
    u32::from(buffer[P_STAT_OFFSET]) != libc::SZOMB && flag & P_WEXIT == 0
}

#[cfg(test)]
mod tests {
    use super::is_running;
    use std::time::{Duration, Instant};

    #[test]
    fn a_pid_that_is_zero_or_negative_is_not_running() {
        assert!(!is_running(0));
        assert!(!is_running(-1));
        assert!(!is_running(i32::MIN));
    }

    #[test]
    fn own_pid_is_running() {
        assert!(is_running(std::process::id() as i32));
    }

    #[test]
    fn zombie_and_reaped_pids_are_not_running() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id() as i32;
        // Not reaped yet: it becomes a zombie once it exits, and `kill(pid, 0)` still succeeds for it.
        let deadline = Instant::now() + Duration::from_secs(5);
        while is_running(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!is_running(pid), "an exited, unreaped child is not running");
        child.wait().unwrap();
        assert!(!is_running(pid), "a reaped pid is not running");
    }
}
