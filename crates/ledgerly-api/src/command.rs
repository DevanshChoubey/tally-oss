//! Commands (card W1): one typed variant for every engine command that exists today.
//!
//! The variants mirror the public command functions of `ledgerly-engine` (`masters_accounts`,
//! `masters_stock`, `vouchers`, `locks`), plus the bank match confirmation (spec 6.5), which has
//! no engine function and is checked here. Settings changes have no engine command yet, so they
//! have no variant here; see the card report.
//!
//! [`run`] is the only place a command reaches the engine. Voucher commands always take the
//! period lock (spec section 5, invariant 8) and never call the lock-less engine functions.

use ledgerly_core::{
    BatchId, CostCategoryId, CostCentreId, Date, GodownId, GroupId, LedgerId, Outcome,
    StockGroupId, StockItemId, UnitId, VoucherId,
};
use ledgerly_engine::{EngineError, locks, masters_accounts, masters_stock, vouchers};
use ledgerly_model::{
    BankMatch, Batch, Books, CostCategory, CostCentre, Godown, Group, Ledger, StockGroup,
    StockItem, Unit, Voucher, VoucherStatus,
};
use ledgerly_store::{CommitReceipt, Event};

use crate::voucher_gst;

/// A change requested through the facade. Each variant is one engine command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Create an account group (reserved groups are seeded at creation and cannot be created).
    CreateGroup(Group),
    /// Alter an account group.
    AlterGroup(Group),
    /// Delete an account group.
    DeleteGroup(GroupId),
    /// Create a ledger.
    CreateLedger(Ledger),
    /// Alter a ledger.
    AlterLedger(Ledger),
    /// Delete a ledger.
    DeleteLedger(LedgerId),
    /// Create a unit of measure.
    CreateUnit(Unit),
    /// Alter a unit of measure.
    AlterUnit(Unit),
    /// Delete a unit of measure.
    DeleteUnit(UnitId),
    /// Create a stock group.
    CreateStockGroup(StockGroup),
    /// Alter a stock group.
    AlterStockGroup(StockGroup),
    /// Delete a stock group.
    DeleteStockGroup(StockGroupId),
    /// Create a stock item.
    CreateStockItem(StockItem),
    /// Alter a stock item.
    AlterStockItem(StockItem),
    /// Delete a stock item.
    DeleteStockItem(StockItemId),
    /// Create a godown.
    CreateGodown(Godown),
    /// Alter a godown.
    AlterGodown(Godown),
    /// Delete a godown.
    DeleteGodown(GodownId),
    /// Create a batch.
    CreateBatch(Batch),
    /// Alter a batch.
    AlterBatch(Batch),
    /// Delete a batch.
    DeleteBatch(BatchId),
    /// Create a cost category.
    CreateCostCategory(CostCategory),
    /// Alter a cost category.
    AlterCostCategory(CostCategory),
    /// Delete a cost category.
    DeleteCostCategory(CostCategoryId),
    /// Create a cost centre.
    CreateCostCentre(CostCentre),
    /// Alter a cost centre.
    AlterCostCentre(CostCentre),
    /// Delete a cost centre.
    DeleteCostCentre(CostCentreId),
    /// Post a new voucher. The number is allocated by the engine when the type numbers vouchers.
    PostVoucher(Voucher),
    /// Alter a stored voucher, matched by its id.
    AlterVoucher(Voucher),
    /// Cancel a voucher: it keeps its number and loses its amounts.
    CancelVoucher(VoucherId),
    /// Delete a voucher.
    DeleteVoucher(VoucherId),
    /// Lock the books up to and including this date (an audited event; the lock date may move).
    LockPeriod(Date),
    /// Confirm a bank match: the user accepts a suggested match, and its bank date is set on the
    /// bank lines of the vouchers (spec 6.5: only a user-confirmed match sets the bank date). The
    /// period lock applies to the vouchers, as for any voucher change.
    ConfirmBankMatch(BankMatch),
}

