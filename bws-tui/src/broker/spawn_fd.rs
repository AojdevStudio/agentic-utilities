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

/// Close every descriptor from `first` up. Runs in a forked child, so it
/// uses raw syscalls only. `ceiling` must exceed every open descriptor.
pub(super) unsafe fn close_all_from(first: i32, ceiling: i32) {
    #[cfg(target_os = "linux")]
    if libc::syscall(libc::SYS_close_range, first as u32, u32::MAX, 0) == 0 {
        return;
    }
    for fd in first..ceiling {
        libc::close(fd);
    }
}

/// One past the highest descriptor number this process can ever hold. The
/// kernel never installs a descriptor at or above the hard `RLIMIT_NOFILE`,
/// and nothing in the daemon raises it, so the hard limit bounds every future
/// descriptor. Inherited descriptors above it are counted at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FdCeiling(i32);

impl FdCeiling {
    /// Fix the ceiling. On macOS an unlimited hard limit is lowered to the
    /// kernel's per-process maximum, which no process can exceed anyway. Call
    /// before the daemon starts threads, so no descriptor appears mid-count.
    pub(crate) fn establish() -> std::io::Result<Self> {
        // SAFETY: getrlimit writes one rlimit through a valid pointer.
        let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        #[cfg(target_os = "macos")]
        {
            let most = per_process_maximum()?;
            if limit.rlim_max > most {
                limit.rlim_max = most;
                limit.rlim_cur = limit.rlim_cur.min(most);
                // SAFETY: setrlimit reads one valid rlimit and only lowers the hard limit.
                if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
        }
        let hard = i32::try_from(limit.rlim_max).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "no finite descriptor limit",
            )
        })?;
        let highest = highest_open_fd().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "cannot list open descriptors")
        })?;
        Ok(Self(hard.max(highest + 1)))
    }

    pub(crate) fn get(self) -> i32 {
        self.0
    }
}

#[cfg(target_os = "macos")]
fn per_process_maximum() -> std::io::Result<libc::rlim_t> {
    let mut most: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    // SAFETY: the name is NUL terminated and the out buffer holds one c_int.
    let read = unsafe {
        libc::sysctlbyname(
            c"kern.maxfilesperproc".as_ptr(),
            (&mut most as *mut libc::c_int).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 || most <= 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(most as libc::rlim_t)
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
