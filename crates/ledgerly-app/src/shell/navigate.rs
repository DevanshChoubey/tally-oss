//! Opening screens from the report menu and from command palette picks (card C5).
//!
//! The palette hands the host an opaque [`PaletteTarget`]; this module turns it into a screen:
//!
//! | Target | Opens |
//! |---|---|
//! | `Report("trial-balance")` and the other menu keys | a worker-mode [`ReportViewScreen`] |
//! | `Master("ledger:N")` | the ledger's vouchers report (`ReportKind::LedgerVouchers`) |
//! | `Voucher("voucher:N")` | the read-only [`VoucherViewScreen`] |
//! | `Report("home")` | the home screen of the open company ([`super::gateway::GatewayScreen`]) |
//! | `Report("gst-returns")` | the GST screen for the company's one GST registration |
//! | `Report("import-tally")` | the import screen |
//! | `Report("bank:N")` | the bank reconciliation of bank ledger N |
//! | `Setting(name)` | nothing: there is no settings screen yet, so it says so |
//! | `Command`, `AiIntent` | nothing: the host handles them ([`NavError::NotNavigation`]) |
//!
//! A report screen made here runs its report on the API worker (`ReportSource::worker`); the UI
//! thread never waits for it. This module computes no figure.
//!
//! # The period a report opens with
//!
//! The company's financial year, from its start to the date of the last voucher in it (or the
//! whole year when it has no voucher). The Trial Balance page of Tally says the report "will be
//! generated as on the date of the last voucher entry"
//! (<https://help.tallysolutions.com/tally-prime/accounting-financial-reports/trial-balance-tally/>,
//! read 2026-11-21). The same page is the source for the detail level: "By default, a group-wise
//! view of the Trial Balance appears", so the reports open condensed (a group or ledger drill
//! opens detailed, as `ledgerly-reports` drills do). That page covers the Trial Balance only; the
//! same default is applied to every report here and is `pending` for the others. The user changes
//! the period with F2 and the detail with Alt+F1.
//!
//! The GST screen and the bank reconciliation open on the same period. No Tally page read on
//! 2026-11-21 says which period GSTR-1 or GSTR-3B opens with (the GSTR-1 and GSTR-3B pages of
//! help.tallysolutions.com list "Period" in the header and say it can be changed), so that default
//! is `pending`.

use ledgerly_api::{AuditReportKind, ColumnSpec, Detail, ReportFilter, ReportKind, ReportParams};
use ledgerly_core::{Date, FinancialYear, GroupId, LedgerId, Period, PeriodError, VoucherId};
use ledgerly_model::{Books, Scenario};
use thiserror::Error;

use crate::keymap::Keymap;
use crate::palette::{PaletteEntry, PaletteTarget, command_entries};
use crate::screens::bank::BankScreen;
use crate::screens::gst::GstScreen;
use crate::screens::import::ImportScreen;
use crate::screens::report_view::{ReportSource, ReportViewScreen, VoucherViewScreen};
use crate::{AppCtx, Screen};

/// Why a pick or a menu entry did not open a screen.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NavError {
    /// No company's books are loaded, so a report period cannot be chosen.
    #[error("no company is open, or its books are not loaded yet, so the report cannot open")]
    NoCompany,
    /// The palette key is not one this module wrote.
    #[error("\"{0}\" is not something the palette can open")]
    BadKey(String),
    /// What was asked for has no screen yet.
    #[error("not supported yet: {what} ({reason})")]
    Unsupported {
        /// What is not supported.
        what: String,
        /// Why.
        reason: String,
    },
    /// The voucher is not in the loaded books.
    #[error("voucher {0} is not in the books")]
    VoucherNotFound(u64),
    /// The company's financial year does not make a valid period.
    #[error("the company's financial year is not a valid period: {0}")]
    Period(#[from] PeriodError),
    /// A command or assistant request: the host handles it, it is not a screen.
    #[error("this choice is a command or an assistant request, not a screen")]
    NotNavigation,
    /// The report screen could not be set up.
    #[error("the report screen could not be set up")]
    Setup,
}

// ---- keys ------------------------------------------------------------------------------------

/// The palette key of a ledger.
#[must_use]
pub fn ledger_key(id: LedgerId) -> String {
    format!("ledger:{}", id.0)
}

/// The palette key of a voucher.
#[must_use]
pub fn voucher_key(id: VoucherId) -> String {
    format!("voucher:{}", id.0)
}

/// The palette key of a group summary.
#[must_use]
pub fn group_key(id: GroupId) -> String {
    format!("group:{}", id.0)
}

