//! The balance sheet as one vertical statement (mock v2: `balance-sheet.default`).
//!
//! The engine builds the balance sheet as two columns, `Liabilities` and `Assets`, and places each
//! row's figure in one of them. This module reads that placement and draws the rows one under the
//! other: the liabilities section, its total, then the assets section and its total. It moves
//! figures between places and never adds, subtracts or rounds one. A report whose rows do not sit
//! cleanly on one side (a row with a figure on both sides, or with no figure at all) is not drawn
//! vertically; the caller falls back to the generic table, so no figure is lost or moved.
//!
//! # Labels: one rule (decision 2)
//!
//! Every label on this screen is either a standard accounting name or a name the company itself
//! gave. The screen never invents a label and never renames one the engine supplies.
//!
//! * The two sections are headed `Liabilities` and `Assets`: the engine's column names, which are
//!   the standard names for the two sides of the statement.
//! * The reserved rows keep the engine's names: `Profit and Loss A/c` for the profit row, and
//!   `Capital Account` for the reserved primary group of that name.
//! * Every other group keeps the name the company gave it.
//!
//! The mock's heading `Equity and liabilities` is replaced by `Liabilities`, the same rule as the
//! other heading.
//!
//! Source read for the vertical layout: the TallyPrime Balance Sheet help page,
//! <https://help.tallysolutions.com/tally/balance-sheet-tally/>, read 2026-11-22. The page says
//! the default view shows liabilities and assets side by side and the vertical view puts the
//! liabilities at the top and the assets at the bottom. It names its own headings, which this
//! screen does not copy; the two section names above are the standard ones.

use egui::{Frame, Margin, Rect, Role, ScrollArea, Sense, Stroke, Ui, vec2};
use ledgerly_core::{Date, DrCr, FinancialYear};

use super::text::amount_on_side;
use super::{TableEvents, ViewCell, ViewReport};
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_META, border_width, shadow_1};
use crate::ui::a11y;
use crate::ui::badge::{BadgeKind, badge, paint_badge};
use crate::ui::table::{
    self, CellPlace, RowState, T_HEAD_GAP, T_HEAD_HEIGHT, T_ROW_HEIGHT, T_TOTAL_HEIGHT,
};
use crate::ui::{bold_font, kind, tokens};

/// Heading of the liabilities section: the engine's first column.
pub const LIABILITIES_HEADING: &str = "Liabilities";
/// Heading of the assets section: the engine's second column.
pub const ASSETS_HEADING: &str = "Assets";

/// Width of the amount column (`th.num` `width:200px`, index.html:1593).
const AMOUNT_WIDTH: f32 = 200.0;
/// The amount column's heading: the currency sign (index.html:1593).
const AMOUNT_HEADING: &str = "\u{20B9}";
/// Padding of the line under the statement (`.spread` `padding:10px 16px`, index.html:1595).
const FOOT_PAD: Margin = Margin::symmetric(16, 10);

/// Which section of the statement a line or total belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// The liabilities section, the engine's first column.
    Liabilities,
    /// The assets section, the engine's second column.
    Assets,
}

impl Section {
    /// The column of the engine's report this section reads its figures from.
    #[must_use]
    pub const fn column(self) -> usize {
        match self {
            Section::Liabilities => 0,
            Section::Assets => 1,
        }
    }

    /// The heading the section is drawn under.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Section::Liabilities => LIABILITIES_HEADING,
            Section::Assets => ASSETS_HEADING,
        }
    }

    /// The side a figure usually has in this section: credit for liabilities, debit for assets.
    #[must_use]
    pub const fn usual_side(self) -> DrCr {
        match self {
            Section::Liabilities => DrCr::Cr,
            Section::Assets => DrCr::Dr,
        }
    }
}

/// The balance sheet's subtitle (index.html:1577): "As at 20 Nov 2026 · FY 2026-27 · Enter opens
/// a row, Esc goes back up". The year is the April-start year holding `as_of`; it is left out at
/// the edges of the calendar, where there is no such year.
#[must_use]
pub fn subtitle(as_of: Date) -> String {
    subtitle_with(
        as_of,
        &crate::i18n::lit("Enter opens a row, Esc goes back up"),
    )
}

/// The subtitle of a level below the balance sheet (index.html:1577: "Enter goes deeper, Esc
/// goes back up").
#[must_use]
pub fn drill_subtitle(as_of: Date) -> String {
    subtitle_with(as_of, "Enter goes deeper, Esc goes back up")
}

