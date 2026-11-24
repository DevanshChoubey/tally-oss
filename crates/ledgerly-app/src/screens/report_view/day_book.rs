//! The day book (mock v2: `day-book.default`, `day-book.month`, `day-book.empty`; render at
//! index.html lines 1636-1660, the `table.t` rules at lines 148-158).
//!
//! The engine's day book is one row per voucher with the columns `Date`, `Voucher type`,
//! `Voucher no.`, `Amount` and `Status`, and the row label is the party (or the first ledger when
//! there is no party). This module maps those columns by their heading and draws them in the
//! mock's order, with the party as the Particulars column, in a card painted to the mock's
//! `table.t` lines ([`crate::ui::table::paint_row`]).
//!
//! What the screen leaves out, because the engine does not give it yet:
//!
//! * narration: the report has no narration field, so the Particulars column shows the party only;
//! * entered by and origin: the report has no field for them (engine item E-4). The screen says
//!   so in its footnote instead of showing a value, and keeps the engine's Status column.
//!
//! Period tabs: `Day` is the single day that ends the report's period; the month tab (named after
//! that day's month, as the mock's "November") runs from the first of that month to the same day.
//! A period that is neither selects no tab. Both are plain periods handed to the engine; no
//! voucher is counted here.

use egui::{Frame, Rect, Role, ScrollArea, Sense, Stroke, Ui, pos2, vec2};
use ledgerly_core::{Date, DrCr, Period};

use super::text::{amount_on_side, cell_text};
use super::{TableEvents, ViewCell, ViewReport};
use crate::keymap::Action;
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_H2, TEXT_META, border_width, shadow_1};
use crate::ui::button::{ButtonKind, button};
use crate::ui::icon::Icon;
use crate::ui::table::{self, CellPlace, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT, T_TOTAL_HEIGHT};
use crate::ui::{a11y, format};
use crate::ui::{bold_font, kind, tokens};

/// The footnote that says what the day book does not yet show.
pub const NOT_RECORDED: &str = "Entered by and origin are not recorded yet (engine item E-4). \
Narration is not in the day book report yet, so the party is shown alone.";

/// Index of the `Day` tab.
pub const DAY_TAB: usize = 0;
/// Index of the month tab.
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

/// The labels of the two tabs: `Day`, and the month of the day the period ends on, in full
/// (index.html:1645: "November").
#[must_use]
pub fn tab_labels(period: Period) -> [String; 2] {
    ["Day".to_owned(), month_name(period.to())]
}

/// The month of `d` in full, as the mock's tab writes it.
fn month_name(d: Date) -> String {
    let name = match d.month() {
        time::Month::January => "January",
        time::Month::February => "February",
        time::Month::March => "March",
        time::Month::April => "April",
        time::Month::May => "May",
        time::Month::June => "June",
        time::Month::July => "July",
        time::Month::August => "August",
        time::Month::September => "September",
        time::Month::October => "October",
        time::Month::November => "November",
        time::Month::December => "December",
    };
    name.to_owned()
}

