//! Starting `bws`. Linux keeps std's spawn: the daemon receives passed
//! descriptors with `MSG_CMSG_CLOEXEC`, so none can reach the child. macOS has
//! no such flag and marks them close-on-exec one call after receipt, so it
//! spawns with `POSIX_SPAWN_CLOEXEC_DEFAULT`: the kernel closes every
//! descriptor not named below. The token goes into wiping buffers there.

use super::bws_child::BwsCall;
use std::os::fd::OwnedFd;

/// The child's pid and its stdout read end. The pid is the caller's to reap.
type Spawned = (libc::pid_t, Option<OwnedFd>);

#[cfg(target_os = "linux")]
pub(super) fn spawn(call: &BwsCall<'_>) -> std::io::Result<Spawned> {
    use secrecy::ExposeSecret;
    use std::process::{Command, Stdio};
    let mut child = Command::new(call.path)
        .env_clear()
        .env("PATH", super::bws_child::BWS_PATH)
        .env("BWS_ACCESS_TOKEN", call.token.expose_secret())
        .args(call.args)
        .args(["-o", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    // std's `Child` never waits on drop, so the pid passes to the child table.
    Ok((
        child.id() as libc::pid_t,
        child.stdout.take().map(OwnedFd::from),
    ))
}

#[cfg(target_os = "macos")]
pub(super) fn spawn(call: &BwsCall<'_>) -> std::io::Result<Spawned> {
    use secrecy::ExposeSecret;
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use zeroize::Zeroizing;

    let program = CString::new(call.path.as_os_str().as_bytes())?;
    let mut args = vec![program.clone()];
    for arg in call.args.iter().chain(&["-o", "json"]) {
        args.push(CString::new(*arg)?);
    }
    let argv: Vec<*mut libc::c_char> = args
        .iter()
        .map(|arg| arg.as_ptr().cast_mut())
        .chain(std::iter::once(std::ptr::null_mut()))
        .collect();
    let entry = |text: &[&[u8]]| {
        let mut bytes = Zeroizing::new(Vec::with_capacity(
            text.iter().map(|t| t.len()).sum::<usize>() + 1,
        ));
        text.iter().for_each(|part| bytes.extend_from_slice(part));
        bytes.push(0);
        bytes
    };
    let token = call.token.expose_secret().as_bytes();
    let env = [
        entry(&[b"PATH=", super::bws_child::BWS_PATH.as_bytes()]),
        entry(&[b"BWS_ACCESS_TOKEN=", token]),
    ];
    let envp: Vec<*mut libc::c_char> = env
        .iter()
        .map(|entry| entry.as_ptr().cast_mut().cast())
        .chain(std::iter::once(std::ptr::null_mut()))
        .collect();
    let mut fds = [0i32; 2];
    // SAFETY: pipe writes two new descriptors into a valid array.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: both descriptors are new and owned only here.
    let (read, write) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    // SAFETY: marks a descriptor owned here close-on-exec in this daemon.
    if unsafe { libc::fcntl(read.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let actions = Actions::new(write.as_raw_fd())?;
    let attributes = Attributes::new()?;
    let mut pid = 0;
    // SAFETY: every pointer is valid for the call; argv and envp end in null.
    let failed = unsafe {
        libc::posix_spawn(
            &mut pid,
            program.as_ptr(),
            &actions.0,
            &attributes.0,
            argv.as_ptr(),
            envp.as_ptr(),
        )
    };
    if failed != 0 {
        return Err(std::io::Error::from_raw_os_error(failed));
    }
    drop(write);
    Ok((pid, Some(read)))
}

/// stdin and stderr from /dev/null, stdout to the daemon's pipe. Nothing else.
#[cfg(target_os = "macos")]
struct Actions(libc::posix_spawn_file_actions_t);

#[cfg(target_os = "macos")]
impl Actions {
    fn new(stdout: libc::c_int) -> std::io::Result<Self> {
        let mut actions = std::mem::MaybeUninit::uninit();
        // SAFETY: init writes the actions object, destroyed by Drop.
        check(unsafe { libc::posix_spawn_file_actions_init(actions.as_mut_ptr()) })?;
        // SAFETY: initialised by the call above.
        let mut this = Self(unsafe { actions.assume_init() });
        let null = c"/dev/null".as_ptr();
        // SAFETY: each call adds one action to the initialised object.
        unsafe {
            check(libc::posix_spawn_file_actions_addopen(
                &mut this.0,
                0,
                null,
                libc::O_RDONLY,
                0,
            ))?;
            check(libc::posix_spawn_file_actions_adddup2(
                &mut this.0,
                stdout,
                1,
            ))?;
            check(libc::posix_spawn_file_actions_addopen(
                &mut this.0,
                2,
                null,
                libc::O_WRONLY,
                0,
            ))?;
        }
        Ok(this)
    }
}

#[cfg(target_os = "macos")]
impl Drop for Actions {
    fn drop(&mut self) {
        // SAFETY: destroys the object initialised in `new`.
        unsafe { libc::posix_spawn_file_actions_destroy(&mut self.0) };
    }
}

/// Close every descriptor by default, unblock every signal, and reset every
/// catchable signal to its default action, as an exec from a shell would.
#[cfg(target_os = "macos")]
struct Attributes(libc::posix_spawnattr_t);

#[cfg(target_os = "macos")]
impl Attributes {
    fn new() -> std::io::Result<Self> {
        let mut attributes = std::mem::MaybeUninit::uninit();
        // SAFETY: init writes the attributes object, destroyed by Drop.
        check(unsafe { libc::posix_spawnattr_init(attributes.as_mut_ptr()) })?;
        // SAFETY: initialised by the call above.
        let mut this = Self(unsafe { attributes.assume_init() });
        let flags = libc::POSIX_SPAWN_CLOEXEC_DEFAULT
            | libc::POSIX_SPAWN_SETSIGMASK
            | libc::POSIX_SPAWN_SETSIGDEF;
        // SAFETY: zeroed sigset_t values are valid buffers for the set calls.
        unsafe {
            let (mut none, mut all) = (std::mem::zeroed(), std::mem::zeroed());
            libc::sigemptyset(&mut none);
            libc::sigfillset(&mut all);
            libc::sigdelset(&mut all, libc::SIGKILL);
            libc::sigdelset(&mut all, libc::SIGSTOP);
            check(libc::posix_spawnattr_setflags(
                &mut this.0,
                flags as libc::c_short,
            ))?;
            check(libc::posix_spawnattr_setsigmask(&mut this.0, &none))?;
            check(libc::posix_spawnattr_setsigdefault(&mut this.0, &all))?;
        }
        Ok(this)
    }
}

#[cfg(target_os = "macos")]
impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: destroys the object initialised in `new`.
        unsafe { libc::posix_spawnattr_destroy(&mut self.0) };
    }
}

#[cfg(target_os = "macos")]
fn check(code: libc::c_int) -> std::io::Result<()> {
    match code {
        0 => Ok(()),
        code => Err(std::io::Error::from_raw_os_error(code)),
    }
}
