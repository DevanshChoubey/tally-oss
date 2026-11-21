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
//! The mapping numbers voucher types from 1, so `align_voucher_types` gives each one the id of
//! the default type of the same name, when the behaviour agrees, and a new id after 18 otherwise.
//! Vouchers are re-pointed to the aligned ids. A Tally voucher type named like a default type but
//! with another behaviour is refused, never merged.
//!
//! # Refusals
//!
//! Anything the mapping cannot represent faithfully (inventory lines, an unknown base voucher
//! kind, an opening balance that is not a plain amount of at most two decimals) makes the whole
//! import [`Outcome::Unsupported`], naming the first object. Nothing is half imported. A Tally
//! export that does not parse is an [`ImportError`].
//!
//! # Opening balances (card C7)
//!
//! A ledger's non-zero `OPENINGBALANCE` is imported as the ledger's opening balance. The sign
//! follows Tally's own: a negative amount is a debit and a positive amount is a credit. Sources
//! (read 2026-11-21): Tally's developer reference, Case Study I, imports the bank ledger `ICICI`
//! with `<OPENINGBALANCE>-12500</OPENINGBALANCE>`, a debit balance on a bank account
//! (<https://help.tallysolutions.com/article/DeveloperReference/integration-capabilities/case_study_1.htm>),
//! and the same page's trial balance sample shows a debit as a negative number. Tally's import
//! FAQ states the same for voucher amounts: "negative ... imported as debit" and "positive ...
//! imported as credit" (<https://help.tallysolutions.com/import-data-faq/>). One other Tally
//! sample shows a positive `13500` on a bank ledger without saying which side it is
//! (<https://help.tallysolutions.com/understanding-tally-xml-tags/>); no Tally page states the
//! rule for `OPENINGBALANCE` outright, so the convention is `derived` and an observed capture is
//! pending. The mapping module still refuses a non-zero opening; this module reads the openings
//! first, hands the mapping zero, and sets the opening on the created ledger afterwards.
//!
//! Openings need not balance. Tally shows any imbalance as "Difference in opening balances"
//! in the Trial Balance (<https://help.tallysolutions.com/trial-balance-tally/>), and so does
//! the import: it warns with the difference and does not invent a balancing ledger.
//!
//! # Vouchers outside the period (card C7)
//!
//! After the pull, the import asks Tally for vouchers dated before the books' beginning and after
//! the end of the period (up to 31 December 2099) and warns with the count and dates of any it
//! finds. They are not imported. A check Tally refuses is itself a warning, never silence.
//!
//! # Period
//!
//! Vouchers are pulled for the period [`import_range`] returns: from the books' beginning to the
//! end of the financial year that holds today. Vouchers dated outside that period are not in the
//! import, and the caller shows the period so the user can see it.

use std::collections::BTreeMap;
use std::path::Path;

use std::str::FromStr;

use ledgerly_core::{
    CancelToken, Date, Decimal, DrCr, Month, Outcome, Paise, Period, RoundingRule, SignedAmount,
    Unsupported, VoucherTypeId, round_rupees_to_paise,
};
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{MasterSnapshot, VoucherType};
use ledgerly_store::Event;
use ledgerly_tally_xml::masters::TallyMaster;
use ledgerly_tally_xml::request::try_vouchers_request;
use ledgerly_tally_xml::vouchers::parse_vouchers_xml;
use thiserror::Error;

use crate::TallyTransport;
use crate::mapping::{MapError, map_with_warnings};
use crate::pull::{PullError, PullResult, pull};