fn subtitle_with(as_of: Date, keys: &str) -> String {
    let date = crate::ui::format::date(as_of);
    match FinancialYear::containing(as_of) {
        Ok(fy) => crate::i18n::tf(
            "report.sub.as_at_fy",
            &[("date", &date), ("fy", &fy.label()), ("keys", keys)],
        ),
        Err(_) => crate::i18n::tf("report.sub.as_at", &[("date", &date), ("keys", keys)]),
    }
}

/// The text of `cell` in `section`: an amount drops its Dr or Cr when it is zero or on the
/// section's own side (the mock prints the bare figure, index.html:1585), so a loss under
/// liabilities or an overdraft under assets keeps its side.
#[must_use]
pub fn amount_in(cell: &ViewCell, section: Section) -> String {
    amount_on_side(cell, section.usual_side())
}

/// The figure drawn in a line's amount column: [`amount_in`] without the reason a provisional
/// figure carries (the reason is said once, in the banner over the statement, and the line wears
/// a "Provisional" badge). The amount is the engine's.
#[must_use]
pub fn figure_in(cell: &ViewCell, section: Section) -> String {
    match cell {
        ViewCell::Provisional { amount, .. } => {
            amount_on_side(&ViewCell::Amount(*amount), section.usual_side())
        }
        ViewCell::Amount(_)
        | ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Text(_)
        | ViewCell::Date(_)
        | ViewCell::Empty
        | ViewCell::Unsupported(_) => amount_in(cell, section),
    }
}

/// Whether `cell` is a provisional figure.
fn is_provisional(cell: &ViewCell) -> bool {
    matches!(cell, ViewCell::Provisional { .. })
}

/// The reasons the statement's figures are provisional, each once, in the order the lines
/// appear (rows, then totals).
#[must_use]
pub fn provisional_reasons(report: &ViewReport, balance: &VerticalBalance) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let rows = balance
        .liabilities
        .iter()
        .chain(&balance.assets)
        .filter_map(|i| report.rows.get(*i))
        .flat_map(|row| row.cells.iter());
    let totals = [&balance.liabilities_total, &balance.assets_total];
    for cell in rows.chain(totals) {
        if let ViewCell::Provisional { reason, .. } = cell
            && !out.contains(reason)
        {
            out.push(reason.clone());
        }
    }
    out
}

/// The balance sheet placed for vertical drawing. Each entry is an index into the report's rows,
/// so selection and drill keep the same row numbers as the rest of the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerticalBalance {
    /// Row indexes of the liabilities section, in report order.
    pub liabilities: Vec<usize>,
    /// Row indexes of the assets section, in report order.
    pub assets: Vec<usize>,
    /// The liabilities total, as the engine gave it.
    pub liabilities_total: ViewCell,
    /// The assets total, as the engine gave it.
    pub assets_total: ViewCell,
}

/// Places the rows of a two-column balance sheet into two sections.
///
/// Returns `None` when the report is not a two-column statement with two totals, when a row has
/// no figure on either side, or when a row has a figure on both sides. The caller then draws the
/// report as a table, which shows every figure in its own column.
#[must_use]
pub fn vertical_balance(report: &ViewReport) -> Option<VerticalBalance> {
    if report.columns.len() != 2 {
        return None;
    }
    let [liabilities_total, assets_total] = report.totals.as_slice() else {
        return None;
    };
    let mut liabilities = Vec::new();
    let mut assets = Vec::new();
    for (index, row) in report.rows.iter().enumerate() {
        let [left, right] = row.cells.as_slice() else {
            return None;
        };
        let left_empty = matches!(left, ViewCell::Empty);
        let right_empty = matches!(right, ViewCell::Empty);
        match (left_empty, right_empty) {
            (false, true) => liabilities.push(index),
            (true, false) => assets.push(index),
            (true, true) | (false, false) => return None,
        }
    }
    Some(VerticalBalance {
        liabilities,
        assets,
        liabilities_total: liabilities_total.clone(),
        assets_total: assets_total.clone(),
    })
}

/// Whether the two totals agree, read from the engine's own totals.
///
/// Both totals are amounts with a side. They agree when the amounts are equal and the sides are
/// opposite (liabilities are credit, assets debit), or when both are zero. This is a comparison of
/// two figures the engine has already computed; nothing is added or subtracted. `None` when a
/// total is not a plain amount, so there is nothing to compare.
#[must_use]
pub fn sides_agree(balance: &VerticalBalance) -> Option<bool> {
    let (ViewCell::Amount(liabilities), ViewCell::Amount(assets)) =
        (&balance.liabilities_total, &balance.assets_total)
    else {
        return None;
    };
    let same_amount = liabilities.amount == assets.amount;
    let opposite_sides = liabilities.side != assets.side;
    Some(same_amount && (liabilities.amount.is_zero() || opposite_sides))
}

