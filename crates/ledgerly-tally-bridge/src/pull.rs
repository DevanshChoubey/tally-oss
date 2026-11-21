//! Batched pull with raw capture. **Card B2** implements this module.
//!
//! Frozen interface (plan card B2):
//!
//! ```text
//! pub struct PullPlan { .. }
//! pub fn pull(t: &dyn TallyTransport, company: &str, range: Period, capture_dir: &Path, cancel: &CancelToken) -> Result<PullResult, BridgeError>;
//! ```
//!
//! # What a pull does (spec 6.7 steps 2 and 3)
//!
//! 1. Masters, one request per collection, in dependency order: groups, ledgers, units, stock
//!    groups, stock categories, stock items, godowns, cost categories, cost centres, voucher types.
//! 2. Vouchers, in date windows. The window starts at [`PullPlan::batch_days`] days. A window
//!    whose response holds more than [`PullPlan::max_vouchers`] vouchers (5,000 by default) is not
//!    accepted: its raw response stays in the capture directory, marked `oversize` in the
//!    manifest, and the same start date is requested again with half the days. A single day that
//!    still exceeds the limit is an error ([`PullError::BatchTooLarge`]); the limit is never
//!    exceeded silently. The window size never grows back after a split, so the number of requests
//!    is deterministic for a given company.
//!
//! # Raw capture
//!
//! Every response is written to the capture directory exactly as the transport returned it (the
//! transport has already decoded it to text, so the file is UTF-8), **before** it is checked or
//! parsed (`<name>.part`, flushed, then renamed to `<name>`). A response that fails to parse is
//! therefore still on disk, and the error names the file.
//!
//! The directory also holds `manifest.tsv`, rewritten atomically (temporary file, then rename)
//! after each collection or voucher window has been fully processed. The first line is
//! `ledgerly-capture-manifest-v1`; every other line is tab separated:
//!
//! ```text
//! masters  <file>  <master count>
//! vouchers <file>  <from YYYYMMDD>  <to YYYYMMDD>  <voucher count>
//! oversize <file>  <from YYYYMMDD>  <to YYYYMMDD>  <voucher count>
//! ```
//!
//! Cancellation is checked before every request, so it takes effect between collections and
//! between voucher windows, never inside one. After a cancel the manifest lists exactly the
//! complete files and no `.part` file is left. A file that is on disk but absent from the manifest
//! is a response whose check or parse failed (or a crash between rename and manifest update); it
//! is kept for diagnosis and is not part of the capture.
//!
//! # Not yet verified against TallyPrime
//!
//! The request envelopes follow the documented shapes in research note R8 (sections 2.2, 2.3 a
//! and b) but the collection type names and `FETCH` field lists are **pending an observed
//! capture** from a real TallyPrime. They are private to this module because card X1 (request
//! builders in `ledgerly-tally-xml`) was not built when this card was written; see the interface
//! request filed with card B2.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ledgerly_core::{CancelToken, Date, Period};
use ledgerly_tally_xml::masters::{TallyMaster, parse_masters_text};
use ledgerly_tally_xml::vouchers::{TVoucher, parse_vouchers_xml};
use ledgerly_tally_xml::{TallyXmlError, XmlWarning};
use thiserror::Error;

use crate::{TallyTransport, TransportError};

/// File name of the capture manifest inside the capture directory.
pub const MANIFEST_FILE: &str = "manifest.tsv";

/// First line of the manifest.
const MANIFEST_HEADER: &str = "ledgerly-capture-manifest-v1";

/// How a pull is batched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PullPlan {
    batch_days: u32,
    max_vouchers: usize,
}

impl PullPlan {
    /// Days in the first voucher window.
    pub const DEFAULT_BATCH_DAYS: u32 = 31;
    /// Most vouchers accepted from one request (spec 6.7 step 2).
    pub const MAX_VOUCHERS_PER_REQUEST: usize = 5_000;

    /// A plan with the given first window and voucher limit.
    ///
    /// # Errors
    /// [`PullError::InvalidPlan`] when either number is zero.
    pub fn new(batch_days: u32, max_vouchers: usize) -> Result<Self, PullError> {
        if batch_days == 0 {
            return Err(PullError::InvalidPlan {
                reason: "the first window must be at least one day".to_owned(),
            });
        }
        if max_vouchers == 0 {
            return Err(PullError::InvalidPlan {
                reason: "the voucher limit must be at least one".to_owned(),
            });
        }
        Ok(PullPlan {
            batch_days,
            max_vouchers,
        })
    }

