//! Token input against real files, a FIFO and a pseudo-terminal.

use super::input::{check_private, hidden_line, private_terminal};
use super::native_host_tests::Driver;
use super::native_io::Owned;
use crate::broker::private_temp_harness::PrivateTemp;
use secrecy::ExposeSecret;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SENTINEL: &str = "SYNTHETIC-hidden-token";

#[test]
fn stdin_must_be_a_pipe_or_an_owner_only_file() {
    let temp = PrivateTemp::new("hush-input-test");
    let private = temp.path().join("private");
    fs::write(&private, SENTINEL).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(check_private(File::open(&private).unwrap().as_fd()).is_ok());
    fs::set_permissions(&private, fs::Permissions::from_mode(0o640)).unwrap();
    assert!(check_private(File::open(&private).unwrap().as_fd()).is_err());
    assert!(check_private(File::open("/dev/null").unwrap().as_fd()).is_err());
    let fifo = temp.path().join("fifo");
    let raw = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: `raw` is a valid NUL-terminated path for the duration of the call.
    assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
    let reader = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&fifo)
        .unwrap();
    assert!(check_private(reader.as_fd()).is_ok());
}

#[test]
fn a_terminal_line_is_read_with_echo_off_and_the_mode_is_restored() {
    let (mut master, slave) = pty();
    // SAFETY: zeroed termios is a valid out buffer for tcgetattr.
    let mut before: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut before) },
        0
    );
    let flags = status_flags(&slave);
    let typed = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        master
            .write_all(format!("{SENTINEL}\n").as_bytes())
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
        unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) };
        let mut echoed = Vec::new();
        let _ = master.read_to_end(&mut echoed);
        (String::from_utf8_lossy(&echoed).into_owned(), master)
    });
    let token = hidden_line(slave.as_fd()).unwrap();
    assert_eq!(token.expose_secret(), SENTINEL);
    let (echoed, _master) = typed.join().unwrap();
    assert!(
        !echoed.contains(SENTINEL),
        "the token was echoed: {echoed:?}"
    );
    let mut after: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut after) }, 0);
    assert_eq!(
        after.c_lflag, before.c_lflag,
        "terminal mode was not restored"
    );
    assert_eq!(status_flags(&slave), flags, "the caller's flags changed");
}

#[test]
fn a_stop_signal_during_the_hidden_prompt_restores_echo_then_ends_the_process() {
    let (_master, slave) = pty();
    assert!(echo(&slave), "a fresh terminal starts with echo on");
    let flags = status_flags(&slave);
    let driver = Driver(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "install::input_tests::hidden_prompt_driver"])
            .arg("--ignored")
            .process_group(0)
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let until = Instant::now() + Duration::from_secs(10);
    while echo(&slave) {
        assert!(Instant::now() < until, "the prompt never turned echo off");
        std::thread::sleep(Duration::from_millis(10));
    }
    // SAFETY: SIGINT to the driver alone, which leads its own group.
    unsafe { libc::kill(driver.0.id() as libc::pid_t, libc::SIGINT) };
    let status = driver.wait_within(Duration::from_secs(10));
    assert_eq!(
        status.map(|status| status.signal()),
        Some(Some(libc::SIGINT))
    );
    assert!(echo(&slave), "the interrupted prompt left echo off");
    assert_eq!(status_flags(&slave), flags, "the caller's flags changed");
}

/// Select readiness does not reserve input, so the prompt reads through its
/// own handle. With nothing queued that read returns at once, never blocks.
#[test]
fn the_prompt_reads_a_private_nonblocking_handle_on_the_same_terminal() {
    let (mut master, slave) = pty();
    let flags = status_flags(&slave);
    let private = private_terminal(slave.as_fd()).unwrap();
    let mut buffer = [0u8; 64];
    let read = |buffer: &mut [u8]| {
        // SAFETY: reads into a live buffer through an owned descriptor.
        let read = unsafe { libc::read(private.as_raw_fd(), buffer.as_mut_ptr().cast(), 64) };
        (read, std::io::Error::last_os_error().kind())
    };
    assert_eq!(read(&mut buffer), (-1, std::io::ErrorKind::WouldBlock));
    assert_eq!(status_flags(&slave), flags, "the caller's flags changed");
    master
        .write_all(format!("{SENTINEL}\n").as_bytes())
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(read(&mut buffer).0, SENTINEL.len() as isize + 1);
    let (pipe, _writer) = std::io::pipe().unwrap();
    assert!(
        private_terminal(pipe.as_fd()).is_err(),
        "a pipe is no terminal"
    );
}

