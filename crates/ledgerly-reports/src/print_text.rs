//! The text of a report as it is printed: Indian digit grouping, a `Dr` or `Cr` wherever a figure
//! is not on the side its column implies, and the same forms the screens draw (the profit and
//! loss as the two-sided trading and profit and loss account, the balance sheet as one vertical
//! statement, the trial balance as debit and credit columns).
//!
//! This module only places and formats figures the engine already computed. It never adds,
//! subtracts, divides or rounds one: the totals printed are the engine's own total rows, and the
//! credit-side total of the profit and loss is the engine's debit-side total with only the side
//! word changed (the engine refuses to build a profit and loss whose two sides differ).
//!
//! Source for the forms: the product's public help pages describe the profit and loss, balance
//! sheet and trial balance as screens and say they print as they are shown (the help page
//! "How to View Profit and Loss Statement in TallyPrime",
//! <https://help.tallysolutions.com/profit-and-loss-statement-tally/>, read 2026-11-26: horizontal
//! form by default; "Balance Sheet", <https://help.tallysolutions.com/tally/balance-sheet-tally/>,
//! and "Trial Balance", <https://help.tallysolutions.com/tally/trial-balance-tally/>, read
//! 2026-11-26: they give no separate print layout). The printed page therefore follows the
//! screens and the mock's print preview page (`profit-and-loss.print-preview`).

use ledgerly_core::format::format_inr;
use ledgerly_core::{Date, DrCr, Month, SignedAmount};

use crate::model::{Cell, Report, ReportError, ReportKind, ReportRow, RowKind};

/// Which edge a printed column's text sits against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintAlign {
    /// Against the left edge (labels, dates, text).
    Left,
    /// Against the right edge (figures).
    Right,
}

/// How a printed row is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintRowStyle {
    /// An ordinary line.
    Plain,
    /// A section heading such as "Trading account": bold, no figures.
    Band,
    /// A total line: bold.
    Total,
}

/// One printed row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintRow {
    /// One cell per heading; an empty cell is `""`.
    pub cells: Vec<String>,
    /// How the row is drawn.
    pub style: PrintRowStyle,
}

/// A report ready to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintText {
    /// The column headings.
    pub headings: Vec<String>,
    /// The alignment of each column.
    pub aligns: Vec<PrintAlign>,
    /// The rows in print order.
    pub rows: Vec<PrintRow>,
    /// Notes printed under the table.
    pub notes: Vec<String>,
}

const fn side_word(side: DrCr) -> &'static str {
    match side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    }
}

/// An amount with its side written out: `12,34,567.00 Dr`.
fn full(amount: SignedAmount) -> String {
    format!("{} {}", format_inr(amount.amount), side_word(amount.side))
}

/// An amount in a column whose usual side is `usual`: the bare figure on that side (or zero), the
/// figure with its side word otherwise.
fn on_side(amount: SignedAmount, usual: DrCr) -> String {
    if amount.amount.is_zero() || amount.side == usual {
        format_inr(amount.amount)
    } else {
        full(amount)
    }
}

fn dmy(date: Date) -> String {
    let month = match date.month() {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    };
    format!("{:02}-{month}-{}", date.day(), date.year())
}

/// A cell's text; `usual` is the side a bare figure stands for in its column, `None` to always
/// write the side. A provisional figure is marked as such (its reason is a note).
fn cell_text(cell: &Cell, usual: Option<DrCr>) -> String {
    let amount = |a: SignedAmount| usual.map_or_else(|| full(a), |u| on_side(a, u));
    match cell {
        Cell::Amount(a) => amount(*a),
        Cell::Provisional { amount: a, .. } => format!("{} (provisional)", amount(*a)),
        Cell::Signed(p) => format_inr(*p),
        Cell::Qty(q) => q.0.to_string(),
        Cell::Text(s) => s.clone(),
        Cell::Date(d) => dmy(*d),
        Cell::Empty => String::new(),
        Cell::Unsupported(u) => format!("Unsupported: {} ({})", u.what, u.reason),
    }
}

