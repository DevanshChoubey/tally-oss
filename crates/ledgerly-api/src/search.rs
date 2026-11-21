//! Search (card W1): ledgers, vouchers, reports and settings, as a typed [`SearchHit`] list.
//!
//! # Matching rules
//!
//! - The query is trimmed and compared case-insensitively. An empty query finds nothing.
//! - **Ledgers:** a substring of the name, or of an alias, matches. A GSTIN matches when it
//!   equals the query. One hit per ledger, on the first rule that applies, in the order name,
//!   alias, GSTIN.
//! - **Vouchers:** a substring of the number matches; a substring of the party ledger's name
//!   matches; the query read as a rupee amount (for example `1500` or `1500.50`) matches when a
//!   ledger line of the voucher has that amount; the query read as `YYYY-MM-DD` matches when it is
//!   the voucher date. One hit per voucher, on the first rule that applies, in that order. A
//!   cancelled voucher keeps its number, party and date, and has no amounts left to match.
//! - **Reports:** a substring of the report title matches. **Settings:** a substring of the
//!   setting name matches.
//!
//! # Order
//!
//! Hits come in a fixed order, so the same query on the same books always gives the same list:
//! ledgers by id, then vouchers by id, then reports in catalogue order, then settings in the
//! order of [`SETTINGS`].

use ledgerly_core::{Date, Decimal, LedgerId, VoucherId};
use ledgerly_model::{Books, Voucher};
use ledgerly_reports::{AuditReportKind, ReportKind};
use time::macros::format_description;

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchHit {
    /// A ledger.
    Ledger {
        /// The ledger.
        id: LedgerId,
        /// Its name.
        name: String,
        /// Which field matched.
        matched: MatchedOn,
    },
    /// A voucher.
    Voucher {
        /// The voucher.
        id: VoucherId,
        /// Its number, once allocated.
        number: Option<String>,
        /// Its date.
        date: Date,
        /// Which field matched.
        matched: MatchedOn,
    },
    /// A report. Run it with `ReportParams` whose `kind` is this one.
    Report {
        /// The report title.
        title: &'static str,
        /// The report to run.
        kind: ReportKind,
    },
    /// A company setting, by name.
    Setting {
        /// The setting name.
        name: &'static str,
    },
    /// Part of the books could not be searched, so ledgers and vouchers may be missing from the
    /// hits. Always the last hit.
    Unavailable {
        /// What is unavailable and why.
        reason: String,
    },
}

/// The field a search hit matched on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchedOn {
    /// A ledger name.
    Name,
    /// A ledger alias.
    Alias,
    /// A ledger GSTIN.
    Gstin,
    /// A voucher number.
    Number,
    /// A party ledger's name.
    Party,
    /// A ledger line amount.
    Amount,
    /// A voucher date.
    Date,
    /// A report title.
    Title,
    /// A setting name.
    SettingName,
}

/// Setting names searched, in order. These are the company feature switches.
pub(crate) const SETTINGS: [&str; 11] = [
    "Inventory",
    "Godowns",
    "Batches",
    "Cost centres",
    "Bill-wise details",
    "GST",
    "Bank reconciliation",
    "Optional vouchers",
    "Post-dated vouchers",
    "Negative stock policy",
    "Input credit default",
];

/// Hits over the books and the catalogue. `q` must be non-empty after trimming.
pub(crate) fn search_books(books: &Books, q: &str) -> Vec<SearchHit> {
    let q = q.trim();
    let needle = q.to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits = ledger_hits(books, q, &needle);
    hits.extend(voucher_hits(books, q, &needle));
    hits.extend(catalogue_hits(&needle));
    hits
}

/// Hits over the catalogue alone (reports and settings). `q` must be non-empty after trimming.
pub(crate) fn search_catalogue(q: &str) -> Vec<SearchHit> {
    let needle = q.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    catalogue_hits(&needle)
}