/// The tab that shows `period`: `Day` for a single day, the month tab for the first of a month
/// to a later day of it, and `None` for any other period.
#[must_use]
pub fn tab_of(period: Period) -> Option<usize> {
    if period.from() == period.to() {
        Some(DAY_TAB)
    } else if period_of_tab(period, MONTH_TAB) == Some(period) {
        Some(MONTH_TAB)
    } else {
        None
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

/// One piece of the page subtitle: text, or a key drawn as a keycap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubPart {
    /// Plain text.
    Text(String),
    /// A key, drawn as a keycap.
    Key(&'static str),
}

/// The page subtitle in pieces: for a day, the day and how to change it with keycaps
/// (index.html:1644: "20 Nov 2026 · F2 another date · Alt+F2 a period"); for a range, the range
/// (index.html:1644: "1 to 21 Nov 2026 · every voucher, in order").
#[must_use]
pub fn subtitle_parts(period: Period) -> Vec<SubPart> {
    let (from, to) = (period.from(), period.to());
    if from == to {
        return vec![
            SubPart::Text(format!("{} \u{00B7} ", format::date(to))),
            SubPart::Key("F2"),
            SubPart::Text(" another date \u{00B7} ".to_owned()),
            SubPart::Key("Alt+F2"),
            SubPart::Text(" a period".to_owned()),
        ];
    }
    let range = if from.year() == to.year() && from.month() == to.month() {
        format!("{} to {}", from.day(), format::date(to))
    } else {
        format!("{} to {}", format::date(from), format::date(to))
    };
    vec![SubPart::Text(format!(
        "{range} \u{00B7} every voucher, in order"
    ))]
}

/// The page subtitle as plain text (keys by name).
#[must_use]
pub fn subtitle(period: Period) -> String {
    subtitle_parts(period)
        .into_iter()
        .map(|part| match part {
            SubPart::Text(text) => text,
            SubPart::Key(key) => key.to_owned(),
        })
        .collect()
}

/// The title of the empty state: no vouchers on the day, or in the range.
#[must_use]
pub fn empty_title(period: Period) -> String {
    if period.from() == period.to() {
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
pub const EMPTY_SENTENCE: &str = "Start with a voucher key. Every voucher you save lands here.";

/// The buttons under the empty day book, each with the voucher it opens and its key
/// (index.html:1646: Sales F8, Purchase F9, Payment F5, Receipt F6).
pub const EMPTY_ACTIONS: [(&str, &str, Action); 4] = [
    ("Sales", "F8", Action::NewSales),
    ("Purchase", "F9", Action::NewPurchase),
    ("Payment", "F5", Action::NewPayment),
    ("Receipt", "F6", Action::NewReceipt),
];

/// Draws the empty day book in a card (index.html:1646): the book icon, the title, the sentence
/// and the four voucher buttons, then the footnote under it. Returns the voucher the user chose
/// to start, and the footnote's rectangle.
pub(super) fn draw_empty(ui: &mut Ui, report: &ViewReport) -> (Option<Action>, Rect) {
    let t = tokens(ui);
    let k = kind(ui);
    let mut chosen = None;
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), t.border))
        .corner_radius(RADIUS_M)
        .shadow(shadow_1(k))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                let (icon_rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                crate::ui::icon::paint(ui.painter(), icon_rect, Icon::Book, t.text_3);
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(empty_title(report.params.period))
                        .font(bold_font(ui, TEXT_H2))
                        .color(t.text),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(EMPTY_SENTENCE)
                        .size(TEXT_BODY)
                        .color(t.text_2),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let widths: f32 = EMPTY_ACTIONS
                        .iter()
                        .map(|(label, key, _)| button_width(ui, label, key) + 8.0)
                        .sum();
                    ui.add_space(((ui.available_width() - widths) / 2.0).max(0.0));
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for (label, key, action) in EMPTY_ACTIONS {
                        if button(ui, label, Some(key), ButtonKind::Secondary).clicked() {
                            chosen = Some(action);
                        }
                    }
                });
                ui.add_space(40.0);
            });
        });
    ui.add_space(6.0);
    let foot = ui
        .scope(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 2.0);
            for line in report
                .notes
                .iter()
                .map(String::as_str)
                .chain([NOT_RECORDED])
            {
                ui.label(egui::RichText::new(line).size(TEXT_META).color(t.text_3));
            }
        })
        .response
        .rect;
    (chosen, foot)
}

/// The width a button reading `label` with `key` takes, so a row of buttons can be centred.
fn button_width(ui: &Ui, label: &str, key: &str) -> f32 {
    let font = egui::FontId::proportional(TEXT_BODY);
    let galley = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), tokens(ui).text)
            .size()
            .x
    };
    // 12 px padding each side, 8 between the label and the keycap, 14 of keycap padding.
    galley(label) + galley(key) + 12.0 * 2.0 + 8.0 + 14.0
}

