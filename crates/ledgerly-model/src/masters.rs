use std::collections::BTreeMap;

use ledgerly_core::{
    BatchId, CostCategoryId, CostCentreId, GodownId, GroupId, LedgerId, StockGroupId, StockItemId,
    UnitId, VoucherTypeId,
};
use serde::{Deserialize, Serialize};

use crate::group::Group;
use crate::inventory::{Batch, CostCategory, CostCentre, Godown, StockGroup, StockItem, Unit};
use crate::ledger::Ledger;
use crate::voucher_type::VoucherType;

/// All masters of a company.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Masters {
    /// Groups.
    pub groups: BTreeMap<GroupId, Group>,
    /// Ledgers.
    pub ledgers: BTreeMap<LedgerId, Ledger>,
    /// Voucher types.
    pub voucher_types: BTreeMap<VoucherTypeId, VoucherType>,
    /// Units.
    pub units: BTreeMap<UnitId, Unit>,
    /// Stock groups.
    pub stock_groups: BTreeMap<StockGroupId, StockGroup>,
    /// Stock items.
    pub stock_items: BTreeMap<StockItemId, StockItem>,
    /// Godowns.
    pub godowns: BTreeMap<GodownId, Godown>,
    /// Batches.
    pub batches: BTreeMap<BatchId, Batch>,
    /// Cost categories.
    pub cost_categories: BTreeMap<CostCategoryId, CostCategory>,
    /// Cost centres.
    pub cost_centres: BTreeMap<CostCentreId, CostCentre>,
}

/// A full copy of one master, as stored in audit events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MasterSnapshot {
    /// A group.
    Group(Group),
    /// A ledger.
    Ledger(Ledger),
    /// A voucher type.
    VoucherType(VoucherType),
    /// A unit.
    Unit(Unit),
    /// A stock group.
    StockGroup(StockGroup),
    /// A stock item.
    StockItem(StockItem),
    /// A godown.
    Godown(Godown),
    /// A batch.
    Batch(Batch),
    /// A cost category.
    CostCategory(CostCategory),
    /// A cost centre.
    CostCentre(CostCentre),
}
