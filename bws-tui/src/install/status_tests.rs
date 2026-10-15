//! Status facts stay independent. A failed query is unknown, never "no".

use super::fake_host::{FakeHost, Input, TOKEN};
use super::model::Fact;
use super::observe::observe;
use super::status::{Answer, Status};

fn status(host: &mut FakeHost) -> Status {
    let layout = host.anchor.layout.clone();
    let binaries = host.binaries();
    Status::from_observed(&observe(&layout, &binaries, host))
}

#[test]
fn an_unreadable_private_tree_is_unknown_not_absent() {
    let mut host = FakeHost::new("status-private");
    let layout = host.anchor.layout.clone();
    super::apply::install_with(&layout, &host.binaries(), &mut host, &mut Input { token: Some(TOKEN), reads: 0 }).unwrap();
    let private = super::layout::private_specs(super::model::Backend::File)[0].path.clone();
    host.anchor.chmod(&private, 0o000);
    let status = status(&mut host);
    host.anchor.chmod(&private, 0o700);
    assert!(matches!(status.credential, Answer::Unknown(_)), "{status:?}");
    assert!(matches!(status.installed, Answer::Unknown(_)), "{status:?}");
    assert!(matches!(status.active, Answer::Yes(_)));
    assert!(status.render().contains("authentication: not checked"));
}

#[test]
fn a_failed_manager_query_is_unknown_not_disabled() {
    let mut host = FakeHost::new("status-manager");
    host.manager_fact = Some(Fact::Unknown("systemctl did not answer"));
    let status = status(&mut host);
    assert_eq!(status.enabled, Answer::Unknown("systemctl did not answer"));
    assert_eq!(status.active, Answer::Unknown("systemctl did not answer"));
    assert!(matches!(status.installed, Answer::No(_)));
}

#[test]
fn reachability_needs_the_managed_pid_on_the_same_connection() {
    let mut host = FakeHost::new("status-peer");
    let socket = host.anchor.path(super::layout::socket_path());
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    host.manager.loaded = true;
    host.manager.pid = Some(std::process::id());
    assert!(matches!(status(&mut host).reachable, Answer::Yes(_)));
    host.manager.pid = Some(1);
    assert_eq!(status(&mut host).reachable, Answer::No("the peer is not the managed service"));
    host.manager.pid = None;
    assert!(matches!(status(&mut host).reachable, Answer::No(_)));
}

#[cfg(target_os = "linux")]
#[test]
fn a_saturated_listener_is_unknown() {
    let mut host = FakeHost::new("status-full");
    let socket = host.anchor.path(super::layout::socket_path());
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    // SAFETY: listen resizes the backlog of an open listening socket.
    assert_eq!(unsafe { libc::listen(std::os::fd::AsRawFd::as_raw_fd(&listener), 0) }, 0);
    let mut queued = Vec::new();
    for _ in 0..4 {
        if let Ok(stream) = std::os::unix::net::UnixStream::connect(&socket) {
            queued.push(stream);
        }
    }
    host.manager.loaded = true;
    host.manager.pid = Some(std::process::id());
    assert!(matches!(status(&mut host).reachable, Answer::Unknown(_)));
}
