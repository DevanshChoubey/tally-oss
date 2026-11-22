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

use egui::{Frame, Margin, Rect, ScrollArea, Sense, Stroke, Ui, vec2};
use ledgerly_core::{Date, DrCr, FinancialYear, SignedAmount, format::format_inr};

use super::text::{amount_text, cell_text};
use super::{TableEvents, ViewCell, ViewReport};
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_META, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge};
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

/// The balance sheet's subtitle (index.html:1577): "As at 21 Nov 2026 · FY 2026-27 · Enter opens
/// a row, Esc goes back up". The year is the April-start year holding `as_of`; it is left out at
/// the edges of the calendar, where there is no such year.
#[must_use]
pub fn subtitle(as_of: Date) -> String {
    let date = crate::ui::format::date(as_of);
    match FinancialYear::containing(as_of) {
        Ok(fy) => format!(
            "As at {date} \u{00B7} FY {} \u{00B7} Enter opens a row, Esc goes back up",
            fy.label()
        ),
        Err(_) => format!("As at {date} \u{00B7} Enter opens a row, Esc goes back up"),
    }
}

/// An amount as this section shows it: without Dr or Cr when it is on the section's usual side
/// or zero (the mock prints the bare figure, index.html:1585), and with its side written out
/// otherwise, so a loss under liabilities or an overdraft under assets is never read as the usual
/// side. The figure is the engine's; only the side word is left out.
fn amount_for(amount: SignedAmount, section: Section) -> String {
    if amount.amount.is_zero() || amount.side == section.usual_side() {
        format_inr(amount.amount)
    } else {
        amount_text(amount)
    }
}

/// The text of `cell` in `section`: an amount drops its Dr or Cr when it is zero or on the
/// section's own side; every other cell reads as everywhere else ([`cell_text`]).
#[must_use]
pub fn amount_in(cell: &ViewCell, section: Section) -> String {
    match cell {
        ViewCell::Amount(a) => amount_for(*a, section),
        ViewCell::Provisional { amount, reason } => {
            format!("{} (provisional: {reason})", amount_for(*amount, section))
        }
        ViewCell::Signed(_)
        | ViewCell::Qty(_)
        | ViewCell::Text(_)
        | ViewCell::Date(_)
        | ViewCell::Empty
        | ViewCell::Unsupported(_) => cell_text(cell),
    }
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
    let room = (ui.available_height() - foot_height - 2.0).max(T_ROW_HEIGHT * 3.0);
    card.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        events = draw_lines(ui, report, balance, selected, scroll_to_selected, room);
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

/// A count as points, saturating at `u16::MAX` (more notes than that is not a real case).
fn f32_from_len(n: usize) -> f32 {
    f32::from(u16::try_from(n).unwrap_or(u16::MAX))
}

/// The line under the statement: whether the two sides agree, then the report's notes.
fn draw_foot(ui: &mut Ui, report: &ViewReport, balance: &VerticalBalance) {
    let t = tokens(ui);
    match sides_agree(balance) {
        Some(true) => {
            badge(ui, BadgeKind::Ok, "Both sides agree");
        }
        Some(false) => {
            badge(ui, BadgeKind::Bad, "Sides differ");
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
    table::paint_cell_text(
        ui,
        label_rect(rect),
        1.0,
        &row.label,
        font.clone(),
        t.text,
        CellPlace::Left(table::indent(row.depth)),
    );
    let amount = row
        .cells
        .get(section.column())
        .map_or_default(|cell| amount_in(cell, section));
    table::paint_cell_text(ui, rect, 1.0, &amount, font, t.text, CellPlace::Right);
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
        "Total",
        font.clone(),
        t.text,
        CellPlace::Left(table::CELL_PAD_X),
    );
    table::paint_cell_text(
        ui,
        rect,
        3.0,
        &amount_in(total, section),
        font,
        t.text,
        CellPlace::Right,
    );
}
