//! Runs the built `hush` against a stub `bws` on PATH. The stub refuses a
//! hyphen-leading token before `--` unless it is a known option, and a split
//! `--opt value` whose value starts with `-`, as bws 2.1.0's parser does.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const PEM: &str =
    "-----BEGIN HUSH TEST VALUE-----\nbm90IGEgc2VjcmV0\n-----END HUSH TEST VALUE-----\n";

const STUB: &str = r#"#!/bin/sh
printf '%s\0' "$@" >> "$HUSH_STUB_LOG"
after=0
want=0
for arg in "$@"; do
  [ "$after" = 1 ] && continue
  if [ "$want" = 1 ]; then
    case "$arg" in -*) exit 2 ;; esac
    want=0
    continue
  fi
  case "$arg" in
    --) after=1 ;;
    -o|--output|--note|--key|--value) want=1 ;;
    --output=*|--note=*|--key=*|--value=*) ;;
    -*) exit 2 ;;
  esac
done
[ "$want" = 0 ] || exit 2
secret='{"id":"s1","key":"HUSH_TEST_KEY","value":"old","note":"","projectId":"p1"}'
case " $* " in
  *" project list "*) echo '[{"id":"p1","name":"proj"}]' ;;
  *" secret list "*) echo "[$secret]" ;;
  *" secret create "*|*" secret edit "*) echo "$secret" ;;
  *) exit 3 ;;
esac
"#;

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("hush-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("bin")).unwrap();
        let stub = root.join("bin/bws");
        fs::write(&stub, STUB).unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        Self { root }
    }

    fn log(&self) -> PathBuf {
        self.root.join("bws.log")
    }

    fn hush(&self, args: &[&str], stdin: Stdio) -> Child {
        let path = format!(
            "{}:{}",
            self.root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_hush"))
            .args(args)
            .env("PATH", path)
            .env("HOME", &self.root)
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_CACHE_HOME", self.root.join("cache"))
            .env("HUSH_STUB_LOG", self.log())
            .stdin(stdin)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    /// Every argument bws received, across all calls.
    fn bws_args(&self) -> Vec<String> {
        let log = fs::read(self.log()).unwrap_or_default();
        String::from_utf8(log)
            .unwrap()
            .split('\0')
            .map(str::to_owned)
            .collect()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn finish_within(mut child: Child, limit: Duration) -> Output {
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > limit {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("hush did not exit within {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "hush failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn sent_whole(sandbox: &Sandbox, value: &str) -> bool {
    sandbox
        .bws_args()
        .iter()
        .any(|arg| arg == value || arg.strip_prefix("--value=") == Some(value))
}

#[test]
fn add_with_value_never_waits_on_an_open_stdin() {
    let sandbox = Sandbox::new("add-open-stdin");
    let mut child = sandbox.hush(
        &[
            "add",
            "--key",
            "HUSH_TEST_KEY",
            "--project",
            "proj",
            "--value",
            "v1",
        ],
        Stdio::piped(),
    );
    let held_open = child.stdin.take();
    let output = finish_within(child, Duration::from_secs(10));
    drop(held_open);
    assert_success(&output);
    assert!(sent_whole(&sandbox, "v1"), "bws never got the --value");
}

#[test]
fn add_sends_a_pem_value_to_bws_intact() {
    let sandbox = Sandbox::new("add-pem");
    let child = sandbox.hush(
        &[
            "add",
            "--key",
            "HUSH_TEST_KEY",
            "--project",
            "proj",
            "--value",
            PEM,
        ],
        Stdio::null(),
    );
    let output = finish_within(child, Duration::from_secs(10));
    assert_success(&output);
    assert!(sent_whole(&sandbox, PEM), "bws did not get the PEM whole");
}

#[test]
fn edit_sends_a_piped_pem_value_to_bws_intact() {
    let sandbox = Sandbox::new("edit-pem");
    let mut child = sandbox.hush(
        &[
            "edit",
            "--key",
            "HUSH_TEST_KEY",
            "--project",
            "proj",
            "--stdin",
        ],
        Stdio::piped(),
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(PEM.as_bytes())
        .unwrap();
    let output = finish_within(child, Duration::from_secs(10));
    assert_success(&output);
    assert!(sent_whole(&sandbox, PEM), "bws did not get the PEM whole");
}

#[test]
fn hyphen_leading_key_and_note_reach_bws() {
    let sandbox = Sandbox::new("edit-hyphen-fields");
    let child = sandbox.hush(
        &[
            "edit",
            "--key",
            "HUSH_TEST_KEY",
            "--project",
            "proj",
            "--new-key=-HUSH_TEST_RENAMED",
            "--note=-- rotated",
        ],
        Stdio::null(),
    );
    let output = finish_within(child, Duration::from_secs(10));
    assert_success(&output);
    let args = sandbox.bws_args();
    assert!(args.iter().any(|a| a.ends_with("-HUSH_TEST_RENAMED")));
    assert!(args.iter().any(|a| a.ends_with("-- rotated")));
}
