//! The day book (mock v2: `day-book.default`, `day-book.month`, `day-book.empty`).
//!
//! The engine's day book is one row per voucher with the columns `Date`, `Voucher type`,
//! `Voucher no.`, `Amount` and `Status`, and the row label is the party (or the first ledger when
//! there is no party). This module maps those columns by their heading and draws them in the
//! mock's order, with the party as the Particulars column.
//!
//! What the screen leaves out, because the engine does not give it yet:
//!
//! * narration: the report has no narration field, so the Particulars column shows the party only;
//! * entered by and origin: the report has no field for them (engine item E-4). The screen says
//!   so in its footnote instead of showing a value.
//!
//! Period tabs: `Day` is the single day that ends the report's period; `Month to date` runs from
//! the first of that month to the same day. Both are plain periods handed to the engine; no
//! voucher is counted here.

use egui::Ui;
use egui_extras::Column;
use ledgerly_core::{Date, Period};

use super::text::cell_text;
use super::{TableEvents, ViewReport};
use crate::ui::format;
use crate::ui::table;

/// The footnote that says what the day book does not yet show.
pub const NOT_RECORDED: &str = "Entered by and origin are not recorded yet (engine item E-4). \
Narration is not in the day book report yet, so the party is shown alone.";

/// Index of the `Day` tab.
pub const DAY_TAB: usize = 0;
/// Index of the `Month to date` tab.
pub const MONTH_TAB: usize = 1;

/// Where the day book's columns sit in the engine's report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayColumns {
    /// Column of the voucher date.
    pub date: usize,
    /// Column of the voucher type.
    pub kind: usize,
    /// Column of the voucher number.
    pub number: usize,
    /// Column of the voucher amount.
    pub amount: usize,
    /// Column of the status.
    pub status: usize,
}

/// Finds the day book's columns by their headings. `None` when any of them is missing, so the
/// caller draws the report as it is rather than showing a figure under the wrong heading.
#[must_use]
pub fn day_columns(columns: &[String]) -> Option<DayColumns> {
    let find = |name: &str| columns.iter().position(|column| column.as_str() == name);
    Some(DayColumns {
        date: find("Date")?,
        kind: find("Voucher type")?,
        number: find("Voucher no.")?,
        amount: find("Amount")?,
        status: find("Status")?,
    })
}

/// The tab that shows `period`: `Day` for a single day, `Month to date` otherwise.
#[must_use]
pub fn tab_of(period: Period) -> usize {
    if period.from() == period.to() {
        DAY_TAB
    } else {
        MONTH_TAB
    }
}

/// The period a tab shows, ending on the day `period` ends on. `None` only if the calendar cannot
/// hold the date, which the caller then ignores.
#[must_use]
pub fn period_of_tab(period: Period, tab: usize) -> Option<Period> {
    let end = period.to();
    let start = if tab == DAY_TAB {
        end
    } else {
        Date::from_calendar_date(end.year(), end.month(), 1).ok()?
    };
    Period::new(start, end).ok()
}

/// The page subtitle: the day, or the range, and how to change it.
#[must_use]
pub fn subtitle(period: Period) -> String {
    if tab_of(period) == DAY_TAB {
        format!("{} · F2 another date", format::date(period.to()))
    } else {
        format!(
            "{} to {} · F2 another period",
            format::date(period.from()),
            format::date(period.to())
        )
    }
}

/// The title of the empty state: no vouchers on the day, or in the range.
#[must_use]
pub fn empty_title(period: Period) -> String {
    if tab_of(period) == DAY_TAB {
        format!("No vouchers on {} yet", format::date(period.to()))
    } else {
        format!(
            "No vouchers from {} to {}",
            format::date(period.from()),
            format::date(period.to())
        )
    }
}

/// The sentence under the empty title: what to do next, with the voucher keys.
pub const EMPTY_SENTENCE: &str = "Start with a voucher key: F8 sales, F9 purchase, F5 payment, \
F6 receipt. Every voucher you save lands here.";

/// Draws the vouchers as one table: date, type, number, particulars (the party), amount and
/// status, with the total row under them when the report has one. The period is in the page
/// subtitle, so the table does not repeat it.
pub(super) fn draw(
    ui: &mut Ui,
    report: &ViewReport,
    columns: DayColumns,
    selected: usize,
    scroll_to_selected: bool,
) -> TableEvents {
    let mut events = TableEvents::default();
    let total_rows = report
        .rows
        .len()
        .saturating_add(usize::from(!report.totals.is_empty()));
    let height = ui.available_height().max(120.0);
    let mut builder = table::builder(ui)
        .id_salt(("u7-day-book-table", report.token.0.as_str()))
        .sense(egui::Sense::click())
        .max_scroll_height(height)
        .auto_shrink([false, false])
        .column(Column::initial(110.0).at_least(90.0).clip(true))
        .column(Column::initial(110.0).at_least(90.0).clip(true))
        .column(Column::initial(100.0).at_least(80.0).clip(true))
        .column(Column::remainder().at_least(160.0).clip(true))
        .column(Column::initial(140.0).at_least(110.0).clip(true))
        .column(Column::initial(100.0).at_least(80.0).clip(true));
    if scroll_to_selected && selected < report.rows.len() {
        builder = builder.scroll_to_row(selected, None);
    }
    builder
        .header(table::HEADER_HEIGHT, |mut header| {
            table::header(&mut header, "Date", false);
            table::header(&mut header, "Type", false);
            table::header(&mut header, "No.", false);
            table::header(&mut header, "Particulars", false);
            table::header(&mut header, "Amount", true);
            table::header(&mut header, "Status", false);
        })
        .body(|body| {
            body.rows(table::ROW_HEIGHT, total_rows, |mut row| {
                let index = row.index();
                match report.rows.get(index) {
                    Some(data) => {
                        row.set_selected(index == selected);
                        let cell = |i: usize| report.rows.get(index).and_then(|r| r.cells.get(i));
                        let text = |i: usize| cell(i).map(cell_text).unwrap_or_default();
                        row.col(|ui| table::text(ui, &text(columns.date)));
                        row.col(|ui| table::text(ui, &text(columns.kind)));
                        row.col(|ui| table::text(ui, &text(columns.number)));
                        row.col(|ui| table::text(ui, data.label.as_str()));
                        row.col(|ui| table::numeric(ui, &text(columns.amount)));
                        row.col(|ui| table::text(ui, &text(columns.status)));
                        let response = row.response();
                        if response.clicked() {
                            events.clicked = Some(index);
                        }
                        if response.double_clicked() {
                            events.opened = Some(index);
                        }
                    }
                    None => {
                        let amount = report
                            .totals
                            .get(columns.amount)
                            .map(cell_text)
                            .unwrap_or_default();
                        row.col(|ui| {
                            ui.label(egui::RichText::new("Total").strong());
                        });
                        row.col(|_ui| {});
                        row.col(|_ui| {});
                        row.col(|_ui| {});
                        row.col(|ui| table::numeric(ui, &amount));
                        row.col(|_ui| {});
                    }
                }
            });
        });
    events
}
