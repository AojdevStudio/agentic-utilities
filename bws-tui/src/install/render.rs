//! The bytes install writes for the service definition and the PAM service.
//! Rendering is pure. A path that would need quoting is refused, not escaped.

use super::layout::{token_source, runtime_token, CREDENTIAL_NAME};
use super::model::{Backend, Fact};
use crate::broker::audit::fixed_audit_path;
use std::path::{Path, PathBuf};

pub(crate) struct Service<'a> {
    pub hush: &'a Path,
    pub bws: &'a Path,
    pub backend: Backend,
}

/// Linux distro layouts with a password stack and an account stack to include.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PamStack {
    /// Debian and Ubuntu: common-auth and common-account.
    Common,
    /// Fedora, Arch, and others: system-auth for both.
    SystemAuth,
}

pub(crate) fn pam(stack: PamStack) -> Vec<u8> {
    let (auth, account) = match stack {
        PamStack::Common => ("common-auth", "common-account"),
        PamStack::SystemAuth => ("system-auth", "system-auth"),
    };
    format!(
        "# hush broker password approval. Written by hush install.\n\
         auth\tinclude\t{auth}\naccount\tinclude\t{account}\n"
    )
    .into_bytes()
}

/// A word systemd and launchd both take literally.
fn plain(path: &Path) -> Option<&str> {
    let text = path.to_str()?;
    let ok = text.starts_with('/')
        && text
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"\"'\\$%;&<>".contains(&b));
    ok.then_some(text)
}

pub(crate) fn definition(service: &Service<'_>) -> Fact<Vec<u8>> {
    let (Some(hush), Some(bws)) = (plain(service.hush), plain(service.bws)) else {
        return Fact::Refused("a service path has characters install will not quote");
    };
    let audit = fixed_audit_path();
    let token = runtime_token(service.backend);
    let args = [
        hush,
        "daemon",
        "--token-file",
        &token,
        "--audit-log",
        &audit.display().to_string(),
        "--bws-path",
        bws,
    ]
    .map(str::to_owned);
    if cfg!(target_os = "macos") {
        Fact::Known(plist(&args))
    } else {
        Fact::Known(unit(&args, service.backend))
    }
}

fn unit(args: &[String], backend: Backend) -> Vec<u8> {
    let source = token_source(backend);
    let credential = match backend {
        Backend::Encrypted => "LoadCredentialEncrypted",
        Backend::LoadCredential | Backend::File => "LoadCredential",
    };
    format!(
        "# Written by hush install. See docs/adr/0002-hush-broker.md.\n\
         [Unit]\n\
         Description=hush secrets broker\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         {credential}={CREDENTIAL_NAME}:{source}\n\
         ExecStart={exec}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        source = source.display(),
        exec = args.join(" "),
    )
    .into_bytes()
}

fn plist(args: &[String]) -> Vec<u8> {
    let strings: String = args
        .iter()
        .map(|arg| format!("\t\t<string>{arg}</string>\n"))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>Label</key>\n\
         \t<string>hush</string>\n\
         \t<key>ProgramArguments</key>\n\
         \t<array>\n\
         {strings}\
         \t</array>\n\
         \t<key>RunAtLoad</key>\n\
         \t<true/>\n\
         \t<key>KeepAlive</key>\n\
         \t<dict>\n\
         \t\t<key>SuccessfulExit</key>\n\
         \t\t<false/>\n\
         \t</dict>\n\
         </dict>\n\
         </plist>\n"
    )
    .into_bytes()
}

/// Read the service executable and bws path back from a definition install
/// wrote, so status describes the installed service, not the caller's binary.
pub(crate) fn installed_binaries(bytes: &[u8]) -> Option<(PathBuf, PathBuf)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let words: Vec<&str> = if cfg!(target_os = "macos") {
        text.split("<string>")
            .skip(2)
            .filter_map(|part| part.split_once("</string>").map(|(word, _)| word))
            .collect()
    } else {
        text.lines()
            .find_map(|line| line.strip_prefix("ExecStart="))?
            .split(' ')
            .collect()
    };
    let bws = words.windows(2).find(|pair| pair[0] == "--bws-path")?[1];
    Some((PathBuf::from(words.first()?), PathBuf::from(bws)))
}
