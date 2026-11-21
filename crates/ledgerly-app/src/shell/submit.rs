//! Sends one `ledgerly-api` command to the worker and turns the answer into what a screen shows
//! (spec 7, rule D1). A screen holds the [`Ticket`] and polls it each frame; it never waits.
//!
//! A screen shows a booked change only from [`CommandReply::Booked`], which carries the engine's
//! own [`CommandResult`]. The draft on screen is never shown as saved.

use std::sync::atomic::{AtomicU64, Ordering};

use ledgerly_api::{Books, Command, CommandResult};
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

/// Counts the changes after which the shell must fetch the books again: a company opened or
/// created, and every command that saved something. Screens never read it; the shell compares it
/// with the value it last saw.
static CHANGES: AtomicU64 = AtomicU64::new(0);

/// Tells the shell that the books changed (a company was opened, or a command saved something),
/// so it fetches them again. [`send`] calls this after a saved command; code that runs a saving
/// command on the worker some other way should call it too.
pub fn note_changed() {
    CHANGES.fetch_add(1, Ordering::SeqCst);
}

/// The current change count, for the shell.
pub(crate) fn change_count() -> u64 {
    CHANGES.load(Ordering::SeqCst)
}

/// The answer to a books fetch.
#[derive(Debug)]
pub enum BooksReply {
    /// A copy of the books as the store holds them now.
    Loaded(Box<Books>),
    /// The books cannot be read in this build. The text says why. No figure is drawn from them.
    Unsupported(Unsupported),
    /// The books could not be read. The text says why.
    Failed(String),
}

/// Queues a read of the books on the worker and returns the ticket. Never blocks. The worker runs
/// jobs in order, so the copy includes every command sent before this call.
#[must_use]
pub fn fetch_books(worker: &ApiWorker) -> Ticket<BooksReply> {
    worker.run(|api| match api.books() {
        Ok(Outcome::Value(books)) => BooksReply::Loaded(Box::new(books)),
        Ok(Outcome::Unsupported(unsupported)) => BooksReply::Unsupported(unsupported),
        Err(e) => BooksReply::Failed(e.to_string()),
    })
}

/// Checks a books ticket without blocking. `None` while the worker is still reading.
pub fn settle_books(ticket: &mut Ticket<BooksReply>) -> Option<BooksReply> {
    match ticket.poll() {
        Poll::Pending | Poll::Taken => None,
        Poll::Ready(reply) => Some(reply),
        Poll::Lost => Some(BooksReply::Failed(
            "the engine stopped before it sent the books; open the company again".to_owned(),
        )),
    }
}

/// Queues `cmd` on the worker and returns the ticket for its answer. Never blocks.
///
/// A saved change also tells the shell to fetch the books again (see [`note_changed`]).
#[must_use]
pub fn send(worker: &ApiWorker, cmd: Command) -> Ticket<CommandReply> {
    worker.run(move |api| match api.execute(cmd) {
        Ok(Outcome::Value(result)) => {
            if result.receipt.is_some() {
                note_changed();
            }
            CommandReply::Booked(Box::new(result))
        }
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