fn indent(row: &ReportRow) -> String {
    format!("{}{}", "  ".repeat(usize::from(row.depth)), row.label)
}

fn is_figure(cell: &Cell) -> bool {
    matches!(
        cell,
        Cell::Amount(_) | Cell::Signed(_) | Cell::Qty(_) | Cell::Provisional { .. }
    )
}

/// The provisional reasons in `report`, each once, as notes.
fn provisional_notes(report: &Report) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cell in report
        .rows
        .iter()
        .flat_map(|r| r.cells.iter())
        .chain(&report.totals)
    {
        if let Cell::Provisional { reason, .. } = cell {
            let note = format!("Provisional: {reason}");
            if !out.contains(&note) {
                out.push(note);
            }
        }
    }
    out
}

fn notes_of(report: &Report) -> Vec<String> {
    let mut notes = report.notes.clone();
    for note in provisional_notes(report) {
        if !notes.contains(&note) {
            notes.push(note);
        }
    }
    notes
}

/// The usual side of a column from its heading.
fn column_side(heading: &str) -> Option<DrCr> {
    match heading {
        "Debit" => Some(DrCr::Dr),
        "Credit" => Some(DrCr::Cr),
        _ => None,
    }
}

/// The report as the engine laid it out, one printed column per engine column: a "Debit" or
/// "Credit" column prints its figures bare on its own side.
fn generic(report: &Report) -> Result<PrintText, ReportError> {
    let width = report.columns.len();
    let mut headings = vec!["Particulars".to_owned()];
    headings.extend(report.columns.iter().map(|c| c.label.clone()));
    let sides: Vec<Option<DrCr>> = report
        .columns
        .iter()
        .map(|c| column_side(&c.label))
        .collect();
    let line = |label: String, cells: &[Cell], style| -> Result<PrintRow, ReportError> {
        if cells.len() > width {
            return Err(ReportError::Invalid {
                field: format!("row '{}'", label.trim()),
                reason: format!(
                    "has {} cells but the report has {width} columns, so the figures would not line up",
                    cells.len()
                ),
            });
        }
        let mut out = vec![label];
        for n in 0..width {
            out.push(cells.get(n).map_or_else(String::new, |c| {
                cell_text(c, sides.get(n).copied().flatten())
            }));
        }
        Ok(PrintRow { cells: out, style })
    };
    let mut rows = Vec::new();
    for row in &report.rows {
        let style = if row.kind == RowKind::Subtotal {
            PrintRowStyle::Total
        } else {
            PrintRowStyle::Plain
        };
        rows.push(line(indent(row), &row.cells, style)?);
    }
    if !report.totals.is_empty() {
        rows.push(line(
            "Total".to_owned(),
            &report.totals,
            PrintRowStyle::Total,
        )?);
    }
    let mut aligns = vec![PrintAlign::Left];
    for n in 0..width {
        let figures = report
            .rows
            .iter()
            .filter_map(|r| r.cells.get(n))
            .chain(report.totals.get(n))
            .any(is_figure);
        aligns.push(if figures {
            PrintAlign::Right
        } else {
            PrintAlign::Left
        });
    }
    Ok(PrintText {
        headings,
        aligns,
        rows,
        notes: notes_of(report),
    })
}

/// One side's line of the two-sided account.
struct Entry {
    label: String,
    cell: Cell,
}

fn side_of(cell: &Cell) -> Option<SignedAmount> {
    match cell {
        Cell::Amount(a) | Cell::Provisional { amount: a, .. } => Some(*a),
        Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Empty
        | Cell::Unsupported(_) => None,
    }
}

fn as_credit(cell: &Cell) -> Cell {
    match cell {
        Cell::Amount(a) => Cell::Amount(SignedAmount::cr(a.amount)),
        Cell::Provisional { amount, reason } => Cell::Provisional {
            amount: SignedAmount::cr(amount.amount),
            reason: reason.clone(),
        },
        Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Empty
        | Cell::Unsupported(_) => cell.clone(),
    }
}

