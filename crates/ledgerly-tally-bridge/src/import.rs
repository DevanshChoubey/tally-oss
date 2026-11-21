//! Import from a running Tally, end to end (spec 6.7 steps 2 to 4). **Card W3** implements this
//! module.
//!
//! [`import_company`] runs one full pull ([`crate::pull::pull`]: masters in dependency order, then
//! vouchers in windows, with every raw response captured to disk first), maps the result with
//! [`crate::mapping::map_with_warnings`], and aligns the voucher types with the company that
//! Ledgerly creates. It returns the events to commit, the warnings and the counts. It writes
//! nothing to a company file: committing the events is the caller's job (`ledgerly-api` commits
//! them as `Actor::Import`).
//!
//! # Target company
//!
//! The events assume a company that holds only what `ledgerly-api` seeds when it creates a
//! company: the 28 reserved groups (ids 1 to 28) and the 18 default voucher types (ids 1 to 18).
//! The mapping numbers voucher types from 1, so [`align_voucher_types`] gives each one the id of
//! the default type of the same name, when the behaviour agrees, and a new id after 18 otherwise.
//! Vouchers are re-pointed to the aligned ids. A Tally voucher type named like a default type but
//! with another behaviour is refused, never merged.
//!
//! # Refusals
//!
//! Anything the mapping cannot represent faithfully (inventory lines, a non-zero opening balance,
//! an unknown base voucher kind) makes the whole import [`Outcome::Unsupported`], naming the first
//! object. Nothing is half imported. A Tally export that does not parse is an [`ImportError`].
//!
//! # Period
//!
//! Vouchers are pulled for the period [`import_range`] returns: from the books' beginning to the
//! end of the financial year that holds today. Vouchers dated outside that period are not in the
//! import, and the caller shows the period so the user can see it.

use std::collections::BTreeMap;
use std::path::Path;

use ledgerly_core::{CancelToken, Date, Month, Outcome, Period, Unsupported, VoucherTypeId};
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{MasterSnapshot, VoucherType};
use ledgerly_store::Event;
use ledgerly_tally_xml::masters::TallyMaster;
use thiserror::Error;

use crate::TallyTransport;
use crate::mapping::{MapError, map_with_warnings};
use crate::pull::{PullError, PullResult, pull};

/// Something in Tally's export that was not imported, with where it was met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportWarning {
    /// Where: a capture file name with a byte offset, or an object such as `ledger 'Cash'`.
    pub place: String,
    /// What was not imported and why.
    pub message: String,
}

/// What Tally returned, by kind, before mapping.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportCounts {
    /// Masters read, by kind (`group`, `ledger`, `voucher type`, ...).
    pub masters: BTreeMap<&'static str, usize>,
    /// Vouchers read, by Tally voucher type name.
    pub vouchers: BTreeMap<String, usize>,
    /// Voucher date windows accepted (after any splitting for the voucher limit).
    pub voucher_windows: usize,
}

/// The result of a complete import, ready to commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedTally {
    /// Events in dependency order, voucher types aligned to the seeded company.
    pub events: Vec<Event>,
    /// Everything that was not imported, from the pull and from the mapping.
    pub warnings: Vec<ImportWarning>,
    /// What Tally returned.
    pub counts: ImportCounts,
}

/// Why an import did not finish. A refusal the user can act on is [`Outcome::Unsupported`]
/// instead.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImportError {
    /// The pull did not finish: transport, a response that is not XML or is refused, a parse
    /// error, or a cancel. The raw responses are in the capture directory.
    #[error(transparent)]
    Pull(#[from] PullError),
    /// A master or voucher could not be mapped, and the mapping gave a reason that is not a
    /// refusal (for example an unknown parent group).
    #[error(transparent)]
    Map(#[from] MapError),
    /// The period cannot be formed from the books' beginning and the current financial year.
    #[error(
        "the import period is outside the supported calendar, or the books begin after the end of the current financial year"
    )]
    DateRange,
}

/// The period of vouchers to pull: from `books_begin` to 31 March of the financial year that
/// holds `today` (the financial year starts on 1 April).
///
/// # Errors
/// [`ImportError::DateRange`] when the period cannot be formed.
pub fn import_range(books_begin: Date, today: Date) -> Result<Period, ImportError> {
    let year = if u8::from(today.month()) >= 4 {
        today.year().checked_add(1)
    } else {
        Some(today.year())
    }
    .ok_or(ImportError::DateRange)?;
    let through =
        Date::from_calendar_date(year, Month::March, 31).map_err(|_| ImportError::DateRange)?;
    Period::new(books_begin, through).map_err(|_| ImportError::DateRange)
}

/// Pulls `company` from Tally over `t` and maps it to events.
///
/// Raw responses go to `capture_dir` before they are read. `range` is the voucher period (see
/// [`import_range`]). Cancelling `cancel` stops the pull between requests.
///
/// # Errors
/// [`ImportError`] when the pull fails or a master or voucher cannot be mapped for a reason other
/// than an unsupported construct.
pub fn import_company(
    t: &dyn TallyTransport,
    company: &str,
    range: Period,
    capture_dir: &Path,
    cancel: &CancelToken,
) -> Result<Outcome<ImportedTally>, ImportError> {
    let _span = tracing::info_span!("tally_import").entered();
    let pulled = pull(t, company, range, capture_dir, cancel)?;
    let mapped = match map_with_warnings(&pulled.masters, &pulled.vouchers) {
        Ok(mapped) => mapped,
        Err(MapError::Unsupported { what, reason }) => {
            return Ok(Outcome::Unsupported(Unsupported {
                what: format!("Tally import: {what}"),
                reason: format!("{reason}. Nothing was imported."),
            }));
        }
        Err(other) => return Err(other.into()),
    };
    let events = match align_voucher_types(mapped.events) {
        Ok(events) => events,
        Err(refusal) => return Ok(Outcome::Unsupported(refusal)),
    };
    let mut warnings: Vec<ImportWarning> = pulled
        .warnings
        .iter()
        .map(|w| ImportWarning {
            place: w.file.clone(),
            message: format!("{} (byte {})", w.warning.message, w.warning.offset),
        })
        .collect();
    warnings.extend(mapped.warnings.into_iter().map(|w| ImportWarning {
        place: w.object,
        message: w.message,
    }));
    Ok(Outcome::Value(ImportedTally {
        events,
        warnings,
        counts: count(&pulled),
    }))
}

