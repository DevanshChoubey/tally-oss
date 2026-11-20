//! Runner and ratchet. **Card Q1** implements this module.
//!
//! Frozen interface (plan card Q1):
//!
//! ```text
//! pub fn run_all(cases_dir: &Path) -> Result<RunReport, ConformanceError>;
//! pub fn check_ratchet(report: &RunReport, pass_list: &str) -> Result<(), Vec<String>>;
//! ```
//!
//! [`run_all`] reads every `*.toml` file directly inside a directory (see [`crate::case`] for the
//! format) and gives each case one of three statuses:
//!
//! - [`CaseStatus::Pending`]: the case has no usable provenance (engineering rule A5). Its check
//!   is **not run**, so a pending case can never count as passing.
//! - [`CaseStatus::Passed`] or [`CaseStatus::Failed`]: the case has provenance and its check ran.
//!   A case with provenance but no check fails: it claims a verified value and verifies nothing.
//!
//! [`check_ratchet`] implements rule A6: every id on the pass-list must still pass.

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ledgerly_core::{Decimal, Paise, gstin_check_character};
use ledgerly_model::{Books, Posting, VoucherStatus};
use thiserror::Error;

use crate::case::{Case, Check, parse_cases};
use crate::fixtures::try_books_small;

/// Why a case directory could not be run at all (as opposed to a case failing).
#[derive(Debug, Error)]
pub enum ConformanceError {
    /// A directory or file could not be read.
    #[error("cannot read {}: {source}", .path.display())]
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// A case file is not valid: bad TOML, an unknown key or an unknown check kind.
    #[error("cannot parse {}: {source}", .path.display())]
    Parse {
        /// The case file.
        path: PathBuf,
        /// The parser error, which names the line.
        #[source]
        source: Box<toml::de::Error>,
    },
    /// Two cases have the same id.
    #[error("case id '{id}' is used in both {} and {}", .first.display(), .second.display())]
    DuplicateId {
        /// The repeated id.
        id: String,
        /// File of the first use.
        first: PathBuf,
        /// File of the second use.
        second: PathBuf,
    },
    /// A case id is empty or contains whitespace.
    #[error("case id {id:?} in {} must be non-empty and contain no whitespace", .path.display())]
    BadId {
        /// The id as written.
        id: String,
        /// The case file.
        path: PathBuf,
    },
}

/// The outcome of one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseStatus {
    /// Provenance recorded, check ran, value matched.
    Passed,
    /// Provenance recorded and the check ran or could not run, and the value did not match.
    Failed {
        /// What differed, with both values.
        detail: String,
    },
    /// No usable provenance. The check was not run.
    Pending {
        /// Why the provenance does not count.
        reason: String,
    },
}

/// One case and its outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    /// The case id.
    pub id: String,
    /// The case description.
    pub description: String,
    /// The outcome.
    pub status: CaseStatus,
}

/// The outcome of a whole directory of cases, ordered by case id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunReport {
    /// One result per case, sorted by id.
    pub results: Vec<CaseResult>,
}

impl RunReport {
    /// The status of the case with this id, if the suite has it.
    pub fn status_of(&self, id: &str) -> Option<&CaseStatus> {
        self.results.iter().find(|r| r.id == id).map(|r| &r.status)
    }

    /// Number of cases that passed.
    pub fn passed_count(&self) -> usize {
        self.counts().0
    }

    /// Number of cases that failed.
    pub fn failed_count(&self) -> usize {
        self.counts().1
    }

    /// Number of cases that are pending.
    pub fn pending_count(&self) -> usize {
        self.counts().2
    }

    /// `(passed, failed, pending)`.
    fn counts(&self) -> (usize, usize, usize) {
        let (mut passed, mut failed, mut pending) = (0usize, 0usize, 0usize);
        for r in &self.results {
            match r.status {
                CaseStatus::Passed => passed = passed.saturating_add(1),
                CaseStatus::Failed { .. } => failed = failed.saturating_add(1),
                CaseStatus::Pending { .. } => pending = pending.saturating_add(1),
            }
        }
        (passed, failed, pending)
    }

    /// The ids of the passing cases in pass-list format: one per line, sorted.
    ///
    /// A reviewer pastes this into `pass-list.txt` to ratchet newly passing cases.
    pub fn pass_list_text(&self) -> String {
        self.results
            .iter()
            .filter(|r| r.status == CaseStatus::Passed)
            .fold(String::new(), |mut out, r| {
                out.push_str(&r.id);
                out.push('\n');
                out
            })
    }
}

