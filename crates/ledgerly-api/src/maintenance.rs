//! Maintenance on the company file (card W2): verify the audit chain, back up, rebuild check, and
//! the engine-only check of a voucher proposal.
//!
//! Every function here reads the company file or writes a new backup file beside it. None of them
//! posts a voucher. A refused proposal is `Err(ApiError::Engine(..))`. Input this build cannot
//! compute is `Ok(Outcome::Unsupported(..))` (rule A1).

use std::path::{Path, PathBuf};

use ledgerly_core::{Clock, Outcome};
use ledgerly_model::Voucher;
use ledgerly_store::SqliteStore;
use ledgerly_store::backup::{RebuildReport, backup_to, rebuild_projections_and_compare};
use ledgerly_store::verify::{VerifyReport, verify_chain};

use crate::command::{self, Command};
use crate::error::ApiError;
use crate::facade::Api;

/// The engine's verdict on a voucher proposal. Nothing was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoucherCheck {
    /// Number of audit events that posting the voucher would write. Computed by the engine.
    pub events_that_would_be_written: usize,
}

/// Verifies the audit chain of the company file at `path` without opening its books.
///
/// Unlike [`Api::open`] followed by [`Api::verify`], this works on a file whose events cannot be
/// decoded into books: a damaged log is reported as problems in the returned [`VerifyReport`].
///
/// # Errors
/// [`ApiError::Store`] when the file cannot be opened or read.
pub fn verify_file(path: &Path) -> Result<VerifyReport, ApiError> {
    let store = SqliteStore::open(path)?;
    Ok(verify_chain(&store)?)
}

impl Api {
    /// Verifies the audit chain and the daily checkpoints of the open company file.
    ///
    /// A damaged log is not an error: it is a report whose [`VerifyReport::is_ok`] is `false`.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the file cannot be read.
    pub fn verify(&self) -> Result<VerifyReport, ApiError> {
        Ok(verify_chain(self.store())?)
    }

    /// Writes a verified, self-contained copy of the open company file into `dir` and returns the
    /// path of the copy. `dir` is created when missing; an existing file is never overwritten.
    ///
    /// The copy's name carries the time of the injected clock in the store. This method uses the
    /// system clock for that name, because the facade does not expose the clock set by
    /// [`Api::set_clock`] to this module. Tests check the directory contents, not the name.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the copy cannot be written or does not verify. Nothing but a
    /// `.partial` file, removed on a best-effort basis, is left behind.
    pub fn backup_to(&self, dir: &Path) -> Result<PathBuf, ApiError> {
        Ok(backup_to(self.store(), dir, Clock::System)?)
    }

    /// Replays the event log and compares the result with the stored projection tables.
    ///
    /// The report is a match only when the stored projections were compared and equal the rebuild
    /// ([`RebuildReport::is_match`]). A log with empty projection tables is reported as
    /// `RebuildOutcome::NotMaintained`, which is not a match.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the log cannot be read or replayed. A voucher this build cannot
    /// project is named in the error (`StoreError::Unsupported`).
    pub fn rebuild_check(&self) -> Result<RebuildReport, ApiError> {
        Ok(rebuild_projections_and_compare(self.store())?)
    }

    /// Checks a voucher proposal against the books with the engine, without posting it.
    ///
    /// The engine runs the same command it would run for a posting and reports how many events
    /// the posting would write. Nothing is committed, and the cache is not changed.
    ///
    /// # Errors
    /// - [`ApiError::Engine`] when the engine refuses the voucher. The message names the reason.
    /// - [`ApiError::Stale`] after a saved change whose reload failed.
    pub fn check_voucher(&self, voucher: Voucher) -> Result<Outcome<VoucherCheck>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
        };
        let events = match command::run(Command::PostVoucher(voucher), books, self.lock())? {
            Outcome::Value(events) => events,
            Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
        };
        Ok(Outcome::Value(VoucherCheck {
            events_that_would_be_written: events.len(),
        }))
    }
}
