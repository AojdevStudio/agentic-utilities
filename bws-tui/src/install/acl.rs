//! Extended access grants beyond the owner, group and other mode bits.
//! Mode bits alone do not prove a private file is private.

use std::io;
use std::os::fd::BorrowedFd;

/// Whether the open file or directory grants access through an extended ACL.
/// Linux flags named user or group entries and any default ACL, which new
/// children would inherit. macOS flags any allow entry. Deny entries, such as
/// the stock one on `/`, grant nothing.
pub(crate) fn grants(fd: BorrowedFd<'_>) -> io::Result<bool> {
    native::grants(fd)
}

#[cfg(target_os = "linux")]
mod native {
    use std::ffi::CStr;
    use std::io;
    use std::os::fd::{AsRawFd, BorrowedFd};

    const ACL_USER: u16 = 0x02;
    const ACL_GROUP: u16 = 0x08;

    pub(super) fn grants(fd: BorrowedFd<'_>) -> io::Result<bool> {
        let named = xattr(fd, c"system.posix_acl_access")?.is_some_and(|acl| {
            acl.get(4..)
                .unwrap_or_default()
                .as_chunks::<8>()
                .0
                .iter()
                .any(|entry| {
                    let tag = u16::from_le_bytes([entry[0], entry[1]]);
                    tag == ACL_USER || tag == ACL_GROUP
                })
        });
        Ok(named || xattr(fd, c"system.posix_acl_default")?.is_some())
    }

    /// One extended attribute, or None when the file has none or the
    /// filesystem has no ACL support.
    fn xattr(fd: BorrowedFd<'_>, name: &CStr) -> io::Result<Option<Vec<u8>>> {
        let mut buffer = vec![0u8; 4096];
        // SAFETY: the name is NUL-terminated and the length matches the buffer.
        let len = unsafe {
            libc::fgetxattr(
                fd.as_raw_fd(),
                name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        let Ok(len) = usize::try_from(len) else {
            let error = io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(libc::ENODATA | libc::EOPNOTSUPP) => Ok(None),
                _ => Err(error),
            };
        };
        buffer.truncate(len);
        Ok(Some(buffer))
    }
}

#[cfg(target_os = "macos")]
mod native {
    use std::ffi::{c_int, c_void};
    use std::io;
    use std::os::fd::{AsRawFd, BorrowedFd};

    const ACL_TYPE_EXTENDED: c_int = 0x0000_0100;
    const ACL_FIRST_ENTRY: c_int = 0;
    const ACL_NEXT_ENTRY: c_int = -1;
    const ACL_EXTENDED_ALLOW: c_int = 1;

    extern "C" {
        fn acl_get_fd_np(fd: c_int, kind: c_int) -> *mut c_void;
        fn acl_get_entry(acl: *mut c_void, entry_id: c_int, entry: *mut *mut c_void) -> c_int;
        fn acl_get_tag_type(entry: *mut c_void, tag: *mut c_int) -> c_int;
        fn acl_free(object: *mut c_void) -> c_int;
    }

    pub(super) fn grants(fd: BorrowedFd<'_>) -> io::Result<bool> {
        // SAFETY: acl_get_fd_np reads the open descriptor and returns an owned ACL or null.
        let acl = unsafe { acl_get_fd_np(fd.as_raw_fd(), ACL_TYPE_EXTENDED) };
        if acl.is_null() {
            let error = io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(libc::ENOENT) => Ok(false),
                _ => Err(error),
            };
        }
        let mut entry_id = ACL_FIRST_ENTRY;
        let mut entry = std::ptr::null_mut();
        let mut allows = false;
        // SAFETY: `acl` stays valid until acl_free, and each entry belongs to it.
        unsafe {
            while acl_get_entry(acl, entry_id, &mut entry) == 0 {
                entry_id = ACL_NEXT_ENTRY;
                let mut tag = 0;
                if acl_get_tag_type(entry, &mut tag) == 0 && tag == ACL_EXTENDED_ALLOW {
                    allows = true;
                    break;
                }
            }
            acl_free(acl);
        }
        Ok(allows)
    }
}
