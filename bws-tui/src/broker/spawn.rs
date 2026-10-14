//! Raw fork and exec for `hush run` children.
//!
//! std's `Command` copies the environment into buffers it never zeroes and
//! changes directory before dropping privileges. This path prepares every
//! buffer before `fork`, zeroes the env buffers after, and runs only
//! async-signal-safe calls in the child: new process group, stdio and
//! delivery pipes, close every other fd, `setgroups`, `setgid`, `setuid`,
//! `chdir`, then `execve`.

use super::identity::Identity;
use anyhow::{bail, Context, Result};
use std::ffi::CString;
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use zeroize::Zeroizing;

pub(crate) const FIRST_DELIVERY_FD: i32 = 3;
/// Delivery pipes per run. Bounds the fixed-size fd table the child uses.
pub(crate) const MAX_DELIVERY_FDS: usize = 32;

pub(crate) struct ChildPlan {
    pub program: CString,
    pub argv: Vec<CString>,
    pub envp: Vec<Zeroizing<Vec<u8>>>,
    pub cwd: CString,
    /// `None` keeps the daemon's own identity; only same-user development uses it.
    pub identity: Option<Identity>,
    pub delivery_fds: usize,
}

pub(crate) struct SpawnedChild {
    pub pid: libc::pid_t,
    pub stdin: File,
    pub stdout: File,
    pub stderr: File,
    pub delivery: Vec<File>,
}

const STAGES: [&str; 8] = [
    "setpgid",
    "dup2",
    "signal reset",
    "setgroups",
    "setgid",
    "setuid",
    "chdir",
    "execve",
];

pub(crate) fn spawn(plan: &ChildPlan) -> Result<SpawnedChild> {
    if plan.delivery_fds > MAX_DELIVERY_FDS {
        bail!("at most {MAX_DELIVERY_FDS} --fd bindings per run");
    }
    let (stdin_r, stdin_w) = cloexec_pipe()?;
    let (stdout_r, stdout_w) = cloexec_pipe()?;
    let (stderr_r, stderr_w) = cloexec_pipe()?;
    let (err_r, err_w) = cloexec_pipe()?;
    let mut delivery = Vec::with_capacity(plan.delivery_fds);
    for _ in 0..plan.delivery_fds {
        delivery.push(cloexec_pipe()?);
    }
    let mut sources = vec![
        stdin_r.as_raw_fd(),
        stdout_w.as_raw_fd(),
        stderr_w.as_raw_fd(),
    ];
    sources.extend(delivery.iter().map(|(read, _)| read.as_raw_fd()));
    let argv: Vec<*const libc::c_char> = plan
        .argv
        .iter()
        .map(|arg| arg.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect();
    let envp: Vec<*const libc::c_char> = plan
        .envp
        .iter()
        .map(|entry| entry.as_ptr().cast())
        .chain(std::iter::once(std::ptr::null()))
        .collect();
    let groups: Vec<libc::gid_t> = plan
        .identity
        .as_ref()
        .map(|identity| {
            identity
                .groups
                .iter()
                .map(|&gid| gid as libc::gid_t)
                .collect()
        })
        .unwrap_or_default();
    let raw = RawChild {
        program: plan.program.as_ptr(),
        argv: argv.as_ptr(),
        envp: envp.as_ptr(),
        cwd: plan.cwd.as_ptr(),
        sources: &sources,
        err: err_w.as_raw_fd(),
        identity: plan
            .identity
            .as_ref()
            .map(|identity| (identity.uid, identity.gid)),
        groups: &groups,
        max_fd: max_fd(),
    };
    // SAFETY: the child branch runs only async-signal-safe calls on data
    // prepared above, then execs or exits. Nothing in it allocates.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(std::io::Error::last_os_error()).context("fork failed");
    }
    if pid == 0 {
        // SAFETY: we are the forked child; `raw` points at live parent-prepared buffers.
        unsafe { raw.exec() }
    }
    drop((stdin_r, stdout_w, stderr_w, err_w));
    let delivery_writers: Vec<File> = delivery
        .into_iter()
        .map(|(read, write)| {
            drop(read);
            File::from(write)
        })
        .collect();
    let mut report = [0u8; 5];
    let read = read_full(&File::from(err_r), &mut report)?;
    if read > 0 {
        let mut status = 0;
        // SAFETY: pid is our own child; waitpid reaps it.
        unsafe { libc::waitpid(pid, &mut status, 0) };
        let stage = STAGES.get(report[0] as usize).copied().unwrap_or("start");
        let errno = i32::from_be_bytes([report[1], report[2], report[3], report[4]]);
        bail!(
            "failed to start the child ({stage}): {}",
            std::io::Error::from_raw_os_error(errno)
        );
    }
    Ok(SpawnedChild {
        pid,
        stdin: File::from(stdin_w),
        stdout: File::from(stdout_r),
        stderr: File::from(stderr_r),
        delivery: delivery_writers,
    })
}