/// Pairs the collected entries into rows, then the section's total.
fn flush(
    left: &mut Vec<Entry>,
    right: &mut Vec<Entry>,
    total: &mut Option<Cell>,
    out: &mut Vec<PrintRow>,
) {
    let mut lhs = std::mem::take(left).into_iter();
    let mut rhs = std::mem::take(right).into_iter();
    loop {
        let (a, b) = (lhs.next(), rhs.next());
        if a.is_none() && b.is_none() {
            break;
        }
        let text = |e: &Option<Entry>, side: DrCr| match e {
            Some(e) => (e.label.clone(), cell_text(&e.cell, Some(side))),
            None => (String::new(), String::new()),
        };
        let (dl, dv) = text(&a, DrCr::Dr);
        let (cl, cv) = text(&b, DrCr::Cr);
        out.push(PrintRow {
            cells: vec![dl, dv, cl, cv],
            style: PrintRowStyle::Plain,
        });
    }
    if let Some(t) = total.take() {
        out.push(PrintRow {
            cells: vec![
                "Total".to_owned(),
                cell_text(&t, Some(DrCr::Dr)),
                "Total".to_owned(),
                cell_text(&as_credit(&t), Some(DrCr::Cr)),
            ],
            style: PrintRowStyle::Total,
        });
    }
}

/// The profit and loss as the two-sided account, or `None` when the report does not have the
/// engine's trading and profit and loss totals.
fn profit_loss(report: &Report) -> Option<PrintText> {
    let has = |label: &str| report.rows.iter().any(|r| r.label == label);
    if !has("Trading total") || !has("Profit and loss total") {
        return None;
    }
    let band = |label: &str| PrintRow {
        cells: vec![
            label.to_owned(),
            String::new(),
            String::new(),
            String::new(),
        ],
        style: PrintRowStyle::Band,
    };
    let mut out: Vec<PrintRow> = Vec::new();
    let (mut left, mut right): (Vec<Entry>, Vec<Entry>) = (Vec::new(), Vec::new());
    let mut total: Option<Cell> = None;
    let mut group_side = DrCr::Dr;
    for row in &report.rows {
        match (row.kind, row.label.as_str()) {
            (RowKind::Text, "Trading account") => out.push(band("Trading account")),
            (RowKind::Text, "Profit and loss account") => {
                flush(&mut left, &mut right, &mut total, &mut out);
                out.push(band("Profit and loss account"));
            }
            (RowKind::Subtotal, "Gross profit" | "Net profit") => {}
            (RowKind::Subtotal, "Trading total" | "Profit and loss total") => {
                total = row.cells.first().cloned();
            }
            (
                RowKind::Item
                | RowKind::Group
                | RowKind::Ledger
                | RowKind::Voucher
                | RowKind::Bill
                | RowKind::Difference
                | RowKind::Subtotal
                | RowKind::Text,
                _,
            ) => {
                let Some(cell) = row.cells.first() else {
                    continue;
                };
                let Some(a) = side_of(cell) else { continue };
                let side = if row.depth == 0 {
                    group_side = a.side;
                    a.side
                } else {
                    group_side
                };
                let entry = Entry {
                    label: indent(row),
                    cell: cell.clone(),
                };
                match side {
                    DrCr::Dr => left.push(entry),
                    DrCr::Cr => right.push(entry),
                }
            }
        }
    }
    flush(&mut left, &mut right, &mut total, &mut out);
    Some(PrintText {
        headings: ["Particulars", "Debit", "Particulars", "Credit"]
            .map(str::to_owned)
            .to_vec(),
        aligns: vec![
            PrintAlign::Left,
            PrintAlign::Right,
            PrintAlign::Left,
            PrintAlign::Right,
        ],
        rows: out,
        // The engine's note about the sign of a profit figure is about the signed lines this
        // two-sided page does not print (every figure here is in its Debit or Credit column).
        notes: notes_of(report)
            .into_iter()
            .filter(|n| !n.starts_with("Profit figures are positive"))
            .collect(),
    })
}

