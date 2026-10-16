//! An inherited signal disposition is tested in a separate process.
use super::native_host::run_within;
use super::native_host_tests::Driver;
use super::native_io::BUDGET;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::Duration;

#[test]
fn a_helper_keeps_its_status_when_sigchld_is_inherited_ignored() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "install::native_signal_tests::ignored_sigchld_driver",
            "--ignored",
            "--nocapture",
        ])
        .process_group(0);
    // SAFETY: only this child changes SIGCHLD before exec. signal is
    // async-signal-safe, and an ignored disposition survives exec.
    unsafe {
        command.pre_exec(|| {
            if libc::signal(libc::SIGCHLD, libc::SIG_IGN) == libc::SIG_ERR {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let status = Driver(command.spawn().unwrap())
        .wait_within(Duration::from_secs(10))
        .expect("the isolated helper driver did not exit");
    assert!(
        status.success(),
        "the inherited SIGCHLD driver failed: {status}"
    );
}

#[test]
#[ignore = "run only by a_helper_keeps_its_status_when_sigchld_is_inherited_ignored"]
fn ignored_sigchld_driver() {
    let result = run_within("/usr/bin/printf", &["valid".into()], None, BUDGET).unwrap();
    assert_eq!(result, (Some(0), "valid".to_owned()));
}