    /// Days in the first voucher window.
    pub const fn batch_days(self) -> u32 {
        self.batch_days
    }

    /// Most vouchers accepted from one request.
    pub const fn max_vouchers(self) -> usize {
        self.max_vouchers
    }
}

impl Default for PullPlan {
    fn default() -> Self {
        PullPlan {
            batch_days: Self::DEFAULT_BATCH_DAYS,
            max_vouchers: Self::MAX_VOUCHERS_PER_REQUEST,
        }
    }
}

/// One accepted voucher window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherBatch {
    /// Dates requested (inclusive).
    pub window: Period,
    /// Raw response file name inside the capture directory.
    pub file: String,
    /// Vouchers in the response.
    pub count: usize,
}

/// A non-fatal parser warning with the capture file it came from (rule A9: nothing is dropped
/// silently).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullWarning {
    /// Raw response file name inside the capture directory.
    pub file: String,
    /// The warning; its offset is a byte offset into that file.
    pub warning: XmlWarning,
}

/// Everything a complete pull returns. A cancelled or failed pull returns no `PullResult`; its
/// raw responses stay in the capture directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullResult {
    /// Company name as requested.
    pub company: String,
    /// Voucher date range as requested.
    pub range: Period,
    /// Masters in request order (dependency order), document order within a collection.
    pub masters: Vec<TallyMaster>,
    /// Raw file name of each master collection, in request order.
    pub master_files: Vec<String>,
    /// Vouchers in date order (window order, document order within a window).
    pub vouchers: Vec<TVoucher>,
    /// The accepted voucher windows, contiguous and in date order, covering `range` exactly.
    pub batches: Vec<VoucherBatch>,
    /// Parser warnings from every file.
    pub warnings: Vec<PullWarning>,
    /// Path of the capture manifest.
    pub manifest: PathBuf,
}

/// Why a pull did not finish.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PullError {
    /// The transport failed.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The company name cannot be sent.
    #[error("invalid company name: {reason}")]
    InvalidCompany {
        /// What is wrong.
        reason: String,
    },
    /// The plan has a zero limit.
    #[error("invalid pull plan: {reason}")]
    InvalidPlan {
        /// What is wrong.
        reason: String,
    },
    /// A file in the capture directory could not be written.
    #[error("cannot write capture file {path}: {message}")]
    Io {
        /// Path involved.
        path: String,
        /// The operating system's message.
        message: String,
    },
    /// The response is not a Tally XML envelope (for example Tally's plain-text error).
    #[error(
        "Tally did not answer with XML for {file}; the raw response is saved there. Check that the company is open in Tally"
    )]
    NotAnEnvelope {
        /// Capture file holding the raw response.
        file: String,
    },
    /// Tally answered with status 0.
    #[error("Tally refused the request for {file}: {message}")]
    Refused {
        /// Capture file holding the raw response.
        file: String,
        /// Tally's `LINEERROR` text, or a note that there was none.
        message: String,
    },
    /// A master response could not be parsed.
    #[error("cannot read masters in {file}: {error}")]
    Masters {
        /// Capture file holding the raw response.
        file: String,
        /// Parser error, with a byte offset where one exists.
        error: TallyXmlError,
    },
    /// A voucher response could not be parsed.
    #[error("cannot read vouchers in {file}: {error}")]
    Vouchers {
        /// Capture file holding the raw response.
        file: String,
        /// Parser error, with a byte offset where one exists.
        error: TallyXmlError,
    },
    /// A voucher dated outside the window that was requested.
    #[error(
        "voucher {number} dated {date} came back for the window {from} to {to} in {file}; the response is not trusted"
    )]
    VoucherOutsideWindow {
        /// Capture file holding the raw response.
        file: String,
        /// Voucher number, or a note that it has none.
        number: String,
        /// Voucher date.
        date: Date,
        /// First day requested.
        from: Date,
        /// Last day requested.
        to: Date,
    },
    /// One day holds more vouchers than a request may return.
    #[error(
        "{count} vouchers on {day} exceed the limit of {limit} per request; this day cannot be split further (raw response in {file})"
    )]
    BatchTooLarge {
        /// The day.
        day: Date,
        /// Vouchers returned.
        count: usize,
        /// The limit.
        limit: usize,
        /// Capture file holding the raw response.
        file: String,
    },
    /// Calendar arithmetic left the supported date range.
    #[error("the voucher date range is outside the supported calendar")]
    DateRange,
    /// The caller cancelled; the capture directory is consistent up to the manifest.
    #[error(
        "pull cancelled after {master_files_done} master collections and {batches_done} voucher windows"
    )]
    Cancelled {
        /// Master collections fully captured.
        master_files_done: usize,
        /// Voucher windows fully captured and accepted.
        batches_done: usize,
    },
}

