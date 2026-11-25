//! Stock summary. **Card RP9** implements this module.
//!
//! Frozen interface (plan card RP9):
//!
//! ```text
//! pub fn stock_summary(books: &Books, params: &ReportParams, valuations: &Outcome<Vec<StockValuation>>) -> Result<Outcome<Report>, ReportError>;
//! ```
//!
//! One row per stock item (item-id order) with nine columns: opening, inwards, outwards and
//! closing, each a quantity and a value, and the closing rate. [`Detail::Detailed`] adds a child row per godown the
//! item was held in (depth 1). An item with no opening stock, no movement up to the period end and
//! no supplied valuation has no row.
//!
//! # Where the figures come from
//!
//! * **Quantities** come from `books.stock_moves` and the items' opening stock, in base units:
//!   opening = opening stock + movements before `period.from()`; inwards and outwards = movements
//!   inside the period; closing = opening + inwards - outwards.
//! * **Closing and opening values** are never computed here. The closing value is the supplied
//!   valuation dated `period.to()`; the opening value is the one dated the day before
//!   `period.from()` (as in the profit and loss report). With no earlier movement and no supplied
//!   valuation, the opening value of an average cost or FIFO item is its opening stock (quantity
//!   x rate, each entry rounded half away from zero to the paisa, as the average cost method
//!   does) and a zero opening quantity is worth zero. Anything else is `Unsupported`.
//! * **Inwards value** is the sum of the voucher values of the receiving movements (a transaction
//!   value, research R3). When a godown transfer in the period receives the item, the inwards
//!   value is `Unsupported`: a transfer's receiving value is not a purchase value.
//! * **Outwards value** is the cost consumed: opening + inwards - closing value, which is the
//!   definition of consumption under periodic average cost and annual FIFO (R3 dataset: 1,776.32
//!   and 1,650.00). Last purchase and standard cost have no verified consumption rule, so it is
//!   `Unsupported`. The value on an issuing voucher is a selling price and is never used.
//! * Item-level inwards and outwards include godown transfers (a voucher that receives and
//!   issues the item); the inwards and outwards values of such an item are `Unsupported`.
//! * A valuation whose quantity differs from the quantity computed here gives an `Unsupported`
//!   value, not a number.
//!
//! # Godowns, provisional values, totals
//!
//! Stock is valued per item across godowns (spec 6.4). A godown row shows its own quantities and
//! inwards value and `Unsupported` for the other values, except when the item was held in one
//! godown only: then it repeats the item's figures. A `ValuationStatus::Provisional` valuation
//! is a [`Cell::Provisional`] with its reason, as is every figure derived from it, and the notes
//! name the item. Totals: quantities are in different units and stay `Cell::Empty`; a value total
//! sums the shown rows, is provisional when any addend is and `Unsupported` when any is.
//!
//! # Parameters
//!
//! `period` gives the opening and closing dates; `as_of` is not used. Regular and post-dated
//! vouchers count; optional ones only in a named scenario listing `Optional`; cancelled ones
//! never (a scenario naming `Cancelled` is unsupported). `filter.items` restricts the items.
//! `exceptions_only` keeps items with a negative closing quantity or a provisional closing
//! valuation. Unsupported: comparative and monthly columns, and ledger, group and voucher type
//! filters.

mod valuation;

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{
    Date, Decimal, GodownId, Outcome, Paise, PeriodError, Qty, StockItemId, Unsupported, VoucherId,
};
use ledgerly_model::{Books, Scenario, StockItem, StockMove, VoucherStatus};
use ledgerly_stock::StockValuation;

use crate::{
    Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportKind,
    ReportParams, ReportRow, RowKind,
};

use self::valuation::{Val, addend, closing_rate, closing_value, opening_value, outwards_value};

/// Column labels, in cell order.
const COLUMN_LABELS: [&str; 9] = [
    "Opening quantity",
    "Opening value",
    "Inwards quantity",
    "Inwards value (at voucher rate)",
    "Outwards quantity",
    "Outwards value (cost of consumption)",
    "Closing quantity",
    "Closing value",
    "Closing rate",
];

