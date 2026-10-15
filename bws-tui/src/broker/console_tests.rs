//! A paused terminal, a client that hangs up, or another reader that wins a
//! race must never hang a daemon worker. Deadlines here are test values.

use super::children::Children;
use super::console::{Console, Interrupt, MAX_OSC52_VALUE};
use super::fdpass_tests::pipe;
use super::fields::MAX_VALUE;
use super::input_pump::{read_input, InputStop};
use super::pty_harness::open_pty;
use super::request::{RequestControl, CLEANUP_BUDGET};
use super::spawn_fd::FdCeiling;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(5);

/// A pty slave opened non-blocking by this test alone, like the daemon's own open.
fn pty() -> (File, OwnedFd) {
    let (master, name) = open_pty();
    let slave = File::options()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(name.to_str().unwrap())
        .unwrap();
    (master, slave.into())
}

/// A value-input read with its own reader table, as one daemon would run it.
fn read_value(fd: OwnedFd, socket: &UnixStream, limit: Duration) -> Result<usize, InputStop> {
    let control = RequestControl::until(socket, limit);
    let children = Children::new(2);
    let ceiling = FdCeiling::establish().unwrap();
    read_input(fd, &control, &children, ceiling).map(|value| value.len())
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
        let control = RequestControl::until(&socket, Duration::from_millis(300));
        let mut console = Console::new(slave, &control);
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
        let control = RequestControl::until(&socket, Duration::from_millis(400));
        let mut console = Console::new(slave, &control);
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
    let result = worker(move || {
        let control = RequestControl::until(&socket, BOUND);
        Console::new(slave, &control).say(&big_text())
    });
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
    let late = InputStop::Stopped(Interrupt::TimedOut);
    let mut stolen_rounds = 0;
    for round in 0..40 {
        let (reader, mut writer) = pipe();
        let competitor = reader.try_clone().unwrap();
        let (socket, _client) = UnixStream::pair().unwrap();
        let stolen = worker(move || (&competitor).read(&mut [0u8; 1]).ok());
        std::thread::sleep(Duration::from_millis(20));
        let limit = Duration::from_millis(150);
        let input = worker(move || read_value(reader.into(), &socket, limit));
        std::thread::sleep(Duration::from_millis(20));
        writer.write_all(b"x").unwrap();
        let reply = input.recv_timeout(BOUND).ok();
        assert_eq!(reply, Some(Err(late)), "round {round} hung");
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
        (MAX_VALUE + 1, Err(InputStop::TooLong)),
    ] {
        let (reader, mut writer) = pipe();
        let feeder = std::thread::spawn(move || drop(writer.write_all(&vec![b'v'; len])));
        let (socket, _client) = UnixStream::pair().unwrap();
        let read = read_value(reader.into(), &socket, BOUND);
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
    let control = RequestControl::until(&socket, BOUND);
    let mut console = Console::new(slave, &control);
    let over = console.osc52(&vec![b'a'; MAX_OSC52_VALUE + 1]);
    assert_eq!(over, Err(Interrupt::TooLong));
    assert_eq!(console.osc52(&vec![b'a'; MAX_OSC52_VALUE]), Ok(()));
    let screen = drained.recv_timeout(BOUND).unwrap();
    assert_eq!(screen.len(), expected);
    assert!(screen.starts_with(b"\x1b]52;c;") && screen.ends_with(b"\x07"));
}

fn shows(screen: &[u8], text: &[u8]) -> bool {
    screen.windows(text.len()).any(|window| window == text)
}

/// Answers the `first: ` prompt. Once `pause_at` shows, it suspends output,
/// lets the client leave, and resumes 100 ms later, inside one cleanup
/// budget. Returns everything the terminal showed until output went quiet.
/// macOS discards unread output when the last non-blocking slave closes, so
/// `flow` stays open until the screen is read.
fn leave_while_paused(
    mut master: File,
    flow: OwnedFd,
    client: UnixStream,
    pause_at: &'static [u8],
) -> Receiver<Vec<u8>> {
    worker(move || {
        let (mut screen, mut chunk, mut answered) = (Vec::new(), [0u8; 8192], false);
        while !shows(&screen, pause_at) {
            let read = master.read(&mut chunk).unwrap();
            screen.extend_from_slice(&chunk[..read]);
            if !answered && shows(&screen, b"first: ") {
                master.write_all(b"x\r").unwrap();
                answered = true;
            }
        }
        // SAFETY: tcflow on a terminal this test owns.
        assert_eq!(unsafe { libc::tcflow(flow.as_raw_fd(), libc::TCOOFF) }, 0);
        drop(client);
        std::thread::sleep(Duration::from_millis(100));
        // SAFETY: tcflow on a terminal this test owns.
        assert_eq!(unsafe { libc::tcflow(flow.as_raw_fd(), libc::TCOON) }, 0);
        let mut ready = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd entry.
        while unsafe { libc::poll(&mut ready, 1, 300) } == 1 {
            match master.read(&mut chunk) {
                Ok(read @ 1..) => screen.extend_from_slice(&chunk[..read]),
                _ => break,
            }
        }
        drop(flow);
        screen
    })
}

/// A request that answered a prompt, read a value, and then worked past one
/// cleanup budget still gets a full budget once its teardown starts.
fn after_a_slow_step<T>(
    pause_at: &'static [u8],
    end: impl FnOnce(&mut Console) -> T,
) -> (T, Vec<u8>) {
    let (master, slave) = pty();
    let flow = slave.try_clone().unwrap();
    let (socket, client) = UnixStream::pair().unwrap();
    let screen = leave_while_paused(master, flow, client, pause_at);
    let control = RequestControl::until(&socket, BOUND);
    let mut console = Console::new(slave, &control);
    console.read_hidden("first: ", 16).unwrap();
    let (reader, mut writer) = pipe();
    writer.write_all(b"FAKE-value-0001").unwrap();
    drop(writer);
    let (children, ceiling) = (Children::new(1), FdCeiling::establish().unwrap());
    assert!(read_input(reader.into(), &control, &children, ceiling).is_ok());
    std::thread::sleep(CLEANUP_BUDGET + Duration::from_millis(100));
    let ended = end(&mut console);
    drop(console);
    (ended, screen.recv_timeout(BOUND).unwrap())
}

#[test]
fn a_reveal_cut_short_after_a_slow_step_is_still_cleared() {
    let (shown, screen) = after_a_slow_step(b"(press Enter to hide)", |console| {
        console.show_value("FAKE title", b"FAKE-value-0002")
    });
    assert_eq!(shown, Err(Interrupt::Disconnected));
    assert!(
        shows(&screen, b"\x1b[2J\x1b[?1049l"),
        "the reveal was left on screen"
    );
}

#[test]
fn a_copy_cut_short_after_a_slow_step_is_still_cancelled() {
    let (copied, screen) = after_a_slow_step(b"\x1b]52;c;", |console| {
        console.osc52(&vec![b'a'; MAX_OSC52_VALUE])
    });
    assert_eq!(copied, Err(Interrupt::Disconnected));
    let start = screen.windows(7).position(|w| w == b"\x1b]52;c;").unwrap();
    assert!(
        screen[start..].contains(&0x18),
        "the cut-short copy was never cancelled"
    );
}