/// One line the statement draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    /// A section heading.
    Heading(Section),
    /// A report row, in its section.
    Line(Section, usize),
    /// The total of a section.
    Total(Section),
}

impl Item {
    /// The line's height (see the `T_*` constants in [`table`]): the second heading has the
    /// mock's extra space above it.
    fn height(self) -> f32 {
        match self {
            Item::Heading(Section::Liabilities) => T_HEAD_HEIGHT,
            Item::Heading(Section::Assets) => T_HEAD_HEIGHT + T_HEAD_GAP,
            Item::Line(..) => T_ROW_HEIGHT,
            Item::Total(_) => T_TOTAL_HEIGHT,
        }
    }
}

/// The lines of the statement, top to bottom.
fn items(balance: &VerticalBalance) -> Vec<Item> {
    let mut out = Vec::with_capacity(
        balance
            .liabilities
            .len()
            .saturating_add(balance.assets.len())
            .saturating_add(4),
    );
    out.push(Item::Heading(Section::Liabilities));
    out.extend(
        balance
            .liabilities
            .iter()
            .map(|row| Item::Line(Section::Liabilities, *row)),
    );
    out.push(Item::Total(Section::Liabilities));
    out.push(Item::Heading(Section::Assets));
    out.extend(
        balance
            .assets
            .iter()
            .map(|row| Item::Line(Section::Assets, *row)),
    );
    out.push(Item::Total(Section::Assets));
    out
}

/// The height of each line of the statement, top to bottom: the heading, the rows and the total
/// of the liabilities, then the same for the assets.
#[must_use]
pub fn line_heights(balance: &VerticalBalance) -> Vec<f32> {
    items(balance).into_iter().map(Item::height).collect()
}

/// Draws the statement in a card, the mock's `#report-body` (index.html:1591-1595): the lines,
/// then the balance check and the report's notes. The card is as tall as its lines when they fit;
/// a statement taller than the screen scrolls inside the card, with the line under it still in
/// view (B7). Clicking a line selects it and double-clicking opens it. Returns the events and
/// the rectangle of the line under the statement.
pub(super) fn draw(
    ui: &mut Ui,
    report: &ViewReport,
    balance: &VerticalBalance,
    selected: usize,
    scroll_to_selected: bool,
) -> (TableEvents, Rect) {
    let t = tokens(ui);
    let k = kind(ui);
    let mut events = TableEvents::default();
    let mut foot = Rect::NOTHING;
    // The line under the statement is measured each frame; its last height leaves room for it
    // under the scroll area (the first frame guesses one badge line and one line per note).
    let foot_id = ui.make_persistent_id(("u7-balance-foot", report.token.0.as_str()));
    let guess = 20.0 + 22.0 + 17.0 * f32_from_len(report.notes.len());
    let foot_height = ui.data(|d| d.get_temp::<f32>(foot_id)).unwrap_or(guess);
    let card = Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(border_width(k), t.border))
        .corner_radius(RADIUS_M)
        .shadow(shadow_1(k));
    let reasons = provisional_reasons(report, balance);
    let mut open_stock = false;
    card.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        if !reasons.is_empty() {
            // index.html:1588: a warning note at the top of the card, margin 12px 16px 0.
            Frame::NONE
                .inner_margin(Margin {
                    left: 16,
                    right: 16,
                    top: 12,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    let (lead, rest) = provisional_banner(&reasons);
                    open_stock = super::draw_provisional_banner(ui, &lead, &rest, true);
                });
        }
        let room = (ui.available_height() - foot_height - 2.0).max(T_ROW_HEIGHT * 3.0);
        events = draw_lines(ui, report, balance, selected, scroll_to_selected, room);
        events.open_stock = open_stock;
        foot = Frame::NONE
            .inner_margin(FOOT_PAD)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
                draw_foot(ui, report, balance);
            })
            .response
            .rect;
    });
    ui.data_mut(|d| d.insert_temp(foot_id, foot.height()));
    (events, foot)
}

