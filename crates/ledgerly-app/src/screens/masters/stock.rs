//! The stock item list's Stock and value columns (`design/mock-v2/index.html:1798-1800`: `Stock`,
//! `Value at average cost ${prov('eng','stock engine')}` and the `Closing stock` total line): each
//! item's closing quantity and closing value from the engine's stock summary, run on the API
//! worker.
//!
//! The screen works nothing out (rule C4). It reads the stock summary's item rows by the
//! "Closing quantity" and "Closing value" headings and joins them to items by id only:
//!
//! * an item row shows its quantity with the item's unit, and its value as the engine gave it;
//! * an item the report has no row for shows nothing, never a made-up 0 (the API values every
//!   item, so each has a row, but the screen does not assume why a row is missing);
//! * a cell the engine could not compute, or one that is not the kind expected, shows a dash;
//! * a provisional value (negative stock) shows the figure marked provisional, never as final;
//! * the total line is the report's own closing value total, never a sum made here;
//! * until the engine answers, or when it cannot, every cell shows a dash and the heading says
//!   why on hover.

use std::collections::BTreeMap;

use ledgerly_api::{
    Api, ApiError, Cell, ColumnSpec, Detail, DrillKey, Report, ReportFilter, ReportKind,
    ReportParams, RowKind,
};
use ledgerly_core::format::format_inr;
use ledgerly_core::{DrCr, Outcome, Period, Qty, SignedAmount, StockItemId};
use ledgerly_model::Scenario;

use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::worker::ApiWorker;

/// What a cell shows where there is no figure.
const DASH: &str = "\u{2014}";
/// The stock summary's heading of the closing quantity.
pub const QTY_HEADING: &str = "Closing quantity";
/// The stock summary's heading of the closing value.
pub const VALUE_HEADING: &str = "Closing value";
/// The heading's hover text before the figures are here.
const LOADING_HINT: &str =
    "The stock figures have not come from the engine yet. Nothing is estimated here.";
/// The heading's hover text when the engine could not give the figures.
const FAILED_HINT: &str =
    "The engine could not give the stock figures, so none are shown. Nothing is estimated here.";

/// A closing value as the engine gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StockValue {
    /// A final value.
    Final(SignedAmount),
    /// A value the engine marks provisional; the text says why.
    Provisional(SignedAmount, String),
    /// No usable value; the text says why.
    Unsupported(String),
}

/// One item's closing figures as the engine gave them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStock {
    /// The closing quantity in the item's base unit, or why there is none.
    pub qty: Result<Qty, String>,
    /// The closing value.
    pub value: StockValue,
}

/// The stock figures the engine gave, by item, with the report's closing value total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockFigures {
    /// The figures of each item the report has a row for.
    pub items: BTreeMap<StockItemId, ItemStock>,
    /// The report's own total of the closing value column.
    pub total: StockValue,
}

/// What the screen holds about the stock figures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StockState {
    /// Not asked yet, or the answer is on its way.
    NotAsked,
    /// The engine's figures.
    Loaded(StockFigures),
    /// The engine could not give them; the text says why.
    Unsupported(String),
}

/// The stock figures of the item list, and the call that fetches them.
#[derive(Debug)]
pub struct ItemStockFigures {
    state: StockState,
    want: Option<Period>,
    latest: Latest<StockState>,
}

impl Default for ItemStockFigures {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemStockFigures {
    /// No figures yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: StockState::NotAsked,
            want: None,
            latest: Latest::default(),
        }
    }

    /// Asks for the stock summary over `period`: the opening is the day before it starts, the
    /// closing figures are at its end. The call goes to the worker on the next
    /// [`ItemStockFigures::tick`]; the figures shown stay until the answer comes.
    pub fn request(&mut self, period: Period) {
        self.want = Some(period);
    }

    /// `true` while a request waits for the worker or runs on it.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.want.is_some() || self.latest.is_pending()
    }

    /// Takes a finished answer, then sends a waiting request to `worker`. With no worker the
    /// request waits.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        match self.latest.poll() {
            Some(Done::Ready(state)) => self.state = state,
            Some(Done::Lost) => {
                self.state = StockState::Unsupported(
                    "the background worker stopped before the stock figures were worked out"
                        .to_owned(),
                );
            }
            None => {}
        }
        let Some(handle) = worker else {
            return;
        };
        if let Some(period) = self.want.take() {
            self.latest.start(handle, move |api| fetch(api, period));
        }
    }

    /// Sets the figures directly, as an answer from the worker would.
    pub fn set(&mut self, state: StockState) {
        self.state = state;
    }

    /// What the screen holds now.
    #[must_use]
    pub fn state(&self) -> &StockState {
        &self.state
    }

    /// The text of item `id`'s Stock cell: the quantity and `unit`, the item's unit symbol.
    #[must_use]
    pub fn qty_text(&self, id: StockItemId, unit: &str) -> String {
        match &self.state {
            StockState::Loaded(figures) => match figures.items.get(&id) {
                Some(ItemStock { qty: Ok(qty), .. }) => {
                    let figure = qty.0.normalize().to_string();
                    if unit.is_empty() {
                        figure
                    } else {
                        format!("{figure} {unit}")
                    }
                }
                Some(ItemStock { qty: Err(_), .. }) => DASH.to_owned(),
                None => String::new(),
            },
            StockState::NotAsked | StockState::Unsupported(_) => DASH.to_owned(),
        }
    }

    /// The text of item `id`'s value cell.
    #[must_use]
    pub fn value_text(&self, id: StockItemId) -> String {
        match &self.state {
            StockState::Loaded(figures) => figures
                .items
                .get(&id)
                .map_or_else(String::new, |item| value_text(&item.value)),
            StockState::NotAsked | StockState::Unsupported(_) => DASH.to_owned(),
        }
    }

    /// The text of the "Closing stock" total line's value, or `None` before the figures are here
    /// (the line is then not drawn).
    #[must_use]
    pub fn total_text(&self) -> Option<String> {
        match &self.state {
            StockState::Loaded(figures) => Some(value_text(&figures.total)),
            StockState::NotAsked | StockState::Unsupported(_) => None,
        }
    }

    /// The headings' hover text: why there are no figures, or `None` once they are here.
    #[must_use]
    pub fn heading_hint(&self) -> Option<&'static str> {
        match self.state {
            StockState::Loaded(_) => None,
            StockState::NotAsked => Some(LOADING_HINT),
            StockState::Unsupported(_) => Some(FAILED_HINT),
        }
    }

    /// Why the engine could not give the stock figures, when it could not.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        match &self.state {
            StockState::Unsupported(why) => Some(why),
            StockState::NotAsked | StockState::Loaded(_) => None,
        }
    }
}