/// The Date cell as the mock writes dates (`fmtDate`, "20 Nov 2026"); any other cell as
/// [`cell_text`] writes it.
#[must_use]
pub fn date_text(cell: &ViewCell) -> String {
    match cell {
        ViewCell::Date(d) => format::date(*d),
        ViewCell::Amount(_)
        | ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Text(_)
        | ViewCell::Empty
        | ViewCell::Provisional { .. }
        | ViewCell::Unsupported(_) => cell_text(cell),
    }
}

/// The Amount cell: the voucher's total with no side word, as the mock's `inr(vTotal(v))`. The
/// engine writes a voucher's amount as its debit total, so only a credit figure keeps its side.
#[must_use]
pub fn amount_text(cell: &ViewCell) -> String {
    amount_on_side(cell, DrCr::Dr)
}

/// The total row's label: the number of vouchers (index.html:1655: "5 vouchers").
#[must_use]
pub fn total_label(count: usize) -> String {
    if count == 1 {
        "1 voucher".to_owned()
    } else {
        format!("{count} vouchers")
    }
}

/// The height of each line of the table, top to bottom: the heading, `rows` vouchers, and the
/// total when there is one (the `table.t` geometry, see [`crate::ui::table`]).
#[must_use]
pub fn line_heights(rows: usize, total: bool) -> Vec<f32> {
    let mut out = Vec::with_capacity(rows.saturating_add(2));
    out.push(T_HEAD_HEIGHT);
    out.extend(std::iter::repeat_n(T_ROW_HEIGHT, rows));
    if total {
        out.push(T_TOTAL_HEIGHT);
    }
    out
}

/// Fixed column widths, left to right: Date, Type, No.; Particulars takes what is left, then
/// Amount and Status. Date, Type and No. are read off the mock's capture at 1280 by 800.
const DATE_W: f32 = 133.0;
const TYPE_W: f32 = 100.0;
const NUMBER_W: f32 = 100.0;
const AMOUNT_W: f32 = 150.0;
const STATUS_W: f32 = 160.0;
const PARTICULARS_MIN: f32 = 160.0;

/// The six column rectangles of a line, left to right.
fn column_rects(rect: Rect) -> [Rect; 6] {
    let fixed = DATE_W + TYPE_W + NUMBER_W + AMOUNT_W + STATUS_W;
    let particulars = (rect.width() - fixed).max(PARTICULARS_MIN);
    let widths = [DATE_W, TYPE_W, NUMBER_W, particulars, AMOUNT_W, STATUS_W];
    let mut x = rect.left();
    widths.map(|w| {
        let r = Rect::from_min_max(pos2(x, rect.top()), pos2(x + w, rect.bottom()));
        x += w;
        r
    })
}

/// Draws the vouchers in a card that hugs them (index.html:1650-1656), then the footnote under
/// the card in the mock's `meta` style. A list taller than the screen scrolls inside the card,
/// with the footnote still in view (B7). Clicking a row selects it and double-clicking opens it.
/// Returns the events and the footnote's rectangle.
pub(super) fn draw(
    ui: &mut Ui,
    report: &ViewReport,
    columns: DayColumns,
    selected: usize,
    scroll_to_selected: bool,
) -> (TableEvents, Rect) {
    let t = tokens(ui);
    let k = kind(ui);
    let mut events = TableEvents::default();
    let foot_id = ui.make_persistent_id(("u7-day-book-foot", report.token.0.as_str()));
    let foot_height = ui.data(|d| d.get_temp::<f32>(foot_id)).unwrap_or(40.0);
    let room = (ui.available_height() - foot_height - T_HEAD_HEIGHT - 8.0).max(T_ROW_HEIGHT * 3.0);
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), t.border))
        .corner_radius(RADIUS_M)
        .shadow(shadow_1(k))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let width = ui.available_width();
            let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
            paint_heading(ui, head);
            events = draw_rows(ui, report, columns, selected, scroll_to_selected, room);
        });
    ui.add_space(6.0);
    let foot = ui
        .scope(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 2.0);
            for line in report
                .notes
                .iter()
                .map(String::as_str)
                .chain([NOT_RECORDED])
            {
                ui.label(egui::RichText::new(line).size(TEXT_META).color(t.text_3));
            }
        })
        .response
        .rect;
    ui.data_mut(|d| d.insert_temp(foot_id, foot.height()));
    (events, foot)
}