/// Note shown under every stock summary.
const BASIS_NOTE: &str = "Stock is valued per item across godowns. Inwards values are at voucher rates; outwards values are the cost of stock consumed (opening + inwards - closing).";

/// Builds the stock summary for `params.period` (module docs).
///
/// `valuations` holds the closing valuation of each item dated `params.period.to()` and, when the
/// opening value is needed, the valuation dated the day before `params.period.from()`.
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not a stock summary, the period starts before
/// the books begin, a valuation is dated neither at the period end nor the day before its start
/// or is given twice for one item, or stored stock data is malformed (negative quantity or
/// value, a movement that both receives and issues, a value without a quantity, a movement
/// before the books begin); [`ReportError::NotFound`] when a movement, valuation or filter names
/// an item, godown or voucher that does not exist; [`ReportError::Money`] on overflow.
pub fn stock_summary(
    books: &Books,
    params: &ReportParams,
    valuations: &Outcome<Vec<StockValuation>>,
) -> Result<Outcome<Report>, ReportError> {
    if params.kind != ReportKind::StockSummary {
        return Err(ReportError::Invalid {
            field: "kind".to_owned(),
            reason: "the stock summary engine only runs the stock summary".to_owned(),
        });
    }
    if let Some(unsupported) = unsupported_params(params) {
        return Ok(unsupported);
    }
    let from = params.period.from();
    if from < books.company.books_begin {
        return Err(ReportError::Invalid {
            field: "period".to_owned(),
            reason: format!(
                "the period starts on {from}, before the books begin on {}",
                books.company.books_begin
            ),
        });
    }
    let day_before = from.previous_day().ok_or(PeriodError::OutOfRange)?;
    let table = match valuation_table(books, valuations, day_before, params.period.to())? {
        Outcome::Value(table) => table,
        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    let flows = collect_flows(books, params)?;
    let wanted = wanted_items(books, params)?;
    let mut rows = Vec::new();
    let mut shown = Vec::new();
    let mut notes = vec![BASIS_NOTE.to_owned()];
    for item in books.masters.stock_items.values() {
        if wanted.as_ref().is_some_and(|w| !w.contains(&item.id)) {
            continue;
        }
        let flow = flows.get(&item.id);
        if flow.is_none() && !table.has(item.id) {
            continue;
        }
        let built = build_item(books, item, flow, &table, params.detail)?;
        if params.exceptions_only && !built.exception {
            continue;
        }
        notes.extend(built.vals.provisional_notes(&item.name));
        rows.push(built.row);
        rows.extend(built.godown_rows);
        shown.push(built.vals);
    }
    Ok(Outcome::Value(Report {
        title: "Stock Summary".to_owned(),
        params: params.clone(),
        columns: COLUMN_LABELS
            .iter()
            .map(|label| ColumnHeader {
                label: (*label).to_owned(),
                period: None,
            })
            .collect(),
        rows,
        totals: totals(&shown)?,
        notes,
    }))
}

/// Parameters this card does not compute.
fn unsupported_params(params: &ReportParams) -> Option<Outcome<Report>> {
    match &params.columns {
        ColumnSpec::Single => {}
        ColumnSpec::Comparative(_) | ColumnSpec::Monthly => {
            return Some(Outcome::unsupported(
                "stock summary: comparative or monthly columns",
                "only the single opening/inwards/outwards/closing layout is computed; run one report per period",
            ));
        }
    }
    let filter = &params.filter;
    if !(filter.ledgers.is_empty() && filter.groups.is_empty() && filter.voucher_types.is_empty()) {
        return Some(Outcome::unsupported(
            "stock summary: ledger, group or voucher type filters",
            "these filters do not apply to stock; filter by item instead",
        ));
    }
    match &params.scenario {
        Scenario::Actual => {}
        Scenario::Including(statuses) => {
            if statuses.contains(&VoucherStatus::Cancelled) {
                return Some(Outcome::unsupported(
                    "stock summary: cancelled vouchers in a named scenario",
                    "a cancelled voucher keeps no stock movement to include",
                ));
            }
        }
    }
    None
}

/// The items the filter selects, or `None` for all.
fn wanted_items(
    books: &Books,
    params: &ReportParams,
) -> Result<Option<BTreeSet<StockItemId>>, ReportError> {
    if params.filter.items.is_empty() {
        return Ok(None);
    }
    for id in &params.filter.items {
        if !books.masters.stock_items.contains_key(id) {
            return Err(ReportError::NotFound {
                what: format!("stock item {} named by the filter", id.0),
            });
        }
    }
    Ok(Some(params.filter.items.iter().copied().collect()))
}

/// The supplied valuations, by item.
struct Table<'a> {
    books: &'a Books,
    opening: BTreeMap<StockItemId, &'a StockValuation>,
    closing: BTreeMap<StockItemId, &'a StockValuation>,
}