/// The balance sheet as one vertical statement, or `None` when a row does not sit on exactly one
/// side or there are not two totals.
fn balance_sheet(report: &Report) -> Option<PrintText> {
    if report.columns.len() != 2 {
        return None;
    }
    let [liabilities_total, assets_total] = report.totals.as_slice() else {
        return None;
    };
    let mut liabilities = Vec::new();
    let mut assets = Vec::new();
    for row in &report.rows {
        let [l, r] = row.cells.as_slice() else {
            return None;
        };
        match (matches!(l, Cell::Empty), matches!(r, Cell::Empty)) {
            (false, true) => liabilities.push((row, l)),
            (true, false) => assets.push((row, r)),
            (true, true) | (false, false) => return None,
        }
    }
    let mut out = Vec::new();
    let section = |name: &str,
                   usual: DrCr,
                   lines: &[(&ReportRow, &Cell)],
                   total: &Cell,
                   out: &mut Vec<PrintRow>| {
        out.push(PrintRow {
            cells: vec![name.to_owned(), String::new()],
            style: PrintRowStyle::Band,
        });
        for (row, cell) in lines {
            out.push(PrintRow {
                cells: vec![indent(row), cell_text(cell, Some(usual))],
                style: PrintRowStyle::Plain,
            });
        }
        out.push(PrintRow {
            cells: vec!["Total".to_owned(), cell_text(total, Some(usual))],
            style: PrintRowStyle::Total,
        });
    };
    section(
        "Liabilities",
        DrCr::Cr,
        &liabilities,
        liabilities_total,
        &mut out,
    );
    section("Assets", DrCr::Dr, &assets, assets_total, &mut out);
    Some(PrintText {
        headings: vec!["Particulars".to_owned(), "Amount".to_owned()],
        aligns: vec![PrintAlign::Left, PrintAlign::Right],
        rows: out,
        notes: notes_of(report),
    })
}

/// The day book as printed: the screen's table without the audit columns ("Entered by" and
/// "Origin" say who typed a voucher and how, which a printed day book does not carry), and
/// without "Status" and "Narration" on a page where no voucher has any. Cells move whole; no
/// figure is touched.
fn day_book(text: PrintText) -> PrintText {
    let keep: Vec<bool> = text
        .headings
        .iter()
        .enumerate()
        .map(|(n, heading)| match heading.as_str() {
            "Entered by" | "Origin" => false,
            "Status" | "Narration" => text
                .rows
                .iter()
                .any(|r| r.cells.get(n).is_some_and(|c| !c.is_empty())),
            _ => true,
        })
        .collect();
    let pick = |cells: &[String]| -> Vec<String> {
        cells
            .iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(c, _)| c.clone())
            .collect()
    };
    PrintText {
        headings: pick(&text.headings),
        aligns: text
            .aligns
            .iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(a, _)| *a)
            .collect(),
        rows: text
            .rows
            .iter()
            .map(|r| PrintRow {
                cells: pick(&r.cells),
                style: r.style,
            })
            .collect(),
        notes: text.notes,
    }
}

/// The text of `report` as printed.
///
/// The profit and loss and the balance sheet take their two-sided and vertical forms when the
/// engine's rows allow it; any report that does not fit its form prints as the engine laid it
/// out, so no figure is lost or moved.
///
/// # Errors
/// [`ReportError::Invalid`] when a row has more cells than the report has columns.
pub fn print_text(report: &Report) -> Result<PrintText, ReportError> {
    let special = match report.params.kind {
        ReportKind::ProfitAndLoss => profit_loss(report),
        ReportKind::BalanceSheet => balance_sheet(report),
        ReportKind::DayBook => return generic(report).map(day_book),
        ReportKind::TrialBalance
        | ReportKind::GroupSummary(_)
        | ReportKind::GroupLedgers(_)
        | ReportKind::LedgerVouchers(_)
        | ReportKind::Register(_)
        | ReportKind::Outstanding { .. }
        | ReportKind::StockSummary
        | ReportKind::Audit(_) => None,
    };
    match special {
        Some(text) => Ok(text),
        None => generic(report),
    }
}
