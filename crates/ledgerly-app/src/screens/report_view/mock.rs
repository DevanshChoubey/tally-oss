//! The mock's looks for the reports that are not the balance sheet or the day book: the trial
//! balance, outstanding bills, the stock summary, the profit and loss and the audit screen
//! (`design/mock-v2/index.html`, states `trial-balance.default`, `outstanding.*`,
//! `stock-summary.*`, `profit-and-loss.*`, `audit.*`).
//!
//! Each projection only moves what the engine returned into the mock's columns: a cell is copied
//! as it came, a figure is never added, divided or rounded here (rule C4). Where the engine's
//! report does not hold what the mock shows (the cost per unit of a stock item, the days a bill is
//! overdue), the column is left out or shows the engine's own wording, and
//! `design/PARITY-GAPS.md` says so.

use ledgerly_api::{AuditReportKind, ReportKind};
use ledgerly_core::{Date, Period, StockItemId};

use super::{ViewCell, ViewDetail, ViewKey, ViewReport, ViewRow, ViewRowKind};

/// Heading of the stock value column. It names the side, so [`super::generic`] can print the
/// figure bare when it is a debit and keep the word when it is not.
pub(super) const VALUE_HEADING: &str = "Value Dr";

/// Heading of the stock cost column (the engine's closing rate), named like [`VALUE_HEADING`].
pub(super) const COST_HEADING: &str = "Cost Dr";

/// Heading of the open amount of a bill, naming the side a bill normally sits on.
pub(super) fn open_heading(receivable: bool) -> &'static str {
    if receivable { "Open Dr" } else { "Open Cr" }
}

/// Heading of the group column of the trial balance, which lists ledgers.
pub(super) const TB_GROUP: &str = "Group";

/// The trial balance as the mock draws it: one line per ledger, with the group it sits in. The
/// engine's detailed report is a tree of groups and ledgers; the group lines are dropped and each
/// ledger names its nearest group. Needs the detailed report; any other shape is returned as it
/// came.
#[must_use]
pub(super) fn trial_balance(report: ViewReport) -> ViewReport {
    let has_ledgers = report.rows.iter().any(|r| r.kind == ViewRowKind::Ledger);
    if report.columns != ["Debit", "Credit"] || !has_ledgers {
        return report;
    }
    let mut groups: Vec<(u8, String)> = Vec::new();
    let mut rows = Vec::new();
    for row in report.rows {
        match row.kind {
            ViewRowKind::Group => {
                groups.retain(|(d, _)| *d < row.depth);
                groups.push((row.depth, row.label));
            }
            ViewRowKind::Ledger => {
                let group = groups
                    .iter()
                    .rev()
                    .find(|(d, _)| *d < row.depth)
                    .map_or(ViewCell::Empty, |(_, g)| ViewCell::Text(g.clone()));
                let mut cells = vec![group];
                cells.extend(row.cells);
                rows.push(ViewRow {
                    depth: 0,
                    cells,
                    ..row
                });
            }
            ViewRowKind::Voucher
            | ViewRowKind::Item
            | ViewRowKind::Bill
            | ViewRowKind::Difference
            | ViewRowKind::Subtotal
            | ViewRowKind::Text => rows.push(row),
        }
    }
    let mut totals = report.totals;
    if !totals.is_empty() {
        totals.insert(0, ViewCell::Empty);
    }
    let mut columns = vec![TB_GROUP.to_owned()];
    columns.extend(report.columns);
    ViewReport {
        columns,
        rows,
        totals,
        ..report
    }
}

/// The engine's ageing columns for a bill past its due date (`outstanding.rs`, "Ageing").
pub(super) const OVERDUE_BUCKETS: [&str; 4] =
    ["0-30 days", "31-60 days", "61-90 days", "Over 90 days"];
/// What a bill due on the as-at date reads in the Overdue column.
const DUE_TODAY: &str = "Due today";

/// The bucket a bill sits in as the Overdue cell. The engine puts a bill due on the as-at date in
/// `0-30 days` though it is 0 days overdue; the mock marks only overdue > 0 (`b.overdue>0`), so
/// that bill reads "Due today" and keeps the plain ink.
fn aged_cell(
    report: &ViewReport,
    row: &ViewRow,
    due: usize,
    buckets: std::ops::RangeInclusive<usize>,
) -> ViewCell {
    let due_today =
        matches!(row.cells.get(due), Some(ViewCell::Date(d)) if *d == report.params.period.to());
    let name = buckets
        .into_iter()
        .find(|i| !matches!(row.cells.get(*i), None | Some(ViewCell::Empty)))
        .and_then(|i| report.columns.get(i));
    match name {
        Some(c) if due_today && c == OVERDUE_BUCKETS[0] => ViewCell::Text(DUE_TODAY.to_owned()),
        Some(c) => ViewCell::Text(c.clone()),
        None => ViewCell::Empty,
    }
}

/// Outstanding bills as the mock draws them: one line per open bill, with the party, the bill,
/// its date and due date, the open amount and where the engine aged it. A report that does not
/// have the engine's columns is returned as it came.
#[must_use]
pub(super) fn outstanding(report: ViewReport, receivable: bool) -> ViewReport {
    let at = |name: &str| report.columns.iter().position(|c| c == name);
    let (Some(bill_date), Some(due), Some(pending), Some(first), Some(last)) = (
        at("Bill date"),
        at("Due date"),
        at("Pending"),
        at("Not yet due"),
        at("No due date"),
    ) else {
        return report;
    };
    let mut party = String::new();
    let mut rows = Vec::new();
    for row in &report.rows {
        match row.kind {
            ViewRowKind::Ledger | ViewRowKind::Group => party.clone_from(&row.label),
            ViewRowKind::Bill => {
                let cell = |i: usize| row.cells.get(i).cloned().unwrap_or(ViewCell::Empty);
                // The mock writes a date as "28 Aug 2026" (`fmtDate`).
                let day = |i: usize| match row.cells.get(i) {
                    Some(ViewCell::Date(d)) => ViewCell::Text(crate::ui::format::date(*d)),
                    Some(other) => other.clone(),
                    None => ViewCell::Empty,
                };
                let aged = aged_cell(&report, row, due, first..=last);
                rows.push(ViewRow {
                    key: row.key.clone(),
                    label: party.clone(),
                    depth: 0,
                    kind: ViewRowKind::Bill,
                    cells: vec![
                        ViewCell::Text(row.label.clone()),
                        day(bill_date),
                        day(due),
                        cell(pending),
                        aged,
                    ],
                });
            }
            ViewRowKind::Voucher
            | ViewRowKind::Item
            | ViewRowKind::Difference
            | ViewRowKind::Subtotal
            | ViewRowKind::Text => {}
        }
    }
    let totals = if report.totals.is_empty() {
        Vec::new()
    } else {
        vec![
            ViewCell::Empty,
            ViewCell::Empty,
            ViewCell::Empty,
            report
                .totals
                .get(pending)
                .cloned()
                .unwrap_or(ViewCell::Empty),
            ViewCell::Empty,
        ]
    };
    ViewReport {
        columns: [
            "Bill",
            "Bill date",
            "Due",
            open_heading(receivable),
            "Overdue",
        ]
        .map(str::to_owned)
        .to_vec(),
        rows,
        totals,
        ..report
    }
}

