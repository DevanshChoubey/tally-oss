//! Shared wiring for the screens that run `ledgerly-api` calls (card W7).
//!
//! Two things every such screen needs, kept in one place so they behave the same:
//!
//! * [`unsupported_message`]: the one way an `Unsupported` outcome is shown to the user. Every
//!   screen uses it, so the same reason reads the same on every screen and never comes with a
//!   figure (rule A1).
//! * [`Latest`]: a single in-flight call per screen. Starting a new call drops the older ticket,
//!   and a result is only handed back for the call that is still wanted. So a slow earlier answer
//!   can never replace a newer one.

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
    /// The call finished.
    Ready(R),
    /// The worker stopped before the call ran. No result will come.
    Lost,
}

/// The newest call a screen has started, and the ticket it waits on.
#[derive(Debug)]
pub struct Latest<R> {
    /// The ticket of the current call, when one is running.
    ticket: Option<Ticket<R>>,
}

impl<R> Default for Latest<R> {
    fn default() -> Self {
        Self { ticket: None }
    }
}

impl<R: Send + 'static> Latest<R> {
    /// Starts `job` on the worker. Any earlier call's ticket is dropped, so its result is never
    /// returned by [`Latest::poll`].
    pub fn start<F>(&mut self, worker: &ApiWorker, job: F)
    where
        F: FnOnce(&mut ledgerly_api::Api) -> R + Send + 'static,
    {
        self.ticket = Some(worker.run(job));
    }

    /// Whether a call is running whose result is still wanted.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.ticket.is_some()
    }

    /// Checks the running call without blocking. Returns its result once.
    pub fn poll(&mut self) -> Option<Done<R>> {
        let ticket = self.ticket.as_mut()?;
        match ticket.poll() {
            Poll::Pending | Poll::Taken => None,
            Poll::Ready(result) => {
                self.ticket = None;
                Some(Done::Ready(result))
            }
            Poll::Lost => {
                self.ticket = None;
                Some(Done::Lost)
            }
        }
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
}
