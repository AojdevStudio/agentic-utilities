//! Token input against real files, a FIFO and a pseudo-terminal.

use super::input::{check_private, hidden_line};
use crate::broker::private_temp_harness::PrivateTemp;
use secrecy::ExposeSecret;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

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
