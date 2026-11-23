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
//! | `Report("import-tally")` | the import screen (its palette row reads "Import from another app") |
//! | `Report("bank-reconciliation")` | the bank reconciliation of the company's one bank ledger (a stated reason when there are none or several) |
//! | `Report("bank:N")` | the bank reconciliation of bank ledger N |
//! | `Report("masters")` | the masters screen, showing ledgers |
//! | `Report("review")`, `"ask"`, `"quick-entry"`, `"purchase-bill"`, `"keys"`, `"ai-settings"`, `"setup"` | a stub screen: its title and a "not supported yet" line ([`crate::screens::draw_stub`]) |
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
//! The day book opens on one day: the date of the last voucher in the year (or the year's last
//! day when it has none). The Tally help page "Viewing Day Book" (read 2026-11-23,
//! <https://help.tallysolutions.com/article/Tally.ERP9/Reports/Display_Registers_Ledgers/Display_Day_Book.htm>):
//! the Day Book is displayed for the current date, "as on the last date of voucher entry".
//!
//! The bank reconciliation opens on the financial-year period above. The GST screen does not: it
//! opens on one calendar month, which is the return period (decision 7 of the UI parity report,
//! monthly). The rule is [`gst_return_period`]:
//!
//! * when today (the India date from [`today`]) is in the company's financial year, the calendar
//!   month of today;
//! * otherwise the calendar month of the last voucher dated in the year;
//! * when the year has no voucher, the first calendar month of the year.
//!
//! The GST portal's returns page (<https://www.gst.gov.in/help/returns>, read 2026-11-22) names
//! the normal return as "Monthly/Quarterly Return by Normal Taxpayer under QRMP Scheme": monthly
//! is the default and quarterly is the QRMP option. The registration model has no filing-frequency
//! field, so a quarterly filer cannot be told apart from a monthly one yet; that is a gap.
//! `ledgerly-gst` accepts one calendar month or one calendar quarter (`check_period`), so a monthly
//! period is a valid input to the engine.

use ledgerly_api::{AuditReportKind, ColumnSpec, Detail, ReportFilter, ReportKind, ReportParams};
use ledgerly_core::{
    Date, FinancialYear, GroupId, LedgerId, Month, Period, PeriodError, VoucherId,
};
use ledgerly_model::{Books, Scenario};
use thiserror::Error;

use crate::keymap::{Action, Keymap};
use crate::palette::{PaletteEntry, PaletteTarget, command_entries};
use crate::screens::ai_settings::AiSettingsScreen;
use crate::screens::ask::AskScreen;
use crate::screens::bank::BankScreen;
use crate::screens::gst::{GstScreen, GstTab};
use crate::screens::import::ImportScreen;
use crate::screens::keymap_map::KeymapScreen;
use crate::screens::masters::MastersScreen;
use crate::screens::purchase_bill::PurchaseBillScreen;
use crate::screens::quick_entry::QuickEntryScreen;
use crate::screens::report_view::{ReportSource, ReportViewScreen, VoucherViewScreen};
use crate::screens::review::ReviewScreen;
use crate::screens::setup::SetupScreen;
use crate::ui::icon::Icon;
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
    /// A command, an assistant request or a fill of the query box: the host or the palette
    /// handles it, it is not a screen.
    #[error("this choice is a command, an assistant request or a fill, not a screen")]
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

/// The words under every report and screen row of the palette (`design/mock-v2/index.html:1126`).
pub const REPORT_OR_SCREEN: &str = "Report or screen";

/// The palette key that opens the ledger form with `name` filled in. The name is kept as typed.
#[must_use]
pub fn new_ledger_key(name: &str) -> String {
    format!("new-ledger:{name}")
}

/// The name a [`new_ledger_key`] carries, or `None` for any other key.
#[must_use]
pub fn new_ledger_name(key: &str) -> Option<&str> {
    key.strip_prefix("new-ledger:")
}