/// The banner over a statement with provisional figures, as the lead sentence (bold) and the rest
/// (index.html:1588): the closing stock is provisional, why (each reason as the engine words it)
/// and what the mark means.
fn provisional_banner(reasons: &[String]) -> (String, String) {
    (
        "Closing stock is Provisional.".to_owned(),
        format!(
            "Why: {}. The stock engine keeps the figure and marks it Provisional; the totals that follow from it are provisional too.",
            reasons.join("; ")
        ),
    )
}

/// A count as points, saturating at `u16::MAX` (more notes than that is not a real case).
fn f32_from_len(n: usize) -> f32 {
    f32::from(u16::try_from(n).unwrap_or(u16::MAX))
}

/// The line under the statement: whether the two sides agree, then the report's notes.
fn draw_foot(ui: &mut Ui, report: &ViewReport, balance: &VerticalBalance) {
    let t = tokens(ui);
    match sides_agree(balance) {
        Some(true) => {
            badge(ui, BadgeKind::Ok, &crate::i18n::lit("Both sides agree"));
        }
        Some(false) => {
            badge(ui, BadgeKind::Bad, &crate::i18n::lit("Sides differ"));
        }
        None => {}
    }
    for line in &report.notes {
        ui.label(
            egui::RichText::new(line.as_str())
                .size(TEXT_META)
                .color(t.text_2),
        );
    }
}

/// The statement's lines, in a scroll area as wide as the card and at most `max_height` tall.
fn draw_lines(
    ui: &mut Ui,
    report: &ViewReport,
    balance: &VerticalBalance,
    selected: usize,
    scroll_to_selected: bool,
    max_height: f32,
) -> TableEvents {
    let mut events = TableEvents::default();
    let list = items(balance);
    ScrollArea::vertical()
        .id_salt(("u7-balance-lines", report.token.0.as_str()))
        .max_height(max_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let width = ui.available_width();
            for item in list {
                let sense = match item {
                    Item::Line(..) => Sense::click(),
                    Item::Heading(_) | Item::Total(_) => Sense::hover(),
                };
                let (rect, response) = ui.allocate_exact_size(vec2(width, item.height()), sense);
                match item {
                    Item::Heading(section) => paint_heading(ui, rect, section),
                    Item::Total(section) => paint_total(ui, rect, section, balance),
                    Item::Line(section, index) => {
                        let state = RowState {
                            selected: index == selected,
                            hovered: response.hovered(),
                            sub: report.rows.get(index).is_some_and(|r| r.depth == 0),
                        };
                        a11y::name(ui, &response, Role::Row, &line_name(report, section, index));
                        a11y::focus_ring(ui, &response, rect);
                        paint_line(ui, rect, report, section, index, state);
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
                }
            }
        });
    events
}

/// What a screen reader says for a statement line: its label, then the amount drawn in its
/// section, as the line shows it.
fn line_name(report: &ViewReport, section: Section, index: usize) -> String {
    let Some(row) = report.rows.get(index) else {
        return String::new();
    };
    let amount = row
        .cells
        .get(section.column())
        .map_or_default(|cell| amount_in(cell, section));
    a11y::join(&[row.label.as_str(), amount.as_str()])
}

/// The label part of a line: everything left of the amount column.
fn label_rect(rect: Rect) -> Rect {
    let right = (rect.right() - AMOUNT_WIDTH).max(rect.left());
    Rect::from_min_max(rect.min, egui::pos2(right, rect.bottom()))
}

/// A section heading in the `th` style; the first one carries the amount column's heading.
fn paint_heading(ui: &Ui, rect: Rect, section: Section) {
    let t = tokens(ui);
    table::paint_head_rule(ui, rect);
    // The heading text sits on the line's last 30 points; the second heading's extra space is
    // above it.
    let text_rect = Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - T_HEAD_HEIGHT),
        rect.max,
    );
    let font = bold_font(ui, TEXT_META);
    table::paint_cell_text(
        ui,
        label_rect(text_rect),
        1.0,
        section.heading(),
        font.clone(),
        t.text_3,
        CellPlace::Left(table::CELL_PAD_X),
    );
    match section {
        Section::Liabilities => table::paint_cell_text(
            ui,
            text_rect,
            1.0,
            AMOUNT_HEADING,
            font,
            t.text_3,
            CellPlace::Right,
        ),
        Section::Assets => {}
    }
}

