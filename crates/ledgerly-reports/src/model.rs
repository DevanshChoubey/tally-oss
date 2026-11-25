use ledgerly_core::{
    Date, GroupId, LedgerId, MoneyError, Paise, Period, PeriodError, Qty, SignedAmount,
    StockItemId, Unsupported, VoucherId, VoucherTypeId,
};
use ledgerly_model::{BaseVoucherKind, Scenario};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Parameters of every report (spec 6.2). One model for all three report engines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportParams {
    /// Which report.
    pub kind: ReportKind,
    /// Period for flow figures (P&L, registers).
    pub period: Period,
    /// Date for balances.
    pub as_of: Date,
    /// Which vouchers are included.
    pub scenario: Scenario,
    /// Condensed or detailed.
    pub detail: Detail,
    /// Column layout.
    pub columns: ColumnSpec,
    /// Filters.
    pub filter: ReportFilter,
    /// Show only exceptions.
    pub exceptions_only: bool,
}

/// Which report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportKind {
    /// Trial balance.
    TrialBalance,
    /// Balance sheet.
    BalanceSheet,
    /// Profit and loss (trading form).
    ProfitAndLoss,
    /// Summary of one group.
    GroupSummary(GroupId),
    /// The ledgers and sub-groups of one group with opening, debit, credit and closing for the
    /// period (the drill from a balance sheet row).
    GroupLedgers(GroupId),
    /// Vouchers of one ledger with running balance.
    LedgerVouchers(LedgerId),
    /// Day book.
    DayBook,
    /// Register of one voucher kind.
    Register(BaseVoucherKind),
    /// Bills outstanding.
    Outstanding {
        /// Receivables when `true`, payables when `false`.
        receivable: bool,
    },
    /// Stock summary.
    StockSummary,
    /// An auditor report.
    Audit(AuditReportKind),
}

/// Auditor reports (spec 6.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditReportKind {
    /// History of one object.
    ObjectHistory,
    /// Edit log.
    EditLog,
    /// Deleted and altered vouchers.
    DeletedAltered,
    /// Back-dated entries.
    BackDated,
    /// Edits after the lock date.
    EditsAfterLock,
    /// Integrity certificate.
    IntegrityCertificate,
}

/// Condensed or detailed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Detail {
    /// Primary groups only.
    Condensed,
    /// Down to ledgers.
    Detailed,
}

/// Column layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnSpec {
    /// One column for the report period.
    Single,
    /// One column per period.
    Comparative(Vec<Period>),
    /// One column per calendar month of the period.
    Monthly,
}

/// Filters.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReportFilter {
    /// Only these ledgers (empty = all).
    pub ledgers: Vec<LedgerId>,
    /// Only these groups (empty = all).
    pub groups: Vec<GroupId>,
    /// Only these voucher types (empty = all).
    pub voucher_types: Vec<VoucherTypeId>,
    /// Only these items (empty = all).
    pub items: Vec<StockItemId>,
}

/// Where a row drills to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DrillKey {
    /// A group.
    Group(GroupId),
    /// A ledger.
    Ledger(LedgerId),
    /// A voucher.
    Voucher(VoucherId),
    /// A stock item.
    Item(StockItemId),
    /// A bill.
    Bill {
        /// Ledger.
        ledger: LedgerId,
        /// Bill name.
        name: String,
    },
}

/// A computed report. Every figure in it came from the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Title.
    pub title: String,
    /// Parameters it was run with.
    pub params: ReportParams,
    /// Column headers.
    pub columns: Vec<ColumnHeader>,
    /// Rows, in display order.
    pub rows: Vec<ReportRow>,
    /// Total cells, one per column.
    pub totals: Vec<Cell>,
    /// Notes shown under the report (for example provisional stock values).
    pub notes: Vec<String>,
}

/// A column header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnHeader {
    /// Label.
    pub label: String,
    /// Period the column covers, when it is a period column.
    pub period: Option<Period>,
}

/// One row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRow {
    /// Drill target.
    pub key: Option<DrillKey>,
    /// Label.
    pub label: String,
    /// Indent level (0 = top).
    pub depth: u8,
    /// Row kind.
    pub kind: RowKind,
    /// Cells, one per column.
    pub cells: Vec<Cell>,
}

/// Row kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RowKind {
    /// A group.
    Group,
    /// A ledger.
    Ledger,
    /// A voucher.
    Voucher,
    /// A stock item.
    Item,
    /// A bill.
    Bill,
    /// Explicit difference line (for example unequal opening balances).
    Difference,
    /// Subtotal.
    Subtotal,
    /// Text only.
    Text,
}

/// One cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cell {
    /// An amount with its side.
    Amount(SignedAmount),
    /// A plain signed amount (for example profit, negative = loss).
    Signed(Paise),
    /// A quantity.
    Qty(Qty),
    /// Text.
    Text(String),
    /// A date.
    Date(Date),
    /// Empty.
    Empty,
    /// A value that is not final, with the reason shown next to it.
    Provisional {
        /// The amount.
        amount: SignedAmount,
        /// Why it is provisional.
        reason: String,
    },
    /// The engine cannot compute this cell.
    Unsupported(Unsupported),
}

/// Report errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReportError {
    /// Bad period.
    #[error(transparent)]
    Period(#[from] PeriodError),
    /// Arithmetic overflow.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// Something referenced does not exist.
    #[error("{what} does not exist")]
    NotFound {
        /// What was looked up.
        what: String,
    },
    /// Export failed.
    #[error("export to {path} failed: {reason}")]
    Export {
        /// Target path.
        path: String,
        /// Cause.
        reason: String,
    },
    /// Any other problem.
    #[error("{field}: {reason}")]
    Invalid {
        /// Field.
        field: String,
        /// Reason.
        reason: String,
    },
}
