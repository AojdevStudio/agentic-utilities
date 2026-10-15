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

#[test]
fn a_stdin_send_failure_preserves_an_unread_daemon_reply() {
    let (mut server, mut client) = UnixStream::pair().unwrap();
    write_frame(
        &mut server,
        &Frame::Response(ResponseFrame::Error {
            code: "run_refused".into(),
            message: "the daemon refused this run".into(),
        }),
    )
    .unwrap();
    let completion = Arc::new(Completion::default());
    let credit = Arc::new(Credit::default());
    let worker = StdinWorker {
        writer: Arc::new(Mutex::new(client.try_clone().unwrap())),
        credit: Arc::clone(&credit),
        completion: Arc::clone(&completion),
        abort: client.try_clone().unwrap(),
    };
    worker.failed(InputFailure::Send(
        std::io::Error::from_raw_os_error(libc::EPIPE).into(),
    ));
    let reply = relay(&mut client, &mut Capture::default(), &credit);
    let error = completion.settle(reply).unwrap_err();
    assert!(
        error.to_string().contains("the daemon refused this run"),
        "queued daemon refusal was lost: {error:#}"
    );
}