/// Gives every voucher type in `events` the id of the default type of the same name, or a new id
/// after the defaults, and re-points the vouchers that use it.
///
/// Assumes the target holds the default voucher types (see the module documentation). Events other
/// than voucher type creations and voucher postings pass through unchanged; the mapping does not
/// produce any other kind for a voucher type.
///
/// # Errors
/// [`Unsupported`] when a Tally voucher type has a default type's name but another behaviour, or
/// when a voucher names a voucher type that is not in the export.
fn align_voucher_types(events: Vec<Event>) -> Result<Vec<Event>, Unsupported> {
    let seeds = default_voucher_types();
    let mut last_id = seeds.iter().map(|t| t.id.0).max().unwrap_or(0);
    let mut remap: BTreeMap<VoucherTypeId, VoucherTypeId> = BTreeMap::new();
    let mut out = Vec::with_capacity(events.len());
    for event in events {
        match event {
            Event::MasterCreated(MasterSnapshot::VoucherType(vt)) => {
                let (aligned, old, new) = align_voucher_type(vt, &seeds, &mut last_id)?;
                remap.insert(old, new);
                out.push(aligned);
            }
            Event::VoucherPosted(mut voucher) => {
                let new = remap
                    .get(&voucher.type_id)
                    .copied()
                    .ok_or_else(|| Unsupported {
                        what: format!("Tally voucher dated {}", voucher.date),
                        reason: "its voucher type is not in the export".to_owned(),
                    })?;
                voucher.type_id = new;
                out.push(Event::VoucherPosted(voucher));
            }
            other @ (Event::MasterCreated(_)
            | Event::MasterAltered { .. }
            | Event::MasterDeleted(_)
            | Event::VoucherAltered { .. }
            | Event::VoucherCancelled { .. }
            | Event::VoucherDeleted { .. }
            | Event::SettingChanged { .. }
            | Event::PeriodLocked { .. }
            | Event::StatementImported(_)
            | Event::MatchConfirmed(_)
            | Event::AiProposalRecorded(_)) => out.push(other),
        }
    }
    Ok(out)
}

/// Aligns one voucher type creation. Returns the event, the id the mapping gave it and the id it
/// has in the company.
fn align_voucher_type(
    vt: VoucherType,
    seeds: &[VoucherType],
    last_id: &mut u64,
) -> Result<(Event, VoucherTypeId, VoucherTypeId), Unsupported> {
    let old = vt.id;
    if let Some(seed) = seeds.iter().find(|s| s.name == vt.name) {
        if seed.base != vt.base {
            return Err(Unsupported {
                what: format!("Tally voucher type '{}'", vt.name),
                reason: "Ledgerly has a default voucher type of this name with another behaviour, \
                         and the import cannot merge the two. Rename it in Tally and import again"
                    .to_owned(),
            });
        }
        let seed_id = seed.id;
        let before = MasterSnapshot::VoucherType(seed.clone());
        let after = MasterSnapshot::VoucherType(VoucherType { id: seed_id, ..vt });
        return Ok((Event::MasterAltered { before, after }, old, seed_id));
    }
    let next = last_id.checked_add(1).ok_or_else(|| Unsupported {
        what: format!("Tally voucher type '{}'", vt.name),
        reason: "too many voucher types to number".to_owned(),
    })?;
    *last_id = next;
    let id = VoucherTypeId(next);
    Ok((
        Event::MasterCreated(MasterSnapshot::VoucherType(VoucherType { id, ..vt })),
        old,
        id,
    ))
}

/// Counts what the pull returned, by kind.
fn count(pulled: &PullResult) -> ImportCounts {
    let mut masters: BTreeMap<&'static str, usize> = BTreeMap::new();
    for master in &pulled.masters {
        let entry = masters.entry(master_kind(master)).or_insert(0);
        *entry = entry.saturating_add(1);
    }
    let mut vouchers: BTreeMap<String, usize> = BTreeMap::new();
    for voucher in &pulled.vouchers {
        let entry = vouchers.entry(voucher.voucher_type.clone()).or_insert(0);
        *entry = entry.saturating_add(1);
    }
    ImportCounts {
        masters,
        vouchers,
        voucher_windows: pulled.batches.len(),
    }
}

/// The name of a master's kind, as shown to the user.
fn master_kind(master: &TallyMaster) -> &'static str {
    match master {
        TallyMaster::Group(_) => "group",
        TallyMaster::Ledger(_) => "ledger",
        TallyMaster::StockItem(_) => "stock item",
        TallyMaster::Unit(_) => "unit",
        TallyMaster::Godown(_) => "godown",
        TallyMaster::VoucherType(_) => "voucher type",
        TallyMaster::CostCentre(_) => "cost centre",
        TallyMaster::CostCategory(_) => "cost category",
        TallyMaster::StockGroup(_) => "stock group",
    }
}