struct RawChild<'a> {
    program: *const libc::c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
    cwd: *const libc::c_char,
    /// stdin, stdout, stderr, then delivery read ends, in target fd order.
    sources: &'a [i32],
    err: i32,
    identity: Option<(u32, u32)>,
    groups: &'a [libc::gid_t],
    max_fd: i32,
}

impl RawChild<'_> {
    unsafe fn fail(&self, err: i32, stage: u8) -> ! {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        let mut report = [stage, 0, 0, 0, 0];
        report[1..].copy_from_slice(&errno.to_be_bytes());
        libc::write(err, report.as_ptr().cast(), report.len());
        libc::_exit(127)
    }

    unsafe fn exec(&self) -> ! {
        let mut err = self.err;
        if libc::setpgid(0, 0) != 0 {
            self.fail(err, 0);
        }
        // Lift every source above the target range first, so a dup2 never
        // overwrites a source that has not moved yet.
        let base = self.sources.len() as i32;
        let mut moved = [0i32; 3 + MAX_DELIVERY_FDS];
        for (index, &source) in self.sources.iter().enumerate() {
            moved[index] = libc::fcntl(source, libc::F_DUPFD, base);
            if moved[index] < 0 {
                self.fail(err, 1);
            }
        }
        err = libc::fcntl(err, libc::F_DUPFD_CLOEXEC, base);
        if err < 0 {
            self.fail(self.err, 1);
        }
        for (target, &source) in moved[..self.sources.len()].iter().enumerate() {
            if libc::dup2(source, target as i32) < 0 {
                self.fail(err, 1);
            }
        }
        close_from(base, err, self.max_fd);
        let mut empty: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut empty);
        if libc::pthread_sigmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut()) != 0 {
            self.fail(err, 2);
        }
        // Ignored dispositions survive exec. A daemon started from a shell can
        // ignore INT or HUP, which would make forwarded signals vanish.
        for signal in 1..32 {
            if signal != libc::SIGKILL && signal != libc::SIGSTOP {
                libc::signal(signal, libc::SIG_DFL);
            }
        }
        if let Some((uid, gid)) = self.identity {
            if libc::setgroups(self.groups.len() as _, self.groups.as_ptr()) != 0 {
                self.fail(err, 3);
            }
            if libc::setgid(gid as libc::gid_t) != 0 {
                self.fail(err, 4);
            }
            if libc::setuid(uid as libc::uid_t) != 0
                || libc::getuid() != uid
                || libc::geteuid() != uid
                || libc::getegid() != gid
            {
                self.fail(err, 5);
            }
        }
        if libc::chdir(self.cwd) != 0 {
            self.fail(err, 6);
        }
        libc::execve(self.program, self.argv, self.envp);
        self.fail(err, 7)
    }
}

unsafe fn close_from(first: i32, keep: i32, max_fd: i32) {
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

fn max_fd() -> i32 {
    // SAFETY: getrlimit writes one rlimit through a valid pointer.
    let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return 65_536;
    }
    limit.rlim_cur.min(65_536) as i32
}

fn cloexec_pipe() -> Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0i32; 2];
    #[cfg(target_os = "linux")]
    // SAFETY: pipe2 writes two fds into a valid array.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    #[cfg(not(target_os = "linux"))]
    // SAFETY: pipe writes two fds into a valid array; each then gets FD_CLOEXEC.
    let rc = unsafe {
        let rc = libc::pipe(fds.as_mut_ptr());
        if rc == 0 {
            libc::fcntl(fds[0], libc::F_SETFD, libc::FD_CLOEXEC);
            libc::fcntl(fds[1], libc::F_SETFD, libc::FD_CLOEXEC);
        }
        rc
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("pipe failed");
    }
    // SAFETY: both fds are fresh and owned by nothing else.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

fn read_full(mut file: &File, buf: &mut [u8]) -> Result<usize> {
    use std::io::Read;
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(filled)
}