/// Reads and runs every `*.toml` case file directly inside `cases_dir`.
///
/// # Errors
/// [`ConformanceError`] when the directory or a file cannot be read, a file does not parse, an id
/// is malformed or an id is used twice. A failing case is not an error: it is a
/// [`CaseStatus::Failed`] in the report.
pub fn run_all(cases_dir: &Path) -> Result<RunReport, ConformanceError> {
    let mut cases: Vec<Case> = Vec::new();
    let mut origin: BTreeMap<String, PathBuf> = BTreeMap::new();
    for path in case_files(cases_dir)? {
        let text = fs::read_to_string(&path).map_err(|source| ConformanceError::Io {
            path: path.clone(),
            source,
        })?;
        let parsed = parse_cases(&text).map_err(|source| ConformanceError::Parse {
            path: path.clone(),
            source: Box::new(source),
        })?;
        for case in parsed {
            if case.id.is_empty() || case.id.chars().any(char::is_whitespace) {
                return Err(ConformanceError::BadId { id: case.id, path });
            }
            if let Some(first) = origin.get(&case.id) {
                return Err(ConformanceError::DuplicateId {
                    id: case.id,
                    first: first.clone(),
                    second: path,
                });
            }
            origin.insert(case.id.clone(), path.clone());
            cases.push(case);
        }
    }
    let fixture = OnceCell::new();
    let mut results: Vec<CaseResult> = cases
        .iter()
        .map(|case| CaseResult {
            id: case.id.clone(),
            description: case.description.clone(),
            status: evaluate(case, &fixture),
        })
        .collect();
    results.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(RunReport { results })
}

