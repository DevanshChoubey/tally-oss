//! Maps a confirmed master request to the one `ledgerly-api` command that carries it out.
//!
//! A create goes out with the form's draft id (0, `UNASSIGNED_ID`). The API gives it the next free
//! id of its kind before the engine runs and reports it in `CommandResult::created`.

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
        MasterRequest::Create(snapshot) => create_command(snapshot),
        MasterRequest::Alter(snapshot) => alter_command(snapshot),
        MasterRequest::Delete(target) => delete_command(target),
    }
}

/// The create command for a snapshot of a new master (id 0, assigned by the API).
fn create_command(snapshot: MasterSnapshot) -> Result<Command, String> {
    Ok(match snapshot {
        MasterSnapshot::Group(g) => Command::CreateGroup(g),
        MasterSnapshot::Ledger(l) => Command::CreateLedger(l),
        MasterSnapshot::VoucherType(_) => return Err(VOUCHER_TYPES_NOT_YET.to_owned()),
        MasterSnapshot::Unit(u) => Command::CreateUnit(u),
        MasterSnapshot::StockGroup(g) => Command::CreateStockGroup(g),
        MasterSnapshot::StockItem(i) => Command::CreateStockItem(i),
        MasterSnapshot::Godown(g) => Command::CreateGodown(g),
        MasterSnapshot::Batch(b) => Command::CreateBatch(b),
        MasterSnapshot::CostCategory(c) => Command::CreateCostCategory(c),
        MasterSnapshot::CostCentre(c) => Command::CreateCostCentre(c),
    })
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
