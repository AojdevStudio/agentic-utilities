//! The production native runner against real short-lived processes. Budgets
//! are scaled down for tests, and one case shrinks only its own child's
//! stdin pipe. No service, account or key tool runs.

use super::native_host::run_within;
use super::native_io::{Budget, BUDGET};
use crate::broker::private_temp_harness::PrivateTemp;
use crate::broker::token::TOKEN_LIMIT;
use secrecy::SecretString;
use std::fs::{self, File};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
const SH: &str = "/usr/bin/dash";
#[cfg(target_os = "macos")]
const SH: &str = "/bin/sh";
#[cfg(target_os = "linux")]
const SLEEP: &str = "/usr/bin/sleep";
#[cfg(target_os = "macos")]
const SLEEP: &str = "/bin/sleep";
const PRINTF: &str = "/usr/bin/printf";
const WC: &str = "/usr/bin/wc";

/// Test only: 150 ms instead of the production 120 s.
const SHORT: Budget = Budget {
    time: Duration::from_millis(150),
    ..BUDGET
};
/// Well past SHORT, well before any child sleep below ends on its own.
const BOUND: Duration = Duration::from_millis(1000);

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| arg.to_string()).collect()
}

#[test]
fn a_helper_that_never_reads_its_token_is_stopped_at_the_deadline() {
    let temp = PrivateTemp::new("hush-native-test");
    let out = File::create(temp.path().join("ciphertext")).unwrap();
    let token = SecretString::from("X".repeat(TOKEN_LIMIT));
    // Test only: this child's stdin pipe holds 4096 bytes, so the token blocks.
    let budget = Budget {
        input_pipe: Some(4096),
        ..SHORT
    };
    let started = Instant::now();
    let outcome = run_within(SLEEP, &args(&["3"]), Some((&token, out)), budget);
    let elapsed = started.elapsed();
    assert!(
        outcome.is_err() && elapsed < BOUND,
        "{outcome:?} after {elapsed:?}"
    );
}

#[test]
fn a_descendant_holding_stdout_is_stopped_at_the_deadline() {
    let temp = PrivateTemp::new("hush-native-test");
    let pid_file = temp.path().join("pid");
    let script = format!("{SLEEP} 3 & echo $! > {}; exit 0", pid_file.display());
    let started = Instant::now();
    let outcome = run_within(SH, &args(&["-c", &script]), None, SHORT);
    let elapsed = started.elapsed();
    let descendant = Descendant(
        fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap(),
    );
    assert!(
        outcome.is_err() && elapsed < BOUND,
        "{outcome:?} after {elapsed:?}"
    );
    assert!(descendant.gone_within(BOUND), "the descendant still runs");
}

#[test]
fn unreadable_or_oversized_output_is_an_error_not_a_partial_result() {
    // Test only: a 16 byte output budget instead of 1 MiB.
    let small = Budget {
        output: 16,
        ..BUDGET
    };
    let invalid = run_within(PRINTF, &args(&["\\377"]), None, BUDGET);
    let oversized = run_within(PRINTF, &args(&["%017d", "0"]), None, small);
    assert_eq!(
        [
            ("invalid UTF-8", invalid.is_err()),
            ("over budget", oversized.is_err())
        ],
        [("invalid UTF-8", true), ("over budget", true)],
        "{invalid:?} {oversized:?}"
    );
    let fits = run_within(PRINTF, &args(&["%016d", "0"]), None, small).unwrap();
    assert_eq!(fits, (Some(0), "0".repeat(16)));
    let failed = run_within(SH, &args(&["-c", "exit 3"]), None, BUDGET).unwrap();
    assert_eq!(failed, (Some(3), String::new()));
}

#[test]
fn a_helper_that_reads_its_token_finishes_within_its_budget() {
    let temp = PrivateTemp::new("hush-native-test");
    let path = temp.path().join("out");
    let token = SecretString::from("X".repeat(TOKEN_LIMIT));
    // Test only: the same shrunk pipe, with a reader on the other end.
    let budget = Budget {
        time: Duration::from_secs(5),
        input_pipe: Some(4096),
        ..BUDGET
    };
    let out = File::create(&path).unwrap();
    let (code, _) = run_within(WC, &args(&["-c"]), Some((&token, out)), budget).unwrap();
    assert_eq!(code, Some(0));
    assert_eq!(
        fs::read_to_string(&path).unwrap().trim(),
        TOKEN_LIMIT.to_string()
    );
}

/// A process the runner's child left behind. Killed on drop if still alive,
/// so a failing test leaves nothing running.
struct Descendant(libc::pid_t);

impl Descendant {
    fn gone_within(&self, limit: Duration) -> bool {
        let until = Instant::now() + limit;
        loop {
            // SAFETY: signal 0 only checks whether the pid exists.
            if unsafe { libc::kill(self.0, 0) } != 0 {
                return true;
            }
            if Instant::now() > until {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Descendant {
    fn drop(&mut self) {
        if !self.gone_within(Duration::ZERO) {
            // SAFETY: SIGKILL to the recorded test descendant only.
            unsafe { libc::kill(self.0, libc::SIGKILL) };
        }
    }
}
