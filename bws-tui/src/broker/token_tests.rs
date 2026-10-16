//! Runtime credential loader checks on real root-owned files.
//! systemd hands a LoadCredential or encrypted credential to the daemon as a root-owned 0400 file.
//! These need root to own the fixture, so they are ignored in ordinary runs.
//! Run them as root with `--ignored --exact`, see .audit for the recorded commands.

use super::private_temp_harness::PrivateTemp;
use super::token;
use secrecy::ExposeSecret;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

const SYNTHETIC: &str = "SYNTHETIC-runtime-credential-not-a-token";

struct Fixture(PrivateTemp);

impl Fixture {
    fn new() -> Self {
        // SAFETY: geteuid takes no arguments and cannot fail.
        let euid = unsafe { libc::geteuid() };
        assert_eq!(euid, 0, "root must own the fixture, run this test as root");
        Self(PrivateTemp::new("hush-runtime-credential-test"))
    }

    fn token(&self, name: &str, mode: u32, owner: u32) -> PathBuf {
        let path = self.0.path().join(name);
        fs::write(&path, SYNTHETIC).unwrap();
        std::os::unix::fs::chown(&path, Some(owner), None).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!((metadata.mode() & 0o7777, metadata.uid()), (mode, owner));
        path
    }
}

fn loads(path: &Path) -> Result<String, String> {
    token::load_checked(path, false)
        .map(|secret| secret.expose_secret().to_owned())
        .map_err(|error| error.to_string())
}

#[test]
#[ignore = "needs root to own the fixture"]
fn linux_root_owned_runtime_credential_loads_at_0400_and_0600() {
    let fixture = Fixture::new();
    for mode in [0o400, 0o600] {
        let path = fixture.token(&format!("credential-{mode:o}"), mode, 0);
        assert_eq!(loads(&path).as_deref(), Ok(SYNTHETIC), "mode {mode:o}");
    }
}

#[test]
#[ignore = "needs root to own the fixture"]
fn linux_loader_still_refuses_readable_writable_or_foreign_credentials() {
    let fixture = Fixture::new();
    for (mode, owner) in [
        (0o440, 0),
        (0o444, 0),
        (0o640, 0),
        (0o200, 0),
        (0o4600, 0),
        (0o2400, 0),
        (0o400, 65534),
    ] {
        let path = fixture.token(&format!("credential-{mode:o}-{owner}"), mode, owner);
        assert!(loads(&path).is_err(), "mode {mode:o} owner {owner} loaded");
    }
}