/// Pulls masters and vouchers for `company` with the default [`PullPlan`].
///
/// # Errors
/// [`PullError`]: transport failure, a response that is not XML or is refused, a parse failure, a
/// voucher outside its window, a day above the voucher limit, a capture file that cannot be
/// written, or [`PullError::Cancelled`] when `cancel` is set between requests.
pub fn pull(
    t: &dyn TallyTransport,
    company: &str,
    range: Period,
    capture_dir: &Path,
    cancel: &CancelToken,
) -> Result<PullResult, PullError> {
    pull_with_plan(t, company, range, capture_dir, cancel, PullPlan::default())
}

/// [`pull`] with an explicit plan (a public addition beside the frozen signature; tests use small
/// limits).
///
/// # Errors
/// As [`pull`].
pub fn pull_with_plan(
    t: &dyn TallyTransport,
    company: &str,
    range: Period,
    capture_dir: &Path,
    cancel: &CancelToken,
    plan: PullPlan,
) -> Result<PullResult, PullError> {
    validate_company(company)?;
    let mut puller = Puller {
        t,
        company,
        cancel,
        capture: Capture::open(capture_dir)?,
    };
    let mut result = PullResult {
        company: company.to_owned(),
        range,
        masters: Vec::new(),
        master_files: Vec::new(),
        vouchers: Vec::new(),
        batches: Vec::new(),
        warnings: Vec::new(),
        manifest: capture_dir.join(MANIFEST_FILE),
    };
    puller.pull_masters(&mut result)?;
    puller.pull_vouchers(range, plan, &mut result)?;
    Ok(result)
}

/// A master collection to request.
struct MasterSpec {
    /// Capture file name; the number keeps the dependency order visible on disk.
    file: &'static str,
    /// Tally collection type name (pending an observed capture).
    tally_type: &'static str,
}

/// Dependency order from spec 6.7 step 2.
const MASTER_COLLECTIONS: [MasterSpec; 10] = [
    MasterSpec {
        file: "masters-01-groups.xml",
        tally_type: "Group",
    },
    MasterSpec {
        file: "masters-02-ledgers.xml",
        tally_type: "Ledger",
    },
    MasterSpec {
        file: "masters-03-units.xml",
        tally_type: "Unit",
    },
    MasterSpec {
        file: "masters-04-stock-groups.xml",
        tally_type: "StockGroup",
    },
    MasterSpec {
        file: "masters-05-stock-categories.xml",
        tally_type: "StockCategory",
    },
    MasterSpec {
        file: "masters-06-stock-items.xml",
        tally_type: "StockItem",
    },
    MasterSpec {
        file: "masters-07-godowns.xml",
        tally_type: "Godown",
    },
    MasterSpec {
        file: "masters-08-cost-categories.xml",
        tally_type: "CostCategory",
    },
    MasterSpec {
        file: "masters-09-cost-centres.xml",
        tally_type: "CostCentre",
    },
    MasterSpec {
        file: "masters-10-voucher-types.xml",
        tally_type: "VoucherType",
    },
];

/// Fields requested for every master (R8 section 2.3 b names these; pending an observed capture).
const MASTER_FETCH: &str = "GUID, AlterID, Name, Parent";

/// Fields requested for vouchers (pending an observed capture).
const VOUCHER_FETCH: &str = "GUID, AlterID, Date, VoucherTypeName, VoucherNumber, PartyLedgerName, Narration, AllLedgerEntries, AllInventoryEntries";

/// What happened to one voucher window.
enum WindowOutcome {
    Accepted,
    TooLarge,
}

struct Puller<'a> {
    t: &'a dyn TallyTransport,
    company: &'a str,
    cancel: &'a CancelToken,
    capture: Capture,
}

