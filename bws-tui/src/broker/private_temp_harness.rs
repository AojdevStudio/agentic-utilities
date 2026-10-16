//! Exclusive private temporary directories for tests.
//! mkdtemp creates a fresh 0700 directory with an unpredictable name, so a test
//! never deletes or reuses a path another process could have made.

use std::ffi::{CString, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

pub(crate) struct PrivateTemp(PathBuf);

impl PrivateTemp {
    pub(crate) fn new(prefix: &str) -> Self {
        let template = std::env::temp_dir().join(format!("{prefix}-XXXXXX"));
        let template = CString::new(template.as_os_str().as_bytes()).unwrap();
        let raw = template.into_raw();
        // SAFETY: `raw` is a writable NUL-terminated template that mkdtemp fills in place.
        let made = unsafe { libc::mkdtemp(raw) };
        // SAFETY: `raw` came from CString::into_raw and mkdtemp kept its length.
        let path = unsafe { CString::from_raw(raw) };
        assert!(
            !made.is_null(),
            "mkdtemp failed: {}",
            std::io::Error::last_os_error()
        );
        Self(PathBuf::from(OsString::from_vec(path.into_bytes())))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for PrivateTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