/// The palette key of a report, or `None` for a report this module does not open by key yet.
#[must_use]
pub fn report_key(kind: &ReportKind) -> Option<String> {
    let plain = |name: &str| Some(name.to_owned());
    match kind {
        ReportKind::TrialBalance => plain("trial-balance"),
        ReportKind::BalanceSheet => plain("balance-sheet"),
        ReportKind::ProfitAndLoss => plain("profit-and-loss"),
        ReportKind::GroupSummary(group) => Some(group_key(*group)),
        ReportKind::LedgerVouchers(ledger) => Some(ledger_key(*ledger)),
        ReportKind::DayBook => plain("day-book"),
        ReportKind::Register(_) => None,
        ReportKind::Outstanding { receivable } => {
            if *receivable {
                plain("receivables")
            } else {
                plain("payables")
            }
        }
        ReportKind::StockSummary => plain("stock-summary"),
        ReportKind::Audit(audit) => match audit {
            AuditReportKind::IntegrityCertificate => plain("integrity-certificate"),
            AuditReportKind::ObjectHistory
            | AuditReportKind::EditLog
            | AuditReportKind::DeletedAltered
            | AuditReportKind::BackDated
            | AuditReportKind::EditsAfterLock => None,
        },
    }
}

/// The report a [`report_key`] names.
#[must_use]
pub fn report_from_key(key: &str) -> Option<ReportKind> {
    match key {
        "trial-balance" => Some(ReportKind::TrialBalance),
        "balance-sheet" => Some(ReportKind::BalanceSheet),
        "profit-and-loss" => Some(ReportKind::ProfitAndLoss),
        "day-book" => Some(ReportKind::DayBook),
        "receivables" => Some(ReportKind::Outstanding { receivable: true }),
        "payables" => Some(ReportKind::Outstanding { receivable: false }),
        "stock-summary" => Some(ReportKind::StockSummary),
        "integrity-certificate" => Some(ReportKind::Audit(AuditReportKind::IntegrityCertificate)),
        other => {
            if let Some(id) = parse_id(other, "group:") {
                Some(ReportKind::GroupSummary(GroupId(id)))
            } else {
                parse_id(other, "ledger:").map(|id| ReportKind::LedgerVouchers(LedgerId(id)))
            }
        }
    }
}

/// The number after `prefix` in `key`, if `key` is exactly `prefix` and digits.
fn parse_id(key: &str, prefix: &str) -> Option<u64> {
    key.strip_prefix(prefix)?.parse().ok()
}

// ---- the report menu -------------------------------------------------------------------------

/// The reports of the report menu, in menu order, with the title each shows.
#[must_use]
pub fn report_menu() -> Vec<(&'static str, ReportKind)> {
    vec![
        ("Trial Balance", ReportKind::TrialBalance),
        ("Balance Sheet", ReportKind::BalanceSheet),
        ("Profit and Loss", ReportKind::ProfitAndLoss),
        ("Day Book", ReportKind::DayBook),
        ("Stock Summary", ReportKind::StockSummary),
        (
            "Receivables Outstanding",
            ReportKind::Outstanding { receivable: true },
        ),
        (
            "Payables Outstanding",
            ReportKind::Outstanding { receivable: false },
        ),
        (
            "Integrity Certificate",
            ReportKind::Audit(AuditReportKind::IntegrityCertificate),
        ),
    ]
}

/// One palette row per report menu entry. A report that has no key is left out (none of the menu
/// reports is such a one).
#[must_use]
pub fn menu_entries() -> Vec<PaletteEntry> {
    report_menu()
        .into_iter()
        .filter_map(|(title, kind)| {
            let key = report_key(&kind)?;
            Some(PaletteEntry::new(PaletteTarget::Report(key), title).with_detail("Report"))
        })
        .collect()
}

/// The rows the host feeds the palette: every keymap command, then the report menu. Ledgers,
/// vouchers and settings come from the API search ([`crate::palette::PaletteSearch`]).
#[must_use]
pub fn host_entries(keymap: &Keymap) -> Vec<PaletteEntry> {
    let mut entries = command_entries(keymap);
    entries.extend(menu_entries());
    entries
}

// ---- the other screens -----------------------------------------------------------------------

/// The palette key of the home screen.
pub const HOME_KEY: &str = "home";
/// The palette key of the GST screen.
pub const GST_KEY: &str = "gst-returns";
/// The palette key of the import screen.
pub const IMPORT_KEY: &str = "import-tally";