impl Puller<'_> {
    /// Stops with [`PullError::Cancelled`] when cancellation was requested. Called before every
    /// request, so a cancel never lands inside a collection or window.
    fn checkpoint(&self, result: &PullResult) -> Result<(), PullError> {
        if self.cancel.is_cancelled() {
            return Err(PullError::Cancelled {
                master_files_done: result.master_files.len(),
                batches_done: result.batches.len(),
            });
        }
        Ok(())
    }

    fn pull_masters(&mut self, result: &mut PullResult) -> Result<(), PullError> {
        for spec in &MASTER_COLLECTIONS {
            self.checkpoint(result)?;
            let request = collection_request(self.company, spec.tally_type, MASTER_FETCH, None);
            let raw = self.t.post(&request)?;
            self.capture.store_raw(spec.file, &raw)?;
            check_envelope(&raw, spec.file)?;
            let parsed = parse_masters_text(&raw).map_err(|e| PullError::Masters {
                file: spec.file.to_owned(),
                error: e.error,
            })?;
            let count = parsed.masters.len();
            result
                .warnings
                .extend(parsed.warnings.into_iter().map(|warning| PullWarning {
                    file: spec.file.to_owned(),
                    warning,
                }));
            result.masters.extend(parsed.masters);
            self.capture
                .record(format!("masters\t{}\t{count}", spec.file))?;
            result.master_files.push(spec.file.to_owned());
        }
        Ok(())
    }

    fn pull_vouchers(
        &mut self,
        range: Period,
        plan: PullPlan,
        result: &mut PullResult,
    ) -> Result<(), PullError> {
        let mut cursor = range.from();
        let mut days = plan.batch_days();
        loop {
            self.checkpoint(result)?;
            let end = window_end(cursor, days, range.to());
            let window = Period::new(cursor, end).map_err(|_| PullError::DateRange)?;
            match self.fetch_window(window, plan, result)? {
                WindowOutcome::Accepted => {
                    if end >= range.to() {
                        return Ok(());
                    }
                    cursor = end.next_day().ok_or(PullError::DateRange)?;
                }
                WindowOutcome::TooLarge => {
                    // fetch_window returns TooLarge only for windows of two days or more.
                    days = (days_in(window)? / 2).max(1);
                }
            }
        }
    }

    fn fetch_window(
        &mut self,
        window: Period,
        plan: PullPlan,
        result: &mut PullResult,
    ) -> Result<WindowOutcome, PullError> {
        let (from, to) = (window.from(), window.to());
        let file = format!("vouchers-{}-{}.xml", ymd(from), ymd(to));
        let request = collection_request(self.company, "Voucher", VOUCHER_FETCH, Some((from, to)));
        let raw = self.t.post(&request)?;
        self.capture.store_raw(&file, &raw)?;
        check_envelope(&raw, &file)?;
        let (vouchers, warnings) =
            parse_vouchers_xml(&raw).map_err(|error| PullError::Vouchers {
                file: file.clone(),
                error,
            })?;
        if let Some(stray) = vouchers.iter().find(|v| !window.contains(v.date)) {
            return Err(PullError::VoucherOutsideWindow {
                file,
                number: stray
                    .number
                    .clone()
                    .unwrap_or_else(|| "(no number)".to_owned()),
                date: stray.date,
                from,
                to,
            });
        }
        let count = vouchers.len();
        let line = |kind: &str| format!("{kind}\t{file}\t{}\t{}\t{count}", ymd(from), ymd(to));
        if count > plan.max_vouchers() {
            self.capture.record(line("oversize"))?;
            if from == to {
                return Err(PullError::BatchTooLarge {
                    day: from,
                    count,
                    limit: plan.max_vouchers(),
                    file,
                });
            }
            return Ok(WindowOutcome::TooLarge);
        }
        self.capture.record(line("vouchers"))?;
        result
            .warnings
            .extend(warnings.into_iter().map(|warning| PullWarning {
                file: file.clone(),
                warning,
            }));
        result.vouchers.extend(vouchers);
        result.batches.push(VoucherBatch {
            window,
            file,
            count,
        });
        Ok(WindowOutcome::Accepted)
    }
}

/// The capture directory and its manifest.
struct Capture {
    dir: PathBuf,
    lines: Vec<String>,
}

impl Capture {
    /// Creates the directory if needed and writes an empty manifest, so a manifest left by an
    /// earlier pull never describes this one.
    fn open(dir: &Path) -> Result<Self, PullError> {
        fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
        let capture = Capture {
            dir: dir.to_path_buf(),
            lines: Vec::new(),
        };
        capture.write_manifest()?;
        Ok(capture)
    }