/// Runs `cmd` against `books` with the period lock `lock` and returns the events to commit.
///
/// Nothing is written here. Every variant is listed, so a new engine command fails to compile
/// until it is wired.
///
/// # Errors
/// [`EngineError`] when the engine refuses the command.
pub(crate) fn run(
    cmd: Command,
    books: &Books,
    lock: Option<Date>,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    match cmd {
        Command::CreateGroup(g) => masters_accounts::create_group(g, books),
        Command::AlterGroup(g) => masters_accounts::alter_group(g, books),
        Command::DeleteGroup(id) => masters_accounts::delete_group(id, books),
        Command::CreateLedger(l) => masters_accounts::create_ledger(l, books),
        Command::AlterLedger(l) => masters_accounts::alter_ledger(l, books),
        Command::DeleteLedger(id) => masters_accounts::delete_ledger(id, books),
        Command::CreateUnit(u) => masters_stock::create_unit(u, books),
        Command::AlterUnit(u) => masters_stock::alter_unit(u, books),
        Command::DeleteUnit(id) => masters_stock::delete_unit(id, books),
        Command::CreateStockGroup(g) => masters_stock::create_stock_group(g, books),
        Command::AlterStockGroup(g) => masters_stock::alter_stock_group(g, books),
        Command::DeleteStockGroup(id) => masters_stock::delete_stock_group(id, books),
        Command::CreateStockItem(it) => masters_stock::create_stock_item(it, books),
        Command::AlterStockItem(it) => masters_stock::alter_stock_item(it, books),
        Command::DeleteStockItem(id) => masters_stock::delete_stock_item(id, books),
        Command::CreateGodown(g) => masters_stock::create_godown(g, books),
        Command::AlterGodown(g) => masters_stock::alter_godown(g, books),
        Command::DeleteGodown(id) => masters_stock::delete_godown(id, books),
        Command::CreateBatch(b) => masters_stock::create_batch(b, books),
        Command::AlterBatch(b) => masters_stock::alter_batch(b, books),
        Command::DeleteBatch(id) => masters_stock::delete_batch(id, books),
        Command::CreateCostCategory(c) => masters_stock::create_cost_category(c, books),
        Command::AlterCostCategory(c) => masters_stock::alter_cost_category(c, books),
        Command::DeleteCostCategory(id) => masters_stock::delete_cost_category(id, books),
        Command::CreateCostCentre(c) => masters_stock::create_cost_centre(c, books),
        Command::AlterCostCentre(c) => masters_stock::alter_cost_centre(c, books),
        Command::DeleteCostCentre(id) => masters_stock::delete_cost_centre(id, books),
        Command::PostVoucher(v) => {
            vouchers::post_voucher_with_lock(voucher_gst::with_gst(v, books)?, books, lock)
        }
        Command::AlterVoucher(v) => {
            vouchers::alter_voucher_with_lock(voucher_gst::with_gst(v, books)?, books, lock)
        }
        Command::CancelVoucher(id) => vouchers::cancel_voucher_with_lock(id, books, lock),
        Command::DeleteVoucher(id) => vouchers::delete_voucher_with_lock(id, books, lock),
        Command::LockPeriod(until) => locks::lock_period(until, books),
        Command::ConfirmBankMatch(matched) => confirm_bank_match(matched, books, lock),
    }
}

/// Checks a confirmed bank match against the books and the period lock, and returns the
/// `MatchConfirmed` event that the store applies (it sets the bank date on the bank lines).
///
/// Every voucher must exist, be `Regular` or `PostDated`, lie outside the lock, have at least one
/// bank line and have no bank date yet. Nothing is changed here.
fn confirm_bank_match(
    matched: BankMatch,
    books: &Books,
    lock: Option<Date>,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    if matched.vouchers.is_empty() {
        return Err(EngineError::Invalid {
            field: "bank match".to_owned(),
            reason: "the match names no voucher".to_owned(),
        });
    }
    for id in &matched.vouchers {
        let voucher = books
            .vouchers
            .get(id)
            .ok_or_else(|| EngineError::NotFound {
                what: format!("voucher {}", id.0),
            })?;
        match voucher.status {
            VoucherStatus::Regular | VoucherStatus::PostDated => {}
            VoucherStatus::Optional | VoucherStatus::Cancelled => {
                return Err(EngineError::Invalid {
                    field: format!("voucher {}", id.0),
                    reason: "only a regular or post-dated voucher can be matched to the bank"
                        .to_owned(),
                });
            }
        }
        locks::check_unlocked(voucher.date, lock)?;
        let allocations: Vec<_> = voucher
            .ledger_lines
            .iter()
            .filter_map(|line| line.bank.as_ref())
            .collect();
        if allocations.is_empty() {
            return Err(EngineError::Invalid {
                field: format!("voucher {}", id.0),
                reason: "the voucher has no bank line to match".to_owned(),
            });
        }
        if allocations.iter().any(|a| a.bank_date.is_some()) {
            return Err(EngineError::Invalid {
                field: format!("voucher {}", id.0),
                reason: "the bank has already cleared this voucher; a match cannot be changed here"
                    .to_owned(),
            });
        }
    }
    Ok(Outcome::Value(vec![Event::MatchConfirmed(matched)]))
}

/// What a successful [`crate::Api::execute`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    /// The commit that saved the change, or `None` when the command changed nothing (for example
    /// an alteration with no difference).
    pub receipt: Option<CommitReceipt>,
    /// The voucher a post created, exactly as stored (with the number the engine allocated).
    pub posted: Option<Voucher>,
}

impl CommandResult {
    /// The result of a command that changed nothing.
    pub(crate) fn unchanged() -> Self {
        Self {
            receipt: None,
            posted: None,
        }
    }

    /// The result of committing `events` with `receipt`.
    pub(crate) fn committed(events: &[Event], receipt: CommitReceipt) -> Self {
        let posted = events.iter().find_map(|event| {
            if let Event::VoucherPosted(v) = event {
                Some(v.clone())
            } else {
                None
            }
        });
        Self {
            receipt: Some(receipt),
            posted,
        }
    }
}
