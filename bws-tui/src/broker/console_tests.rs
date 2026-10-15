//! A paused terminal, a client that hangs up, or another reader that wins a
//! race must never hang a daemon worker. Deadlines here are test values.

use super::console::{Console, Interrupt, MAX_OSC52_VALUE};
use super::fdpass_tests::pipe;
use super::mutate::{invalid, read_input, MAX_VALUE};
use super::operation::Refusal;
use super::pty_harness::open_pty;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(5);

fn pty() -> (File, OwnedFd) {
    let (master, name) = open_pty();
    let slave = File::options()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(name.to_str().unwrap())
        .unwrap();
    (master, slave.into())
}

/// Output suspended, as after Ctrl-S.
fn paused_pty() -> (File, OwnedFd) {
    let (master, slave) = pty();
    // SAFETY: tcflow suspends output on a terminal this test owns.
    assert_eq!(unsafe { libc::tcflow(slave.as_raw_fd(), libc::TCOOFF) }, 0);
    (master, slave)
}

fn worker<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (sender, receiver) = channel();
    std::thread::spawn(move || {
        let _ = sender.send(job());
    });
    receiver
}

fn big_text() -> String {
    "x".repeat(256 * 1024)
}

#[test]
fn a_paused_terminal_times_out_instead_of_hanging() {
    let (_master, slave) = paused_pty();
    let (socket, _client) = UnixStream::pair().unwrap();
    let started = Instant::now();
    let result = worker(move || {
        let console = Console::new(slave, &socket).unwrap();
        let mut console = console.with_timeout(Duration::from_millis(300));
        console.say(&big_text())
    });
    assert_eq!(
        result.recv_timeout(BOUND).ok(),
        Some(Err(Interrupt::TimedOut))
    );
    assert!(started.elapsed() < BOUND, "took {:?}", started.elapsed());
}

/// A prompt does not restart the clock. The second wait ends at the deadline
/// the request started with, not a fresh one.
#[test]
fn every_wait_in_a_request_shares_one_deadline() {
    let (_master, slave) = pty();
    let (socket, _client) = UnixStream::pair().unwrap();
    let started = Instant::now();
    let result = worker(move || {
        let console = Console::new(slave, &socket).unwrap();
        let mut console = console.with_timeout(Duration::from_millis(400));
        console.say("first: ")?;
        std::thread::sleep(Duration::from_millis(300));
        console.read_hidden("second: ", 16).map(drop)
    });
    let reply = result.recv_timeout(BOUND).ok();
    assert_eq!(reply, Some(Err(Interrupt::TimedOut)));
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_millis(600), "took {elapsed:?}");
}

#[test]
fn a_client_hangup_cancels_a_blocked_terminal_write() {
    let (_master, slave) = paused_pty();
    let (socket, client) = UnixStream::pair().unwrap();
    let result = worker(move || Console::new(slave, &socket).unwrap().say(&big_text()));
    std::thread::sleep(Duration::from_millis(200));
    drop(client);
    let reply = result.recv_timeout(BOUND).ok();
    assert_eq!(reply, Some(Err(Interrupt::Disconnected)));
}

/// Another reader is already blocked on the same pipe, so it often takes the
/// byte that woke the daemon. The daemon must go back to waiting, not block.
/// Each round's input stays open, so the only correct reply is the deadline.
#[test]
fn a_reader_that_wins_the_race_cannot_hang_value_input() {
    let late = invalid("the value input did not finish in time");
    let mut stolen_rounds = 0;
    for round in 0..40 {
        let (reader, mut writer) = pipe();
        let competitor = reader.try_clone().unwrap();
        let (socket, _client) = UnixStream::pair().unwrap();
        let stolen = worker(move || (&competitor).read(&mut [0u8; 1]).ok());
        std::thread::sleep(Duration::from_millis(20));
        let deadline = Instant::now() + Duration::from_millis(150);
        let input = worker(move || {
            read_input(reader.into(), &socket, deadline).map(|value| value.expose_bytes().len())
        });
        std::thread::sleep(Duration::from_millis(20));
        writer.write_all(b"x").unwrap();
        let reply = input.recv_timeout(BOUND).ok();
        assert_eq!(reply, Some(Err(late.clone())), "round {round} hung");
        drop(writer);
        if stolen.recv_timeout(BOUND).ok().flatten() == Some(1) {
            stolen_rounds += 1;
        }
    }
    eprintln!("the competing reader won {stolen_rounds} of 40 rounds");
}

#[test]
fn value_input_stops_at_64_kib() {
    for (len, expected) in [
        (MAX_VALUE, Ok(MAX_VALUE)),
        (
            MAX_VALUE + 1,
            Err(invalid("the value is larger than 64 KiB")),
        ),
    ] {
        let (reader, mut writer) = pipe();
        let feeder = std::thread::spawn(move || drop(writer.write_all(&vec![b'v'; len])));
        let (socket, _client) = UnixStream::pair().unwrap();
        let deadline = Instant::now() + BOUND;
        let read: Result<usize, Refusal> =
            read_input(reader.into(), &socket, deadline).map(|value| value.expose_bytes().len());
        assert_eq!(read, expected, "{len} bytes");
        feeder.join().unwrap();
    }
}

#[test]
fn copy_writes_at_most_128_kib() {
    let (mut master, slave) = pty();
    let (socket, _client) = UnixStream::pair().unwrap();
    let expected = "\x1b]52;c;".len() + MAX_OSC52_VALUE.div_ceil(3) * 4 + 1;
    let drained = worker(move || {
        let mut screen = Vec::new();
        let mut buf = [0u8; 8192];
        while screen.len() < expected {
            match master.read(&mut buf) {
                Ok(read @ 1..) => screen.extend_from_slice(&buf[..read]),
                _ => break,
            }
        }
        screen
    });
    let mut console = Console::new(slave, &socket).unwrap();
    let over = console.osc52(&vec![b'a'; MAX_OSC52_VALUE + 1]);
    assert_eq!(over, Err(Interrupt::TooLong));
    assert_eq!(console.osc52(&vec![b'a'; MAX_OSC52_VALUE]), Ok(()));
    let screen = drained.recv_timeout(BOUND).unwrap();
    assert_eq!(screen.len(), expected);
    assert!(screen.starts_with(b"\x1b]52;c;") && screen.ends_with(b"\x07"));
}
