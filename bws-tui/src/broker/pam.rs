//! Linux password approval through PAM service `hush`: `pam_authenticate`,
//! then `pam_acct_mgmt`. The password reaches PAM only through the
//! conversation callback, never argv or the environment. A missing service
//! file is refused before any prompt. Installing it is a later install step.

use super::approve::{Authenticator, Verdict};
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::{Path, PathBuf};

const SERVICE: &str = "hush";
/// Where Linux-PAM looks for a service file without an explicit confdir.
const SYSTEM_DIRS: [&str; 2] = ["/etc/pam.d", "/usr/lib/pam.d"];

const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_PERM_DENIED: c_int = 6;
const PAM_AUTH_ERR: c_int = 7;
const PAM_USER_UNKNOWN: c_int = 10;
const PAM_MAXTRIES: c_int = 11;
const PAM_NEW_AUTHTOK_REQD: c_int = 12;
const PAM_ACCT_EXPIRED: c_int = 13;
const PAM_CONV_ERR: c_int = 19;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;
const PAM_SILENT: c_int = 0x8000;
const PAM_DISALLOW_NULL_AUTHTOK: c_int = 0x0001;

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

type Converse =
    extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int;

#[repr(C)]
struct PamConv {
    conv: Converse,
    appdata_ptr: *mut c_void,
}

enum PamHandle {}