    /// Writes a raw response byte for byte. Nothing has looked at it yet.
    fn store_raw(&self, name: &str, raw: &str) -> Result<(), PullError> {
        write_atomic(&self.dir, name, raw)
    }

    /// Adds a line to the manifest and rewrites it atomically.
    fn record(&mut self, line: String) -> Result<(), PullError> {
        self.lines.push(line);
        self.write_manifest()
    }

    fn write_manifest(&self) -> Result<(), PullError> {
        let mut text = String::from(MANIFEST_HEADER);
        text.push('\n');
        for line in &self.lines {
            text.push_str(line);
            text.push('\n');
        }
        write_atomic(&self.dir, MANIFEST_FILE, &text)
    }
}

/// Writes `<name>.part`, flushes it to disk and renames it to `<name>`, so `<name>` is either
/// absent (or the previous version) or complete.
fn write_atomic(dir: &Path, name: &str, text: &str) -> Result<(), PullError> {
    let part = dir.join(format!("{name}.part"));
    let target = dir.join(name);
    let written = fs::File::create(&part).and_then(|mut file| {
        file.write_all(text.as_bytes())?;
        file.sync_all()
    });
    if let Err(e) = written {
        // Best effort: the write error is the one worth reporting.
        let _ = fs::remove_file(&part);
        return Err(io_error(&part, &e));
    }
    fs::rename(&part, &target).map_err(|e| io_error(&target, &e))
}

fn io_error(path: &Path, e: &std::io::Error) -> PullError {
    PullError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}

/// Refuses responses that are not Tally XML or that carry status 0 (R8 section 2.2: the response
/// header has `STATUS`, 1 = success, 0 = failure). The raw response is already on disk.
fn check_envelope(raw: &str, file: &str) -> Result<(), PullError> {
    if !raw.contains("<ENVELOPE") {
        return Err(PullError::NotAnEnvelope {
            file: file.to_owned(),
        });
    }
    let header = raw.split_once("<BODY").map_or(raw, |(head, _)| head);
    if header.contains("<STATUS>0</STATUS>") {
        let message = raw
            .split_once("<LINEERROR>")
            .and_then(|(_, rest)| rest.split_once("</LINEERROR>"))
            .map_or_else(
                || "no error text in the response".to_owned(),
                |(text, _)| text.trim().to_owned(),
            );
        return Err(PullError::Refused {
            file: file.to_owned(),
            message,
        });
    }
    Ok(())
}

fn validate_company(company: &str) -> Result<(), PullError> {
    if company.trim().is_empty() {
        return Err(PullError::InvalidCompany {
            reason: "the name is empty".to_owned(),
        });
    }
    if company.chars().any(char::is_control) {
        return Err(PullError::InvalidCompany {
            reason: "the name contains a control character".to_owned(),
        });
    }
    Ok(())
}

/// An export request for a TDL collection (R8 section 2.3 b), optionally for a date range
/// (`SVFROMDATE`/`SVTODATE`, R8 section 2.2).
fn collection_request(
    company: &str,
    tally_type: &str,
    fetch: &str,
    dates: Option<(Date, Date)>,
) -> String {
    let id = format!("Ledgerly{tally_type}");
    let range = dates.map_or_else(String::new, |(from, to)| {
        format!(
            "<SVFROMDATE>{}</SVFROMDATE><SVTODATE>{}</SVTODATE>",
            ymd(from),
            ymd(to)
        )
    });
    let company = xml_escape(company);
    format!(
        "<ENVELOPE><HEADER><VERSION>1</VERSION><TALLYREQUEST>Export</TALLYREQUEST>\
         <TYPE>Collection</TYPE><ID>{id}</ID></HEADER><BODY><DESC><STATICVARIABLES>\
         <SVCURRENTCOMPANY>{company}</SVCURRENTCOMPANY>{range}\
         <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT></STATICVARIABLES>\
         <TDL><TDLMESSAGE><COLLECTION NAME=\"{id}\"><TYPE>{tally_type}</TYPE>\
         <FETCH>{fetch}</FETCH></COLLECTION></TDLMESSAGE></TDL></DESC></BODY></ENVELOPE>"
    )
}

fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

