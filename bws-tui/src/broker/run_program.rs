//! Program paths a run's child tries, built before `fork`. Repeated empty
//! entries or a long name multiply one request, so the total is bounded by one
//! request frame before anything is allocated.

use super::protocol::MAX_PAYLOAD;
use anyhow::{bail, Context, Result};
use std::ffi::{CString, OsStr};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

/// Per candidate: its `CString`, the child's pointer to it, and a conservative
/// allowance for allocator headers and rounding.
const CANDIDATE_OVERHEAD: usize =
    size_of::<CString>() + size_of::<*const libc::c_char>() + 4 * size_of::<usize>();

/// A program with a slash resolves against `cwd`. A bare name yields one
/// candidate per entry of the request PATH. Relative and empty entries resolve
/// against `cwd`, as they would in the caller's shell. The child tries them in
/// order after dropping to the run identity, so no access check happens here.
pub(crate) fn program_candidates(
    program: &OsStr,
    cwd: &Path,
    path_var: Option<&[u8]>,
) -> Result<Vec<CString>> {
    let program = program.as_bytes();
    if program.is_empty() {
        bail!("no program given after --");
    }
    // One empty entry joins a slash program to `cwd`, or keeps it when absolute.
    let search: &[u8] = if program.contains(&b'/') {
        b""
    } else {
        path_var.unwrap_or(b"/usr/bin:/bin")
    };
    let entries = || search.split(|&byte| byte == b':');
    let fits = entries().try_fold(0usize, |total, dir| {
        total
            .checked_add(joined_len(cwd, dir, program))?
            .checked_add(CANDIDATE_OVERHEAD)
            .filter(|&total| total <= MAX_PAYLOAD)
    });
    if fits.is_none() {
        bail!(
            "refusing the program search: its PATH candidates need more than {MAX_PAYLOAD} bytes. \
             Shorten PATH or name the program by path"
        );
    }
    let mut candidates = Vec::with_capacity(entries().count());
    for dir in entries() {
        let mut path = PathBuf::with_capacity(joined_len(cwd, dir, program));
        path.push(cwd);
        path.push(OsStr::from_bytes(dir));
        path.push(OsStr::from_bytes(program));
        let bytes = path.into_os_string().into_vec();
        candidates.push(CString::new(bytes).context("the program or PATH holds a NUL byte")?);
    }
    Ok(candidates)
}

/// Upper bound on `cwd.join(dir).join(program)` plus its NUL.
fn joined_len(cwd: &Path, dir: &[u8], program: &[u8]) -> usize {
    let base = if program.starts_with(b"/") {
        0
    } else if dir.starts_with(b"/") {
        dir.len() + 1
    } else {
        cwd.as_os_str().len() + dir.len() + 2
    };
    base.saturating_add(program.len()).saturating_add(1)
}
