//! The daemon reads and writes only its own open of the client's terminal,
//! so a client that changes the flags of the terminal it passed cannot hold
//! a worker.

use super::daemon_harness::{human_daemon, FakeAuth};
use super::human_tests::{key, vault};
use super::operation::Operation;
use super::pty_harness::{Fd, Session, Step};
use super::run_harness::tempdir;
use std::sync::mpsc::channel;
use std::time::Duration;

#[test]
fn a_client_clearing_its_terminal_flags_cannot_hold_a_worker() {
    let dir = tempdir("tty-owner");
    let vault = vault();
    let (socket, daemon) = human_daemon(&dir, vault.clone(), FakeAuth::new(), 1);
    let step = Step {
        op: Operation::Reveal {
            secret: key("TOKEN"),
        },
        fds: vec![Fd::TtyFlipper],
    };
    let mut session = Session::start(&socket, vec![step]);
    session.expect("Password for", 1);
    std::thread::sleep(Duration::from_millis(100));
    session.type_keys("x");
    std::thread::sleep(Duration::from_millis(200));
    session.kill();
    let (done, finished) = channel();
    std::thread::spawn(move || done.send(daemon.join().is_ok()));
    let returned = finished.recv_timeout(Duration::from_secs(3)).is_ok();
    if !returned {
        // Release a worker stuck in a read, so the test process can end.
        session.type_keys("\r");
    }
    assert!(
        returned,
        "the daemon worker stayed in a read on the terminal"
    );
    assert_eq!(vault.fetch_count(), 0);
}
