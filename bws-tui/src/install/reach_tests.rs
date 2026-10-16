//! Reachability against real Unix sockets in a private directory.

use super::reach::{probe, Reach};
use super::Fact;
use crate::broker::private_temp_harness::PrivateTemp;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{UnixListener, UnixStream};

#[test]
fn a_listener_answers_with_this_process_as_its_peer() {
    let temp = PrivateTemp::new("hush-reach-test");
    let socket = temp.path().join("hush.sock");
    let _listener = UnixListener::bind(&socket).unwrap();
    let Fact::Known(Reach::Peer(peer)) = probe(&socket) else {
        panic!("listener not reached");
    };
    // SAFETY: geteuid takes no arguments and cannot fail.
    assert_eq!(
        (peer.uid, peer.pid),
        (unsafe { libc::geteuid() }, std::process::id() as i32)
    );
}

#[test]
fn no_socket_is_nothing_and_a_regular_file_is_refused() {
    let temp = PrivateTemp::new("hush-reach-test");
    assert_eq!(
        probe(&temp.path().join("absent.sock")),
        Fact::Known(Reach::Nothing)
    );
    std::fs::write(temp.path().join("file.sock"), b"").unwrap();
    assert!(matches!(
        probe(&temp.path().join("file.sock")),
        Fact::Refused(_)
    ));
}

#[test]
fn a_stale_socket_is_nothing_on_linux_and_unknown_on_macos() {
    let temp = PrivateTemp::new("hush-reach-test");
    let socket = temp.path().join("stale.sock");
    drop(UnixListener::bind(&socket).unwrap());
    let outcome = probe(&socket);
    if cfg!(target_os = "macos") {
        assert!(matches!(outcome, Fact::Unknown(_)), "{outcome:?}");
    } else {
        assert_eq!(outcome, Fact::Known(Reach::Nothing));
    }
}

#[test]
fn a_saturated_listener_is_unknown_not_reachable() {
    let temp = PrivateTemp::new("hush-reach-test");
    let socket = temp.path().join("full.sock");
    let listener = backlog_zero(&socket);
    let mut queued = Vec::new();
    while let Ok(stream) = connect_nonblocking(&socket) {
        queued.push(stream);
        assert!(queued.len() < 4096, "the accept queue never filled");
    }
    assert!(matches!(probe(&socket), Fact::Unknown(_)));
    drop((queued, listener));
}

fn backlog_zero(path: &std::path::Path) -> OwnedFd {
    let listener = UnixListener::bind(path).unwrap();
    // SAFETY: listen on a bound socket this test owns. A zero backlog makes the queue fill fast.
    assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
    // SAFETY: into_raw_fd hands over the descriptor this test owns.
    unsafe { OwnedFd::from_raw_fd(std::os::fd::IntoRawFd::into_raw_fd(listener)) }
}

fn connect_nonblocking(path: &std::path::Path) -> std::io::Result<UnixStream> {
    // SAFETY: a fresh socket, configured and connected below, owned by the returned stream.
    unsafe {
        let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0);
        let stream = UnixStream::from_raw_fd(fd);
        libc::fcntl(
            fd,
            libc::F_SETFL,
            libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK,
        );
        let mut addr: libc::sockaddr_un = std::mem::zeroed();
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
        for (slot, byte) in addr.sun_path.iter_mut().zip(path.as_os_str().as_bytes()) {
            *slot = *byte as libc::c_char;
        }
        let len = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
        match libc::connect(fd, (&addr as *const libc::sockaddr_un).cast(), len) {
            0 => Ok(stream),
            _ => Err(std::io::Error::last_os_error()),
        }
    }
}
