//! The supervisor's outgoing side. Encoded frames wait here for a slow client
//! under a strict byte cap, control frames included. Beside them waits at most
//! one masked release, zeroed on drop, framed a piece at a time as the client
//! drains. A held secret prefix can be far larger than one wire frame, so a
//! release is never encoded whole.

use super::protocol::{write_frame, Frame, ResponseFrame, WireBytes, MAX_PAYLOAD};
use super::runspec::StreamKind;
use anyhow::{bail, Result};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::Duration;
use zeroize::Zeroizing;

/// Encoded frames waiting for a slow client. Output readers stall at this cap.
const OUTGOING_CAP: usize = 256 * 1024;
// Every frame that fits the queue also fits the wire payload limit.
const _: () = assert!(OUTGOING_CAP <= MAX_PAYLOAD);
/// Smallest output piece worth its own frame while more of a release waits.
const MIN_PIECE: usize = 16 * 1024;

#[cfg(target_os = "linux")]
const SEND_FLAGS: libc::c_int = libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL;
#[cfg(not(target_os = "linux"))]
const SEND_FLAGS: libc::c_int = libc::MSG_DONTWAIT;

/// Masked bytes not yet framed. `framed` counts the bytes already queued.
struct Release {
    kind: StreamKind,
    bytes: Zeroizing<Vec<u8>>,
    framed: usize,
}

#[derive(Default)]
pub(super) struct Outbox {
    encoded: Vec<u8>,
    release: Option<Release>,
}

impl Outbox {
    /// Queue one control frame if it fits now. False leaves it with the caller.
    pub(super) fn offer(&mut self, frame: &ResponseFrame) -> Result<bool> {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &Frame::Response(frame.clone()))?;
        if bytes.len() > OUTGOING_CAP {
            bail!("a run control frame is larger than the outgoing queue");
        }
        if self.encoded.len() + bytes.len() > OUTGOING_CAP {
            return Ok(false);
        }
        self.encoded.extend_from_slice(&bytes);
        Ok(true)
    }

    /// The supervisor may read more raw output only while this holds.
    pub(super) fn accepts_output(&self) -> bool {
        self.release.is_none() && self.encoded.len() < OUTGOING_CAP
    }

    /// Take masked output. Call only while `accepts_output` holds.
    pub(super) fn release(&mut self, kind: StreamKind, masked: Vec<u8>) -> Result<()> {
        if self.release.is_some() {
            bail!("output arrived while an earlier release was still waiting");
        }
        if !masked.is_empty() {
            self.release = Some(Release {
                kind,
                bytes: Zeroizing::new(masked),
                framed: 0,
            });
        }
        self.frame_release()
    }

    pub(super) fn release_waiting(&self) -> bool {
        self.release.is_some()
    }

    pub(super) fn is_drained(&self) -> bool {
        self.encoded.is_empty() && self.release.is_none()
    }

    /// Frame what fits, then send what the socket takes without blocking.
    /// `Ok(false)` means the client is gone.
    pub(super) fn send(&mut self, stream: &UnixStream) -> Result<bool> {
        self.frame_release()?;
        Ok(self.flush(stream))
    }

    pub(super) fn wait_writable(&self, stream: &UnixStream, tick: Duration) {
        let mut poll = libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        // SAFETY: one valid pollfd for the duration of the call.
        unsafe { libc::poll(&mut poll, 1, tick.as_millis() as i32) };
    }

    fn frame_release(&mut self) -> Result<()> {
        while let Some(release) = &mut self.release {
            let overhead = envelope_len(release.kind)?;
            let room = OUTGOING_CAP.saturating_sub(self.encoded.len() + overhead);
            // Base64 turns every 3 bytes into 4, so whole groups of 3 fit exactly.
            let remaining = release.bytes.len() - release.framed;
            let piece = (room / 4 * 3).min(remaining);
            if piece == 0 || (piece < MIN_PIECE && piece < remaining) {
                return Ok(());
            }
            let data = release.bytes[release.framed..release.framed + piece].to_vec();
            let frame = ResponseFrame::RunOutput {
                stream: release.kind,
                data: WireBytes(data),
            };
            write_frame(&mut self.encoded, &Frame::Response(frame))?;
            if self.encoded.len() > OUTGOING_CAP {
                bail!("an output frame overran the outgoing queue");
            }
            release.framed += piece;
            if release.framed == release.bytes.len() {
                self.release = None;
            }
        }
        Ok(())
    }

    /// False means the client is gone.
    fn flush(&mut self, stream: &UnixStream) -> bool {
        while !self.encoded.is_empty() {
            // SAFETY: the pointer and length describe `encoded`, which outlives the call.
            let sent = unsafe {
                libc::send(
                    stream.as_raw_fd(),
                    self.encoded.as_ptr().cast(),
                    self.encoded.len(),
                    SEND_FLAGS,
                )
            };
            match sent {
                0 => return false,
                sent if sent > 0 => {
                    self.encoded.drain(..sent as usize);
                }
                _ => match std::io::Error::last_os_error().kind() {
                    std::io::ErrorKind::WouldBlock => return true,
                    std::io::ErrorKind::Interrupted => {}
                    _ => return false,
                },
            }
        }
        true
    }
}

/// Encoded size of an output frame with no data: header plus JSON envelope.
fn envelope_len(kind: StreamKind) -> Result<usize> {
    let empty = ResponseFrame::RunOutput {
        stream: kind,
        data: WireBytes::default(),
    };
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &Frame::Response(empty))?;
    Ok(bytes.len())
}