/// A value as the list shows it: a debit value (stock in hand) as the bare figure, as the mock's
/// `inr(..)` does; a credit value with its side, so it is never read as stock in hand; a
/// provisional value marked so.
fn value_text(value: &StockValue) -> String {
    match value {
        StockValue::Final(amount) => amount_figure(*amount),
        StockValue::Provisional(amount, _) => format!("{} (provisional)", amount_figure(*amount)),
        StockValue::Unsupported(_) => DASH.to_owned(),
    }
}

fn amount_figure(amount: SignedAmount) -> String {
    match amount.side {
        DrCr::Dr => format_inr(amount.amount),
        DrCr::Cr => format!("{} Cr", format_inr(amount.amount)),
    }
}

/// Runs the condensed stock summary over `period` on the worker's `Api`.
fn fetch(api: &Api, period: Period) -> StockState {
    let params = ReportParams {
        kind: ReportKind::StockSummary,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail: Detail::Condensed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    from_answer(api.report(&params))
}

/// The state for one answer of `Api::report`.
#[must_use]
pub fn from_answer(answer: Result<Outcome<Report>, ApiError>) -> StockState {
    match answer {
        Ok(Outcome::Value(report)) => from_report(&report),
        Ok(Outcome::Unsupported(u)) => StockState::Unsupported(unsupported_text(&u)),
        Err(e) => StockState::Unsupported(format!("the stock summary failed: {e}")),
    }
}

/// The closing figures of every item row of a stock summary, by item id. Group and other rows
/// belong to no item and are skipped. A report without the closing headings is not one the screen
/// can read, so it gives no figures at all.
#[must_use]
pub fn from_report(report: &Report) -> StockState {
    let at = |label: &str| report.columns.iter().position(|c| c.label == label);
    let (Some(qty_at), Some(value_at)) = (at(QTY_HEADING), at(VALUE_HEADING)) else {
        return StockState::Unsupported(format!(
            "the stock summary has no \"{QTY_HEADING}\" and \"{VALUE_HEADING}\" columns"
        ));
    };
    let mut items = BTreeMap::new();
    for row in &report.rows {
        let id = match (&row.kind, &row.key) {
            (RowKind::Item, Some(DrillKey::Item(id))) => *id,
            (
                RowKind::Item
                | RowKind::Ledger
                | RowKind::Group
                | RowKind::Voucher
                | RowKind::Bill
                | RowKind::Difference
                | RowKind::Subtotal
                | RowKind::Text,
                _,
            ) => continue,
        };
        // A condensed report has one row per item; a second row for the same item (a godown
        // line) would be part of it, so the first is the item's own.
        items.entry(id).or_insert_with(|| ItemStock {
            qty: qty_cell(row.cells.get(qty_at)),
            value: value_cell(row.cells.get(value_at)),
        });
    }
    StockState::Loaded(StockFigures {
        items,
        total: value_cell(report.totals.get(value_at)),
    })
}

fn qty_cell(cell: Option<&Cell>) -> Result<Qty, String> {
    match cell {
        Some(Cell::Qty(qty)) => Ok(*qty),
        Some(Cell::Unsupported(u)) => Err(unsupported_text(u)),
        Some(
            Cell::Empty
            | Cell::Amount(_)
            | Cell::Signed(_)
            | Cell::Text(_)
            | Cell::Date(_)
            | Cell::Provisional { .. },
        ) => Err("the stock summary's closing quantity is not a quantity".to_owned()),
        None => Err("the stock summary row has no closing quantity".to_owned()),
    }
}

fn value_cell(cell: Option<&Cell>) -> StockValue {
    match cell {
        Some(Cell::Amount(amount)) => StockValue::Final(*amount),
        Some(Cell::Provisional { amount, reason }) => {
            StockValue::Provisional(*amount, reason.clone())
        }
        Some(Cell::Unsupported(u)) => StockValue::Unsupported(unsupported_text(u)),
        Some(Cell::Empty | Cell::Signed(_) | Cell::Qty(_) | Cell::Text(_) | Cell::Date(_)) => {
            StockValue::Unsupported("the stock summary's closing value is not an amount".to_owned())
        }
        None => StockValue::Unsupported("the stock summary row has no closing value".to_owned()),
    }
}
