//! Private wire protocol. Versioned, length-prefixed JSON frames.
//!
//! Frame layout: one version byte, then a 3-byte big-endian payload length,
//! then the JSON payload. No frame variant can hold secret material.

use super::meta::SecretMeta;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Current wire protocol version.
pub(crate) const PROTOCOL_VERSION: u8 = 1;
/// Largest payload either side accepts. Rejects oversized frames before allocating.
pub(crate) const MAX_PAYLOAD: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RequestFrame {
    Catalog { project: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ResponseFrame {
    Catalog { entries: Vec<SecretMeta> },
    Error { code: String, message: String },
}

/// One frame on the wire. Every payload is metadata. Values never appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "direction", content = "frame", rename_all = "snake_case")]
pub(crate) enum Frame {
    Request(RequestFrame),
    Response(ResponseFrame),
}

pub(crate) fn write_frame(writer: &mut impl Write, frame: &Frame) -> Result<()> {
    let payload = serde_json::to_vec(frame)?;
    if payload.len() > MAX_PAYLOAD {
        bail!("frame payload too large");
    }
    let len = payload.len() as u32;
    writer.write_all(&[PROTOCOL_VERSION])?;
    writer.write_all(&len.to_be_bytes()[1..])?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

pub(crate) fn read_frame(reader: &mut impl Read) -> Result<Frame> {
    let mut head = [0u8; 4];
    reader.read_exact(&mut head)?;
    if head[0] != PROTOCOL_VERSION {
        bail!("unsupported protocol version {}", head[0]);
    }
    let len = u32::from_be_bytes([0, head[1], head[2], head[3]]) as usize;
    if len > MAX_PAYLOAD {
        bail!("frame payload too large");
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}