/// First day asked for when looking for vouchers before the books' beginning.
const PROBE_EARLIEST: (i32, Month, u8) = (1900, Month::January, 1);
/// Last day asked for when looking for vouchers after the end of the period.
const PROBE_LATEST: (i32, Month, u8) = (2099, Month::December, 31);

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
    let (masters, openings) = match split_openings(&pulled.masters) {
        Ok(split) => split,
        Err(refusal) => return Ok(Outcome::Unsupported(refusal)),
    };
    let mut mapped = match map_with_warnings(&masters, &pulled.vouchers) {
        Ok(mapped) => mapped,
        Err(MapError::Unsupported { what, reason }) => {
            return Ok(Outcome::Unsupported(Unsupported {
                what: format!("Tally import: {what}"),
                reason: format!("{reason}. Nothing was imported."),
            }));
        }
        Err(other) => return Err(other.into()),
    };
    let opening_warnings = match apply_openings(&mut mapped.events, openings) {
        Ok(found) => found,
        Err(refusal) => return Ok(Outcome::Unsupported(refusal)),
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
    warnings.extend(opening_warnings);
    warnings.extend(probe_outside_period(
        t,
        company,
        range,
        capture_dir,
        cancel,
    )?);
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

/// Opening balances read from the ledgers, by ledger name, in document order.
type Openings = Vec<(String, SignedAmount)>;

/// Reads each ledger's non-zero `OPENINGBALANCE` and returns the masters with those openings
/// replaced by `0`, so the mapping accepts them, together with the openings.
///
/// A ledger whose opening is absent, empty or not a plain decimal is passed on unchanged: the
/// mapping refuses it with its own reason.
///
/// # Errors
/// [`Unsupported`] for an opening with more than two decimal places or too large for paise.
fn split_openings(masters: &[TallyMaster]) -> Result<(Vec<TallyMaster>, Openings), Unsupported> {
    let mut out = masters.to_vec();
    let mut openings = Openings::new();
    for master in &mut out {
        let TallyMaster::Ledger(rec) = master else {
            continue;
        };
        let Some(element) = rec.extra.iter_mut().find(|e| e.name == "OPENINGBALANCE") else {
            continue;
        };
        let text = element.text.trim();
        let Ok(value) = Decimal::from_str(text) else {
            continue;
        };
        if value.is_zero() {
            continue;
        }
        let refuse = |reason: &str| Unsupported {
            what: format!(
                "Tally import: ledger '{}' has an opening balance of {text}",
                rec.name
            ),
            reason: format!("{reason}. Nothing was imported."),
        };
        if value.normalize().scale() > 2 {
            return Err(refuse("it has more than two decimal places"));
        }
        let amount = round_rupees_to_paise(value.abs(), RoundingRule::Truncate)
            .map_err(|_| refuse("it is too large"))?;
        // Tally: negative is a debit, positive is a credit (see the module documentation).
        let side = if value.is_sign_negative() {
            DrCr::Dr
        } else {
            DrCr::Cr
        };
        openings.push((rec.name.clone(), SignedAmount { amount, side }));
        "0".clone_into(&mut element.text);
        element.children.clear();
    }
    Ok((out, openings))
}

/// Sets each opening on the ledger the mapping created for it, and returns the warnings: an
/// opening whose ledger was not imported, and the "Difference in opening balances".
///
/// # Errors
/// [`Unsupported`] when the openings add up to more than paise can hold.
fn apply_openings(
    events: &mut [Event],
    openings: Openings,
) -> Result<Vec<ImportWarning>, Unsupported> {
    let mut warnings = Vec::new();
    let mut net = Paise::ZERO;
    for (name, opening) in openings {
        let mut applied = false;
        for event in events.iter_mut() {
            if let Event::MasterCreated(MasterSnapshot::Ledger(ledger)) = event
                && ledger.name == name
            {
                ledger.opening = opening;
                applied = true;
            }
        }
        if !applied {
            warnings.push(ImportWarning {
                place: format!("ledger '{name}'"),
                message: "its opening balance was not imported because the ledger was not \
                          imported"
                    .to_owned(),
            });
            continue;
        }
        let overflow = || Unsupported {
            what: "Tally import: the opening balances".to_owned(),
            reason: "they add up to more than Ledgerly can hold. Nothing was imported.".to_owned(),
        };
        let signed = opening.to_signed().map_err(|_| overflow())?;
        net = net.checked_add(signed).map_err(|_| overflow())?;
    }
    if !net.is_zero() {
        let side = if net.is_negative() { "credit" } else { "debit" };
        let shown = net.get().unsigned_abs();
        warnings.push(ImportWarning {
            place: "opening balances".to_owned(),
            message: format!(
                "the imported opening balances do not balance: the difference is {}.{:02} {side} \
                 (Tally shows this as 'Difference in opening balances' in the Trial Balance)",
                shown / 100,
                shown % 100
            ),
        });
    }
    Ok(warnings)
}

/// Asks Tally for vouchers dated before the period and after it, and warns about any found.
fn probe_outside_period(
    t: &dyn TallyTransport,
    company: &str,
    range: Period,
    capture_dir: &Path,
    cancel: &CancelToken,
) -> Result<Vec<ImportWarning>, ImportError> {
    let (ey, em, ed) = PROBE_EARLIEST;
    let (ly, lm, ld) = PROBE_LATEST;
    let earliest = Date::from_calendar_date(ey, em, ed).map_err(|_| ImportError::DateRange)?;
    let latest = Date::from_calendar_date(ly, lm, ld).map_err(|_| ImportError::DateRange)?;
    let mut windows: Vec<(&'static str, Date, Date)> = Vec::new();
    if let Some(day_before) = range.from().previous_day()
        && earliest <= day_before
    {
        windows.push(("before", earliest, day_before));
    }
    if let Some(day_after) = range.to().next_day()
        && day_after <= latest
    {
        windows.push(("after", day_after, latest));
    }
    let mut warnings = Vec::new();
    for (which, from, to) in windows {
        if cancel.is_cancelled() {
            return Err(PullError::Cancelled {
                master_files_done: 0,
                batches_done: 0,
            }
            .into());
        }
        let file = format!("vouchers-outside-{which}.xml");
        let request =
            try_vouchers_request(company, from, to).map_err(|error| PullError::Vouchers {
                file: file.clone(),
                error,
            })?;
        let raw = t.post(&request).map_err(PullError::from)?;
        let path = capture_dir.join(&file);
        std::fs::write(&path, &raw).map_err(|e| PullError::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        warnings.push(outside_warning(which, from, to, range, &file, &raw)?);
    }
    Ok(warnings.into_iter().flatten().collect())
}

/// The warning for one probe response, or `None` when Tally holds nothing there.
fn outside_warning(
    which: &str,
    from: Date,
    to: Date,
    range: Period,
    file: &str,
    raw: &str,
) -> Result<Option<ImportWarning>, ImportError> {
    let place = format!("vouchers dated {which} the import period");
    let header = raw.split_once("<BODY").map_or(raw, |(head, _)| head);
    if !raw.contains("<ENVELOPE") || header.contains("<STATUS>0</STATUS>") {
        return Ok(Some(ImportWarning {
            place,
            message: format!(
                "Ledgerly could not check whether Tally holds vouchers from {from} to {to}: Tally \
                 did not answer with data (see {file}). Vouchers outside {} to {} are not imported",
                range.from(),
                range.to()
            ),
        }));
    }
    let (vouchers, _) = parse_vouchers_xml(raw).map_err(|error| PullError::Vouchers {
        file: file.to_owned(),
        error,
    })?;
    let (Some(first), Some(last)) = (
        vouchers.iter().map(|v| v.date).min(),
        vouchers.iter().map(|v| v.date).max(),
    ) else {
        return Ok(None);
    };
    Ok(Some(ImportWarning {
        place,
        message: format!(
            "{} voucher(s) dated {first} to {last} are outside the import period {} to {} and \
             were not imported",
            vouchers.len(),
            range.from(),
            range.to()
        ),
    }))
}