/// One palette row per report menu entry. A report that has no key is left out (none of the menu
/// reports is such a one).
#[must_use]
pub fn menu_entries() -> Vec<PaletteEntry> {
    report_menu()
        .into_iter()
        .filter_map(|(title, kind)| {
            let key = report_key(&kind)?;
            Some(PaletteEntry::new(PaletteTarget::Report(key), title).with_detail(REPORT_OR_SCREEN))
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
/// The palette key of the masters screen (ledgers).
pub const MASTERS_KEY: &str = "masters";
/// The palette key of the bank reconciliation of the company's one bank ledger.
pub const BANK_RECON_KEY: &str = "bank-reconciliation";
/// The palette key of the Review queue stub.
pub const REVIEW_KEY: &str = "review";
/// The palette key of the Ask your books stub.
pub const ASK_KEY: &str = "ask";
/// The palette key of the Plain-words entry stub.
pub const QUICK_ENTRY_KEY: &str = "quick-entry";
/// The palette key of the Purchase from bill photo stub.
pub const PURCHASE_BILL_KEY: &str = "purchase-bill";
/// The palette key of the Keyboard map stub.
pub const KEYS_KEY: &str = "keys";
/// The palette key of the AI and privacy stub.
pub const AI_SETTINGS_KEY: &str = "ai-settings";
/// The palette key of the Company setup stub.
pub const SETUP_KEY: &str = "setup";

/// The icon a palette row for `target` shows. Screens take the icon the mock's `PAL_SCREENS`
/// gives them (`design/mock-v2/index.html:1109`); the kinds take the icons of `palResults`
/// (lines 1113-1128): commands `sliders`, vouchers `file`, ledgers `users`, questions `spark`.
/// A report the mock does not list takes `scale`, its report icon; the import screen, which the
/// mock does not have, takes `recv` (a file coming in).
#[must_use]
pub fn row_icon(target: &PaletteTarget) -> Icon {
    match target {
        PaletteTarget::Command(_) => Icon::Sliders,
        PaletteTarget::Master(key) => {
            if new_ledger_name(key).is_some() {
                Icon::Plus
            } else {
                Icon::Users
            }
        }
        PaletteTarget::Voucher(_) => Icon::File,
        PaletteTarget::Setting(_) => Icon::Shield,
        PaletteTarget::AiIntent(_) => Icon::Spark,
        PaletteTarget::Fill(text) => fill_icon(text),
        PaletteTarget::Report(key) => report_icon(key),
    }
}

/// The icon of a row that fills the query box: a bare prefix takes the icon the mock gives it
/// (`index.html:1116`); a prefix with text after it is a search (`search`, `index.html:1129`).
fn fill_icon(text: &str) -> Icon {
    match text {
        "#" => Icon::File,
        "@" => Icon::Users,
        ">" => Icon::Sliders,
        "?" => Icon::Spark,
        "+" => Icon::Chat,
        // The text is free, not one of our enums: anything longer than a prefix is a search.
        _ => Icon::Search,
    }
}

/// The icon of the screen or report a palette key opens (see [`row_icon`]).
fn report_icon(key: &str) -> Icon {
    match key {
        HOME_KEY => Icon::Home,
        "day-book" => Icon::Book,
        GST_KEY => Icon::Gst,
        BANK_RECON_KEY => Icon::Bank,
        MASTERS_KEY => Icon::Users,
        REVIEW_KEY => Icon::Inbox,
        KEYS_KEY => Icon::Kbd,
        AI_SETTINGS_KEY => Icon::Shield,
        SETUP_KEY => Icon::Building,
        ASK_KEY => Icon::Spark,
        QUICK_ENTRY_KEY => Icon::Chat,
        PURCHASE_BILL_KEY => Icon::Purchase,
        IMPORT_KEY => Icon::Recv,
        other if other.starts_with("bank:") => Icon::Bank,
        other if other.starts_with("ledger:") => Icon::Users,
        // A key is free text, not one of our enums: every other report draws the report icon.
        _ => Icon::Scale,
    }
}

/// The palette key of the bank reconciliation of `ledger`.
#[must_use]
pub fn bank_key(ledger: LedgerId) -> String {
    format!("bank:{}", ledger.0)
}

/// The palette rows for the screens that are not reports: Home, the Review queue, GST returns,
/// the import, the masters, the bank reconciliation, the stubs of the mock's screens, and one bank
/// reconciliation per bank ledger of `books` (none while no books are loaded).
#[must_use]
pub fn screen_entries(books: Option<&Books>) -> Vec<PaletteEntry> {
    let screen = |key: &str, title: &str| {
        PaletteEntry::new(PaletteTarget::Report(key.to_owned()), title)
            .with_detail(REPORT_OR_SCREEN)
    };
    let mut entries = vec![
        screen(HOME_KEY, "Home"),
        screen(REVIEW_KEY, "Review queue"),
        screen(GST_KEY, "GST Returns"),
        screen(IMPORT_KEY, "Import from another app"),
        screen(MASTERS_KEY, "Masters"),
        screen(BANK_RECON_KEY, "Bank reconciliation"),
        screen(QUICK_ENTRY_KEY, "Plain-words entry"),
        screen(ASK_KEY, "Ask your books"),
        screen(PURCHASE_BILL_KEY, "Purchase from bill photo"),
        screen(KEYS_KEY, "Keyboard map"),
        screen(AI_SETTINGS_KEY, "AI and privacy"),
        screen(SETUP_KEY, "Company setup"),
    ];
    if let Some(books) = books {
        for ledger in books.masters.ledgers.values() {
            if ledger.bank.is_some() {
                entries.push(
                    PaletteEntry::new(
                        PaletteTarget::Report(bank_key(ledger.id)),
                        format!("Bank Reconciliation: {}", ledger.name),
                    )
                    .with_detail(REPORT_OR_SCREEN),
                );
            }
        }
    }
    entries
}

// ---- the sidebar -----------------------------------------------------------------------------

/// One row of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarEntry {
    /// The label shown in the sidebar, in the mock's words.
    pub label: &'static str,
    /// The key hint shown beside the label (classic preset keys, as the mock shows them), if any.
    pub hint: Option<&'static str>,
    /// The count shown beside the label (open proposals in the Review queue). `None` until the
    /// shell fills it from the API; no count is guessed here.
    pub count: Option<u64>,
    /// What choosing the row does: the same target the palette uses for it.
    pub target: PaletteTarget,
}

/// A titled group of sidebar rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarGroup {
    /// The group's title.
    pub title: &'static str,
    /// The rows, in display order.
    pub entries: Vec<SidebarEntry>,
}

/// One sidebar row with no count.
fn row(label: &'static str, hint: Option<&'static str>, target: PaletteTarget) -> SidebarEntry {
    SidebarEntry {
        label,
        hint,
        count: None,
        target,
    }
}

/// The sidebar routes, as the mock's side bar shows them (`design/mock-v2/DESIGN.md`, "Side bar
/// groups"): Overview, Enter, Books, Compliance and Settings, in that order. The reports the app
/// has and the mock does not (Trial Balance, Profit and Loss, Stock Summary, the receivables and
/// payables reports, the Integrity Certificate) are not here; they stay in the palette.
///
/// Sales, Purchase, Payment and Receipt are [`PaletteTarget::Command`] rows: the host opens the
/// voucher they name. Every other row is a report-key row that [`open_target`] opens. The count of
/// the Review queue is left `None` for the shell to fill.
#[must_use]
pub fn sidebar() -> Vec<SidebarGroup> {
    let screen = |key: &str| PaletteTarget::Report(key.to_owned());
    vec![
        SidebarGroup {
            title: "Overview",
            entries: vec![
                row("Home", None, screen(HOME_KEY)),
                row("Review queue", None, screen(REVIEW_KEY)),
            ],
        },
        SidebarGroup {
            title: "Enter",
            entries: vec![
                row(
                    "Sales",
                    Some("F8"),
                    PaletteTarget::Command(Action::NewSales),
                ),
                row(
                    "Purchase",
                    Some("F9"),
                    PaletteTarget::Command(Action::NewPurchase),
                ),
                row(
                    "Payment",
                    Some("F5"),
                    PaletteTarget::Command(Action::NewPayment),
                ),
                row(
                    "Receipt",
                    Some("F6"),
                    PaletteTarget::Command(Action::NewReceipt),
                ),
                row("Plain-words entry", None, screen(QUICK_ENTRY_KEY)),
            ],
        },
        SidebarGroup {
            title: "Books",
            entries: vec![
                row("Day book", None, screen("day-book")),
                row("Balance sheet", None, screen("balance-sheet")),
                row("Masters", None, screen(MASTERS_KEY)),
            ],
        },
        SidebarGroup {
            title: "Compliance",
            entries: vec![
                row("GST returns", None, screen(GST_KEY)),
                row("Bank reconciliation", None, screen(BANK_RECON_KEY)),
            ],
        },
        SidebarGroup {
            title: "Settings",
            entries: vec![
                row("AI and privacy", None, screen(AI_SETTINGS_KEY)),
                row("Keyboard map", Some("?"), screen(KEYS_KEY)),
                row("Company setup", None, screen(SETUP_KEY)),
            ],
        },
    ]
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

/// The hour of the day now, 0 to 23, on India Standard Time (UTC+05:30), as [`today`] dates it.
#[must_use]
pub fn hour_now() -> u8 {
    let now = time::OffsetDateTime::now_utc();
    match time::UtcOffset::from_hms(5, 30, 0) {
        Ok(india) => now.to_offset(india).hour(),
        Err(_) => now.hour(),
    }
}

/// The calendar month of `date`, as a period from its first day to its last.
///
/// # Errors
/// [`NavError::Period`] when `date` is outside the supported calendar range.
fn calendar_month(date: Date) -> Result<Period, NavError> {
    let first = Date::from_calendar_date(date.year(), date.month(), 1)
        .map_err(|_| PeriodError::OutOfRange)?;
    let mut last = first;
    while let Some(next) = last.next_day() {
        if next.month() != first.month() {
            break;
        }
        last = next;
    }
    Ok(Period::new(first, last)?)
}

/// The return period a GST screen opens with (the rule is in the module header): the calendar
/// month of `today` when `today` is in the company's financial year, otherwise the calendar month
/// of the last voucher dated in that year, or the first month of the year when it has none.
///
/// `today` is passed in so the rule can be tested; [`gst_screen`] passes [`today`]. The API has no
/// clock of its own that the app can read (`ledgerly_api::Api` keeps its `Clock` private), so the
/// app's India date stands in for it.
///
/// # Errors
/// [`NavError::Period`] when the financial year or the month is not a valid period.
pub fn gst_return_period(books: &Books, today: Date) -> Result<Period, NavError> {
    let year = FinancialYear {
        start: books.company.fy_start,
    }
    .period()?;
    let anchor = if year.contains(today) {
        today
    } else {
        books
            .vouchers
            .values()
            .map(|voucher| voucher.date)
            .filter(|date| year.contains(*date))
            .max()
            .unwrap_or(year.from())
    };
    calendar_month(anchor)
}

/// A return period's first month as the label reads, for example `November 2026`.
#[must_use]
pub fn return_label(period: Period) -> String {
    let month = match period.from().month() {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    };
    format!("{month} {}", period.from().year())
}

/// The GST screen for the company's GST registration, loading GSTR-1 and GSTR-3B for the return
/// period of [`gst_return_period`] (monthly), opening on the GSTR-1 tab as the mock does.
///
/// # Errors
/// [`NavError::Unsupported`] when the company has no GST registration, or more than one (the
/// screen cannot choose between them yet); [`NavError::Period`] for an invalid financial year.
pub fn gst_screen(books: &Books) -> Result<GstScreen, NavError> {
    let (gstin, state) = match books.company.registrations.as_slice() {
        [only] => (only.gstin.clone(), only.state),
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
    let period = gst_return_period(books, today())?;
    let mut screen = GstScreen::new(return_label(period), Some(gstin.clone())).with_state(state);
    screen.switch_tab(GstTab::Gstr1);
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

/// The bank reconciliation of the company's only bank ledger.
///
/// # Errors
/// [`NavError::Unsupported`] when the company has no bank ledger, or more than one (the screen
/// cannot choose between them; the palette's "Bank Reconciliation: name" rows open each one).
/// Otherwise as [`bank_screen`].
pub fn sole_bank_screen(books: &Books) -> Result<BankScreen, NavError> {
    let banks: Vec<LedgerId> = books
        .masters
        .ledgers
        .values()
        .filter(|ledger| ledger.bank.is_some())
        .map(|ledger| ledger.id)
        .collect();
    match banks.as_slice() {
        [only] => bank_screen(books, *only),
        [] => Err(NavError::Unsupported {
            what: "Bank reconciliation".to_owned(),
            reason: "the company has no bank ledger".to_owned(),
        }),
        many => Err(NavError::Unsupported {
            what: "Bank reconciliation".to_owned(),
            reason: format!(
                "the company has {} bank ledgers and this screen cannot choose one; \
                 pick one by name in the palette",
                many.len()
            ),
        }),
    }
}

/// The screen a non-report key names, or `None` when `key` is not one of them.
fn screen_for_key(key: &str, ctx: &AppCtx) -> Option<Result<Box<dyn Screen>, NavError>> {
    let built: Result<Box<dyn Screen>, NavError> = match key {
        HOME_KEY => Ok(Box::new(super::gateway::GatewayScreen::new())),
        GST_KEY => books_of(ctx).and_then(|b| Ok(Box::new(gst_screen(b)?) as Box<dyn Screen>)),
        IMPORT_KEY => ImportScreen::new(today())
            .map(|s| Box::new(s) as Box<dyn Screen>)
            .map_err(NavError::from),
        MASTERS_KEY => Ok(Box::new(MastersScreen::new())),
        BANK_RECON_KEY => {
            books_of(ctx).and_then(|b| Ok(Box::new(sole_bank_screen(b)?) as Box<dyn Screen>))
        }
        REVIEW_KEY => Ok(Box::new(ReviewScreen)),
        ASK_KEY => Ok(Box::new(AskScreen)),
        QUICK_ENTRY_KEY => Ok(Box::new(QuickEntryScreen)),
        PURCHASE_BILL_KEY => Ok(Box::new(PurchaseBillScreen)),
        KEYS_KEY => Ok(Box::new(KeymapScreen::default())),
        AI_SETTINGS_KEY => Ok(Box::new(AiSettingsScreen)),
        SETUP_KEY => Ok(Box::new(SetupScreen)),
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
    let year_to_date = default_period(books)?;
    let period = if kind == ReportKind::DayBook {
        Period::new(year_to_date.to(), year_to_date.to())?
    } else {
        year_to_date
    };
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
        PaletteTarget::Command(_) | PaletteTarget::AiIntent(_) | PaletteTarget::Fill(_) => {
            Err(NavError::NotNavigation)
        }
        PaletteTarget::Report(key) => {
            if let Some(screen) = screen_for_key(key, ctx) {
                return screen;
            }
            let kind = report_from_key(key).ok_or_else(|| report_key_error(key))?;
            open_report(kind, ctx)
        }
        PaletteTarget::Master(key) => {
            if let Some(name) = new_ledger_name(key) {
                return Ok(Box::new(MastersScreen::creating_ledger(name)));
            }
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
