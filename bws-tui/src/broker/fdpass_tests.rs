//! Descriptor passing on real socket pairs. Pipes prove closure: a pipe
//! reader sees end of file only after every copy of its writer is closed.

use super::fdpass::{send_with_rights, RightsViolation};
use super::protocol::{read_frame_with_rights, write_frame, write_frame_with_rights};
use super::{Frame, RequestFrame};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd};
use std::os::unix::net::UnixStream;

pub(super) fn pipe() -> (File, File) {
    let mut fds = [0i32; 2];
    // SAFETY: pipe writes two new descriptors into a valid array.
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    // SAFETY: both descriptors are new and owned only here.
    unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
}

/// True when every writer of this pipe is closed. Waits up to two seconds.
pub(super) fn writers_closed(reader: &File) -> bool {
    let mut poll = libc::pollfd {
        fd: reader.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd.
    let ready = unsafe { libc::poll(&mut poll, 1, 2000) };
    let mut byte = [0u8; 1];
    ready == 1 && (&*reader).read(&mut byte).ok() == Some(0)
}

fn catalog() -> Frame {
    Frame::Request(RequestFrame::Catalog { project: None })
}

#[test]
fn one_descriptor_rides_the_first_byte_with_close_on_exec() {
    let (client, daemon) = UnixStream::pair().unwrap();
    let (mut reader, writer) = pipe();
    write_frame_with_rights(&client, &catalog(), &[writer.as_fd()]).unwrap();
    drop(writer);
    let received = read_frame_with_rights(&daemon).unwrap();
    assert_eq!(received.frame, catalog());
    assert_eq!(received.violation, None);
    assert_eq!(received.fds.len(), 1);
    // SAFETY: F_GETFD reads flags of a descriptor this test owns.
    let flags = unsafe { libc::fcntl(received.fds[0].as_raw_fd(), libc::F_GETFD) };
    assert_eq!(flags & libc::FD_CLOEXEC, libc::FD_CLOEXEC);
    let mut passed = File::from(received.fds.into_iter().next().unwrap());
    passed.write_all(b"through").unwrap();
    drop(passed);
    let mut text = String::new();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text, "through");
}

#[test]
fn plain_frames_read_without_descriptors() {
    let (mut client, daemon) = UnixStream::pair().unwrap();
    write_frame(&mut client, &catalog()).unwrap();
    let received = read_frame_with_rights(&daemon).unwrap();
    assert_eq!(received.frame, catalog());
    assert!(received.fds.is_empty());
    assert_eq!(received.violation, None);
}

#[test]
fn rights_after_the_first_byte_are_closed_and_reported() {
    let (mut client, daemon) = UnixStream::pair().unwrap();
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &catalog()).unwrap();
    let (reader, writer) = pipe();
    client.write_all(&bytes[..1]).unwrap();
    send_with_rights(client.as_fd(), &bytes[1..], &[writer.as_fd()]).unwrap();
    drop(writer);
    let received = read_frame_with_rights(&daemon).unwrap();
    assert_eq!(received.frame, catalog(), "the frame stays in sync");
    assert!(received.fds.is_empty());
    assert_eq!(received.violation, Some(RightsViolation::LateRights));
    assert!(writers_closed(&reader), "late descriptor was closed");
}

#[test]
fn truncated_rights_close_every_descriptor() {
    let (client, daemon) = UnixStream::pair().unwrap();
    let pipes: Vec<(File, File)> = (0..9).map(|_| pipe()).collect();
    let writers: Vec<_> = pipes.iter().map(|(_, writer)| writer.as_fd()).collect();
    write_frame_with_rights(&client, &catalog(), &writers).unwrap();
    drop(writers);
    let readers: Vec<File> = pipes.into_iter().map(|(reader, _)| reader).collect();
    let received = read_frame_with_rights(&daemon).unwrap();
    assert!(received.fds.is_empty());
    assert_eq!(received.violation, Some(RightsViolation::Truncated));
    for (index, reader) in readers.iter().enumerate() {
        assert!(writers_closed(reader), "descriptor {index} stayed open");
    }
}
