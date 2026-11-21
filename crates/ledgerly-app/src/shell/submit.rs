//! Sends one `ledgerly-api` command to the worker and turns the answer into what a screen shows
//! (spec 7, rule D1). A screen holds the [`Ticket`] and polls it each frame; it never waits.
//!
//! A screen shows a booked change only from [`CommandReply::Booked`], which carries the engine's
//! own [`CommandResult`]. The draft on screen is never shown as saved.

use ledgerly_api::{Command, CommandResult};
use ledgerly_core::{Outcome, Unsupported};

use crate::worker::{ApiWorker, Poll, Ticket};

/// The answer to one command, as a screen needs it.
#[derive(Debug)]
pub enum CommandReply {
    /// The engine computed the change and the store saved it.
    Booked(Box<CommandResult>),
    /// The engine does not handle this case yet. Nothing was saved.
    Unsupported(Unsupported),
    /// The engine refused the change, or the store failed. Nothing was saved. The text says why.
    Failed(String),
}

/// Queues `cmd` on the worker and returns the ticket for its answer. Never blocks.
#[must_use]
pub fn send(worker: &ApiWorker, cmd: Command) -> Ticket<CommandReply> {
    worker.run(move |api| match api.execute(cmd) {
        Ok(Outcome::Value(result)) => CommandReply::Booked(Box::new(result)),
        Ok(Outcome::Unsupported(unsupported)) => CommandReply::Unsupported(unsupported),
        Err(e) => CommandReply::Failed(e.to_string()),
    })
}

/// Checks a ticket without blocking. `None` while the command is still running.
///
/// A ticket whose worker stopped gives a [`CommandReply::Failed`], because nothing is known to be
/// saved and the user must check the books before saving again.
pub fn settle(ticket: &mut Ticket<CommandReply>) -> Option<CommandReply> {
    match ticket.poll() {
        Poll::Pending | Poll::Taken => None,
        Poll::Ready(reply) => Some(reply),
        Poll::Lost => Some(CommandReply::Failed(
            "the engine stopped before it answered; check the books before saving again".to_owned(),
        )),
    }
}