/// The edit log as the mock draws it.
#[must_use]
///
/// The engine's rows carry a label ("#109 Posted voucher ...") and the same sequence number in
/// their first column. The label column leads with the number and that column is dropped, so the
/// table starts with Seq and has no repeated "Particulars" column. Every cell is copied.
pub(super) fn audit_log(report: ViewReport) -> ViewReport {
    if report.columns.first().is_none_or(|c| c != "Seq")
        || report
            .rows
            .iter()
            .any(|r| !matches!(r.cells.first(), Some(ViewCell::Text(_))))
    {
        return report;
    }
    let rows = report
        .rows
        .into_iter()
        .map(|row| {
            let mut cells = row.cells.into_iter();
            let label = match cells.next() {
                Some(ViewCell::Text(seq)) => seq,
                Some(_) | None => row.label,
            };
            ViewRow {
                label,
                cells: cells.collect(),
                ..row
            }
        })
        .collect();
    let columns = report.columns.into_iter().skip(1).collect();
    let totals = report.totals.into_iter().skip(1).collect();
    ViewReport {
        columns,
        rows,
        totals,
        ..report
    }
}

/// The tabs of the audit screen (`design/mock-v2/index.html`, `SCREENS.audit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuditTab {
    /// Who entered each voucher.
    EditLog,
    /// The hash-chain check.
    Integrity,
    /// Edits made after the books were locked.
    AfterLock,
}

impl AuditTab {
    /// The three tabs, in the mock's order.
    pub(super) const ALL: [AuditTab; 3] =
        [AuditTab::EditLog, AuditTab::Integrity, AuditTab::AfterLock];

    /// The tab's label.
    pub(super) fn label(self) -> &'static str {
        match self {
            AuditTab::EditLog => "Edit log",
            AuditTab::Integrity => "Integrity certificate",
            AuditTab::AfterLock => "Edits after lock",
        }
    }

    /// The engine report the tab shows.
    pub(super) fn kind(self) -> ReportKind {
        ReportKind::Audit(match self {
            AuditTab::EditLog => AuditReportKind::EditLog,
            AuditTab::Integrity => AuditReportKind::IntegrityCertificate,
            AuditTab::AfterLock => AuditReportKind::EditsAfterLock,
        })
    }
}

/// The reports drawn to the mock's states beyond the balance sheet and the day book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mock {
    /// `trial-balance.default`.
    Trial,
    /// `outstanding.receivables` and `outstanding.payables`.
    Outstanding {
        /// Receivables when true, payables when false.
        receivable: bool,
    },
    /// `stock-summary.*`.
    Stock,
    /// `profit-and-loss.*`.
    ProfitLoss,
    /// `audit.*`.
    Audit(AuditTab),
}

impl Mock {
    /// The mock look of an engine report kind, if the mock has one.
    pub(super) fn of(kind: &ReportKind) -> Option<Mock> {
        match kind {
            ReportKind::TrialBalance => Some(Mock::Trial),
            ReportKind::Outstanding { receivable } => Some(Mock::Outstanding {
                receivable: *receivable,
            }),
            ReportKind::StockSummary => Some(Mock::Stock),
            ReportKind::ProfitAndLoss => Some(Mock::ProfitLoss),
            ReportKind::Audit(AuditReportKind::EditLog) => Some(Mock::Audit(AuditTab::EditLog)),
            ReportKind::Audit(AuditReportKind::IntegrityCertificate) => {
                Some(Mock::Audit(AuditTab::Integrity))
            }
            ReportKind::Audit(AuditReportKind::EditsAfterLock) => {
                Some(Mock::Audit(AuditTab::AfterLock))
            }
            ReportKind::Audit(
                AuditReportKind::ObjectHistory
                | AuditReportKind::DeletedAltered
                | AuditReportKind::BackDated,
            )
            | ReportKind::BalanceSheet
            | ReportKind::GroupSummary(_)
            | ReportKind::GroupLedgers(_)
            | ReportKind::LedgerVouchers(_)
            | ReportKind::DayBook
            | ReportKind::Register(_) => None,
        }
    }

    /// The page title.
    pub(super) fn page_title(self) -> &'static str {
        match self {
            Mock::Trial => "Trial balance",
            Mock::Outstanding { receivable: true } => "Bills receivable",
            Mock::Outstanding { receivable: false } => "Bills payable",
            Mock::Stock => "Stock summary",
            Mock::ProfitLoss => "Profit and loss",
            Mock::Audit(_) => "Audit",
        }
    }

    /// The breadcrumb's last part.
    pub(super) fn crumb(self) -> &'static str {
        match self {
            Mock::Outstanding { receivable: true } => "Receivables",
            Mock::Outstanding { receivable: false } => "Payables",
            Mock::Trial | Mock::Stock | Mock::ProfitLoss | Mock::Audit(_) => self.page_title(),
        }
    }

    /// The key bar's name.
    pub(super) fn key_bar_name(self) -> &'static str {
        match self {
            Mock::Outstanding { .. } => "Outstanding bills",
            Mock::Trial | Mock::Stock | Mock::ProfitLoss | Mock::Audit(_) => self.page_title(),
        }
    }

    /// The line under the title. `as_of` is the report's balance date and `period` its period.
    pub(super) fn subtitle(self, as_of: Date, period: Period, detail: ViewDetail) -> String {
        let day = crate::ui::format::date;
        match self {
            Mock::Trial => format!(
                "As at {} \u{b7} every ledger with a balance \u{b7} Enter opens a ledger",
                day(as_of)
            ),
            Mock::Outstanding { .. } => format!(
                "As at {} \u{b7} bill-wise, with the due date from the party's credit days",
                day(as_of)
            ),
            Mock::Stock => format!(
                "As at {} \u{b7} quantity and value from the stock engine",
                day(as_of)
            ),
            Mock::ProfitLoss => {
                let from = period.from();
                let to = period.to();
                let start = format!(
                    "{} {}",
                    from.day(),
                    crate::ui::format::month_abbrev(from.month())
                );
                // index.html:2235 `plScreen`: "... · by group" or "... · ledgers under each
                // group" in the detailed view.
                let form = match detail {
                    ViewDetail::Condensed => "by group",
                    ViewDetail::Detailed => "ledgers under each group",
                };
                match ledgerly_core::FinancialYear::containing(to) {
                    Ok(fy) => format!(
                        "From {start} to {} \u{b7} FY {} \u{b7} {form}",
                        day(to),
                        fy.label()
                    ),
                    Err(_) => format!("From {start} to {} \u{b7} {form}", day(to)),
                }
            }
            Mock::Audit(AuditTab::Integrity) => {
                "Read-only. Nothing here changes a voucher. The check covers every event in the log."
                    .to_owned()
            }
            Mock::Audit(AuditTab::EditLog | AuditTab::AfterLock) => format!(
                "Read-only. Nothing here changes a voucher. Period from {} to {}",
                day(period.from()),
                day(period.to())
            ),
        }
    }

    /// The key hints after the report's name, as (key, action).
    pub(super) fn keys(self) -> Vec<(&'static str, &'static str)> {
        match self {
            Mock::Trial | Mock::Outstanding { .. } => {
                vec![("J / K", "Move"), ("Enter", "Open ledger"), ("Esc", "Back")]
            }
            Mock::Stock => vec![("J / K", "Move"), ("Enter", "Stock items"), ("Esc", "Back")],
            Mock::ProfitLoss => vec![
                ("Alt+F1", "Groups or ledgers"),
                ("Ctrl+P", "Print preview"),
                ("Ctrl+E", "Export"),
                ("Esc", "Back"),
            ],
            Mock::Audit(_) => vec![("J / K", "Move"), ("Ctrl+E", "Export"), ("Esc", "Back")],
        }
    }
}

