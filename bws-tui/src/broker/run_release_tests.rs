//! Masked output larger than one wire frame: a held secret prefix that the
//! masker releases at EOF or on a mismatch must reach the client whole, even
//! when the client pauses, and a whole secret must still be masked.

use super::mask::MASK;
use super::protocol::{read_frame, write_frame, Frame, RequestFrame, ResponseFrame};
use super::run_harness::*;
use super::run_prepare::RunConfig;
use super::runspec::{Delivery, ForwardSignal, RunOutcome, StreamKind};
use std::os::unix::net::UnixStream;
use std::time::Duration;

const PREFIX: usize = 800_000;

/// A secret of `PREFIX + 1` letters and digits with no long self-repeats.
fn mixed_secret() -> String {
    let mut state: u32 = 0x2545_f491;
    (0..=PREFIX)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            char::from(b"abcdefghijklmnopqrstuvwxyz0123456789"[(state >> 24) as usize % 36])
        })
        .collect()
}

/// Start a run bound to `BIG` (as fd `BIG_FD`) on a raw socket.
fn raw_big_run(tag: &str, secret: String, script: &str) -> UnixStream {
    let dir = tempdir(tag);
    let vault = backend_with(&[("big", "BIG", secret)]);
    let (socket, _done) = serve_one(&dir, RunConfig::default(), vault);
    let mut stream = UnixStream::connect(socket).unwrap();
    let spec = sh(vec![binding("BIG", "BIG", Delivery::Fd)], script, &dir);
    let spec = super::client_run::to_wire(spec).unwrap();
    write_frame(&mut stream, &Frame::Request(RequestFrame::Run { spec })).unwrap();
    write_frame(&mut stream, &Frame::Request(RequestFrame::RunStdinEof)).unwrap();
    stream
}

fn stdout_to_exit(stream: &mut UnixStream) -> (Vec<u8>, RunOutcome) {
    let mut out = Vec::new();
    loop {
        match read_frame(stream).unwrap() {
            Frame::Response(ResponseFrame::RunOutput {
                stream: StreamKind::Stdout,
                data,
            }) => out.extend(data.0),
            Frame::Response(ResponseFrame::RunExit { outcome }) => return (out, outcome),
            Frame::Response(ResponseFrame::Error { message, .. }) => panic!("{message}"),
            _ => {}
        }
    }
}

#[test]
fn a_large_held_prefix_is_delivered_at_eof() {
    let secret = format!("{}B", "A".repeat(PREFIX));
    let script = format!("head -c {PREFIX} /dev/zero | tr '\\000' A");
    let mut stream = raw_big_run("release-eof", secret, &script);
    let (out, outcome) = stdout_to_exit(&mut stream);
    assert_eq!(outcome, RunOutcome::Exited(0));
    assert_eq!(out.len(), PREFIX, "ordinary bytes lost at EOF");
    assert!(out.iter().all(|&byte| byte == b'A'));
}

#[test]
fn a_large_held_prefix_is_delivered_on_mismatch() {
    let secret = mixed_secret();
    let expected = format!("{}C", &secret[..PREFIX]);
    let script = format!("head -c {PREFIX} <&$BIG_FD; printf C");
    let mut stream = raw_big_run("release-mismatch", secret, &script);
    let (out, outcome) = stdout_to_exit(&mut stream);
    assert_eq!(outcome, RunOutcome::Exited(0));
    assert_eq!(out.len(), expected.len(), "ordinary bytes lost on mismatch");
    assert!(out == expected.as_bytes(), "released bytes changed");
}

#[test]
fn a_whole_large_secret_is_still_masked() {
    let script = "printf x; cat <&$BIG_FD; printf y";
    let mut stream = raw_big_run("release-whole", mixed_secret(), script);
    let (out, outcome) = stdout_to_exit(&mut stream);
    assert_eq!(outcome, RunOutcome::Exited(0));
    assert_eq!(out, [b"x", MASK, b"y"].concat());
}

#[test]
fn a_paused_client_gets_a_large_release_and_signals_still_arrive() {
    let secret = mixed_secret();
    let expected = format!("{}C", &secret[..PREFIX]);
    let script = format!(
        "trap 'exit 45' TERM; head -c {PREFIX} <&$BIG_FD; printf C; while :; do sleep 0.1; done"
    );
    let mut stream = raw_big_run("release-paused", secret, &script);
    std::thread::sleep(Duration::from_secs(2));
    let signal = RequestFrame::RunSignal {
        signal: ForwardSignal::Term,
    };
    write_frame(&mut stream, &Frame::Request(signal)).unwrap();
    let (out, outcome) = stdout_to_exit(&mut stream);
    assert_eq!(outcome, RunOutcome::Exited(45));
    assert_eq!(
        out.len(),
        expected.len(),
        "paused client lost released bytes"
    );
    assert!(out == expected.as_bytes(), "released bytes changed");
}