/// The palette key of the bank reconciliation of `ledger`.
#[must_use]
pub fn bank_key(ledger: LedgerId) -> String {
    format!("bank:{}", ledger.0)
}

/// The palette rows for the screens that are not reports: Home, GST returns, Import, and one
/// bank reconciliation per bank ledger of `books` (none while no books are loaded).
#[must_use]
pub fn screen_entries(books: Option<&Books>) -> Vec<PaletteEntry> {
    let screen = |key: &str, title: &str| {
        PaletteEntry::new(PaletteTarget::Report(key.to_owned()), title).with_detail("Screen")
    };
    let mut entries = vec![
        screen(HOME_KEY, "Home"),
        screen(GST_KEY, "GST Returns"),
        screen(IMPORT_KEY, "Import from Tally"),
    ];
    if let Some(books) = books {
        for ledger in books.masters.ledgers.values() {
            if ledger.bank.is_some() {
                entries.push(
                    PaletteEntry::new(
                        PaletteTarget::Report(bank_key(ledger.id)),
                        format!("Bank Reconciliation: {}", ledger.name),
                    )
                    .with_detail("Banking"),
                );
            }
        }
    }
    entries
}

/// Today's date in India (UTC+05:30), which the import screen and new vouchers start from.
#[must_use]
pub fn today() -> Date {
    let now = time::OffsetDateTime::now_utc();
    match time::UtcOffset::from_hms(5, 30, 0) {
        Ok(india) => now.to_offset(india).date(),
        // The offset is a constant in range; a failure would leave the UTC date.
        Err(_) => now.date(),
    }
}

/// The GST screen for the company's GST registration, loading GSTR-1 and GSTR-3B for
/// [`default_period`].
///
/// # Errors
/// [`NavError::Unsupported`] when the company has no GST registration, or more than one (the
/// screen cannot choose between them yet); [`NavError::Period`] for an invalid financial year.
pub fn gst_screen(books: &Books) -> Result<GstScreen, NavError> {
    let gstin = match books.company.registrations.as_slice() {
        [only] => only.gstin.clone(),
        [] => {
            return Err(NavError::Unsupported {
                what: "GST returns".to_owned(),
                reason: "the company has no GST registration".to_owned(),
            });
        }
        many => {
            return Err(NavError::Unsupported {
                what: "GST returns".to_owned(),
                reason: format!(
                    "the company has {} GST registrations and the screen cannot choose one yet",
                    many.len()
                ),
            });
        }
    };
    let period = default_period(books)?;
    let label = format!("{} to {}", period.from(), period.to());
    let mut screen = GstScreen::new(label, Some(gstin.clone()));
    screen.request_load(gstin, period);
    Ok(screen)
}

/// The bank reconciliation of `ledger`, as of the end of [`default_period`].
///
/// # Errors
/// [`NavError::BadKey`] when `ledger` is not a bank ledger of `books`; [`NavError::Period`] for
/// an invalid financial year.
pub fn bank_screen(books: &Books, ledger: LedgerId) -> Result<BankScreen, NavError> {
    let Some(found) = books
        .masters
        .ledgers
        .get(&ledger)
        .filter(|l| l.bank.is_some())
    else {
        return Err(NavError::BadKey(bank_key(ledger)));
    };
    let as_of = default_period(books)?.to();
    Ok(BankScreen::worker(&found.name, ledger, as_of))
}

/// The screen a non-report key names, or `None` when `key` is not one of them.
fn screen_for_key(key: &str, ctx: &AppCtx) -> Option<Result<Box<dyn Screen>, NavError>> {
    let built: Result<Box<dyn Screen>, NavError> = match key {
        HOME_KEY => Ok(Box::new(super::gateway::GatewayScreen::new())),
        GST_KEY => books_of(ctx).and_then(|b| Ok(Box::new(gst_screen(b)?) as Box<dyn Screen>)),
        IMPORT_KEY => ImportScreen::new(today())
            .map(|s| Box::new(s) as Box<dyn Screen>)
            .map_err(NavError::from),
        other => {
            let id = parse_id(other, "bank:")?;
            books_of(ctx)
                .and_then(|b| Ok(Box::new(bank_screen(b, LedgerId(id))?) as Box<dyn Screen>))
        }
    };
    Some(built)
}

// ---- parameters and screens ------------------------------------------------------------------