/// The `*.toml` files directly in `dir`, sorted by path.
fn case_files(dir: &Path) -> Result<Vec<PathBuf>, ConformanceError> {
    let io_error = |source| ConformanceError::Io {
        path: dir.to_path_buf(),
        source,
    };
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(io_error)? {
        let path = entry.map_err(io_error)?.path();
        if path.extension().and_then(std::ffi::OsStr::to_str) == Some("toml") && path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// The fixture company, built at most once per run. The error is kept as text.
type Fixture = OnceCell<Result<Books, String>>;

fn evaluate(case: &Case, fixture: &Fixture) -> CaseStatus {
    if let Some(reason) = case.provenance.pending_reason() {
        return CaseStatus::Pending { reason };
    }
    let Some(check) = &case.check else {
        return CaseStatus::Failed {
            detail: "provenance is recorded but the case has no check".to_owned(),
        };
    };
    match run_check(check, fixture) {
        Ok(()) => CaseStatus::Passed,
        Err(detail) => CaseStatus::Failed { detail },
    }
}

fn run_check(check: &Check, fixture: &Fixture) -> Result<(), String> {
    match check {
        Check::GstinCheckCharacter { first14, expected } => gstin_check(first14, *expected),
        Check::FixturePostingsBalance => with_books(fixture, postings_balance),
        Check::FixtureLedgerNet { ledger, net_paise } => {
            with_books(fixture, |books| ledger_net(books, ledger, *net_paise))
        }
        Check::FixtureStockQty { item, closing_qty } => {
            with_books(fixture, |books| stock_qty(books, item, *closing_qty))
        }
    }
}

fn with_books(
    fixture: &Fixture,
    check: impl FnOnce(&Books) -> Result<(), String>,
) -> Result<(), String> {
    match fixture.get_or_init(|| try_books_small().map_err(|e| e.to_string())) {
        Ok(books) => check(books),
        Err(why) => Err(format!("the fixture company could not be built: {why}")),
    }
}

fn gstin_check(first14: &str, expected: char) -> Result<(), String> {
    match gstin_check_character(first14) {
        Some(got) if got == expected => Ok(()),
        Some(got) => Err(format!(
            "check character of {first14} is '{got}', the case expects '{expected}'"
        )),
        None => Err(format!(
            "'{first14}' is not 14 characters of 0-9 and A-Z, so it has no check character"
        )),
    }
}

/// Sum of the amounts of `postings`, debit positive, with checked arithmetic.
fn signed_sum<'a>(postings: impl Iterator<Item = &'a Posting>) -> Result<Paise, String> {
    postings.try_fold(Paise::ZERO, |sum, p| {
        let value = p.amount.to_signed().map_err(|e| e.to_string())?;
        sum.checked_add(value).map_err(|e| e.to_string())
    })
}

fn postings_balance(books: &Books) -> Result<(), String> {
    let sum = signed_sum(books.postings.iter())?;
    if sum.is_zero() {
        Ok(())
    } else {
        Err(format!(
            "all postings sum to {} paise (debit positive), expected 0",
            sum.get()
        ))
    }
}

/// The one item of `found`, or an error saying there is none or several.
fn unique<T>(what: &str, name: &str, mut found: impl Iterator<Item = T>) -> Result<T, String> {
    match (found.next(), found.next()) {
        (Some(only), None) => Ok(only),
        (None, _) => Err(format!("the fixture company has no {what} named '{name}'")),
        (Some(_), Some(_)) => Err(format!(
            "the fixture company has more than one {what} named '{name}'"
        )),
    }
}

fn ledger_net(books: &Books, name: &str, expected: Paise) -> Result<(), String> {
    let id = unique(
        "ledger",
        name,
        books
            .masters
            .ledgers
            .values()
            .filter(|l| l.name == name)
            .map(|l| l.id),
    )?;
    let regular = books
        .postings
        .iter()
        .filter(|p| p.ledger == id && p.status == VoucherStatus::Regular);
    let net = signed_sum(regular)?;
    if net == expected {
        Ok(())
    } else {
        Err(format!(
            "net of ledger '{name}' is {} paise (debit positive), the case expects {} paise",
            net.get(),
            expected.get()
        ))
    }
}

fn stock_qty(books: &Books, name: &str, expected: Decimal) -> Result<(), String> {
    let overflow = || format!("quantity arithmetic for item '{name}' overflowed");
    let item = unique(
        "stock item",
        name,
        books
            .masters
            .stock_items
            .values()
            .filter(|i| i.name == name),
    )?;
    let mut qty = Decimal::ZERO;
    for opening in &item.opening {
        qty = qty.checked_add(opening.qty.0).ok_or_else(overflow)?;
    }
    for m in books.stock_moves.iter().filter(|m| m.item == item.id) {
        qty = qty.checked_add(m.qty_in.0).ok_or_else(overflow)?;
        qty = qty.checked_sub(m.qty_out.0).ok_or_else(overflow)?;
    }
    if qty == expected {
        Ok(())
    } else {
        Err(format!(
            "closing quantity of item '{name}' is {qty}, the case expects {expected}"
        ))
    }
}

/// Parses a pass-list: one case id per line, `#` comments and blank lines ignored.
///
/// # Errors
/// One message per malformed line (an id containing whitespace) and per id listed twice.
pub fn parse_pass_list(text: &str) -> Result<Vec<String>, Vec<String>> {
    let mut ids = Vec::new();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index.saturating_add(1);
        if line.chars().any(char::is_whitespace) {
            errors.push(format!(
                "pass-list line {number}: '{line}' is not a case id (ids contain no whitespace)"
            ));
        } else if !seen.insert(line.to_owned()) {
            errors.push(format!("pass-list line {number}: '{line}' is listed twice"));
        } else {
            ids.push(line.to_owned());
        }
    }
    if errors.is_empty() {
        Ok(ids)
    } else {
        Err(errors)
    }
}

/// The ratchet (engineering rule A6): every case on the pass-list must still pass.
///
/// A case may be missing from the list while it passes (newly passing cases are added by a
/// reviewed change to the list). A listed case that fails, has lost its provenance, or has left
/// the suite is an error.
///
/// # Errors
/// One message per violation, or per malformed pass-list line.
pub fn check_ratchet(report: &RunReport, pass_list: &str) -> Result<(), Vec<String>> {
    let listed = parse_pass_list(pass_list)?;
    let mut errors = Vec::new();
    for id in &listed {
        match report.status_of(id) {
            None => errors.push(format!(
                "{id}: is on the pass-list but not in the suite (case deleted or renamed)"
            )),
            Some(CaseStatus::Passed) => {}
            Some(CaseStatus::Failed { detail }) => {
                errors.push(format!("{id}: passed before and now fails: {detail}"));
            }
            Some(CaseStatus::Pending { reason }) => {
                errors.push(format!("{id}: passed before and is now pending: {reason}"));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