/// What the stock summary needs from the masters for an item: its HSN or SAC text and its unit.
pub(super) struct ItemFacts {
    /// The HSN, or `none` when the item has no code.
    pub hsn: String,
    /// The unit's symbol.
    pub unit: String,
}

/// The stock summary as the mock draws it: item, HSN, closing quantity, unit, cost (the engine's
/// closing rate) and closing value.
/// `facts` gives the masters' data for an item. A report that does not have the engine's columns
/// is returned as it came.
#[must_use]
pub(super) fn stock_summary(
    report: ViewReport,
    facts: &dyn Fn(StockItemId) -> Option<ItemFacts>,
) -> ViewReport {
    let at = |name: &str| report.columns.iter().position(|c| c == name);
    let (Some(qty), Some(value), Some(rate)) = (
        at("Closing quantity"),
        at("Closing value"),
        at("Closing rate"),
    ) else {
        return report;
    };
    let rows = report
        .rows
        .iter()
        .map(|row| {
            let cell = |i: usize| row.cells.get(i).cloned().unwrap_or(ViewCell::Empty);
            let item = match &row.key {
                Some(ViewKey::Item(id)) => facts(*id),
                Some(
                    ViewKey::Group(_)
                    | ViewKey::Ledger(_)
                    | ViewKey::Voucher(_)
                    | ViewKey::Bill { .. },
                )
                | None => None,
            };
            let (hsn, unit) = item.map_or((ViewCell::Empty, ViewCell::Empty), |f| {
                (ViewCell::Text(f.hsn), ViewCell::Text(f.unit))
            });
            ViewRow {
                cells: vec![hsn, cell(qty), unit, cell(rate), cell(value)],
                ..row.clone()
            }
        })
        .collect();
    let totals = if report.totals.is_empty() {
        Vec::new()
    } else {
        let t = |i: usize| report.totals.get(i).cloned().unwrap_or(ViewCell::Empty);
        vec![
            ViewCell::Empty,
            t(qty),
            ViewCell::Empty,
            ViewCell::Empty,
            t(value),
        ]
    };
    ViewReport {
        columns: ["HSN", "Quantity", "Unit", COST_HEADING, VALUE_HEADING]
            .map(str::to_owned)
            .to_vec(),
        rows,
        totals,
        ..report
    }
}

/// Heading of the debit amount column of the profit and loss.
pub(super) const PL_DEBIT: &str = "Debit";
/// Heading of the credit amount column of the profit and loss.
pub(super) const PL_CREDIT: &str = "Credit";

/// Which side of the two-sided account a cell stands on, and the cell as it came (a provisional
/// figure keeps its wrapper so the screen marks it with a badge). `None` for a cell that is not
/// an amount.
fn pl_side(cell: &ViewCell) -> Option<(ledgerly_core::DrCr, ViewCell)> {
    match cell {
        ViewCell::Amount(a) => Some((a.side, cell.clone())),
        ViewCell::Provisional { amount, .. } => Some((amount.side, cell.clone())),
        ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Text(_)
        | ViewCell::Date(_)
        | ViewCell::Empty
        | ViewCell::Unsupported(_) => None,
    }
}

/// The label of the one line the profit and loss badges: the closing stock is the figure the
/// stock engine marks; every other figure that follows from it is told in the banner.
const BADGED_LINE: &str = "Closing stock";

/// The figure of a cell without its provisional wrapper (the amount is the engine's, unchanged).
fn plain(cell: ViewCell) -> ViewCell {
    match cell {
        ViewCell::Provisional { amount, .. } => ViewCell::Amount(amount),
        ViewCell::Amount(_)
        | ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Text(_)
        | ViewCell::Date(_)
        | ViewCell::Empty
        | ViewCell::Unsupported(_) => cell,
    }
}

/// The engine's total, which it states as a debit, as the same figure on the credit side: both
/// sides come to it (the engine refuses to build the report otherwise), so only the side word
/// changes, never the figure.
fn as_credit(total: &ViewCell) -> ViewCell {
    match total {
        ViewCell::Amount(a) => ViewCell::Amount(ledgerly_core::SignedAmount::cr(a.amount)),
        ViewCell::Provisional { amount, reason } => ViewCell::Provisional {
            amount: ledgerly_core::SignedAmount::cr(amount.amount),
            reason: reason.clone(),
        },
        ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Text(_)
        | ViewCell::Date(_)
        | ViewCell::Empty
        | ViewCell::Unsupported(_) => total.clone(),
    }
}

/// One side's line of the two-sided account.
struct PlEntry {
    key: Option<ViewKey>,
    label: String,
    depth: u8,
    kind: ViewRowKind,
    cell: ViewCell,
}

/// The reasons the profit and loss is provisional, each once, in the order the lines come.
#[must_use]
pub(super) fn pl_provisional_reasons(report: &ViewReport) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cell in report
        .rows
        .iter()
        .flat_map(|r| r.cells.iter())
        .chain(&report.totals)
    {
        if let ViewCell::Provisional { reason, .. } = cell
            && !out.contains(reason)
        {
            out.push(reason.clone());
        }
    }
    out
}

