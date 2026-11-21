//! Import wizard. **Card U10** implements this module.
//!
//! Frozen interface (plan card U10):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `Import`: [`ImportScreen`] is the wizard of spec 6.7 with five steps, in order:
//! detect Tally, choose a company, pull (progress and cancel), map, and the reconciliation report.
//!
//! The screen does no I/O and starts no thread (rule D1). It is a state machine
//! ([`ImportWizard`]) that the host drives:
//!
//! * the wizard records what it wants done as [`ImportRequest`] values, which the host takes with
//!   [`ImportWizard::take_requests`] and runs on a worker thread (detect, pull, map, commit,
//!   reconcile; the bridge crate and the engine do the work);
//! * the host answers through the reply methods ([`ImportWizard::detect_result`],
//!   [`ImportWizard::pull_progress`], [`ImportWizard::map_ready`], [`ImportWizard::recon_ready`]
//!   and so on). A reply that does not fit the current step is refused with
//!   [`ImportError::WrongStep`], so a late answer cannot move the wizard.
//!
//! The wizard owns no numbers of its own (rule C4): every amount in the reconciliation report is
//! handed in by the host (our figure from the reports crate, Tally's from its own exported
//! report) and only compared for equality and formatted here. Nothing is subtracted, so a
//! difference is shown as the two figures, never as a computed delta.
//!
//! What spec 6.7 asks of this screen, and where it lives:
//!
//! * Detect Tally on `localhost:9000` and list companies ([`ImportWizard::start_detect`],
//!   [`TALLY_PORT`]); the offline path reads an exported XML file through the same parser
//!   ([`ImportSource::Export`]).
//! * Pull masters in dependency order, then vouchers in batches, with progress and cancel
//!   ([`PullStage`], [`PullProgress`], [`ImportWizard::cancel`]). A cancelled pull leaves nothing
//!   imported: the wizard does not advance and the raw capture stays on disk.
//! * Map: every Tally item with its status; an item with no mapping blocks the import, a warning
//!   (for example an unknown field kept opaque) does not ([`MapReport`]).
//! * Reconciliation: our figures against Tally's own for TB, P&L, BS, stock, outstandings and GST
//!   ([`ReconReport`]). Differences are listed and counted, never hidden; a section that could not
//!   be compared says so ([`ReconReport::summary`]).
//!
//! Not built here, and said so rather than guessed: AI column mapping (spec 6.8), a pull period
//! that is not a run of whole financial years, and the live two-way sync (v1.1).

mod types;
mod view;
mod wizard;

pub use types::*;
pub use view::{ImportScreen, format_signed_amount};
pub use wizard::ImportWizard;
