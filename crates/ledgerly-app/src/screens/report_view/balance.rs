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

use egui::{RichText, Ui};
use egui_extras::Column;

use super::text::cell_text;
use super::{TableEvents, ViewCell, ViewReport, ViewRowKind};
use crate::ui::table;

/// Heading of the liabilities section: the engine's first column.
pub const LIABILITIES_HEADING: &str = "Liabilities";
/// Heading of the assets section: the engine's second column.
pub const ASSETS_HEADING: &str = "Assets";

/// Indent per nesting level of a row label, in points.
const INDENT: f32 = 16.0;

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

/// One row the vertical table draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    /// A section heading.
    Heading(Section),
    /// A report row, in its section.
    Line(Section, usize),
    /// The total of a section.
    Total(Section),
}

/// The table rows of the statement, top to bottom.
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

/// Draws the statement as one table: the headings, the rows and the totals. Clicking or pressing
/// Enter on a line selects or opens that report row.
pub(super) fn draw(
    ui: &mut Ui,
    report: &ViewReport,
    balance: &VerticalBalance,
    selected: usize,
    scroll_to_selected: bool,
) -> TableEvents {
    let mut events = TableEvents::default();
    let list = items(balance);
    let height = ui.available_height().max(120.0);
    let mut builder = table::builder(ui)
        .id_salt(("u7-balance-table", report.token.0.as_str()))
        .sense(egui::Sense::click())
        .max_scroll_height(height)
        .auto_shrink([false, false])
        .column(Column::remainder().at_least(160.0).clip(true))
        .column(Column::initial(200.0).at_least(140.0).clip(true));
    if scroll_to_selected {
        let position = list
            .iter()
            .position(|item| *item == Item::Line(Section::Liabilities, selected))
            .or_else(|| {
                list.iter()
                    .position(|item| *item == Item::Line(Section::Assets, selected))
            });
        if let Some(row) = position {
            builder = builder.scroll_to_row(row, None);
        }
    }
    builder
        .header(table::HEADER_HEIGHT, |mut header| {
            table::header(&mut header, "Particulars", false);
            table::header(&mut header, "Amount", true);
        })
        .body(|body| {
            body.rows(table::ROW_HEIGHT, list.len(), |mut row| {
                let Some(item) = list.get(row.index()).copied() else {
                    return;
                };
                match item {
                    Item::Heading(section) => {
                        row.col(|ui| {
                            ui.label(RichText::new(section.heading()).strong());
                        });
                        row.col(|_ui| {});
                    }
                    Item::Line(section, index) => {
                        let Some(data) = report.rows.get(index) else {
                            return;
                        };
                        row.set_selected(index == selected);
                        let emphasised = matches!(
                            data.kind,
                            ViewRowKind::Group | ViewRowKind::Subtotal | ViewRowKind::Difference
                        );
                        row.col(|ui| {
                            ui.add_space(f32::from(data.depth) * INDENT);
                            let label = RichText::new(data.label.as_str());
                            ui.label(if emphasised { label.strong() } else { label });
                        });
                        let amount = data
                            .cells
                            .get(section.column())
                            .map(cell_text)
                            .unwrap_or_default();
                        row.col(|ui| table::numeric(ui, &amount));
                        let response = row.response();
                        if response.clicked() {
                            events.clicked = Some(index);
                        }
                        if response.double_clicked() {
                            events.opened = Some(index);
                        }
                    }
                    Item::Total(section) => {
                        let total = match section {
                            Section::Liabilities => cell_text(&balance.liabilities_total),
                            Section::Assets => cell_text(&balance.assets_total),
                        };
                        row.col(|ui| {
                            ui.label(RichText::new("Total").strong());
                        });
                        row.col(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(RichText::new(total).strong());
                                },
                            );
                        });
                    }
                }
            });
        });
    events
}
