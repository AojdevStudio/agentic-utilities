//! Run boundaries owned by the daemon: the direct child dies with the run even
//! after it leaves its group, PATH search runs under the run identity, and the
//! supplementary groups fit the kernel limit with the primary group first.

use super::identity::{resolve_identity, Account, AccountDb};
use super::peercred::PeerCredentials;
use super::protocol::{read_frame, write_frame, Frame, RequestFrame, ResponseFrame};
use super::run_harness::*;
use super::run_prepare::RunConfig;
use super::runspec::{Delivery, EnvName, RunAs, RunOutcome, RunSpec};
use std::collections::HashSet;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[test]
fn a_disconnect_kills_a_direct_child_that_left_its_group() {
    let dir = tempdir("escaped");
    let (socket, done) = serve_one(&dir, RunConfig::default(), backend());
    let mut stream = UnixStream::connect(socket).unwrap();
    let code = "import os,time; os.setpgid(0, os.getpgid(os.getppid())); \
                print(os.getpid(), flush=True); time.sleep(30)";
    let spec = RunSpec {
        program: "/usr/bin/python3".into(),
        args: vec!["-c".into(), code.into()],
        ..sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], "", &dir)
    };
    let spec = super::client_run::to_wire(spec).unwrap();
    write_frame(&mut stream, &Frame::Request(RequestFrame::Run { spec })).unwrap();
    let pid: i32 = loop {
        match read_frame(&mut stream).unwrap() {
            Frame::Response(ResponseFrame::RunOutput { data, .. }) => {
                break String::from_utf8(data.0).unwrap().trim().parse().unwrap();
            }
            Frame::Response(ResponseFrame::RunExit { outcome }) => panic!("{outcome:?}"),
            _ => {}
        }
    };
    stream.shutdown(std::net::Shutdown::Both).unwrap();
    let finished = done.recv_timeout(Duration::from_secs(5)).is_ok();
    // SAFETY: signal 0 only checks that the process exists.
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    if alive {
        // SAFETY: SIGKILL to this test's own escaped child.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    assert!(
        finished && !alive,
        "server_finished={finished} child_alive={alive}"
    );
}

#[test]
fn path_search_skips_an_entry_the_run_identity_cannot_execute() {
    // SAFETY: geteuid takes no arguments and cannot fail.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("skipped: root executes any file with an execute bit");
        return;
    }
    let dir = tempdir("path-identity");
    for (entry, mode) in [("first", 0o001), ("second", 0o755)] {
        std::fs::create_dir_all(dir.join(entry)).unwrap();
        let tool = dir.join(entry).join("tool");
        std::fs::write(&tool, format!("#!/bin/sh\necho from-{entry}\n")).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    let mut spec = sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], "", &dir);
    spec.program = "tool".into();
    spec.args.clear();
    spec.env_passthrough = vec![(
        EnvName::try_from("PATH".to_string()).unwrap(),
        "first:second".into(),
    )];
    let mut sink = Capture::default();
    let outcome = daemon(&dir, RunConfig::default()).run(spec, &mut sink);
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "from-second\n");
}

/// One account whose database primary group differs from the peer's gid.
struct Member(Vec<u32>);

impl AccountDb for Member {
    fn by_name(&self, _name: &str) -> anyhow::Result<Option<Account>> {
        Ok(None)
    }
    fn by_uid(&self, uid: u32) -> anyhow::Result<Option<Account>> {
        Ok(Some(Account {
            name: "member".into(),
            uid,
            gid: 30,
        }))
    }
    fn groups(&self, _account: &Account) -> anyhow::Result<Vec<u32>> {
        Ok(self.0.clone())
    }
}

#[test]
fn requester_groups_put_the_peer_gid_first_and_fit_the_kernel_limit() {
    // SAFETY: sysconf reads one configuration value.
    let limit = unsafe { libc::sysconf(libc::_SC_NGROUPS_MAX) } as usize;
    let mut discovered: Vec<u32> = (100..100 + limit as u32 + 40).collect();
    discovered.insert(0, 30);
    discovered.insert(3, 101);
    discovered.insert(5, 20);
    let peer = PeerCredentials {
        uid: 501,
        gid: 20,
        pid: 1,
    };
    let identity = resolve_identity(peer, RunAs::Requester, &Member(discovered.clone())).unwrap();
    let mut seen = HashSet::from([20]);
    let mut expected = vec![20];
    expected.extend(discovered.into_iter().filter(|gid| seen.insert(*gid)));
    expected.truncate(limit);
    assert_eq!((identity.uid, identity.gid), (501, 20));
    assert_eq!(identity.groups.len(), expected.len());
    assert!(
        identity.groups == expected,
        "groups are not primary-first, distinct, and capped"
    );
}