/// A report row: fill, rule and bar by [`table::paint_row`], the label at its depth's indent and
/// the amount at the right. Top-level rows are semibold (`tr.sub`).
fn paint_line(
    ui: &Ui,
    rect: Rect,
    report: &ViewReport,
    section: Section,
    index: usize,
    state: RowState,
) {
    let Some(row) = report.rows.get(index) else {
        return;
    };
    let t = tokens(ui);
    table::paint_row(ui, rect, state);
    let font = if state.sub {
        bold_font(ui, TEXT_BODY)
    } else {
        egui::FontId::proportional(TEXT_BODY)
    };
    let indent = table::indent(row.depth);
    let label = super::generic::row_label(row);
    table::paint_cell_text(
        ui,
        label_rect(rect),
        1.0,
        &label,
        font.clone(),
        t.text,
        CellPlace::Left(indent),
    );
    let cell = row.cells.get(section.column());
    if cell.is_some_and(is_provisional) {
        paint_provisional_mark(ui, rect, indent, &label, font.clone());
    }
    let amount = cell.map_or_default(|cell| figure_in(cell, section));
    table::paint_cell_text(ui, rect, 1.0, &amount, font, t.text, CellPlace::Right);
}

/// The "Provisional" badge after a label that starts `indent` points in (index.html:1583).
fn paint_provisional_mark(ui: &Ui, rect: Rect, indent: f32, label: &str, font: egui::FontId) {
    let t = tokens(ui);
    let width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, t.text)
        .size()
        .x;
    paint_badge(
        ui,
        rect.left() + indent + width + 6.0,
        rect.center().y,
        BadgeKind::Warn,
        "Provisional",
    );
}

/// A section's total: semibold, with the rule above and the double rule below.
fn paint_total(ui: &Ui, rect: Rect, section: Section, balance: &VerticalBalance) {
    let t = tokens(ui);
    table::paint_total_rules(ui, rect);
    let font = bold_font(ui, TEXT_BODY);
    let total = match section {
        Section::Liabilities => &balance.liabilities_total,
        Section::Assets => &balance.assets_total,
    };
    table::paint_cell_text(
        ui,
        label_rect(rect),
        3.0,
        &crate::i18n::lit("Total"),
        font.clone(),
        t.text,
        CellPlace::Left(table::CELL_PAD_X),
    );
    if is_provisional(total) {
        paint_provisional_mark(ui, rect, table::CELL_PAD_X, "Total", font.clone());
    }
    table::paint_cell_text(
        ui,
        rect,
        3.0,
        &figure_in(total, section),
        font,
        t.text,
        CellPlace::Right,
    );
}

