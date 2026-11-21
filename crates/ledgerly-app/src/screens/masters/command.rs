//! Maps a confirmed master request to the one `ledgerly-api` command that carries it out.
//!
//! Creates are refused here. A new master needs an id that the company file assigns, and this
//! build cannot read the masters to pick the next one, so the form's draft id (0) would be stored
//! as the master's id. The refusal says so (see `docs/interface-requests.md`, W6).

use ledgerly_api::Command;
use ledgerly_core::{
    BatchId, CostCategoryId, CostCentreId, GodownId, GroupId, LedgerId, StockGroupId, StockItemId,
    UnitId,
};
use ledgerly_model::MasterSnapshot;

use super::{MasterKind, MasterRef, MasterRequest};

/// Why voucher types cannot be changed from this screen yet. No command exists for them.
const VOUCHER_TYPES_NOT_YET: &str = "voucher types cannot be changed from this screen yet";

/// The command for `request`.
///
/// # Errors
/// A message for the user when the request has no command in this build.
pub(crate) fn command_for(request: MasterRequest) -> Result<Command, String> {
    match request {
        MasterRequest::Create(snapshot) => Err(format!(
            "a new {} needs an id from the company file, which this build cannot assign yet",
            kind_of(&snapshot).label()
        )),
        MasterRequest::Alter(snapshot) => alter_command(snapshot),
        MasterRequest::Delete(target) => delete_command(target),
    }
}

/// The kind a snapshot belongs to.
fn kind_of(snapshot: &MasterSnapshot) -> MasterKind {
    match snapshot {
        MasterSnapshot::Group(_) => MasterKind::Group,
        MasterSnapshot::Ledger(_) => MasterKind::Ledger,
        MasterSnapshot::VoucherType(_) => MasterKind::VoucherType,
        MasterSnapshot::Unit(_) => MasterKind::Unit,
        MasterSnapshot::StockGroup(_) => MasterKind::StockGroup,
        MasterSnapshot::StockItem(_) => MasterKind::StockItem,
        MasterSnapshot::Godown(_) => MasterKind::Godown,
        MasterSnapshot::Batch(_) => MasterKind::Batch,
        MasterSnapshot::CostCategory(_) => MasterKind::CostCategory,
        MasterSnapshot::CostCentre(_) => MasterKind::CostCentre,
    }
}

/// The alter command for a snapshot of an existing master.
fn alter_command(snapshot: MasterSnapshot) -> Result<Command, String> {
    Ok(match snapshot {
        MasterSnapshot::Group(g) => Command::AlterGroup(g),
        MasterSnapshot::Ledger(l) => Command::AlterLedger(l),
        MasterSnapshot::VoucherType(_) => return Err(VOUCHER_TYPES_NOT_YET.to_owned()),
        MasterSnapshot::Unit(u) => Command::AlterUnit(u),
        MasterSnapshot::StockGroup(g) => Command::AlterStockGroup(g),
        MasterSnapshot::StockItem(i) => Command::AlterStockItem(i),
        MasterSnapshot::Godown(g) => Command::AlterGodown(g),
        MasterSnapshot::Batch(b) => Command::AlterBatch(b),
        MasterSnapshot::CostCategory(c) => Command::AlterCostCategory(c),
        MasterSnapshot::CostCentre(c) => Command::AlterCostCentre(c),
    })
}

/// The delete command for the master named by `target`.
fn delete_command(target: MasterRef) -> Result<Command, String> {
    let id = target.id;
    Ok(match target.kind {
        MasterKind::Group => Command::DeleteGroup(GroupId(id)),
        MasterKind::Ledger => Command::DeleteLedger(LedgerId(id)),
        MasterKind::VoucherType => return Err(VOUCHER_TYPES_NOT_YET.to_owned()),
        MasterKind::Unit => Command::DeleteUnit(UnitId(id)),
        MasterKind::StockGroup => Command::DeleteStockGroup(StockGroupId(id)),
        MasterKind::StockItem => Command::DeleteStockItem(StockItemId(id)),
        MasterKind::Godown => Command::DeleteGodown(GodownId(id)),
        MasterKind::Batch => Command::DeleteBatch(BatchId(id)),
        MasterKind::CostCategory => Command::DeleteCostCategory(CostCategoryId(id)),
        MasterKind::CostCentre => Command::DeleteCostCentre(CostCentreId(id)),
    })
}