/// The labels of the rows that have a provisional cell, in row order (the stock summary's items
/// whose value is provisional).
#[must_use]
pub(super) fn provisional_row_labels(report: &ViewReport) -> Vec<String> {
    report
        .rows
        .iter()
        .filter(|r| {
            r.cells
                .iter()
                .any(|c| matches!(c, ViewCell::Provisional { .. }))
        })
        .map(|r| r.label.clone())
        .collect()
}

/// The warning over a stock summary or profit and loss that has provisional figures, as the
/// lead sentence (drawn bold) and the rest: the engine's reasons, each once, then what the screen
/// does with the figure (index.html `plScreen` and `stockScreen`). `items` are the stock summary's
/// provisional rows: the lead names them and, when one item has one reason that starts with its
/// name, the sentence says "Its quantity ..." instead of repeating the name. `None` when there is
/// no reason or the report has no such banner.
#[must_use]
pub(super) fn provisional_banner(
    mock: Mock,
    reasons: &[String],
    items: &[String],
) -> Option<(String, String)> {
    if reasons.is_empty() {
        return None;
    }
    let capital = |s: &str| {
        let mut chars = s.chars();
        chars.next().map_or_else(String::new, |c| {
            c.to_uppercase().chain(chars).collect::<String>()
        })
    };
    match mock {
        Mock::ProfitLoss => Some((
            "Closing stock is Provisional.".to_owned(),
            format!(
                "{}. The stock engine keeps the figure and marks it Provisional. The profit figures that follow from it are provisional too.",
                capital(&reasons.join("; "))
            ),
        )),
        Mock::Stock => {
            let tail = match (items, reasons) {
                ([item], [reason]) => reason.strip_prefix(item.as_str()),
                _ => None,
            };
            let (lead, why) = match (items.is_empty(), tail) {
                (true, _) => (
                    "Value is Provisional.".to_owned(),
                    capital(&reasons.join("; ")),
                ),
                (false, Some(tail)) => (
                    format!("Value is Provisional for {}.", items.join(", ")),
                    format!("Its quantity{tail}"),
                ),
                (false, None) => (
                    format!("Value is Provisional for {}.", items.join(", ")),
                    capital(&reasons.join("; ")),
                ),
            };
            Some((
                lead,
                format!("{why}. The value is shown and marked, and the total with it."),
            ))
        }
        Mock::Trial | Mock::Outstanding { .. } | Mock::Audit(_) => None,
    }
}

/// Pairs the collected left and right entries into rows, then the section's total row.
fn flush_sides(
    left: &mut Vec<PlEntry>,
    right: &mut Vec<PlEntry>,
    total: &mut Option<ViewCell>,
    out: &mut Vec<ViewRow>,
) {
    let (mut lhs, mut rhs) = (
        std::mem::take(left).into_iter(),
        std::mem::take(right).into_iter(),
    );
    loop {
        let (first, second) = (lhs.next(), rhs.next());
        if first.is_none() && second.is_none() {
            break;
        }
        let side_label =
            |entry: &PlEntry| format!("{}{}", "  ".repeat(usize::from(entry.depth)), entry.label);
        let cells = vec![
            first.as_ref().map_or(ViewCell::Empty, |e| e.cell.clone()),
            second
                .as_ref()
                .map_or(ViewCell::Empty, |e| ViewCell::Text(side_label(e))),
            second.as_ref().map_or(ViewCell::Empty, |e| e.cell.clone()),
        ];
        let (key, label, depth, kind) = match (&first, &second) {
            (Some(e), _) => (e.key.clone(), e.label.clone(), e.depth, e.kind),
            (None, Some(e)) => (e.key.clone(), String::new(), 0, e.kind),
            (None, None) => break,
        };
        out.push(ViewRow {
            key,
            label,
            depth,
            kind,
            cells,
        });
    }
    if let Some(t) = total.take() {
        out.push(ViewRow {
            key: None,
            label: "Total".to_owned(),
            depth: 0,
            kind: ViewRowKind::Subtotal,
            cells: vec![t.clone(), ViewCell::Text("Total".to_owned()), as_credit(&t)],
        });
    }
}

