use super::*;

#[test]
fn reveal_ignores_ordinary_keys_until_enter() {
    let (mut master, slave) = pty();
    let keep = slave.try_clone().unwrap();
    let (socket, _client) = UnixStream::pair().unwrap();
    let (sent, done) = channel();
    let worker = std::thread::spawn(move || {
        let control = RequestControl::until(&socket, Duration::from_secs(5));
        let result = Console::new(slave, &control).show_value("Test", b"fixture");
        sent.send(result).unwrap();
    });
    let mut screen = Vec::new();
    let mut chunk = [0; 256];
    while !screen
        .windows(b"(press Enter to hide)".len())
        .any(|w| w == b"(press Enter to hide)")
    {
        let mut ready = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid poll descriptor owned by this test.
        assert_eq!(
            unsafe { libc::poll(&mut ready, 1, 5_000) },
            1,
            "missing prompt"
        );
        let read = master.read(&mut chunk).unwrap();
        assert!(read > 0);
        screen.extend_from_slice(&chunk[..read]);
    }
    master.write_all(b"ordinary").unwrap();
    let early = done.recv_timeout(Duration::from_millis(150));
    master.write_all(b"\r").unwrap();
    let result = early
        .or_else(|_| done.recv_timeout(Duration::from_secs(5)))
        .unwrap();
    worker.join().unwrap();
    drop(keep);
    assert_eq!(result, Ok(()));
    assert!(early.is_err(), "ordinary keys dismissed the reveal");
}
