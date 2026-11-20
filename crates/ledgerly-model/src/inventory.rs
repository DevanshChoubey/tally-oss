use ledgerly_core::{
    BatchId, CostCategoryId, CostCentreId, Date, Decimal, GodownId, Hsn, Qty, Rate, StockGroupId,
    StockItemId, TallyRef, UnitId,
};
use serde::{Deserialize, Serialize};

use crate::ledger::Taxability;

/// A unit of measure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unit {
    /// Identifier.
    pub id: UnitId,
    /// Symbol, such as `pcs`.
    pub symbol: String,
    /// Formal name, such as `Pieces`.
    pub formal_name: String,
    /// Decimal places allowed in quantities.
    pub decimals: u8,
    /// Simple or compound.
    pub kind: UnitKind,
    /// GST unit quantity code.
    pub uqc: Option<String>,
    /// Identity in Tally, when imported.
    pub tally: Option<TallyRef>,
}

/// Simple or compound unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitKind {
    /// A base unit.
    Simple,
    /// `1 first = factor × second`, such as 1 box = 12 pcs.
    Compound {
        /// Larger unit.
        first: UnitId,
        /// How many `second` in one `first`.
        factor: Decimal,
        /// Smaller unit.
        second: UnitId,
    },
}

/// A stock group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockGroup {
    /// Identifier.
    pub id: StockGroupId,
    /// Name.
    pub name: String,
    /// Parent group.
    pub parent: Option<StockGroupId>,
    /// Identity in Tally, when imported.
    pub tally: Option<TallyRef>,
}

/// A stock item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockItem {
    /// Identifier.
    pub id: StockItemId,
    /// Name.
    pub name: String,
    /// Alternative names for search.
    pub aliases: Vec<String>,
    /// Stock group.
    pub group: Option<StockGroupId>,
    /// Base unit.
    pub unit: UnitId,
    /// Alternate unit.
    pub alt_unit: Option<AltUnit>,
    /// HSN/SAC.
    pub hsn: Option<Hsn>,
    /// GST rate history, effective-dated.
    pub gst_history: Vec<GstHistory>,
    /// Closing stock valuation method.
    pub valuation: ValuationMethod,
    /// Whether batches are tracked.
    pub batches_enabled: bool,
    /// Opening stock at books-begin.
    pub opening: Vec<OpeningStock>,
    /// Dated standard costs (for [`ValuationMethod::StandardCost`]).
    pub standard_costs: Vec<StandardCost>,
    /// Identity in Tally, when imported.
    pub tally: Option<TallyRef>,
}

/// Alternate unit of an item: `base_per_alt` base units make one alternate unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AltUnit {
    /// The alternate unit.
    pub unit: UnitId,
    /// Base units per alternate unit.
    pub base_per_alt: Decimal,
}

/// GST details effective from a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GstHistory {
    /// First date the entry applies (by time of supply).
    pub effective_from: Date,
    /// Taxability.
    pub taxability: Taxability,
    /// Rate in percent.
    pub rate_percent: Option<Rate>,
}

/// Valuation method for closing stock (spec 6.4). LIFO is deliberately absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValuationMethod {
    /// Periodic weighted average cost.
    AverageCost,
    /// First in, first out (annual).
    Fifo,
    /// Rate of the last purchase.
    LastPurchaseCost,
    /// Dated standard cost; no fallback when missing.
    StandardCost,
}

/// Opening stock of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpeningStock {
    /// Godown.
    pub godown: GodownId,
    /// Batch.
    pub batch: Option<BatchId>,
    /// Quantity in base units.
    pub qty: Qty,
    /// Rate per base unit, in rupees.
    pub rate: Rate,
}

/// A standard cost from a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardCost {
    /// First date.
    pub from: Date,
    /// Rate per base unit, in rupees.
    pub rate: Rate,
}

/// A godown (storage location).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Godown {
    /// Identifier.
    pub id: GodownId,
    /// Name.
    pub name: String,
    /// Parent godown.
    pub parent: Option<GodownId>,
    /// Identity in Tally, when imported.
    pub tally: Option<TallyRef>,
}

/// A batch of an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Batch {
    /// Identifier.
    pub id: BatchId,
    /// Item.
    pub item: StockItemId,
    /// Batch name or number.
    pub name: String,
    /// Manufacturing date.
    pub mfg: Option<Date>,
    /// Expiry date.
    pub expiry: Option<Date>,
}

/// A cost category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostCategory {
    /// Identifier.
    pub id: CostCategoryId,
    /// Name.
    pub name: String,
}

/// A cost centre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostCentre {
    /// Identifier.
    pub id: CostCentreId,
    /// Name.
    pub name: String,
    /// Category.
    pub category: CostCategoryId,
    /// Parent centre.
    pub parent: Option<CostCentreId>,
}