/// The profit and loss as the two-sided account: the trading account, then the profit and loss
/// account, each with the debit side on the left and the credit side on the right and both sides
/// totalled. The engine's amounts carry the side they stand on, its `Trading total` and `Profit
/// and loss total` rows are the totals, and its `c/o` and `b/f` rows are the balancing lines, so
/// this only places each line (rule C4). A provisional figure keeps its wrapper, so the screen draws
/// the bare figure with a "Provisional" badge beside its label; the reason is the banner's (see [`pl_provisional_reasons`]). A report that
/// does not have the engine's rows is returned as it came.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the engine's rows, one arm per row kind"
)]
pub(super) fn profit_loss(report: ViewReport) -> ViewReport {
    let has = |label: &str| report.rows.iter().any(|r| r.label == label);
    if !has("Trading total") || !has("Profit and loss total") {
        return report;
    }
    let mut out: Vec<ViewRow> = Vec::new();
    let mut left: Vec<PlEntry> = Vec::new();
    let mut right: Vec<PlEntry> = Vec::new();
    let mut total: Option<ViewCell> = None;
    // The side of the last top-level line, which its ledgers follow.
    let mut group_side = ledgerly_core::DrCr::Dr;
    for row in &report.rows {
        match (row.kind, row.label.as_str()) {
            (ViewRowKind::Text, "Trading account") => out.push(row.clone()),
            (ViewRowKind::Text, "Profit and loss account") => {
                flush_sides(&mut left, &mut right, &mut total, &mut out);
                out.push(row.clone());
            }
            (ViewRowKind::Subtotal, "Gross profit" | "Net profit") => {}
            (ViewRowKind::Subtotal, "Trading total" | "Profit and loss total") => {
                total = row.cells.first().and_then(pl_side).map(|(_, c)| plain(c));
            }
            (
                ViewRowKind::Item
                | ViewRowKind::Group
                | ViewRowKind::Ledger
                | ViewRowKind::Voucher
                | ViewRowKind::Bill
                | ViewRowKind::Difference
                | ViewRowKind::Subtotal
                | ViewRowKind::Text,
                _,
            ) => {
                let Some((side, cell)) = row.cells.first().and_then(pl_side) else {
                    continue;
                };
                let side = if row.depth == 0 {
                    group_side = side;
                    side
                } else {
                    group_side
                };
                let entry = PlEntry {
                    key: row.key.clone(),
                    label: row.label.clone(),
                    depth: row.depth,
                    kind: row.kind,
                    cell: if row.label == BADGED_LINE {
                        cell
                    } else {
                        plain(cell)
                    },
                };
                match side {
                    ledgerly_core::DrCr::Dr => left.push(entry),
                    ledgerly_core::DrCr::Cr => right.push(entry),
                }
            }
        }
    }
    flush_sides(&mut left, &mut right, &mut total, &mut out);
    ViewReport {
        columns: vec![
            PL_DEBIT.to_owned(),
            "Particulars".to_owned(),
            PL_CREDIT.to_owned(),
        ],
        rows: out,
        totals: Vec::new(),
        ..report
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "test code: rule B2 lets tests unwrap and panic"
    )]

    use ledgerly_core::{Decimal, Paise, Qty, SignedAmount};
    use time::macros::date;

    use super::*;

    #[test]
    fn the_edit_log_leads_with_the_sequence_number_and_does_not_repeat_the_label() {
        // provenance: index.html `audit.default`: the table's first column is "Seq" (135, 136,
        // ...), then Date, Time, User, Action, Object. The engine's report has a label per row
        // ("#109 Posted voucher ...") and the same number again in its first column, which drew a
        // "Particulars" column before the Seq column in the 2026-11-25 capture.
        let line = |seq: &str, label: &str| ViewRow {
            key: None,
            label: label.to_owned(),
            depth: 0,
            kind: ViewRowKind::Item,
            cells: vec![
                text(seq),
                ViewCell::Date(date!(2026 - 11 - 02)),
                text("user 1"),
            ],
        };
        let report = ViewReport {
            token: super::super::ReportToken("audit".to_owned()),
            title: "Edit log".to_owned(),
            params: super::super::ViewParams {
                period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap(),
                detail: ViewDetail::Detailed,
            },
            columns: ["Seq", "Date", "User"].map(str::to_owned).to_vec(),
            rows: vec![line("135", "#135 Posted voucher")],
            totals: Vec::new(),
            notes: Vec::new(),
        };
        let out = audit_log(report);
        assert_eq!(out.columns, ["Date", "User"]);
        assert_eq!(out.rows[0].label, "135");
        assert_eq!(out.rows[0].cells.len(), 2);
        assert_eq!(super::super::generic::label_heading(&out), "Seq");
    }

    fn text(s: &str) -> ViewCell {
        ViewCell::Text(s.to_owned())
    }
    use crate::screens::report_view::{ReportToken, ViewDetail, ViewParams};

    fn rs(n: i64) -> SignedAmount {
        SignedAmount::dr(Paise::new(n * 100))
    }

    fn base(columns: &[&str], rows: Vec<ViewRow>, totals: Vec<ViewCell>) -> ViewReport {
        ViewReport {
            token: ReportToken("t".to_owned()),
            title: "t".to_owned(),
            params: ViewParams {
                period: ledgerly_core::Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20))
                    .unwrap(),
                detail: ViewDetail::Detailed,
            },
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
            rows,
            totals,
            notes: Vec::new(),
        }
    }

    fn row(label: &str, depth: u8, kind: ViewRowKind, cells: Vec<ViewCell>) -> ViewRow {
        ViewRow {
            key: None,
            label: label.to_owned(),
            depth,
            kind,
            cells,
        }
    }

    #[test]
    fn the_trial_balance_lists_ledgers_each_with_its_group() {
        // provenance: derived from design/mock-v2/index.html `trial-balance.default`: the columns
        // are Ledger, Group, Debit, Credit and a line is one ledger ("Ananya Kulkarni, capital"
        // under "Capital account"); group lines are not listed. The engine's detailed report
        // nests Sundry Debtors under Current Assets, so the group named is the nearest one.
        let cr = |n: i64| ViewCell::Amount(SignedAmount::cr(Paise::new(n * 100)));
        let dr = |n: i64| ViewCell::Amount(rs(n));
        let report = base(
            &["Debit", "Credit"],
            vec![
                row(
                    "Capital Account",
                    0,
                    ViewRowKind::Group,
                    vec![ViewCell::Empty, cr(20)],
                ),
                row(
                    "Ananya",
                    1,
                    ViewRowKind::Ledger,
                    vec![ViewCell::Empty, cr(15)],
                ),
                row(
                    "Rohan",
                    1,
                    ViewRowKind::Ledger,
                    vec![ViewCell::Empty, cr(5)],
                ),
                row(
                    "Current Assets",
                    0,
                    ViewRowKind::Group,
                    vec![dr(7), ViewCell::Empty],
                ),
                row(
                    "Sundry Debtors",
                    1,
                    ViewRowKind::Group,
                    vec![dr(7), ViewCell::Empty],
                ),
                row(
                    "Shree Ganesh",
                    2,
                    ViewRowKind::Ledger,
                    vec![dr(7), ViewCell::Empty],
                ),
            ],
            vec![dr(7), cr(20)],
        );
        let out = trial_balance(report);
        assert_eq!(out.columns, vec!["Group", "Debit", "Credit"]);
        let listed: Vec<(String, ViewCell)> = out
            .rows
            .iter()
            .map(|r| (r.label.clone(), r.cells[0].clone()))
            .collect();
        assert_eq!(
            listed,
            vec![
                ("Ananya".to_owned(), text("Capital Account")),
                ("Rohan".to_owned(), text("Capital Account")),
                ("Shree Ganesh".to_owned(), text("Sundry Debtors")),
            ]
        );
        assert_eq!(out.rows[2].cells[1], dr(7));
        assert_eq!(out.totals.len(), 3);
        assert_eq!(out.totals[1], dr(7));
        assert!(out.rows.iter().all(|r| r.depth == 0));
    }

    fn outstanding_columns() -> Vec<&'static str> {
        vec![
            "Bill date",
            "Due date",
            "Pending",
            "Not yet due",
            "0-30 days",
            "31-60 days",
            "61-90 days",
            "Over 90 days",
            "No due date",
            "On account",
            "Net outstanding",
        ]
    }

    #[test]
    fn outstanding_lists_each_open_bill_under_its_party_with_the_bucket_the_engine_put_it_in() {
        // provenance: derived from `outstanding.receivables` in the mock: Customer, Bill, Bill
        // date, Due, Open, Overdue, one line per open bill. The engine ages a bill into a bucket
        // and does not give the days (crates/ledgerly-reports/src/outstanding.rs, "Ageing"), so
        // the Overdue cell names the bucket: the column the engine put the amount in.
        let e = ViewCell::Empty;
        let dr = |n: i64| ViewCell::Amount(rs(n));
        let party = row(
            "Patil Hardware",
            0,
            ViewRowKind::Ledger,
            vec![
                e.clone(),
                e.clone(),
                dr(5),
                dr(1),
                e.clone(),
                dr(4),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                dr(5),
            ],
        );
        let bill_old = row(
            "SI/0350",
            1,
            ViewRowKind::Bill,
            vec![
                ViewCell::Date(date!(2026 - 08 - 28)),
                ViewCell::Date(date!(2026 - 09 - 27)),
                dr(4),
                e.clone(),
                e.clone(),
                dr(4),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
            ],
        );
        let bill_new = row(
            "SI/0417",
            1,
            ViewRowKind::Bill,
            vec![
                ViewCell::Date(date!(2026 - 11 - 20)),
                ViewCell::Date(date!(2026 - 12 - 20)),
                dr(1),
                dr(1),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
            ],
        );
        let mut totals = vec![e.clone(); 11];
        totals[2] = dr(5);
        let out = outstanding(
            base(
                &outstanding_columns(),
                vec![party, bill_old, bill_new],
                totals,
            ),
            true,
        );
        assert_eq!(
            out.columns,
            vec!["Bill", "Bill date", "Due", "Open Dr", "Overdue"]
        );
        assert_eq!(out.rows.len(), 2);
        assert_eq!(out.rows[0].label, "Patil Hardware");
        assert_eq!(out.rows[0].cells[0], text("SI/0350"));
        assert_eq!(out.rows[0].cells[1], text("28 Aug 2026"));
        assert_eq!(out.rows[0].cells[2], text("27 Sep 2026"));
        assert_eq!(out.rows[0].cells[3], dr(4));
        assert_eq!(out.rows[0].cells[4], text("31-60 days"));
        assert_eq!(out.rows[1].cells[4], text("Not yet due"));
        assert_eq!(out.totals[3], dr(5));
    }

    #[test]
    fn a_bill_due_on_the_as_at_date_is_due_today_not_overdue() {
        // provenance: derived. The engine's ageing doc (outstanding.rs, "Ageing"): "A bill due on
        // `as_of` is 0 days overdue", and it falls in the `0-30 days` column. The mock draws red
        // only where overdue > 0 (index.html:2331, `b.overdue>0`), so a bill due on the as-at
        // date (20 Nov 2026, the period's end) must not carry an overdue bucket's name.
        let e = ViewCell::Empty;
        let dr = |n: i64| ViewCell::Amount(rs(n));
        let mut cells = vec![e.clone(); 11];
        cells[0] = ViewCell::Date(date!(2026 - 10 - 21));
        cells[1] = ViewCell::Date(date!(2026 - 11 - 20));
        cells[2] = dr(3);
        cells[4] = dr(3);
        let party = row("Mehta Agro", 0, ViewRowKind::Ledger, vec![e.clone(); 11]);
        let bill = row("SI/0001", 1, ViewRowKind::Bill, cells);
        let out = outstanding(
            base(
                &outstanding_columns(),
                vec![party, bill],
                vec![e.clone(); 11],
            ),
            true,
        );
        assert_eq!(out.rows[0].cells[4], text("Due today"));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the report literal is the test's data"
    )]
    fn the_stock_summary_shows_closing_quantity_and_value_with_the_masters_hsn_and_unit() {
        // provenance: derived from `stock-summary.default` in the mock: Item, HSN, Quantity,
        // Unit, Cost, Value. The closing quantity, rate and value are the engine's cells (the
        // rate is its "Closing rate" column: Drip lateral 5,83,440.00 over 187 rolls is 3,120.00
        // each, Steel pickaxe 67,680.00 over 144 is 470.00); the HSN and unit come from the item
        // master. Cost is the engine's rate, never worked out here.
        let e = ViewCell::Empty;
        let q = |n: i64| ViewCell::Qty(Qty(Decimal::from(n)));
        let item = ViewRow {
            key: Some(ViewKey::Item(StockItemId(1))),
            label: "Drip lateral".to_owned(),
            depth: 0,
            kind: ViewRowKind::Item,
            cells: vec![
                e.clone(),
                e.clone(),
                q(212),
                e.clone(),
                q(25),
                e.clone(),
                q(187),
                ViewCell::Amount(rs(583_440)),
                ViewCell::Amount(rs(3_120)),
            ],
        };
        let none = ViewRow {
            key: Some(ViewKey::Item(StockItemId(2))),
            label: "Steel pickaxe".to_owned(),
            depth: 0,
            kind: ViewRowKind::Item,
            cells: vec![
                e.clone(),
                e.clone(),
                q(150),
                e.clone(),
                q(6),
                e.clone(),
                q(144),
                ViewCell::Amount(rs(67_680)),
                ViewCell::Amount(rs(470)),
            ],
        };
        let report = base(
            &[
                "Opening quantity",
                "Opening value",
                "Inwards quantity",
                "Inwards value (at voucher rate)",
                "Outwards quantity",
                "Outwards value (cost of consumption)",
                "Closing quantity",
                "Closing value",
                "Closing rate",
            ],
            vec![item, none],
            vec![
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                e.clone(),
                ViewCell::Amount(rs(651_120)),
                e.clone(),
            ],
        );
        let facts = |id: StockItemId| {
            Some(match id.0 {
                1 => ItemFacts {
                    hsn: "3917".to_owned(),
                    unit: "roll".to_owned(),
                },
                _ => ItemFacts {
                    hsn: "none".to_owned(),
                    unit: "nos".to_owned(),
                },
            })
        };
        let out = stock_summary(report, &facts);
        assert_eq!(
            out.columns,
            vec!["HSN", "Quantity", "Unit", "Cost Dr", "Value Dr"]
        );
        assert_eq!(
            out.rows[0].cells,
            vec![
                text("3917"),
                q(187),
                text("roll"),
                ViewCell::Amount(rs(3_120)),
                ViewCell::Amount(rs(583_440))
            ]
        );
        assert_eq!(out.rows[1].cells[0], text("none"));
        assert_eq!(out.rows[1].cells[1], q(144));
        assert_eq!(out.totals[4], ViewCell::Amount(rs(651_120)));
        assert_eq!(out.totals[3], ViewCell::Empty);
    }

    #[test]
    fn a_report_without_the_engines_columns_is_shown_as_it_came() {
        // A changed engine report must not be guessed at: the projections leave it alone.
        let odd = base(&["Something else"], Vec::new(), Vec::new());
        assert_eq!(outstanding(odd.clone(), true), odd);
        assert_eq!(stock_summary(odd.clone(), &|_| None), odd);
        assert_eq!(trial_balance(odd.clone()), odd);
    }

    #[test]
    fn each_report_is_worded_as_the_mock_words_it() {
        // provenance: derived from design/mock-v2/index.html: titles and subtitles of
        // `trial-balance.default`, `outstanding.receivables`, `stock-summary.default`,
        // `audit.default` (phead calls and keys()), with the dates the engine gives: a report is
        // as on the date of the last voucher entry (help.tallysolutions.com Trial Balance page,
        // read 2026-11-25), 20 Nov 2026 in the capture company, and the mock says the same.
        let as_of = date!(2026 - 11 - 20);
        let period = ledgerly_core::Period::new(date!(2026 - 04 - 01), as_of).unwrap();
        let trial = Mock::of(&ReportKind::TrialBalance).unwrap();
        assert_eq!(trial.page_title(), "Trial balance");
        assert_eq!(trial.crumb(), "Trial balance");
        assert_eq!(
            trial.subtitle(as_of, period, ViewDetail::Condensed),
            "As at 20 Nov 2026 \u{b7} every ledger with a balance \u{b7} Enter opens a ledger"
        );
        assert_eq!(
            trial.keys(),
            vec![("J / K", "Move"), ("Enter", "Open ledger"), ("Esc", "Back")]
        );
        let bills = Mock::of(&ReportKind::Outstanding { receivable: true }).unwrap();
        assert_eq!(bills.page_title(), "Bills receivable");
        assert_eq!(bills.crumb(), "Receivables");
        assert_eq!(bills.key_bar_name(), "Outstanding bills");
        assert_eq!(
            Mock::of(&ReportKind::Outstanding { receivable: false })
                .unwrap()
                .page_title(),
            "Bills payable"
        );
        assert_eq!(
            bills.subtitle(as_of, period, ViewDetail::Condensed),
            "As at 20 Nov 2026 \u{b7} bill-wise, with the due date from the party's credit days"
        );
        let stock = Mock::of(&ReportKind::StockSummary).unwrap();
        assert_eq!(
            stock.keys(),
            vec![("J / K", "Move"), ("Enter", "Stock items"), ("Esc", "Back")]
        );
        let audit = Mock::of(&ReportKind::Audit(AuditReportKind::EditLog)).unwrap();
        assert_eq!(audit.page_title(), "Audit");
        assert_eq!(
            audit.subtitle(as_of, period, ViewDetail::Condensed),
            "Read-only. Nothing here changes a voucher. Period from 1 Apr 2026 to 20 Nov 2026"
        );
        // The integrity certificate checks the whole log, so it names no period: the engine's own
        // period for it is the whole calendar, which must never be printed as if it were data.
        let cert = Mock::of(&ReportKind::Audit(AuditReportKind::IntegrityCertificate)).unwrap();
        assert_eq!(
            cert.subtitle(as_of, period, ViewDetail::Condensed),
            "Read-only. Nothing here changes a voucher. The check covers every event in the log."
        );
        assert_eq!(
            audit.keys(),
            vec![("J / K", "Move"), ("Ctrl+E", "Export"), ("Esc", "Back")]
        );
        let pl = Mock::of(&ReportKind::ProfitAndLoss).unwrap();
        assert_eq!(pl.page_title(), "Profit and loss");
        assert_eq!(
            pl.subtitle(as_of, period, ViewDetail::Condensed),
            // provenance: index.html:2235 `plScreen` head: "From 1 Apr to 20 Nov 2026 · FY 2026-27
            // · by group" (s14: the mock dropped "Ledgerly's trading form, with sub-totals").
            "From 1 Apr to 20 Nov 2026 \u{b7} FY 2026-27 \u{b7} by group"
        );
        assert_eq!(
            pl.subtitle(as_of, period, ViewDetail::Detailed),
            // provenance: index.html:2235: `detailed ? 'ledgers under each group' : 'by group'`.
            "From 1 Apr to 20 Nov 2026 \u{b7} FY 2026-27 \u{b7} ledgers under each group"
        );
    }

    fn cr(n: i64) -> ViewCell {
        ViewCell::Amount(SignedAmount::cr(Paise::new(n * 100)))
    }

    fn dr(n: i64) -> ViewCell {
        ViewCell::Amount(rs(n))
    }

    /// The engine's rows for the hand set of `rp4_profit_loss.rs` (opening stock 20,000, closing
    /// stock 30,000, sales 1,00,000, purchases 60,000, freight 5,000, rent 10,000, interest
    /// received 2,000), condensed.
    fn hand_set_rows() -> Vec<ViewRow> {
        let signed = |n: i64| ViewCell::Signed(Paise::new(n * 100));
        vec![
            row(
                "Trading account",
                0,
                ViewRowKind::Text,
                vec![ViewCell::Empty],
            ),
            row("Opening stock", 0, ViewRowKind::Item, vec![dr(20_000)]),
            row("Purchase Accounts", 0, ViewRowKind::Group, vec![dr(60_000)]),
            row("Direct Expenses", 0, ViewRowKind::Group, vec![dr(5_000)]),
            row("Sales Accounts", 0, ViewRowKind::Group, vec![cr(100_000)]),
            row("Closing stock", 0, ViewRowKind::Item, vec![cr(30_000)]),
            row(
                "Gross profit",
                0,
                ViewRowKind::Subtotal,
                vec![signed(45_000)],
            ),
            row("Gross profit c/o", 0, ViewRowKind::Item, vec![dr(45_000)]),
            row("Trading total", 0, ViewRowKind::Subtotal, vec![dr(130_000)]),
            row(
                "Profit and loss account",
                0,
                ViewRowKind::Text,
                vec![ViewCell::Empty],
            ),
            row("Gross profit b/f", 0, ViewRowKind::Item, vec![cr(45_000)]),
            row("Indirect Expenses", 0, ViewRowKind::Group, vec![dr(10_000)]),
            row("Indirect Incomes", 0, ViewRowKind::Group, vec![cr(2_000)]),
            row("Net profit", 0, ViewRowKind::Subtotal, vec![signed(37_000)]),
            row("Net profit c/o", 0, ViewRowKind::Item, vec![dr(37_000)]),
            row(
                "Profit and loss total",
                0,
                ViewRowKind::Subtotal,
                vec![dr(47_000)],
            ),
        ]
    }

    #[test]
    fn the_profit_and_loss_is_drawn_as_two_sides_with_the_engines_totals() {
        // provenance: derived. Trading account: debit side opening stock 20,000, purchases
        // 60,000, direct expenses 5,000, gross profit c/o 45,000 (total 1,30,000); credit side
        // sales 1,00,000 and closing stock 30,000 (total 1,30,000). Profit and loss account:
        // debit side indirect expenses 10,000 and net profit c/o 37,000; credit side gross
        // profit b/f 45,000 and indirect incomes 2,000; both 47,000. The totals are the
        // engine's cells, copied.
        let report = base(&[], hand_set_rows(), vec![signed_total()]);
        let out = profit_loss(report);
        assert_eq!(out.columns, vec!["Debit", "Particulars", "Credit"]);
        let lines: Vec<(String, ViewCell, ViewCell, ViewCell)> = out
            .rows
            .iter()
            .map(|r| {
                (
                    r.label.clone(),
                    r.cells.first().cloned().unwrap_or(ViewCell::Empty),
                    r.cells.get(1).cloned().unwrap_or(ViewCell::Empty),
                    r.cells.get(2).cloned().unwrap_or(ViewCell::Empty),
                )
            })
            .collect();
        let e = ViewCell::Empty;
        assert_eq!(
            lines,
            vec![
                (
                    "Trading account".to_owned(),
                    e.clone(),
                    e.clone(),
                    e.clone()
                ),
                (
                    "Opening stock".to_owned(),
                    dr(20_000),
                    text("Sales Accounts"),
                    cr(100_000)
                ),
                (
                    "Purchase Accounts".to_owned(),
                    dr(60_000),
                    text("Closing stock"),
                    cr(30_000)
                ),
                (
                    "Direct Expenses".to_owned(),
                    dr(5_000),
                    e.clone(),
                    e.clone()
                ),
                (
                    "Gross profit c/o".to_owned(),
                    dr(45_000),
                    e.clone(),
                    e.clone()
                ),
                ("Total".to_owned(), dr(130_000), text("Total"), cr(130_000)),
                (
                    "Profit and loss account".to_owned(),
                    e.clone(),
                    e.clone(),
                    e.clone()
                ),
                (
                    "Indirect Expenses".to_owned(),
                    dr(10_000),
                    text("Gross profit b/f"),
                    cr(45_000)
                ),
                (
                    "Net profit c/o".to_owned(),
                    dr(37_000),
                    text("Indirect Incomes"),
                    cr(2_000)
                ),
                ("Total".to_owned(), dr(47_000), text("Total"), cr(47_000)),
            ]
        );
    }

    fn signed_total() -> ViewCell {
        ViewCell::Signed(Paise::new(3_700_000))
    }

    #[test]
    fn only_the_closing_stock_line_wears_the_provisional_wrapper_the_rest_go_to_the_banner() {
        // provenance: derived from index.html `plScreen`: the badge is on the Closing stock line
        // alone; the gross profit c/o, the gross profit b/f, the net profit c/o and the totals
        // are plain figures, and the banner says the stock engine marks the closing stock.
        let reason = "Steel pickaxe first went below zero on 19 Nov 2026".to_owned();
        let prov = |side: fn(Paise) -> SignedAmount, n: i64| ViewCell::Provisional {
            amount: side(Paise::new(n)),
            reason: reason.clone(),
        };
        let mut rows = hand_set_rows();
        rows[5].cells = vec![prov(SignedAmount::cr, 3_000_000)];
        rows[7].cells = vec![prov(SignedAmount::dr, 4_500_000)];
        rows[8].cells = vec![prov(SignedAmount::dr, 13_000_000)];
        let report = base(&[], rows, vec![signed_total()]);
        assert_eq!(pl_provisional_reasons(&report), vec![reason.clone()]);
        let out = profit_loss(report);
        let on = |label: &str| {
            out.rows
                .iter()
                .find(|r| {
                    r.label == label
                        || matches!(r.cells.get(1), Some(ViewCell::Text(t)) if t == label)
                })
                .unwrap()
        };
        let closing = on("Closing stock");
        assert_eq!(closing.cells[1], text("Closing stock"));
        assert_eq!(closing.cells[2], prov(SignedAmount::cr, 3_000_000));
        // Gross profit c/o is on the debit side of the same line as Opening stock's pair.
        let gp = on("Gross profit c/o");
        assert_eq!(
            gp.cells[0],
            ViewCell::Amount(SignedAmount::dr(Paise::new(4_500_000)))
        );
        let total = out.rows.iter().find(|r| r.label == "Total").unwrap();
        assert_eq!(
            total.cells[0],
            ViewCell::Amount(SignedAmount::dr(Paise::new(13_000_000)))
        );
    }

    #[test]
    fn the_provisional_banner_names_the_item_and_gives_the_engines_reason() {
        // provenance: derived from index.html `plScreen` ("Closing stock is Provisional.
        // Steel pickaxe 2.5 kg first went below zero on 19 Nov 2026 (-30 nos after SI/0416); a
        // purchase on 20 Nov brought it back to 70. The stock engine keeps the figure and marks
        // it Provisional.") and `stockScreen` ("Value is Provisional for Steel pickaxe 2.5 kg.
        // Its quantity first went below zero on ... The value is shown and marked, and the total
        // with it."). The reason is the engine's sentence; the stock banner swaps the item's
        // name for "Its quantity".
        let reasons = vec![
            "Steel pickaxe 2.5 kg first went below zero on 19 Nov 2026 (\u{2212}30 nos after SI/0416); a purchase on 20 Nov brought it back to 70"
                .to_owned(),
        ];
        let item = vec!["Steel pickaxe 2.5 kg".to_owned()];
        assert_eq!(
            provisional_banner(Mock::ProfitLoss, &reasons, &[]),
            Some((
                "Closing stock is Provisional.".to_owned(),
                "Steel pickaxe 2.5 kg first went below zero on 19 Nov 2026 (\u{2212}30 nos after SI/0416); a purchase on 20 Nov brought it back to 70. The stock engine keeps the figure and marks it Provisional. The profit figures that follow from it are provisional too.".to_owned()
            ))
        );
        assert_eq!(
            provisional_banner(Mock::Stock, &reasons, &item),
            Some((
                "Value is Provisional for Steel pickaxe 2.5 kg.".to_owned(),
                "Its quantity first went below zero on 19 Nov 2026 (\u{2212}30 nos after SI/0416); a purchase on 20 Nov brought it back to 70. The value is shown and marked, and the total with it.".to_owned()
            ))
        );
        assert_eq!(provisional_banner(Mock::Stock, &[], &item), None);
        assert_eq!(provisional_banner(Mock::Trial, &reasons, &[]), None);
    }

    #[test]
    fn the_audit_tabs_are_the_mocks_three_and_each_names_its_engine_report() {
        // provenance: derived from `SCREENS.audit` in design/mock-v2/index.html: kinds = Edit log,
        // Integrity certificate, Edits after lock.
        let labels: Vec<&str> = AuditTab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(
            labels,
            vec!["Edit log", "Integrity certificate", "Edits after lock"]
        );
        for tab in AuditTab::ALL {
            assert_eq!(Mock::of(&tab.kind()), Some(Mock::Audit(tab)));
        }
    }
}
