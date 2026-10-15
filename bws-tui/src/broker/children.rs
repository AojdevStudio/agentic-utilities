//! Fixed-capacity ownership of the daemon's helper children: value-input
//! readers and `bws` calls. A slot is reserved before the child exists and is
//! released only after this table reaps it, so a killed child that will not
//! exit keeps its slot and admission stays bounded. Only this table calls
//! `waitpid` for these pids, always with `WNOHANG`. No thread ever waits on a
//! child without a deadline.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Free,
    /// Held before fork or spawn. No pid yet.
    Reserved,
    /// A live owner holds this pid.
    Owned(libc::pid_t),
    /// The owner left after killing it. Reaped by the next admission.
    Retained(libc::pid_t),
}

pub(crate) struct Children {
    slots: Mutex<Vec<Slot>>,
}

impl Children {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            slots: Mutex::new(vec![Slot::Free; capacity]),
        }
    }

    /// Reap retained children that have exited, then take a free slot.
    /// `None` when every slot is busy.
    pub(crate) fn reserve(&self) -> Option<Reservation<'_>> {
        let mut slots = self.lock();
        for slot in slots.iter_mut() {
            if let Slot::Retained(pid) = *slot {
                if reap(pid).is_some() {
                    *slot = Slot::Free;
                }
            }
        }
        let index = slots.iter().position(|slot| *slot == Slot::Free)?;
        slots[index] = Slot::Reserved;
        Some(Reservation { table: self, index })
    }

    /// Slots not free. A retained child counts until it is reaped.
    #[cfg(test)]
    pub(crate) fn busy(&self) -> usize {
        self.lock()
            .iter()
            .filter(|slot| **slot != Slot::Free)
            .count()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Slot>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set(&self, index: usize, slot: Slot) {
        self.lock()[index] = slot;
    }
}

/// `waitpid` without blocking. The status once this pid is reaped. A pid the
/// kernel no longer knows counts as reaped with an unknown status, which is
/// never a clean exit, so its slot cannot leak and its output is never trusted.
fn reap(pid: libc::pid_t) -> Option<libc::c_int> {
    let mut status = 0;
    loop {
        // SAFETY: waitpid with WNOHANG on a pid this table owns.
        match unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } {
            0 => return None,
            reaped if reaped == pid => return Some(status),
            _ if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) => {}
            _ => return Some(-1),
        }
    }
}

/// A slot held before the child exists. Dropping it unused frees the slot.
pub(crate) struct Reservation<'a> {
    table: &'a Children,
    index: usize,
}

impl<'a> Reservation<'a> {
    /// The child now exists. Its owner must stop or reap it.
    pub(crate) fn started(self, pid: libc::pid_t) -> OwnedChild<'a> {
        let (table, index) = (self.table, self.index);
        std::mem::forget(self);
        table.set(index, Slot::Owned(pid));
        OwnedChild {
            table,
            index,
            pid,
            killed: false,
            status: None,
        }
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        self.table.set(self.index, Slot::Free);
    }
}

/// One live child and its slot. Drop kills it if it was not reaped and
/// leaves it retained, without waiting.
pub(crate) struct OwnedChild<'a> {
    table: &'a Children,
    index: usize,
    pid: libc::pid_t,
    killed: bool,
    status: Option<libc::c_int>,
}

impl OwnedChild<'_> {
    /// SIGKILL, at most once and only while this pid is still unreaped, so
    /// the signal can never reach a reused pid.
    pub(crate) fn kill(&mut self) {
        if self.status.is_none() && !self.killed {
            // SAFETY: an unreaped child keeps its pid, so this is our child.
            unsafe { libc::kill(self.pid, libc::SIGKILL) };
            self.killed = true;
        }
    }

    /// Reap without blocking. The raw wait status once reaped.
    pub(crate) fn try_reap(&mut self) -> Option<libc::c_int> {
        if self.status.is_none() {
            self.status = reap(self.pid);
            if self.status.is_some() {
                self.table.set(self.index, Slot::Free);
            }
        }
        self.status
    }

    /// Reap by `until`, checking every millisecond. A child still running then
    /// is killed and keeps its slot. `None` means it was not reaped.
    pub(crate) fn wait_until(&mut self, until: Instant) -> Option<libc::c_int> {
        loop {
            if let Some(status) = self.try_reap() {
                return Some(status);
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                self.kill();
                return self.try_reap();
            }
            std::thread::sleep(left.min(Duration::from_millis(1)));
        }
    }

    /// Kill, then reap by `until`.
    pub(crate) fn stop(&mut self, until: Instant) -> Option<libc::c_int> {
        self.kill();
        self.wait_until(until)
    }
}

impl Drop for OwnedChild<'_> {
    fn drop(&mut self) {
        if self.try_reap().is_none() {
            self.kill();
            if self.try_reap().is_none() {
                self.table.set(self.index, Slot::Retained(self.pid));
            }
        }
    }
}

/// True when a raw wait status is a normal exit with code 0.
pub(crate) fn exited_cleanly(status: libc::c_int) -> bool {
    libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0
}
