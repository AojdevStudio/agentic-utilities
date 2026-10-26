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
const CALL_END: &str = "@@end-of-call@@";

const STUB: &str = r#"#!/bin/sh
printf '%s\0' "$@" '@@end-of-call@@' >> "$HUSH_STUB_LOG"
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

    /// A real token in the environment must never reach a bws that shadows the stub.
    fn hush(&self, args: &[&str], stdin: Stdio) -> Child {
        let path = format!(
            "{}:{}",
            self.root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_hush"))
            .args(args)
            .env_remove("BWS_ACCESS_TOKEN")
            .env_remove("BWS_SERVER_URL")
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

    /// The argv of the one `bws secret <subcommand>` call, if hush made it.
    fn secret_call(&self, subcommand: &str) -> Option<Vec<String>> {
        let log = String::from_utf8(fs::read(self.log()).unwrap_or_default()).unwrap();
        let args: Vec<String> = log.split('\0').map(str::to_owned).collect();
        args.split(|arg| arg == CALL_END)
            .find(|call| {
                call.windows(2)
                    .any(|pair| pair[0] == "secret" && pair[1] == subcommand)
            })
            .map(<[String]>::to_vec)
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

fn argv(parts: &[&str]) -> Option<Vec<String>> {
    Some(parts.iter().map(|part| part.to_string()).collect())
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
    assert_eq!(
        sandbox.secret_call("create"),
        argv(&[
            "-o",
            "json",
            "secret",
            "create",
            "--",
            "HUSH_TEST_KEY",
            "v1",
            "p1"
        ])
    );
}

#[test]
fn add_without_value_reads_piped_stdin() {
    let sandbox = Sandbox::new("add-piped");
    let mut child = sandbox.hush(
        &["add", "--key", "HUSH_TEST_KEY", "--project", "proj"],
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
    assert_eq!(
        sandbox.secret_call("create"),
        argv(&[
            "-o",
            "json",
            "secret",
            "create",
            "--",
            "HUSH_TEST_KEY",
            PEM,
            "p1"
        ])
    );
}

#[test]
fn add_sends_hyphen_leading_key_value_and_note_to_bws_intact() {
    let sandbox = Sandbox::new("add-pem");
    let value = format!("--value={PEM}");
    let child = sandbox.hush(
        &[
            "add",
            "--key=-HUSH_TEST_KEY",
            "--project",
            "proj",
            &value,
            "--note=-n",
        ],
        Stdio::null(),
    );
    let output = finish_within(child, Duration::from_secs(10));
    assert_success(&output);
    assert_eq!(
        sandbox.secret_call("create"),
        argv(&[
            "-o",
            "json",
            "secret",
            "create",
            "--note=-n",
            "--",
            "-HUSH_TEST_KEY",
            PEM,
            "p1"
        ])
    );
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
    let value = format!("--value={PEM}");
    assert_eq!(
        sandbox.secret_call("edit"),
        argv(&["-o", "json", "secret", "edit", &value, "--", "s1"])
    );
}

#[test]
fn edit_sends_hyphen_leading_key_and_note_to_bws_intact() {
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
    assert_eq!(
        sandbox.secret_call("edit"),
        argv(&[
            "-o",
            "json",
            "secret",
            "edit",
            "--key=-HUSH_TEST_RENAMED",
            "--note=-- rotated",
            "--",
            "s1"
        ])
    );
}

#[test]
fn value_never_takes_the_next_flag_as_the_secret() {
    let sandbox = Sandbox::new("edit-value-swallow");
    let child = sandbox.hush(
        &[
            "edit",
            "--key",
            "HUSH_TEST_KEY",
            "--project",
            "proj",
            "--value",
            "--stdin",
        ],
        Stdio::null(),
    );
    let output = finish_within(child, Duration::from_secs(10));
    assert!(!output.status.success(), "hush accepted --value --stdin");
    assert_eq!(sandbox.secret_call("edit"), None);
}