impl Table<'_> {
    fn has(&self, item: StockItemId) -> bool {
        self.opening.contains_key(&item) || self.closing.contains_key(&item)
    }
}

fn valuation_table<'a>(
    books: &'a Books,
    input: &'a Outcome<Vec<StockValuation>>,
    day_before: Date,
    to: Date,
) -> Result<Outcome<Table<'a>>, ReportError> {
    let valuations = match input {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => {
            return Ok(Outcome::unsupported(
                format!("stock summary: stock valuation ({})", u.what),
                format!("stock cannot be valued: {}", u.reason),
            ));
        }
    };
    let mut table = Table {
        books,
        opening: BTreeMap::new(),
        closing: BTreeMap::new(),
    };
    for v in valuations {
        if !books.masters.stock_items.contains_key(&v.item) {
            return Err(ReportError::NotFound {
                what: format!("stock item {} named by a valuation", v.item.0),
            });
        }
        let slot = if v.as_of == to {
            &mut table.closing
        } else if v.as_of == day_before {
            &mut table.opening
        } else {
            return Err(ReportError::Invalid {
                field: "valuations".to_owned(),
                reason: format!(
                    "valuation of item {} is dated {} but the report needs {day_before} (opening) or {to} (closing)",
                    v.item.0, v.as_of
                ),
            });
        };
        if slot.insert(v.item, v).is_some() {
            return Err(ReportError::Invalid {
                field: "valuations".to_owned(),
                reason: format!("item {} is valued twice for {}", v.item.0, v.as_of),
            });
        }
    }
    Ok(Outcome::Value(table))
}

/// Quantities of one item in one godown.
#[derive(Clone, Copy)]
struct Flow {
    open: Decimal,
    inwards: Decimal,
    outwards: Decimal,
    in_value: Paise,
}

impl Flow {
    const ZERO: Flow = Flow {
        open: Decimal::ZERO,
        inwards: Decimal::ZERO,
        outwards: Decimal::ZERO,
        in_value: Paise::ZERO,
    };

    fn closing(&self) -> Result<Decimal, ReportError> {
        let pool = dec_add(self.open, self.inwards, "closing quantity")?;
        pool.checked_sub(self.outwards)
            .ok_or_else(|| overflow("closing quantity"))
    }

    fn plus(&self, other: &Flow) -> Result<Flow, ReportError> {
        Ok(Flow {
            open: dec_add(self.open, other.open, "opening quantity")?,
            inwards: dec_add(self.inwards, other.inwards, "inward quantity")?,
            outwards: dec_add(self.outwards, other.outwards, "outward quantity")?,
            in_value: self.in_value.checked_add(other.in_value)?,
        })
    }
}

/// Everything known about one item's quantities.
#[derive(Default)]
struct ItemFlow {
    by_godown: BTreeMap<GodownId, Flow>,
    /// Some included movement is dated before the period starts.
    moves_before: bool,
    /// Per voucher dated inside the period: whether it receives and whether it issues the item.
    directions: BTreeMap<VoucherId, (bool, bool)>,
}