/// Whether a report opens at the detailed level: a group summary or a ledger's vouchers, as the
/// drills of `ledgerly-reports` do. Every other report opens condensed.
fn opens_detailed(kind: &ReportKind) -> bool {
    match kind {
        ReportKind::GroupSummary(_) | ReportKind::LedgerVouchers(_) => true,
        ReportKind::TrialBalance
        | ReportKind::BalanceSheet
        | ReportKind::ProfitAndLoss
        | ReportKind::DayBook
        | ReportKind::Register(_)
        | ReportKind::Outstanding { .. }
        | ReportKind::StockSummary
        | ReportKind::Audit(_) => false,
    }
}

/// The period a report opens with: the financial year from its start to the last voucher dated in
/// it, or the whole year when there is none.
///
/// # Errors
/// [`NavError::Period`] when the company's financial year is not a valid period.
pub fn default_period(books: &Books) -> Result<Period, NavError> {
    let year = FinancialYear {
        start: books.company.fy_start,
    }
    .period()?;
    let last = books
        .vouchers
        .values()
        .map(|voucher| voucher.date)
        .filter(|date| year.contains(*date))
        .max();
    Ok(Period::new(year.from(), last.unwrap_or_else(|| year.to()))?)
}

/// The parameters a report of `kind` opens with in `books` (see the module header).
///
/// # Errors
/// [`NavError::Period`] when the company's financial year is not a valid period.
pub fn default_params(kind: ReportKind, books: &Books) -> Result<ReportParams, NavError> {
    let period = default_period(books)?;
    let detail = if opens_detailed(&kind) {
        Detail::Detailed
    } else {
        Detail::Condensed
    };
    Ok(ReportParams {
        kind,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    })
}

/// A worker-mode report screen for `kind`: it asks the API worker for the report on its first
/// frame.
///
/// # Errors
/// [`NavError::Period`] for an invalid financial year, [`NavError::Setup`] if the screen cannot
/// be built.
pub fn report_screen(kind: ReportKind, books: &Books) -> Result<ReportViewScreen, NavError> {
    let params = default_params(kind, books)?;
    ReportViewScreen::worker(ReportSource::worker(params)).ok_or(NavError::Setup)
}

/// The books of the open company.
fn books_of(ctx: &AppCtx) -> Result<&Books, NavError> {
    ctx.books.as_ref().ok_or(NavError::NoCompany)
}

/// The screen for a report menu entry or a report pick.
///
/// # Errors
/// [`NavError::NoCompany`] when no books are loaded, otherwise as [`report_screen`].
pub fn open_report(kind: ReportKind, ctx: &AppCtx) -> Result<Box<dyn Screen>, NavError> {
    Ok(Box::new(report_screen(kind, books_of(ctx)?)?))
}

/// The screen a palette pick opens.
///
/// # Errors
/// [`NavError::NotNavigation`] for a command or an assistant request (the host handles those),
/// [`NavError::Unsupported`] for a setting or a report with no key, [`NavError::BadKey`] for a key
/// this module did not write, [`NavError::NoCompany`] when no books are loaded,
/// [`NavError::VoucherNotFound`] for a voucher the books do not hold.
pub fn open_target(target: &PaletteTarget, ctx: &AppCtx) -> Result<Box<dyn Screen>, NavError> {
    match target {
        PaletteTarget::Command(_) | PaletteTarget::AiIntent(_) => Err(NavError::NotNavigation),
        PaletteTarget::Report(key) => {
            if let Some(screen) = screen_for_key(key, ctx) {
                return screen;
            }
            let kind = report_from_key(key).ok_or_else(|| report_key_error(key))?;
            open_report(kind, ctx)
        }
        PaletteTarget::Master(key) => {
            let id = parse_id(key, "ledger:").ok_or_else(|| NavError::BadKey(key.clone()))?;
            open_report(ReportKind::LedgerVouchers(LedgerId(id)), ctx)
        }
        PaletteTarget::Voucher(key) => {
            let id = parse_id(key, "voucher:").ok_or_else(|| NavError::BadKey(key.clone()))?;
            if !books_of(ctx)?.vouchers.contains_key(&VoucherId(id)) {
                return Err(NavError::VoucherNotFound(id));
            }
            Ok(Box::new(VoucherViewScreen::new(VoucherId(id))))
        }
        PaletteTarget::Setting(name) => Err(NavError::Unsupported {
            what: format!("opening the setting \"{name}\""),
            reason: "there is no settings screen yet".to_owned(),
        }),
    }
}

/// The error for a report key that names no report.
fn report_key_error(key: &str) -> NavError {
    match key.strip_prefix("unsupported-report:") {
        Some(title) => NavError::Unsupported {
            what: format!("the report \"{title}\""),
            reason: "the app cannot open it from a search result yet".to_owned(),
        },
        None => NavError::BadKey(key.to_owned()),
    }
}
