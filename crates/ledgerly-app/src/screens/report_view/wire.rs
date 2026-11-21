//! Shared wiring for the screens that run `ledgerly-api` calls (card W7).
//!
//! Two things every such screen needs, kept in one place so they behave the same:
//!
//! * [`unsupported_message`]: the one way an `Unsupported` outcome is shown to the user. Every
//!   screen uses it, so the same reason reads the same on every screen and never comes with a
//!   figure (rule A1).
//! * [`Latest`]: a single in-flight call per screen. Starting a new call drops the older ticket,
//!   and a result is applied only when its sequence number is the latest one started. So a
//!   slow earlier answer can never replace a newer one (review finding U7, U12).

use ledgerly_api::Api;
use ledgerly_core::Unsupported;

use crate::worker::{ApiWorker, Poll, Ticket};

/// The words shown for an `Unsupported` outcome: what is not supported and why. Every screen
/// shows its `Unsupported` outcomes through this function.
#[must_use]
pub fn unsupported_message(what: &str, reason: &str) -> String {
    format!("Not supported yet: {what} ({reason})")
}

/// [`unsupported_message`] for an engine `Unsupported` value.
#[must_use]
pub fn unsupported_text(u: &Unsupported) -> String {
    unsupported_message(&u.what, &u.reason)
}

/// A finished call the screen should look at.
#[derive(Debug, PartialEq, Eq)]
pub enum Done<R> {
    /// The call finished. The sequence number is the one [`Latest::start`] returned.
    Ready(u64, R),
    /// The worker stopped before the call ran. No result will come.
    Lost(u64),
}

/// The newest call a screen has started, and the ticket it waits on.
#[derive(Debug)]
pub struct Latest<R> {
    /// The last sequence number handed out.
    next: u64,
    /// The sequence number of the call whose result is still wanted.
    current: u64,
    /// The ticket of the current call, when one is running.
    ticket: Option<Ticket<R>>,
}

impl<R> Clone for Latest<R> {
    /// A copy keeps the sequence numbers but not the running call: a ticket cannot be shared, so
    /// the copy's results are never shown.
    fn clone(&self) -> Self {
        Self {
            next: self.next,
            current: self.current,
            ticket: None,
        }
    }
}

impl<R> Default for Latest<R> {
    fn default() -> Self {
        Self {
            next: 0,
            current: 0,
            ticket: None,
        }
    }
}

impl<R: Send + 'static> Latest<R> {
    /// Starts `job` on the worker. Any earlier call's result is no longer wanted, and its ticket
    /// is dropped. Returns the sequence number of the new call.
    pub fn start<F>(&mut self, worker: &ApiWorker, job: F) -> u64
    where
        F: FnOnce(&mut Api) -> R + Send + 'static,
    {
        self.next = self.next.saturating_add(1);
        self.current = self.next;
        self.ticket = Some(worker.run(job));
        self.current
    }

    /// Forgets the running call, so its result is never applied.
    pub fn cancel(&mut self) {
        self.ticket = None;
        self.current = self.next;
    }

    /// Whether a call is running whose result is still wanted.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.ticket.is_some()
    }

    /// Checks the running call without blocking. Returns its result once, when it is the latest
    /// call; an older result is dropped here.
    pub fn poll(&mut self) -> Option<Done<R>> {
        let ticket = self.ticket.as_mut()?;
        let seq = self.current;
        match ticket.poll() {
            Poll::Pending | Poll::Taken => None,
            Poll::Ready(result) => {
                self.ticket = None;
                Some(Done::Ready(seq, result))
            }
            Poll::Lost => {
                self.ticket = None;
                Some(Done::Lost(seq))
            }
        }
    }

    /// Whether `seq` is the call whose result the screen still wants.
    #[must_use]
    pub fn is_current(&self, seq: u64) -> bool {
        self.current == seq
    }

    /// The sequence number of the latest call started.
    #[must_use]
    pub fn latest(&self) -> u64 {
        self.current
    }

    /// Starts `job` under a sequence number the screen already handed out with
    /// [`Latest::next_seq`], so the answer matches that request.
    pub fn start_as<F>(&mut self, seq: u64, worker: &ApiWorker, job: F)
    where
        F: FnOnce(&mut Api) -> R + Send + 'static,
    {
        self.next = self.next.max(seq);
        self.current = seq;
        self.ticket = Some(worker.run(job));
    }

    /// Sequence number for a request the screen will answer itself (no worker call), so a call
    /// started earlier is no longer the current one.
    pub fn next_seq(&mut self) -> u64 {
        self.next = self.next.saturating_add(1);
        self.current = self.next;
        self.ticket = None;
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_message_names_what_and_why() {
        // provenance: derived - the wording is the rule of this module: what, then why, in one
        // sentence; the expected string is written out here by hand.
        assert_eq!(
            unsupported_message("ageing of advances", "no rule yet"),
            "Not supported yet: ageing of advances (no rule yet)"
        );
    }

    #[test]
    fn next_seq_makes_older_requests_stale() {
        let mut latest: Latest<u8> = Latest::default();
        let first = latest.next_seq();
        let second = latest.next_seq();
        assert!(!latest.is_current(first));
        assert!(latest.is_current(second));
        assert_eq!(latest.latest(), second);
    }
}
