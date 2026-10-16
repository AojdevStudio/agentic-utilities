//! A real terminal fixture for human operation tests. A helper process runs
//! this test binary in a new session whose controlling terminal is a pty
//! slave, so the daemon checks genuine kernel terminal evidence. The test
//! plays the human on the pty master.

use super::client;
use super::fdpass_tests::{pipe, writers_closed};
use super::operation::{Operation, Outcome, Refusal};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HELPER_ENV: &str = "HUSH_PTY_HELPER_SPEC";

/// How the helper fills one descriptor slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum Fd {
    Tty,
    /// The terminal, while a helper thread keeps clearing O_NONBLOCK on the
    /// client's open of it, as a hostile or careless client could.
    TtyFlipper,
    Pipe(String),
    PipeWriter,
    DevNull,
    ForeignPty,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Step {
    pub op: Operation,
    pub fds: Vec<Fd>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct StepResult {
    pub reply: Result<Outcome, Refusal>,
    /// For `PipeWriter`: whether the daemon closed its copy of the writer.
    pub writer_closed: Option<bool>,
}

/// A new pty pair whose descriptors close on exec. Returns the master and slave path.
pub(super) fn open_pty() -> (File, CString) {
    // SAFETY: posix_openpt returns a new descriptor or -1.
    let master = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | cloexec()) };
    assert!(master >= 0, "posix_openpt failed");
    // SAFETY: master is a valid pty master owned here.
    unsafe {
        assert_eq!(libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC), 0);
        assert_eq!(libc::grantpt(master), 0);
        assert_eq!(libc::unlockpt(master), 0);
    }
    static PTSNAME: Mutex<()> = Mutex::new(());
    let _guard = PTSNAME.lock().unwrap();
    // SAFETY: ptsname returns a static string, read under the lock.
    let name = unsafe { std::ffi::CStr::from_ptr(libc::ptsname(master)) }.to_owned();
    // SAFETY: master is new and owned only here.
    (unsafe { File::from_raw_fd(master) }, name)
}

#[cfg(target_os = "linux")]
fn cloexec() -> i32 {
    libc::O_CLOEXEC
}

#[cfg(not(target_os = "linux"))]
fn cloexec() -> i32 {
    0
}

/// One helper process and the human side of its terminal.
pub(super) struct Session {
    master: File,
    screen: Arc<Mutex<Vec<u8>>>,
    child: Child,
}

pub(super) struct Finished {
    pub results: Vec<StepResult>,
    pub stdout: String,
    pub stderr: String,
    pub screen: String,
}

impl Session {
    pub(super) fn start(socket: &Path, steps: Vec<Step>) -> Self {
        let (master, slave) = open_pty();
        let spec = serde_json::to_string(&(socket, steps)).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "broker::pty_harness::helper_entry",
                "--nocapture",
            ])
            .env(HELPER_ENV, spec)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: setsid, open, ioctl, dup2, and close are async-signal-safe.
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let fd = libc::open(slave.as_ptr(), libc::O_RDWR);
                if fd < 0 || libc::ioctl(fd, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // The terminal is stdin, as in an interactive shell.
                if libc::dup2(fd, libc::STDIN_FILENO) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                libc::close(fd);
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        let screen = Arc::new(Mutex::new(Vec::new()));
        let (mut reader, sink) = (master.try_clone().unwrap(), Arc::clone(&screen));
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(read @ 1..) = reader.read(&mut buf) {
                sink.lock().unwrap().extend_from_slice(&buf[..read]);
            }
        });
        Self {
            master,
            screen,
            child,
        }
    }

    pub(super) fn screen(&self) -> String {
        String::from_utf8_lossy(&self.screen.lock().unwrap()).into_owned()
    }

    /// Wait until `text` has appeared `count` times on the terminal.
    pub(super) fn wait_for(&self, text: &str, count: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self.screen().matches(text).count() >= count {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Like [`Session::wait_for`], but a timeout stops the helper and panics
    /// with everything it printed.
    pub(super) fn expect(&mut self, text: &str, count: usize) {
        if self.wait_for(text, count) {
            return;
        }
        let _ = self.child.kill();
        let mut out = String::new();
        if let Some(mut stdout) = self.child.stdout.take() {
            let _ = stdout.read_to_string(&mut out);
        }
        if let Some(mut stderr) = self.child.stderr.take() {
            let _ = stderr.read_to_string(&mut out);
        }
        panic!(
            "{text:?} x{count} never appeared.\nhelper: {out}\nscreen: {:?}",
            self.screen()
        );
    }

    /// Type raw bytes, as a human pressing keys.
    pub(super) fn type_keys(&mut self, keys: &str) {
        self.master.write_all(keys.as_bytes()).unwrap();
    }

    pub(super) fn kill(&mut self) {
        let _ = self.child.kill();
    }

    pub(super) fn finish(self) -> Finished {
        let output = self.child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let results = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("HUSH-RESULT:"))
            .map(|json| serde_json::from_str(json).unwrap())
            .collect();
        let screen = String::from_utf8_lossy(&self.screen.lock().unwrap()).into_owned();
        Finished {
            results,
            stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            screen,
        }
    }
}

/// The helper side. Runs only inside a helper process.
#[test]
fn helper_entry() {
    let Ok(spec) = std::env::var(HELPER_ENV) else {
        return;
    };
    let (socket, steps): (PathBuf, Vec<Step>) = serde_json::from_str(&spec).unwrap();
    let mut client = client::Client::connect_to(&socket).unwrap();
    for step in steps {
        let mut keep: Vec<File> = Vec::new();
        let mut watched = None;
        for fd in &step.fds {
            keep.push(match fd {
                Fd::Tty => super::client_human::open_terminal().unwrap(),
                Fd::TtyFlipper => {
                    let tty = super::client_human::open_terminal().unwrap();
                    let raw = std::os::fd::AsRawFd::as_raw_fd(&tty);
                    std::thread::spawn(move || loop {
                        // SAFETY: changes only status flags; the helper exits with the fd open.
                        unsafe {
                            let flags = libc::fcntl(raw, libc::F_GETFL);
                            libc::fcntl(raw, libc::F_SETFL, flags & !libc::O_NONBLOCK);
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    });
                    tty
                }
                Fd::Pipe(content) => {
                    let (reader, mut writer) = pipe();
                    writer.write_all(content.as_bytes()).unwrap();
                    reader
                }
                Fd::PipeWriter => {
                    let (reader, writer) = pipe();
                    watched = Some(reader);
                    writer
                }
                Fd::DevNull => File::options()
                    .read(true)
                    .write(true)
                    .open("/dev/null")
                    .unwrap(),
                Fd::ForeignPty => {
                    let (master, slave) = open_pty();
                    std::mem::forget(master);
                    let path = slave.to_str().unwrap();
                    let mut options = File::options();
                    options.read(true).write(true).custom_flags(libc::O_NOCTTY);
                    options.open(path).unwrap()
                }
            });
        }
        let fds: Vec<_> = keep.iter().map(|file| file.as_fd()).collect();
        let reply = client.operate(step.op, &fds).unwrap();
        drop(fds);
        drop(keep);
        let writer_closed = watched.map(|reader| writers_closed(&reader));
        let result = StepResult {
            reply,
            writer_closed,
        };
        println!("HUSH-RESULT:{}", serde_json::to_string(&result).unwrap());
    }
}