impl ItemFlow {
    /// A voucher in the period both receives and issues the item (a godown transfer).
    fn has_transfer(&self) -> bool {
        self.directions
            .values()
            .any(|&(received, issued)| received && issued)
    }

    fn total(&self) -> Result<Flow, ReportError> {
        let mut sum = Flow::ZERO;
        for flow in self.by_godown.values() {
            sum = sum.plus(flow)?;
        }
        Ok(sum)
    }
}

fn overflow(what: &str) -> ReportError {
    ReportError::Invalid {
        field: what.to_owned(),
        reason: "decimal overflow".to_owned(),
    }
}

fn dec_add(a: Decimal, b: Decimal, what: &str) -> Result<Decimal, ReportError> {
    a.checked_add(b).ok_or_else(|| overflow(what))
}

fn known_godown(books: &Books, godown: GodownId, by: &str) -> Result<(), ReportError> {
    if books.masters.godowns.contains_key(&godown) {
        Ok(())
    } else {
        Err(ReportError::NotFound {
            what: format!("godown {} named by {by}", godown.0),
        })
    }
}

/// Seeds every item's godown quantities with its opening stock.
fn add_opening_stock(
    books: &Books,
    flows: &mut BTreeMap<StockItemId, ItemFlow>,
) -> Result<(), ReportError> {
    for item in books.masters.stock_items.values() {
        for o in &item.opening {
            if o.qty.0.is_sign_negative() || o.rate.0.is_sign_negative() {
                return Err(ReportError::Invalid {
                    field: "opening".to_owned(),
                    reason: format!("item {} has a negative opening quantity or rate", item.id.0),
                });
            }
            known_godown(
                books,
                o.godown,
                &format!("the opening stock of item {}", item.id.0),
            )?;
            let flow = flows
                .entry(item.id)
                .or_default()
                .by_godown
                .entry(o.godown)
                .or_insert(Flow::ZERO);
            flow.open = dec_add(flow.open, o.qty.0, "opening quantity")?;
        }
    }
    Ok(())
}

fn collect_flows(
    books: &Books,
    params: &ReportParams,
) -> Result<BTreeMap<StockItemId, ItemFlow>, ReportError> {
    let mut flows: BTreeMap<StockItemId, ItemFlow> = BTreeMap::new();
    add_opening_stock(books, &mut flows)?;
    let to = params.period.to();
    for m in &books.stock_moves {
        if m.date > to || !counts(books, m, &params.scenario)? {
            continue;
        }
        check_move(books, m)?;
        let item_flow = flows.entry(m.item).or_default();
        let flow = item_flow.by_godown.entry(m.godown).or_insert(Flow::ZERO);
        if m.date < params.period.from() {
            let net = dec_add(flow.open, m.qty_in.0, "opening quantity")?
                .checked_sub(m.qty_out.0)
                .ok_or_else(|| overflow("opening quantity"))?;
            flow.open = net;
            item_flow.moves_before = true;
        } else {
            flow.inwards = dec_add(flow.inwards, m.qty_in.0, "inward quantity")?;
            flow.outwards = dec_add(flow.outwards, m.qty_out.0, "outward quantity")?;
            let seen = item_flow
                .directions
                .entry(m.voucher)
                .or_insert((false, false));
            seen.0 |= !m.qty_in.0.is_zero();
            seen.1 |= !m.qty_out.0.is_zero();
            if !m.qty_in.0.is_zero() {
                flow.in_value = flow.in_value.checked_add(m.value)?;
            }
        }
    }
    Ok(flows)
}