/// The heading line in the `th` style.
fn paint_heading(ui: &Ui, rect: Rect) {
    let t = tokens(ui);
    table::paint_head_rule(ui, rect);
    let font = bold_font(ui, TEXT_META);
    let [date, kind, number, particulars, amount, status] = column_rects(rect);
    let left = CellPlace::Left(table::CELL_PAD_X);
    for (r, label) in [
        (date, "Date"),
        (kind, "Type"),
        (number, "No."),
        (particulars, "Particulars"),
        (status, "Status"),
    ] {
        table::paint_cell_text(ui, r, 1.0, label, font.clone(), t.text_3, left);
    }
    table::paint_cell_text(ui, amount, 1.0, "Amount", font, t.text_3, CellPlace::Right);
}

/// The voucher rows and the total, in a scroll area at most `max_height` tall.
fn draw_rows(
    ui: &mut Ui,
    report: &ViewReport,
    columns: DayColumns,
    selected: usize,
    scroll_to_selected: bool,
    max_height: f32,
) -> TableEvents {
    let mut events = TableEvents::default();
    ScrollArea::vertical()
        .id_salt(("u7-day-book-rows", report.token.0.as_str()))
        .max_height(max_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let width = ui.available_width();
            for index in 0..report.rows.len() {
                let (rect, response) =
                    ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::click());
                let state = RowState {
                    selected: index == selected,
                    hovered: response.hovered(),
                    sub: false,
                };
                let name = paint_voucher(ui, rect, report, columns, index, state);
                a11y::name(ui, &response, Role::Row, &name);
                a11y::focus_ring(ui, &response, rect);
                if state.selected && scroll_to_selected {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    events.clicked = Some(index);
                }
                if response.double_clicked() {
                    events.opened = Some(index);
                }
            }
            if !report.totals.is_empty() {
                let (rect, _) = ui.allocate_exact_size(vec2(width, T_TOTAL_HEIGHT), Sense::hover());
                paint_total(ui, rect, report, columns);
            }
        });
    events
}

/// One voucher: date, type, number, party, amount and status. Type and status are in the mock's
/// secondary ink (`td.t2`).
///
/// Returns what a screen reader says for the row: the words and the figure as drawn, in column
/// order.
fn paint_voucher(
    ui: &Ui,
    rect: Rect,
    report: &ViewReport,
    columns: DayColumns,
    index: usize,
    state: RowState,
) -> String {
    let Some(row) = report.rows.get(index) else {
        return String::new();
    };
    let t = tokens(ui);
    table::paint_row(ui, rect, state);
    let font = egui::FontId::proportional(TEXT_BODY);
    let cell = |i: usize| row.cells.get(i);
    let [date, kind, number, particulars, amount, status] = column_rects(rect);
    let left = CellPlace::Left(table::CELL_PAD_X);
    // The cells as text, once each, so the spoken name and the painted cells agree.
    let date_text_of = cell(columns.date).map_or_default(date_text);
    let kind_text_of = cell(columns.kind).map_or_default(cell_text);
    let number_text_of = cell(columns.number).map_or_default(cell_text);
    let status_text_of = cell(columns.status).map_or_default(cell_text);
    let figure = cell(columns.amount).map_or_default(amount_text);
    // Spoken in the order the columns are drawn: date, type, number, particulars, amount, status.
    let name = a11y::join(&[
        date_text_of.as_str(),
        kind_text_of.as_str(),
        number_text_of.as_str(),
        row.label.as_str(),
        figure.as_str(),
        status_text_of.as_str(),
    ]);
    let texts = [
        (date, date_text_of, t.text),
        (kind, kind_text_of, t.text_2),
        (number, number_text_of, t.text),
        (particulars, row.label.clone(), t.text),
        (status, status_text_of, t.text_2),
    ];
    for (r, text, ink) in texts {
        table::paint_cell_text(ui, r, 1.0, &text, font.clone(), ink, left);
    }
    table::paint_cell_text(ui, amount, 1.0, &figure, font, t.text, CellPlace::Right);
    name
}

