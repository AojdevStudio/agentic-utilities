//! macOS password approval through OpenDirectory: find the peer account on
//! the authentication search node and verify its password. The password goes
//! to OpenDirectory as a CoreFoundation string, never argv or the environment.
//! CoreFoundation keeps its own copy of that string, which hush cannot wipe.

use super::approve::{Authenticator, Verdict};
use std::ffi::c_void;
use std::ptr::{null, null_mut};

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type ODSessionRef = *const c_void;
type ODNodeRef = *const c_void;
type ODRecordRef = *const c_void;

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const OD_NODE_TYPE_AUTHENTICATION: u32 = 0x2201;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithBytes(
        allocator: CFTypeRef,
        bytes: *const u8,
        length: isize,
        encoding: u32,
        external: u8,
    ) -> CFStringRef;
    fn CFRelease(object: CFTypeRef);
}

#[link(name = "OpenDirectory", kind = "framework")]
extern "C" {
    static kODSessionDefault: ODSessionRef;
    static kODRecordTypeUsers: CFStringRef;
    fn ODNodeCreateWithNodeType(
        allocator: CFTypeRef,
        session: ODSessionRef,
        node_type: u32,
        error: *mut *mut c_void,
    ) -> ODNodeRef;
    fn ODNodeCopyRecord(
        node: ODNodeRef,
        record_type: CFStringRef,
        name: CFStringRef,
        attributes: CFTypeRef,
        error: *mut *mut c_void,
    ) -> ODRecordRef;
    fn ODRecordVerifyPassword(
        record: ODRecordRef,
        password: CFStringRef,
        error: *mut *mut c_void,
    ) -> bool;
}

/// Releases one CoreFoundation object when dropped.
struct Owned(CFTypeRef);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the object came from a CoreFoundation Create or Copy call.
            unsafe { CFRelease(self.0) };
        }
    }
}

fn cf_string(bytes: &[u8]) -> Option<Owned> {
    // SAFETY: CoreFoundation copies `length` bytes; null on invalid UTF-8.
    let string = unsafe {
        CFStringCreateWithBytes(
            null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            CF_STRING_ENCODING_UTF8,
            0,
        )
    };
    (!string.is_null()).then_some(Owned(string))
}

pub(crate) struct OpenDirectoryAuthenticator;

impl Authenticator for OpenDirectoryAuthenticator {
    fn available(&self) -> Result<(), String> {
        Ok(())
    }

    fn verify(&self, account: &str, password: &[u8]) -> Result<Verdict, String> {
        let name = cf_string(account.as_bytes()).ok_or("account name is not UTF-8")?;
        let Some(password) = cf_string(password) else {
            return Ok(Verdict::Denied);
        };
        // SAFETY: every argument is a live CoreFoundation object or a framework constant.
        unsafe {
            let node = Owned(ODNodeCreateWithNodeType(
                null(),
                kODSessionDefault,
                OD_NODE_TYPE_AUTHENTICATION,
                null_mut(),
            ));
            if node.0.is_null() {
                return Err("OpenDirectory authentication node is unavailable".into());
            }
            let record = Owned(ODNodeCopyRecord(
                node.0,
                kODRecordTypeUsers,
                name.0,
                null(),
                null_mut(),
            ));
            if record.0.is_null() {
                return Err(format!("OpenDirectory has no user record for {account}"));
            }
            Ok(
                if ODRecordVerifyPassword(record.0, password.0, null_mut()) {
                    Verdict::Approved
                } else {
                    Verdict::Denied
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::identity::{AccountDb, SystemAccounts};

    /// Native negative proof: a wrong password for this user is denied.
    #[test]
    fn a_wrong_password_is_denied_by_opendirectory() {
        // SAFETY: getuid takes no arguments and cannot fail.
        let uid = unsafe { libc::getuid() };
        let account = SystemAccounts.by_uid(uid).unwrap().unwrap().name;
        let wrong = format!("hush-wrong-password-{}", std::process::id());
        assert_eq!(
            OpenDirectoryAuthenticator.verify(&account, wrong.as_bytes()),
            Ok(Verdict::Denied)
        );
    }
}