/// Whether `haystack` contains `needle`, ignoring case. `needle` is already lowercase.
fn contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(needle)
}

fn ledger_hits(books: &Books, q: &str, needle: &str) -> Vec<SearchHit> {
    books
        .masters
        .ledgers
        .values()
        .filter_map(|ledger| {
            let matched = if contains(&ledger.name, needle) {
                MatchedOn::Name
            } else if ledger.aliases.iter().any(|alias| contains(alias, needle)) {
                MatchedOn::Alias
            } else if gstin_of(ledger).is_some_and(|gstin| gstin.eq_ignore_ascii_case(q)) {
                MatchedOn::Gstin
            } else {
                return None;
            };
            Some(SearchHit::Ledger {
                id: ledger.id,
                name: ledger.name.clone(),
                matched,
            })
        })
        .collect()
}

fn gstin_of(ledger: &ledgerly_model::Ledger) -> Option<&str> {
    ledger
        .party
        .as_ref()
        .and_then(|party| party.gstin.as_ref())
        .map(|gstin| gstin.as_str())
}

fn voucher_hits(books: &Books, q: &str, needle: &str) -> Vec<SearchHit> {
    let amount = Decimal::from_str_exact(q).ok();
    let date = Date::parse(q, &format_description!("[year]-[month]-[day]")).ok();
    books
        .vouchers
        .values()
        .filter_map(|voucher| {
            let matched = voucher_match(books, voucher, needle, amount, date)?;
            Some(SearchHit::Voucher {
                id: voucher.id,
                number: voucher.number.clone(),
                date: voucher.date,
                matched,
            })
        })
        .collect()
}

/// The first rule that matches `voucher`, in the order number, party, amount, date.
fn voucher_match(
    books: &Books,
    voucher: &Voucher,
    needle: &str,
    amount: Option<Decimal>,
    date: Option<Date>,
) -> Option<MatchedOn> {
    if voucher
        .number
        .as_deref()
        .is_some_and(|number| contains(number, needle))
    {
        return Some(MatchedOn::Number);
    }
    let party = voucher
        .party
        .and_then(|id| books.masters.ledgers.get(&id))
        .is_some_and(|ledger| contains(&ledger.name, needle));
    if party {
        return Some(MatchedOn::Party);
    }
    if let Some(wanted) = amount {
        // A paise amount m is the rupee value m / 100, exactly.
        let matches = voucher
            .ledger_lines
            .iter()
            .any(|line| Decimal::new(line.amount.amount.get(), 2) == wanted);
        if matches {
            return Some(MatchedOn::Amount);
        }
    }
    if date == Some(voucher.date) {
        return Some(MatchedOn::Date);
    }
    None
}

fn catalogue_hits(needle: &str) -> Vec<SearchHit> {
    let mut hits: Vec<SearchHit> = report_catalogue()
        .into_iter()
        .filter(|(title, _)| contains(title, needle))
        .map(|(title, kind)| SearchHit::Report { title, kind })
        .collect();
    hits.extend(
        SETTINGS
            .iter()
            .filter(|name| contains(name, needle))
            .map(|&name| SearchHit::Setting { name }),
    );
    hits
}

/// Reports that take no parameters beyond the period, with the titles they are searched by.
fn report_catalogue() -> Vec<(&'static str, ReportKind)> {
    vec![
        ("Trial balance", ReportKind::TrialBalance),
        ("Balance sheet", ReportKind::BalanceSheet),
        ("Profit and loss", ReportKind::ProfitAndLoss),
        ("Day book", ReportKind::DayBook),
        ("Stock summary", ReportKind::StockSummary),
        (
            "Receivables outstanding",
            ReportKind::Outstanding { receivable: true },
        ),
        (
            "Payables outstanding",
            ReportKind::Outstanding { receivable: false },
        ),
        (
            "Integrity certificate",
            ReportKind::Audit(AuditReportKind::IntegrityCertificate),
        ),
    ]
}