/// Whether the movement's voucher is in the scenario.
fn counts(books: &Books, m: &StockMove, scenario: &Scenario) -> Result<bool, ReportError> {
    let voucher = books
        .vouchers
        .get(&m.voucher)
        .ok_or_else(|| ReportError::NotFound {
            what: format!("voucher {} named by a stock movement", m.voucher.0),
        })?;
    Ok(match voucher.status {
        VoucherStatus::Regular | VoucherStatus::PostDated => true,
        VoucherStatus::Cancelled => false,
        VoucherStatus::Optional => match scenario {
            Scenario::Actual => false,
            Scenario::Including(statuses) => statuses.contains(&VoucherStatus::Optional),
        },
    })
}

fn check_move(books: &Books, m: &StockMove) -> Result<(), ReportError> {
    let bad = |reason: String| ReportError::Invalid {
        field: "stock_moves".to_owned(),
        reason,
    };
    let voucher = m.voucher.0;
    if !books.masters.stock_items.contains_key(&m.item) {
        return Err(ReportError::NotFound {
            what: format!("stock item {} named by voucher {voucher}", m.item.0),
        });
    }
    known_godown(books, m.godown, &format!("voucher {voucher}"))?;
    if m.date < books.company.books_begin {
        return Err(bad(format!(
            "voucher {voucher} moves stock on {} before the books begin on {}",
            m.date, books.company.books_begin
        )));
    }
    if m.qty_in.0.is_sign_negative() || m.qty_out.0.is_sign_negative() || m.value.is_negative() {
        return Err(bad(format!(
            "voucher {voucher} has a negative quantity or value"
        )));
    }
    if !m.qty_in.0.is_zero() && !m.qty_out.0.is_zero() {
        return Err(bad(format!(
            "voucher {voucher} has one movement that both receives and issues"
        )));
    }
    if m.qty_in.0.is_zero() && m.qty_out.0.is_zero() && !m.value.is_zero() {
        return Err(bad(format!(
            "voucher {voucher} moves value without quantity, which is not supported"
        )));
    }
    Ok(())
}

/// The four value figures of one item.
#[derive(Clone)]
struct Vals {
    opening: Val,
    inwards: Val,
    outwards: Val,
    closing: Val,
    /// The closing rate cell (the valuation's rate to the paisa, or why there is none).
    rate: Cell,
}

impl Vals {
    fn provisional_notes(&self, name: &str) -> Vec<String> {
        let mut reasons = BTreeSet::new();
        for v in [&self.opening, &self.closing] {
            if let Val::Provisional(_, reason) = v {
                reasons.insert(reason.as_str());
            }
        }
        reasons
            .into_iter()
            .map(|reason| format!("{name}: provisional value, {reason}"))
            .collect()
    }
}

struct BuiltItem {
    row: ReportRow,
    godown_rows: Vec<ReportRow>,
    vals: Vals,
    exception: bool,
}

/// Inwards value of a row: the voucher values of the receiving movements. A godown transfer in
/// the period makes it `Unsupported`, since a transfer's receiving value is not a purchase value.
fn inwards_value(name: &str, in_value: Paise, transfer: bool) -> Val {
    if transfer {
        Val::unsupported(
            format!("stock summary: inwards value of {name}"),
            "a voucher in the period moves this item between godowns, so its receiving value is a transfer value, not a purchase value",
        )
    } else {
        Val::Final(in_value)
    }
}

fn build_item(
    books: &Books,
    item: &StockItem,
    flow: Option<&ItemFlow>,
    table: &Table<'_>,
    detail: Detail,
) -> Result<BuiltItem, ReportError> {
    let empty = ItemFlow::default();
    let flow = flow.unwrap_or(&empty);
    let sum = flow.total()?;
    let qty_close = sum.closing()?;
    let opening = opening_value(item, flow, sum.open, table)?;
    let closing = closing_value(item, qty_close, table);
    let transfer = flow.has_transfer();
    let outwards = outwards_value(item, &opening, sum.in_value, &closing, qty_close, transfer)?;
    let rate = closing_rate(item, &closing, qty_close, table)?;
    let vals = Vals {
        opening,
        inwards: inwards_value(&item.name, sum.in_value, transfer),
        outwards,
        closing,
        rate,
    };
    let exception = qty_close < Decimal::ZERO || matches!(vals.closing, Val::Provisional(..));
    let row = ReportRow {
        key: Some(DrillKey::Item(item.id)),
        label: item.name.clone(),
        depth: 0,
        kind: RowKind::Item,
        cells: row_cells(&sum, &vals)?,
    };
    let godown_rows = match detail {
        Detail::Condensed => Vec::new(),
        Detail::Detailed => godown_rows(books, item, flow, &vals)?,
    };
    Ok(BuiltItem {
        row,
        godown_rows,
        vals,
        exception,
    })
}

