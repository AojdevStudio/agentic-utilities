//! Run decisions a same-user test daemon cannot exercise end to end: root
//! refusal, the hushrun account, and the privilege switch. These run as unit
//! tests because the real paths need a root daemon.

use super::identity::{resolve_identity, Account, AccountDb, Identity};
use super::peercred::PeerCredentials;
use super::policy::Policy;
use super::protocol::{write_frame, Frame, RequestFrame, ResponseFrame, WireBytes};
use super::run_harness::{binding, daemon, sh, tempdir, Capture, ENV_VALUE};
use super::run_prepare::{child_env, decide_switch, RunConfig};
use super::runspec::{Delivery, ForwardSignal, RunAs, RunOutcome, StreamKind};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use zeroize::Zeroizing;

struct FakeAccounts {
    hushrun: Option<Account>,
}

impl AccountDb for FakeAccounts {
    fn by_name(&self, name: &str) -> anyhow::Result<Option<Account>> {
        Ok(self.hushrun.clone().filter(|account| account.name == name))
    }
    fn by_uid(&self, _uid: u32) -> anyhow::Result<Option<Account>> {
        Ok(None)
    }
    fn groups(&self, account: &Account) -> anyhow::Result<Vec<u32>> {
        Ok(vec![account.gid])
    }
}

fn peer(uid: u32) -> PeerCredentials {
    PeerCredentials {
        uid,
        gid: 20,
        pid: 1,
    }
}

fn hushrun(uid: u32) -> FakeAccounts {
    FakeAccounts {
        hushrun: Some(Account {
            name: "hushrun".into(),
            uid,
            gid: 600,
        }),
    }
}

#[test]
fn root_is_never_a_run_identity() {
    let none = FakeAccounts { hushrun: None };
    let error = resolve_identity(peer(0), RunAs::Requester, &none).unwrap_err();
    assert!(error.to_string().contains("uid 0"));
    assert!(resolve_identity(peer(501), RunAs::HushRun, &hushrun(0)).is_err());
}

#[test]
fn requester_runs_as_the_peer_and_hushrun_as_its_account() {
    let none = FakeAccounts { hushrun: None };
    let requester = resolve_identity(peer(501), RunAs::Requester, &none).unwrap();
    assert_eq!((requester.uid, requester.gid), (501, 20));
    let dedicated = resolve_identity(peer(501), RunAs::HushRun, &hushrun(480)).unwrap();
    assert_eq!(
        (dedicated.uid, dedicated.gid, dedicated.groups),
        (480, 600, vec![600])
    );
}

#[test]
fn missing_hushrun_account_fails_closed() {
    let none = FakeAccounts { hushrun: None };
    let error = resolve_identity(peer(501), RunAs::HushRun, &none).unwrap_err();
    assert!(error.to_string().contains("'hushrun' account"), "{error}");
}

#[test]
fn only_a_root_daemon_switches_identity() {
    let target = Identity {
        uid: 501,
        gid: 20,
        groups: vec![20],
    };
    assert_eq!(
        decide_switch(0, target.clone()).unwrap(),
        Some(target.clone())
    );
    assert_eq!(decide_switch(501, target.clone()).unwrap(), None);
    assert!(decide_switch(502, target).is_err());
}

#[test]
fn policy_forces_hushrun_for_named_keys_and_is_honored_by_the_daemon() {
    let policy = Policy {
        require_hushrun: vec!["TOKEN".into()],
    };
    assert_eq!(policy.run_as_for(["OTHER", "TOKEN"]), RunAs::HushRun);
    assert_eq!(policy.run_as_for(["OTHER"]), RunAs::Requester);

    let dir = tempdir("policy-run");
    let run = RunConfig {
        policy,
        accounts: Arc::new(FakeAccounts { hushrun: None }),
    };
    let mut client = daemon(&dir, run);
    let spec = sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], "true", &dir);
    let error = client.run(spec, &mut Capture::default()).unwrap_err();
    assert!(error.to_string().contains("hushrun"), "{error}");
    let audit = fs::read_to_string(dir.join("audit.log")).unwrap();
    assert!(audit.contains("run_as=hushrun"), "{audit}");
}

#[test]
fn policy_file_must_not_be_writable_by_others() {
    let dir = tempdir("policy-file");
    let path = dir.join("hushd.toml");
    assert_eq!(Policy::load(&path, true).unwrap(), Policy::default());
    fs::write(&path, "require_hushrun = [\"PROD_DB\"]\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        Policy::load(&path, true).unwrap().require_hushrun,
        vec!["PROD_DB"]
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(Policy::load(&path, true).is_err());
}

#[test]
fn child_env_keeps_the_allowlist_and_strips_forbidden_names() {
    let passthrough: Vec<(&str, &[u8])> = vec![
        ("PATH", b"/usr/bin"),
        ("LC_CTYPE", b"C"),
        ("SHELL", b"/bin/zsh"),
        ("BWS_ACCESS_TOKEN", b"FAKE"),
        ("LD_PRELOAD", b"/tmp/x.so"),
        ("DYLD_INSERT_LIBRARIES", b"/tmp/x.dylib"),
    ];
    let bound = vec![
        ("TOKEN".to_string(), Zeroizing::new(b"FAKE-bound".to_vec())),
        (
            "LD_LIBRARY_PATH".to_string(),
            Zeroizing::new(b"/tmp".to_vec()),
        ),
    ];
    let env: Vec<String> = child_env(&passthrough, bound)
        .iter()
        .map(|entry| String::from_utf8_lossy(&entry[..entry.len() - 1]).into_owned())
        .collect();
    assert_eq!(env, ["LC_CTYPE=C", "PATH=/usr/bin", "TOKEN=FAKE-bound"]);
}

#[test]
fn run_frames_carry_no_value_field() {
    let frames = [
        RequestFrame::RunStdin {
            data: WireBytes(b"input".to_vec()),
        }
        .into(),
        RequestFrame::RunSignal {
            signal: ForwardSignal::Int,
        }
        .into(),
        Frame::Response(ResponseFrame::RunOutput {
            stream: StreamKind::Stdout,
            data: WireBytes(b"<hidden by hush>".to_vec()),
        }),
        Frame::Response(ResponseFrame::RunNotice {
            message: "hush: TINY is short".into(),
        }),
        Frame::Response(ResponseFrame::RunExit {
            outcome: RunOutcome::Signaled(15),
        }),
    ];
    for frame in frames {
        let mut buf = Vec::new();
        write_frame(&mut buf, &frame).unwrap();
        let text = String::from_utf8_lossy(&buf);
        assert!(
            !text.contains("\"value\"") && !text.contains(ENV_VALUE),
            "{text}"
        );
    }
}

impl From<RequestFrame> for Frame {
    fn from(frame: RequestFrame) -> Self {
        Frame::Request(frame)
    }
}
