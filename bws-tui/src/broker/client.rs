//! Broker client. Holds no token and never receives secret bytes.

use super::meta::SecretMeta;
use super::paths::default_socket_path;
use super::protocol::{read_frame, write_frame, Frame, RequestFrame, ResponseFrame};
use anyhow::{bail, Result};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

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

    /// Short timeout so `hush list` can fall back when no daemon is running.
    pub(crate) fn try_connect() -> Result<Self> {
        let path = default_socket_path();
        let stream = UnixStream::connect(&path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Self::from_stream(stream)
    }

    pub(crate) fn from_stream(stream: UnixStream) -> Result<Self> {
        Ok(Self { stream })
    }

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