fn status_flags(fd: &OwnedFd) -> libc::c_int {
    // SAFETY: F_GETFL on a descriptor this test owns.
    unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) }
}

#[test]
#[ignore = "run only by a_stop_signal_during_the_hidden_prompt_restores_echo_then_ends_the_process"]
fn hidden_prompt_driver() {
    // Observation arms the native stop handlers before any prompt.
    drop(Owned::take().unwrap());
    let _ = hidden_line(std::io::stdin().as_fd());
    panic!("the hidden prompt returned instead of ending the process");
}

fn echo(terminal: &OwnedFd) -> bool {
    // SAFETY: zeroed termios is a valid out buffer for tcgetattr.
    let mut mode: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(terminal.as_raw_fd(), &mut mode) },
        0
    );
    mode.c_lflag & libc::ECHO != 0
}

#[test]
fn named_inputs_need_owner_only_access_and_anonymous_pipes_stay_accepted() {
    let temp = PrivateTemp::new("hush-input-test");
    let open = fifo(temp.path(), "open", 0o666);
    let reader = nonblocking(&open);
    let mut writer = OpenOptions::new().write(true).open(&open).unwrap();
    writer.write_all(SENTINEL.as_bytes()).unwrap();
    let granted_fifo = fifo(temp.path(), "granted-fifo", 0o600);
    grant(&granted_fifo, 0o600);
    let granted_file = temp.path().join("granted-file");
    fs::write(&granted_file, SENTINEL).unwrap();
    grant(&granted_file, 0o600);
    let private_fifo = fifo(temp.path(), "private-fifo", 0o600);
    let (pipe_reader, _pipe_writer) = std::io::pipe().unwrap();
    let (socket, _peer) = UnixStream::pair().unwrap();
    let accepted = |fd| check_private(fd).is_ok();
    let outcomes = [
        ("world-readable named FIFO", accepted(reader.as_fd())),
        (
            "named FIFO with an ACL grant",
            accepted(nonblocking(&granted_fifo).as_fd()),
        ),
        (
            "0600 file with an ACL grant",
            accepted(nonblocking(&granted_file).as_fd()),
        ),
        (
            "owner-only named FIFO",
            accepted(nonblocking(&private_fifo).as_fd()),
        ),
        ("anonymous pipe", accepted(pipe_reader.as_fd())),
        ("socket pair", accepted(socket.as_fd())),
    ];
    let expected = [false, false, false, true, true, true];
    assert_eq!(
        outcomes,
        std::array::from_fn(|index| (outcomes[index].0, expected[index]))
    );
    // Closing the writer turns the queued bytes into a complete read.
    drop(writer);
    let mut queued = String::new();
    (&reader).read_to_string(&mut queued).unwrap();
    assert_eq!(queued, SENTINEL, "the refused FIFO was read");
}

/// A named FIFO at exactly `mode`, whatever the umask.
fn fifo(dir: &Path, name: &str, mode: u32) -> PathBuf {
    let path = dir.join(name);
    let raw = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: `raw` is a valid NUL-terminated path for the duration of the call.
    assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    path
}

fn nonblocking(path: &Path) -> File {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .unwrap()
}

/// Grant read through an extended ACL, then restore `mode`, so only the ACL
/// differs from a protected path. On Linux this masks the named entry.
pub(super) fn grant(path: &Path, mode: u32) {
    #[cfg(target_os = "linux")]
    let status = Command::new("setfacl")
        .args(["-m", "u:65534:r"])
        .arg(path)
        .status();
    #[cfg(target_os = "macos")]
    let status = Command::new("chmod")
        .args(["+a", "everyone allow read"])
        .arg(path)
        .status();
    assert!(status.unwrap().success(), "{}", path.display());
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn pty() -> (File, OwnedFd) {
    let (mut master, mut slave) = (0, 0);
    // SAFETY: openpty writes two fresh descriptors this test then owns.
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(rc, 0, "openpty failed");
    // SAFETY: both descriptors are fresh and owned here.
    unsafe { (File::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) }
}
