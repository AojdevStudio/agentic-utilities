//! Closing inherited fds in a freshly forked child, before it execs.

pub(super) unsafe fn close_from(first: i32, keep: i32, max_fd: i32) {
    #[cfg(target_os = "linux")]
    {
        let below = libc::syscall(libc::SYS_close_range, first as u32, (keep - 1) as u32, 0);
        let above = libc::syscall(libc::SYS_close_range, (keep + 1) as u32, u32::MAX, 0);
        if below == 0 && above == 0 {
            return;
        }
    }
    for fd in first..max_fd {
        if fd != keep {
            libc::close(fd);
        }
    }
}

/// The close loop must reach every inherited fd, even one above a lowered soft limit.
pub(crate) fn close_bound(soft_limit: u64, highest_open: Option<i32>) -> i32 {
    let limit = soft_limit.min(1 << 20) as i32;
    highest_open.map_or(limit, |fd| limit.max(fd + 1))
}

pub(super) fn max_fd() -> i32 {
    // SAFETY: getrlimit writes one rlimit through a valid pointer.
    let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
    let soft = match unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } {
        0 => limit.rlim_cur,
        _ => u64::MAX,
    };
    close_bound(soft, highest_open_fd())
}

fn highest_open_fd() -> Option<i32> {
    let listing = if cfg!(target_os = "linux") {
        "/proc/self/fd"
    } else {
        "/dev/fd"
    };
    std::fs::read_dir(listing)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .max()
}