#[cfg(test)]
mod provisional_once {
    //! The provisional reason is drawn once, in the banner, not in every line (the 2026-11-24
    //! capture of `balance-sheet.provisional` drew it over the figures, four times).
    //!
    //! provenance: index.html:1583 and 1588: the mock marks each provisional line with a
    //! "Provisional" badge beside its label, keeps the figure bare in the amount column, and says
    //! why once, in a warning note at the top of the card.
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "test code: the fixture is known to build"
    )]

    use super::*;
    use crate::screens::report_view::{ReportToken, ViewDetail, ViewParams, ViewRow, ViewRowKind};
    use egui::Shape;
    use ledgerly_core::{Paise, Period, SignedAmount};
    use time::macros::date;

    const REASON: &str = "negative stock: item pickaxe first went below zero on 2026-11-10";

    fn provisional(paise: i64) -> ViewCell {
        ViewCell::Provisional {
            amount: SignedAmount::dr(Paise::new(paise)),
            reason: REASON.to_owned(),
        }
    }

    fn statement() -> ViewReport {
        let line = |label: &str, cells: Vec<ViewCell>| ViewRow {
            key: None,
            label: label.to_owned(),
            depth: 0,
            kind: ViewRowKind::Ledger,
            cells,
        };
        ViewReport {
            token: ReportToken("provisional".to_owned()),
            title: "Balance Sheet".to_owned(),
            params: ViewParams {
                period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 30)).unwrap(),
                detail: ViewDetail::Condensed,
            },
            columns: ["Liabilities", "Assets"].map(str::to_owned).to_vec(),
            rows: vec![
                line(
                    "Capital Account",
                    vec![
                        ViewCell::Amount(SignedAmount::cr(Paise::new(500_000))),
                        ViewCell::Empty,
                    ],
                ),
                line("Closing stock", vec![ViewCell::Empty, provisional(120_000)]),
            ],
            totals: vec![
                ViewCell::Amount(SignedAmount::cr(Paise::new(500_000))),
                provisional(120_000),
            ],
            notes: Vec::new(),
        }
    }

    fn texts(shape: &Shape, out: &mut Vec<String>) {
        if let Shape::Text(t) = shape {
            out.push(t.galley.text().to_owned());
        } else if let Shape::Vec(inner) = shape {
            for s in inner {
                texts(s, out);
            }
        }
    }

    #[test]
    fn a_provisional_figure_is_drawn_bare() {
        // derived: 1,200.00 debit under assets is the usual side, so the bare figure is
        // "1,200.00"; the reason is not part of the figure.
        let cell = provisional(120_000);
        assert_eq!(figure_in(&cell, Section::Assets), "1,200.00");
    }

    #[test]
    fn each_reason_is_listed_once() {
        // derived: the row and the total carry the same reason; the banner lists it once.
        let report = statement();
        let balance = vertical_balance(&report).unwrap();
        assert_eq!(provisional_reasons(&report, &balance), [REASON]);
    }

    #[test]
    fn the_banner_leads_with_the_closing_stock_and_offers_the_stock_items() {
        // provenance: index.html:1588 (`balance-sheet.provisional`): a warning note whose bold
        // lead is "Closing stock is Provisional." and which ends with an "Open stock items"
        // button (the same button as the profit and loss note, index.html:1661).
        let report = statement();
        let balance = vertical_balance(&report).unwrap();
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw(ui, &report, &balance, 0, false);
        });
        let mut drawn = Vec::new();
        for clipped in &out.shapes {
            texts(&clipped.shape, &mut drawn);
        }
        out.drop_without_applying_deltas();
        assert!(
            drawn
                .iter()
                .any(|t| t.starts_with("Closing stock is Provisional.")),
            "texts drawn: {drawn:?}"
        );
        assert!(
            drawn.iter().any(|t| t == "Open stock items"),
            "texts drawn: {drawn:?}"
        );
    }

    #[test]
    fn the_reason_is_drawn_once() {
        // derived: one banner holds the reason; the line and the total draw only the badge and
        // the figure. Catches the reason painted in each line (over the figures).
        let report = statement();
        let balance = vertical_balance(&report).unwrap();
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw(ui, &report, &balance, 0, false);
        });
        let mut drawn = Vec::new();
        for clipped in &out.shapes {
            texts(&clipped.shape, &mut drawn);
        }
        out.drop_without_applying_deltas();
        let with_reason = drawn.iter().filter(|t| t.contains(REASON)).count();
        assert_eq!(with_reason, 1, "texts drawn: {drawn:?}");
    }
}

#[cfg(test)]
mod a11y_names {
    //! Card S7: a statement line reads its label, then the figure drawn in its section.
    //!
    //! derived: a liabilities line shows its figure in the liabilities column (the credit side
    //! is that section's usual side, so no side word), and an assets line in the assets column.
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "test code: the fixture is known to build"
    )]

    use super::*;
    use crate::screens::report_view::{ReportToken, ViewDetail, ViewParams, ViewRow, ViewRowKind};
    use ledgerly_core::{Paise, Period, SignedAmount};
    use time::macros::date;

    fn statement() -> ViewReport {
        let line = |label: &str, cells: Vec<ViewCell>| ViewRow {
            key: None,
            label: label.to_owned(),
            depth: 0,
            kind: ViewRowKind::Ledger,
            cells,
        };
        ViewReport {
            token: ReportToken("balance sheet".to_owned()),
            title: "Balance Sheet".to_owned(),
            params: ViewParams {
                period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 30)).unwrap(),
                detail: ViewDetail::Condensed,
            },
            columns: ["Liabilities", "Assets"].map(str::to_owned).to_vec(),
            rows: vec![
                line(
                    "Capital Account",
                    vec![
                        ViewCell::Amount(SignedAmount::cr(Paise::new(500_000))),
                        ViewCell::Empty,
                    ],
                ),
                line(
                    "Cash",
                    vec![
                        ViewCell::Empty,
                        ViewCell::Amount(SignedAmount::dr(Paise::new(120_000))),
                    ],
                ),
            ],
            totals: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn a_liability_line_reads_its_label_and_its_figure() {
        // derived: 5,000.00 credit under liabilities, the section's usual side, so no "Cr".
        assert_eq!(
            line_name(&statement(), Section::Liabilities, 0),
            "Capital Account, 5,000.00"
        );
    }

    #[test]
    fn an_asset_line_reads_its_label_and_its_figure() {
        // derived: 1,200.00 debit under assets, the usual side, so no "Dr".
        assert_eq!(
            line_name(&statement(), Section::Assets, 1),
            "Cash, 1,200.00"
        );
    }
}