/// The total: the number of vouchers and the engine's total, semibold between the total rules.
fn paint_total(ui: &Ui, rect: Rect, report: &ViewReport, columns: DayColumns) {
    let t = tokens(ui);
    table::paint_total_rules(ui, rect);
    let font = bold_font(ui, TEXT_BODY);
    let [date, _, _, _, amount, _] = column_rects(rect);
    let label_rect = Rect::from_min_max(date.min, pos2(amount.left(), rect.bottom()));
    table::paint_cell_text(
        ui,
        label_rect,
        3.0,
        &total_label(report.rows.len()),
        font.clone(),
        t.text,
        CellPlace::Left(table::CELL_PAD_X),
    );
    let figure = report
        .totals
        .get(columns.amount)
        .map_or_default(amount_text);
    table::paint_cell_text(ui, amount, 3.0, &figure, font, t.text, CellPlace::Right);
}

#[cfg(test)]
mod a11y_names {
    //! Card S7: a voucher row reads its cells in the order they are drawn, so a screen reader
    //! hears the same sequence as the eye.
    //!
    //! derived: the day book draws Date, Voucher type, Voucher no., particulars (the row label),
    //! Amount and Status from left to right (`column_rects`). The spoken name follows that order.
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "test code: the fixture is known to build"
    )]

    use super::*;
    use crate::screens::report_view::{ReportToken, ViewDetail, ViewParams, ViewRow, ViewRowKind};
    use crate::ui::table::RowState;
    use egui::{RawInput, pos2, vec2};
    use ledgerly_core::{Paise, Period, SignedAmount};
    use time::macros::date;

    fn voucher_report() -> ViewReport {
        ViewReport {
            token: ReportToken("day book".to_owned()),
            title: "Day Book".to_owned(),
            params: ViewParams {
                period: Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap(),
                detail: ViewDetail::Condensed,
            },
            columns: ["Date", "Voucher type", "Voucher no.", "Amount", "Status"]
                .map(str::to_owned)
                .to_vec(),
            rows: vec![ViewRow {
                key: None,
                label: "Sharma Traders".to_owned(),
                depth: 0,
                kind: ViewRowKind::Voucher,
                cells: vec![
                    ViewCell::Date(date!(2026 - 11 - 20)),
                    ViewCell::Text("Sales".to_owned()),
                    ViewCell::Text("12".to_owned()),
                    ViewCell::Amount(SignedAmount::dr(Paise::new(120_000))),
                    ViewCell::Text("Paid".to_owned()),
                ],
            }],
            totals: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// What `paint_voucher` returns for the first row of `report`, drawn in a headless context.
    fn spoken(report: &ViewReport) -> String {
        let columns = day_columns(&report.columns).expect("the day book headings");
        let ctx = egui::Context::default();
        let mut name = String::new();
        let out = ctx.run_ui(RawInput::default(), |ui| {
            let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 24.0));
            let state = RowState {
                selected: false,
                hovered: false,
                sub: false,
            };
            name = paint_voucher(ui, rect, report, columns, 0, state);
        });
        out.drop_without_applying_deltas();
        name
    }

    #[test]
    fn a_voucher_row_reads_its_cells_in_column_order() {
        // derived: the cells in drawn order are the date, the type, the number, the particulars,
        // the amount and then the status. The amount is the voucher total (1,200.00, debit, so no
        // side word), and the date is written as the mock writes it (20 Nov 2026).
        assert_eq!(
            spoken(&voucher_report()),
            "20 Nov 2026, Sales, 12, Sharma Traders, 1,200.00, Paid"
        );
    }
}
