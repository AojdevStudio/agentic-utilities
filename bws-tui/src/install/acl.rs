//! Access list grants beyond the mode bits. A grant lets another principal
//! read or change an object that its mode alone keeps private.

use super::files::Stop;
use std::os::fd::RawFd;

/// A minimal POSIX access list holds only the owner, group, and other
/// entries that mirror the mode: a 4 byte header and three 8 byte entries.
#[cfg(target_os = "linux")]
pub(crate) fn grants_access(fd: RawFd) -> Result<bool, Stop> {
    const MINIMAL: usize = 4 + 3 * 8;
    // SAFETY: a null buffer with size zero asks fgetxattr for the length only.
    let size = unsafe {
        libc::fgetxattr(
            fd,
            c"system.posix_acl_access".as_ptr(),
            std::ptr::null_mut(),
            0,
        )
    };
    if size < 0 {
        return match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ENODATA | libc::EOPNOTSUPP) => Ok(false),
            _ => Err(Stop::Failed),
        };
    }
    Ok(size as usize > MINIMAL)
}

/// macOS extended access lists. Deny entries narrow access and pass. Any
/// allow entry is a grant.
#[cfg(target_os = "macos")]
pub(crate) fn grants_access(fd: RawFd) -> Result<bool, Stop> {
    use std::ffi::{c_int, c_void};
    const ACL_TYPE_EXTENDED: c_int = 0x100;
    const ACL_FIRST_ENTRY: c_int = 0;
    const ACL_NEXT_ENTRY: c_int = -1;
    const ACL_EXTENDED_ALLOW: c_int = 1;
    extern "C" {
        fn acl_get_fd_np(fd: c_int, kind: c_int) -> *mut c_void;
        fn acl_get_entry(acl: *mut c_void, id: c_int, entry: *mut *mut c_void) -> c_int;
        fn acl_get_tag_type(entry: *mut c_void, tag: *mut c_int) -> c_int;
        fn acl_free(object: *mut c_void) -> c_int;
    }
    // SAFETY: acl_get_fd_np reads the list of an open descriptor or returns null.
    let acl = unsafe { acl_get_fd_np(fd, ACL_TYPE_EXTENDED) };
    if acl.is_null() {
        return match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ENOENT) => Ok(false),
            _ => Err(Stop::Failed),
        };
    }
    let mut grant = false;
    let mut entry: *mut c_void = std::ptr::null_mut();
    let mut id = ACL_FIRST_ENTRY;
    // SAFETY: every entry comes from this list, which stays alive until acl_free.
    unsafe {
        while acl_get_entry(acl, id, &mut entry) == 0 {
            let mut tag: c_int = 0;
            if acl_get_tag_type(entry, &mut tag) != 0 || tag == ACL_EXTENDED_ALLOW {
                grant = true;
                break;
            }
            id = ACL_NEXT_ENTRY;
        }
        acl_free(acl);
    }
    Ok(grant)
}
