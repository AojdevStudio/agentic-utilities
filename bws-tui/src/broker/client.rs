//! Broker client. Holds no token and never receives secret bytes.

use super::meta::SecretMeta;
use super::paths::default_socket_path;
use super::protocol::{read_frame, write_frame, Frame, RequestFrame, ResponseFrame};
use anyhow::{bail, Result};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Result of probing for a daemon. Only `Unavailable` may fall back.
pub(crate) enum ConnectOutcome {
    Connected(Client),
    Unavailable,
}

pub(crate) struct Client {
    stream: UnixStream,
}

impl Client {
    #[allow(dead_code)]
    pub(crate) fn connect() -> Result<Self> {
        Self::connect_to(&default_socket_path())
    }

    #[allow(dead_code)]
    pub(crate) fn connect_to(path: &PathBuf) -> Result<Self> {
        let stream = UnixStream::connect(path)?;
        Self::from_stream(stream)
    }

    /// Probe the default socket. Unreachable daemons report `Unavailable`.
    pub(crate) fn try_connect() -> Result<ConnectOutcome> {
        Self::try_connect_to(&default_socket_path())
    }

    /// Probe one socket path. Unreachable daemons report `Unavailable`.
    pub(crate) fn try_connect_to(path: &Path) -> Result<ConnectOutcome> {
        match std::fs::metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ConnectOutcome::Unavailable);
            }
            Err(error) => return Err(error.into()),
            Ok(metadata) => {
                if !metadata.file_type().is_socket() {
                    return Ok(ConnectOutcome::Unavailable);
                }
            }
        }
        match UnixStream::connect(path) {
            Ok(stream) => {
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                Ok(ConnectOutcome::Connected(Self::from_stream(stream)?))
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(ConnectOutcome::Unavailable)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Wrap one connected stream.
    pub(crate) fn from_stream(stream: UnixStream) -> Result<Self> {
        Ok(Self { stream })
    }

    /// Fetch metadata. Every connected failure propagates as an error.
    pub(crate) fn catalog(&mut self, project: Option<&str>) -> Result<Vec<SecretMeta>> {
        write_frame(
            &mut self.stream,
            &Frame::Request(RequestFrame::Catalog {
                project: project.map(ToOwned::to_owned),
            }),
        )?;
        match read_frame(&mut self.stream)? {
            Frame::Response(ResponseFrame::Catalog { entries }) => Ok(entries),
            Frame::Response(ResponseFrame::Error { message, .. }) => bail!("{message}"),
            Frame::Request(_) => bail!("unexpected frame from daemon"),
        }
    }
}