#[link(name = "pam")]
extern "C" {
    fn pam_start(
        service: *const c_char,
        user: *const c_char,
        conv: *const PamConv,
        handle: *mut *mut PamHandle,
    ) -> c_int;
    fn pam_start_confdir(
        service: *const c_char,
        user: *const c_char,
        conv: *const PamConv,
        confdir: *const c_char,
        handle: *mut *mut PamHandle,
    ) -> c_int;
    fn pam_authenticate(handle: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_acct_mgmt(handle: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_end(handle: *mut PamHandle, status: c_int) -> c_int;
}

pub(crate) struct PamAuthenticator {
    /// Where service files live. `None` is the system configuration.
    confdir: Option<PathBuf>,
}

impl PamAuthenticator {
    pub(crate) fn system() -> Self {
        Self { confdir: None }
    }

    #[cfg(test)]
    pub(crate) fn with_confdir(confdir: PathBuf) -> Self {
        Self {
            confdir: Some(confdir),
        }
    }

    fn service_dirs(&self) -> Vec<&Path> {
        match &self.confdir {
            Some(dir) => vec![dir.as_path()],
            None => SYSTEM_DIRS.iter().map(Path::new).collect(),
        }
    }
}

impl Authenticator for PamAuthenticator {
    fn available(&self) -> Result<(), String> {
        let dirs = self.service_dirs();
        if dirs.iter().any(|dir| dir.join(SERVICE).is_file()) {
            return Ok(());
        }
        Err(format!(
            "PAM service '{SERVICE}' is not installed (looked in {}). Without it, distributions \
fall back to different defaults, so hush refuses",
            dirs.iter()
                .map(|d| d.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    fn verify(&self, account: &str, password: &[u8]) -> Result<Verdict, String> {
        let user = CString::new(account).map_err(|_| "account name holds a NUL byte")?;
        let service = CString::new(SERVICE).map_err(|_| "bad service name")?;
        let mut secret: &[u8] = password;
        let conv = PamConv {
            conv: converse,
            appdata_ptr: (&mut secret as *mut &[u8]).cast(),
        };
        let mut handle: *mut PamHandle = std::ptr::null_mut();
        let started = match &self.confdir {
            Some(dir) => {
                let dir = CString::new(dir.as_os_str().as_encoded_bytes())
                    .map_err(|_| "confdir holds a NUL byte")?;
                // SAFETY: every pointer is valid for the call; PAM copies the strings.
                unsafe {
                    pam_start_confdir(
                        service.as_ptr(),
                        user.as_ptr(),
                        &conv,
                        dir.as_ptr(),
                        &mut handle,
                    )
                }
            }
            // SAFETY: every pointer is valid for the call; PAM copies the strings.
            None => unsafe { pam_start(service.as_ptr(), user.as_ptr(), &conv, &mut handle) },
        };
        if started != PAM_SUCCESS {
            return Err(format!("PAM could not start (code {started})"));
        }
        let flags = PAM_SILENT | PAM_DISALLOW_NULL_AUTHTOK;
        // SAFETY: handle came from a successful pam_start; `conv` and `secret` outlive pam_end.
        let status = unsafe {
            let mut status = pam_authenticate(handle, flags);
            if status == PAM_SUCCESS {
                status = pam_acct_mgmt(handle, flags);
            }
            pam_end(handle, status);
            status
        };
        match status {
            PAM_SUCCESS => Ok(Verdict::Approved),
            PAM_PERM_DENIED | PAM_AUTH_ERR | PAM_USER_UNKNOWN | PAM_MAXTRIES
            | PAM_NEW_AUTHTOK_REQD | PAM_ACCT_EXPIRED => Ok(Verdict::Denied),
            other => Err(format!("PAM failed (code {other})")),
        }
    }
}

/// Answers every hidden or visible prompt with the password. PAM frees each
/// response with `free`, so answers are `malloc` copies. Informational
/// messages get empty answers.
extern "C" fn converse(
    count: c_int,
    messages: *mut *const PamMessage,
    responses: *mut *mut PamResponse,
    appdata: *mut c_void,
) -> c_int {
    if count <= 0 || messages.is_null() || responses.is_null() || appdata.is_null() {
        return PAM_CONV_ERR;
    }
    let count = count as usize;
    // SAFETY: appdata is the `&[u8]` that `verify` keeps alive across pam_end.
    let password = unsafe { *appdata.cast::<&[u8]>() };
    if password.contains(&0) {
        return PAM_CONV_ERR;
    }
    // SAFETY: calloc returns zeroed memory for `count` responses or null.
    let answers =
        unsafe { libc::calloc(count, std::mem::size_of::<PamResponse>()) }.cast::<PamResponse>();
    if answers.is_null() {
        return PAM_BUF_ERR;
    }
    for index in 0..count {
        // SAFETY: Linux-PAM passes an array of `count` message pointers.
        let style = unsafe { (**messages.add(index)).msg_style };
        if style != PAM_PROMPT_ECHO_OFF && style != PAM_PROMPT_ECHO_ON {
            continue;
        }
        // SAFETY: room for the password and a NUL; filled before PAM reads it.
        unsafe {
            let copy = libc::malloc(password.len() + 1).cast::<u8>();
            if copy.is_null() {
                release(answers, count);
                return PAM_BUF_ERR;
            }
            std::ptr::copy_nonoverlapping(password.as_ptr(), copy, password.len());
            *copy.add(password.len()) = 0;
            (*answers.add(index)).resp = copy.cast();
        }
    }
    // SAFETY: PAM owns the answers from here and frees them.
    unsafe { *responses = answers };
    PAM_SUCCESS
}

/// Wipe and free answers when the conversation fails part way.
///
/// # Safety
/// `answers` holds `count` responses from `calloc`, each `resp` null or `malloc`ed.
unsafe fn release(answers: *mut PamResponse, count: usize) {
    for index in 0..count {
        let resp = (*answers.add(index)).resp;
        if !resp.is_null() {
            let len = libc::strlen(resp);
            std::ptr::write_bytes(resp, 0, len);
            libc::free(resp.cast());
        }
    }
    libc::free(answers.cast());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn confdir(tag: &str, service: Option<&str>) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hush-pam-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(body) = service {
            std::fs::write(dir.join(SERVICE), body).unwrap();
        }
        dir
    }

    fn account() -> String {
        use super::super::identity::{AccountDb, SystemAccounts};
        // SAFETY: getuid takes no arguments and cannot fail.
        let uid = unsafe { libc::getuid() };
        SystemAccounts.by_uid(uid).unwrap().unwrap().name
    }

    #[test]
    fn real_pam_permits_and_denies_through_the_service_file() {
        let permit = PamAuthenticator::with_confdir(confdir(
            "permit",
            Some("auth required pam_permit.so\naccount required pam_permit.so\n"),
        ));
        assert_eq!(permit.available(), Ok(()));
        assert_eq!(
            permit.verify(&account(), b"any password"),
            Ok(Verdict::Approved)
        );
        let deny = PamAuthenticator::with_confdir(confdir(
            "deny",
            Some("auth required pam_deny.so\naccount required pam_permit.so\n"),
        ));
        assert_eq!(
            deny.verify(&account(), b"any password"),
            Ok(Verdict::Denied)
        );
        let account_denied = PamAuthenticator::with_confdir(confdir(
            "account",
            Some("auth required pam_permit.so\naccount required pam_deny.so\n"),
        ));
        assert_eq!(
            account_denied.verify(&account(), b"any password"),
            Ok(Verdict::Denied),
            "an approved password on a refused account is still denied"
        );
    }

    #[test]
    fn a_missing_service_file_is_refused_before_any_prompt() {
        let missing = PamAuthenticator::with_confdir(confdir("missing", None));
        let reason = missing.available().unwrap_err();
        assert!(reason.contains("PAM service"), "{reason}");
    }
}
