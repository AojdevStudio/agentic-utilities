use super::*;
use crate::broker::run_harness::Capture;

#[test]
fn a_daemon_refusal_outranks_a_late_stdin_send_failure() {
    let (mut server, mut client) = UnixStream::pair().unwrap();
    write_frame(
        &mut server,
        &Frame::Response(ResponseFrame::Error {
            code: "run_refused".into(),
            message: "failed to start the requested child".into(),
        }),
    )
    .unwrap();
    let completion = Completion::default();
    completion.fail(InputFailure::Send(
        std::io::Error::from_raw_os_error(libc::EPIPE).into(),
    ));
    let reply = relay(&mut client, &mut Capture::default(), &Credit::default());
    let error = completion.settle(reply).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("failed to start the requested child"),
        "daemon refusal was hidden: {error:#}"
    );
}
