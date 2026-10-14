//! Descriptor passing on real socket pairs. Pipes prove closure: a pipe
//! reader sees end of file only after every copy of its writer is closed.

use super::fdpass::{parse_control, send_with_rights, ParsedControl, RightsViolation};
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
fn oversized_rights_close_every_descriptor() {
    for count in [3, 9] {
        let (client, daemon) = UnixStream::pair().unwrap();
        let pipes: Vec<(File, File)> = (0..count).map(|_| pipe()).collect();
        let writers: Vec<_> = pipes.iter().map(|(_, writer)| writer.as_fd()).collect();
        write_frame_with_rights(&client, &catalog(), &writers).unwrap();
        drop(writers);
        let readers: Vec<File> = pipes.into_iter().map(|(reader, _)| reader).collect();
        let received = read_frame_with_rights(&daemon).unwrap();
        assert_eq!(received.frame, catalog(), "the frame stays in sync");
        assert!(received.fds.is_empty());
        assert_eq!(received.violation, Some(RightsViolation::TooMany));
        for (index, reader) in readers.iter().enumerate() {
            assert!(
                writers_closed(reader),
                "descriptor {index} of {count} stayed open"
            );
        }
    }
}

/// macOS starts processes with a 256-descriptor soft limit, and XNU refuses
/// to install a message's rights past it. Raise it so 253 copies fit.
fn room_for_descriptors(wanted: libc::rlim_t) {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit and setrlimit read and write one valid rlimit.
    unsafe {
        assert_eq!(libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit), 0);
        if limit.rlim_cur < wanted {
            limit.rlim_cur = wanted.min(limit.rlim_max);
            assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &limit), 0);
        }
    }
}

/// 253 is Linux's `SCM_MAX_FD`. Copies of one writer keep the sender small,
/// and the reader sees end of file only after the daemon closed every copy.
#[test]
fn the_most_rights_a_kernel_sends_are_all_closed() {
    room_for_descriptors(1024);
    let (client, daemon) = UnixStream::pair().unwrap();
    let (reader, writer) = pipe();
    let copies = vec![writer.as_fd(); 253];
    write_frame_with_rights(&client, &catalog(), &copies).unwrap();
    drop(copies);
    drop(writer);
    let received = read_frame_with_rights(&daemon).unwrap();
    assert_eq!(received.frame, catalog());
    assert!(received.fds.is_empty());
    assert_eq!(received.violation, Some(RightsViolation::TooMany));
    assert!(writers_closed(&reader), "a received copy stayed open");
}

/// Control data as the kernel returns it: one rights header claiming
/// `claimed` numbers, cut to `kept` numbers. Numbers are never opened.
fn control(claimed: usize, kept: &[i32]) -> Vec<u8> {
    let size = |count: usize| (count * std::mem::size_of::<i32>()) as libc::c_uint;
    // SAFETY: CMSG_LEN and CMSG_SPACE only compute sizes.
    let (len, room) = unsafe {
        (
            libc::CMSG_LEN(size(claimed)),
            libc::CMSG_SPACE(size(kept.len())),
        )
    };
    let mut bytes = vec![0u8; room as usize];
    // SAFETY: zeroed cmsghdr is valid, and bytes holds one header.
    let mut header: libc::cmsghdr = unsafe { std::mem::zeroed() };
    header.cmsg_len = len as _;
    header.cmsg_level = libc::SOL_SOCKET;
    header.cmsg_type = libc::SCM_RIGHTS;
    // SAFETY: the header fits at the start of bytes.
    unsafe { std::ptr::write_unaligned(bytes.as_mut_ptr().cast(), header) };
    // SAFETY: CMSG_LEN only computes a size.
    let data = unsafe { libc::CMSG_LEN(0) } as usize;
    for (index, fd) in kept.iter().enumerate() {
        let at = data + index * 4;
        bytes[at..at + 4].copy_from_slice(&fd.to_ne_bytes());
    }
    bytes
}

#[test]
fn parsing_trusts_only_the_control_bytes_the_kernel_returned() {
    let whole = parse_control(&control(2, &[3, 4]));
    assert_eq!(
        whole,
        ParsedControl {
            fds: vec![3, 4],
            truncated: false,
            foreign: false
        }
    );
    let cut: Vec<i32> = (10..18).collect();
    let clipped = parse_control(&control(9, &cut));
    assert_eq!(
        clipped,
        ParsedControl {
            fds: cut,
            truncated: true,
            foreign: false
        }
    );
    let mut underflow = control(0, &[]);
    underflow[..std::mem::size_of::<libc::cmsghdr>()].fill(0);
    assert_eq!(
        parse_control(&underflow),
        ParsedControl {
            fds: vec![],
            truncated: true,
            foreign: false
        }
    );
}