fn row_cells(flow: &Flow, vals: &Vals) -> Result<Vec<Cell>, ReportError> {
    Ok(vec![
        Cell::Qty(Qty(flow.open)),
        vals.opening.cell()?,
        Cell::Qty(Qty(flow.inwards)),
        vals.inwards.cell()?,
        Cell::Qty(Qty(flow.outwards)),
        vals.outwards.cell()?,
        Cell::Qty(Qty(flow.closing()?)),
        vals.closing.cell()?,
        vals.rate.clone(),
    ])
}

fn godown_rows(
    books: &Books,
    item: &StockItem,
    flow: &ItemFlow,
    item_vals: &Vals,
) -> Result<Vec<ReportRow>, ReportError> {
    let single = flow.by_godown.len() == 1;
    let mut rows = Vec::new();
    for (id, part) in &flow.by_godown {
        let godown = books
            .masters
            .godowns
            .get(id)
            .ok_or_else(|| ReportError::NotFound {
                what: format!("godown {}", id.0),
            })?;
        let vals = if single {
            item_vals.clone()
        } else {
            let by_item = |which: &str| {
                Val::unsupported(
                    format!(
                        "stock summary: {which} value of {} in {}",
                        item.name, godown.name
                    ),
                    "stock is valued per item across godowns, so a godown has no value of its own",
                )
            };
            Vals {
                opening: by_item("opening"),
                inwards: inwards_value(&item.name, part.in_value, flow.has_transfer()),
                outwards: by_item("outwards"),
                closing: by_item("closing"),
                rate: Cell::Unsupported(Unsupported {
                    what: format!("stock summary: closing rate of {} in {}", item.name, godown.name),
                    reason: "stock is valued per item across godowns, so a godown has no rate of its own".to_owned(),
                }),
            }
        };
        rows.push(ReportRow {
            key: Some(DrillKey::Item(item.id)),
            label: godown.name.clone(),
            depth: 1,
            kind: RowKind::Item,
            cells: row_cells(part, &vals)?,
        });
    }
    Ok(rows)
}

/// One total per column: quantities are in different units and stay empty.
fn totals(shown: &[Vals]) -> Result<Vec<Cell>, ReportError> {
    Ok(vec![
        Cell::Empty,
        total_cell(shown.iter().map(|v| &v.opening))?,
        Cell::Empty,
        total_cell(shown.iter().map(|v| &v.inwards))?,
        Cell::Empty,
        total_cell(shown.iter().map(|v| &v.outwards))?,
        Cell::Empty,
        total_cell(shown.iter().map(|v| &v.closing))?,
        Cell::Empty,
    ])
}

fn total_cell<'a>(values: impl Iterator<Item = &'a Val>) -> Result<Cell, ReportError> {
    let mut sum = Paise::ZERO;
    let mut reasons: Vec<&str> = Vec::new();
    for v in values {
        match addend(v) {
            Ok((amount, reason)) => {
                sum = sum.checked_add(amount)?;
                reasons.extend(reason);
            }
            Err(u) => {
                return Ok(Cell::Unsupported(Unsupported {
                    what: "stock summary: total".to_owned(),
                    reason: format!("a row in this column is unsupported ({})", u.what),
                }));
            }
        }
    }
    let total = if reasons.is_empty() {
        Val::Final(sum)
    } else {
        Val::Provisional(sum, reasons.join("; "))
    };
    total.cell()
}
