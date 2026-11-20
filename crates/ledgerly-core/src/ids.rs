use serde::{Deserialize, Serialize};

macro_rules! ids {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);
    )*};
}

ids! {
    /// A company (one store file per company).
    CompanyId,
    /// An account group.
    GroupId,
    /// A ledger account.
    LedgerId,
    /// A voucher type.
    VoucherTypeId,
    /// A voucher.
    VoucherId,
    /// A cost centre.
    CostCentreId,
    /// A cost category.
    CostCategoryId,
    /// A unit of measure.
    UnitId,
    /// A stock group.
    StockGroupId,
    /// A stock item.
    StockItemId,
    /// A godown (storage location).
    GodownId,
    /// A batch of a stock item.
    BatchId,
    /// An imported bank statement.
    BankStatementId,
    /// An application user.
    UserId,
    /// Position of an event in the audit log (starts at 1, strictly increasing).
    EventSeq,
}
