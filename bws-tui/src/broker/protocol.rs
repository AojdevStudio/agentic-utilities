//! Private wire protocol. Versioned, length-prefixed JSON frames.
//!
//! Frame layout: one version byte, then a 3-byte big-endian payload length,
//! then the JSON payload. No frame variant can hold secret material.

use super::fdpass::{recv_exact, send_with_rights, RightsViolation};
use super::meta::{ProjectMeta, SecretMeta};
use super::operation::{Operation, Outcome, Refusal};
use super::runspec::{Delivery, EnvName, ForwardSignal, RunOutcome, StreamKind};
use anyhow::{bail, Result};
use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;

/// Current wire protocol version.
pub(crate) const PROTOCOL_VERSION: u8 = 1;
/// Largest payload either side accepts. Rejects oversized frames before allocating.
pub(crate) const MAX_PAYLOAD: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RequestFrame {
    Catalog {
        project: Option<String>,
    },
    Projects,
    /// A human operation. Its descriptors ride on the first byte of this frame.
    Human {
        op: Operation,
    },
    /// Starts a child. The connection then carries only run frames until exit.
    Run {
        spec: WireRunSpec,
    },
    RunStdin {
        data: WireBytes,
    },
    RunStdinEof,
    RunSignal {
        signal: ForwardSignal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ResponseFrame {
    Catalog {
        entries: Vec<SecretMeta>,
    },
    Projects {
        projects: Vec<ProjectMeta>,
    },
    Done {
        outcome: Outcome,
    },
    Refused {
        refusal: Refusal,
    },
    Error {
        code: String,
        message: String,
    },
    /// Child output after masking. Raw child bytes never cross the socket.
    RunOutput {
        stream: StreamKind,
        data: WireBytes,
    },
    RunNotice {
        message: String,
    },
    /// The client may send this many more stdin bytes.
    RunStdinCredit {
        bytes: u32,
    },
    RunExit {
        outcome: RunOutcome,
    },
}

/// A run request on the wire. Every field is a name or client context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WireRunSpec {
    pub bindings: Vec<WireBinding>,
    pub program: WireBytes,
    pub args: Vec<WireBytes>,
    pub cwd: WireBytes,
    pub env: Vec<(EnvName, WireBytes)>,
}

/// One binding on the wire: a key name, never a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WireBinding {
    pub key: String,
    pub project: Option<String>,
    pub env: EnvName,
    pub delivery: Delivery,
}

/// Raw bytes as base64 text, for paths, argv, and masked output.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct WireBytes(pub Vec<u8>);

impl Serialize for WireBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for WireBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(text)
            .map(WireBytes)
            .map_err(serde::de::Error::custom)
    }
}

/// One frame on the wire. Every payload is metadata. Values never appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "direction", content = "frame", rename_all = "snake_case")]
pub(crate) enum Frame {
    Request(RequestFrame),
    Response(ResponseFrame),
}

fn encode(frame: &Frame) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(frame)?;
    if payload.len() > MAX_PAYLOAD {
        bail!("frame payload too large");
    }
    let len = payload.len() as u32;
    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.push(PROTOCOL_VERSION);
    bytes.extend_from_slice(&len.to_be_bytes()[1..]);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn payload_len(head: [u8; 4]) -> Result<usize> {
    if head[0] != PROTOCOL_VERSION {
        bail!("unsupported protocol version {}", head[0]);
    }
    let len = u32::from_be_bytes([0, head[1], head[2], head[3]]) as usize;
    if len > MAX_PAYLOAD {
        bail!("frame payload too large");
    }
    Ok(len)
}

pub(crate) fn write_frame(writer: &mut impl Write, frame: &Frame) -> Result<()> {
    writer.write_all(&encode(frame)?)?;
    writer.flush()?;
    Ok(())
}

pub(crate) fn read_frame(reader: &mut impl Read) -> Result<Frame> {
    let mut head = [0u8; 4];
    reader.read_exact(&mut head)?;
    let mut payload = vec![0u8; payload_len(head)?];
    reader.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}

/// Write one frame with descriptors attached to its first byte.
pub(crate) fn write_frame_with_rights(
    stream: &UnixStream,
    frame: &Frame,
    fds: &[BorrowedFd<'_>],
) -> Result<()> {
    send_with_rights(stream.as_fd(), &encode(frame)?, fds)
}

/// One frame and the descriptors that rode on its first byte.
pub(crate) struct Received {
    pub frame: Frame,
    pub fds: Vec<OwnedFd>,
    pub violation: Option<RightsViolation>,
}

/// Read one frame, taking descriptors only from its first byte.
pub(crate) fn read_frame_with_rights(stream: &UnixStream) -> Result<Received> {
    let socket = stream.as_fd();
    let mut head = [0u8; 4];
    let (fds, first) = recv_exact(socket, &mut head[..1], true)?;
    let (_, rest) = recv_exact(socket, &mut head[1..], false)?;
    let mut payload = vec![0u8; payload_len(head)?];
    let (_, body) = recv_exact(socket, &mut payload, false)?;
    Ok(Received {
        frame: serde_json::from_slice(&payload)?,
        fds,
        violation: first.or(rest).or(body),
    })
}