/// `YYYYMMDD`, Tally's date format.
fn ymd(date: Date) -> String {
    format!(
        "{:04}{:02}{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// Last day of a window of `days` days starting at `start`, never past `last`.
fn window_end(start: Date, days: u32, last: Date) -> Date {
    i32::try_from(days.saturating_sub(1))
        .ok()
        .and_then(|span| start.to_julian_day().checked_add(span))
        .and_then(|julian| Date::from_julian_day(julian).ok())
        .filter(|end| *end < last)
        .unwrap_or(last)
}

/// Number of days in a window, both ends included.
fn days_in(window: Period) -> Result<u32, PullError> {
    window
        .to()
        .to_julian_day()
        .checked_sub(window.from().to_julian_day())
        .and_then(|diff| diff.checked_add(1))
        .and_then(|days| u32::try_from(days).ok())
        .ok_or(PullError::DateRange)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test code: rule B2 lets tests unwrap")]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn window_end_is_start_plus_days_minus_one() {
        // provenance: derived — a 31-day window starting 1 April ends 1 April + 30 days = 1 May
        // (April has 30 days, so day 31 of the window is 1 May).
        assert_eq!(
            window_end(date!(2026 - 04 - 01), 31, date!(2027 - 03 - 31)),
            date!(2026 - 05 - 01)
        );
        // A window of one day ends where it starts.
        assert_eq!(
            window_end(date!(2026 - 04 - 01), 1, date!(2027 - 03 - 31)),
            date!(2026 - 04 - 01)
        );
    }

    #[test]
    fn window_end_is_clamped_to_the_last_day() {
        // provenance: derived — 31 days from 20 April would reach 20 May, past the range end.
        assert_eq!(
            window_end(date!(2026 - 04 - 20), 31, date!(2026 - 04 - 30)),
            date!(2026 - 04 - 30)
        );
        // A huge window cannot overflow.
        assert_eq!(
            window_end(date!(2026 - 04 - 20), u32::MAX, date!(2026 - 04 - 30)),
            date!(2026 - 04 - 30)
        );
    }

    #[test]
    fn days_in_counts_both_ends() {
        // provenance: derived — 1 to 30 April inclusive is 30 days; one day is 1.
        let april = Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap();
        assert_eq!(days_in(april).unwrap(), 30);
        let one = Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 01)).unwrap();
        assert_eq!(days_in(one).unwrap(), 1);
    }

    #[test]
    fn ymd_pads_month_and_day() {
        // provenance: derived — Tally's date format is YYYYMMDD (R8 section 2.3 a: 20260401).
        assert_eq!(ymd(date!(2026 - 04 - 01)), "20260401");
        assert_eq!(ymd(date!(2027 - 12 - 31)), "20271231");
    }

    #[test]
    fn xml_escape_covers_the_five_predefined_entities() {
        // provenance: derived — XML 1.0 section 4.6 predefined entities.
        assert_eq!(
            xml_escape("A&B <\"C\"> 'D'"),
            "A&amp;B &lt;&quot;C&quot;&gt; &apos;D&apos;"
        );
    }

    #[test]
    fn plan_rejects_zero() {
        assert!(matches!(
            PullPlan::new(0, 5),
            Err(PullError::InvalidPlan { .. })
        ));
        assert!(matches!(
            PullPlan::new(5, 0),
            Err(PullError::InvalidPlan { .. })
        ));
        let d = PullPlan::default();
        // provenance: derived — spec 6.7 step 2: at most 5,000 vouchers per request.
        assert_eq!(d.max_vouchers(), 5_000);
    }

    #[test]
    fn check_envelope_reads_status_only_from_the_header() {
        let refused = "<ENVELOPE><HEADER><STATUS>0</STATUS></HEADER><BODY><LINEERROR> no company </LINEERROR></BODY></ENVELOPE>";
        assert_eq!(
            check_envelope(refused, "f.xml"),
            Err(PullError::Refused {
                file: "f.xml".to_owned(),
                message: "no company".to_owned()
            })
        );
        // A STATUS 0 inside the body (some other field) is not a refusal.
        let body_status = "<ENVELOPE><HEADER><STATUS>1</STATUS></HEADER><BODY><STATUS>0</STATUS></BODY></ENVELOPE>";
        assert_eq!(check_envelope(body_status, "f.xml"), Ok(()));
        assert!(matches!(
            check_envelope("Unknown Request", "f.xml"),
            Err(PullError::NotAnEnvelope { .. })
        ));
    }
}
