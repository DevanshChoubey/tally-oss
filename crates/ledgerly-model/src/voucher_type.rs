use ledgerly_core::{TallyRef, VoucherTypeId};
use serde::{Deserialize, Serialize};

/// A voucher type. Users create their own types on top of a [`BaseVoucherKind`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherType {
    /// Identifier.
    pub id: VoucherTypeId,
    /// Name, such as `Sales` or `Sales - Mumbai`.
    pub name: String,
    /// Behaviour it inherits.
    pub base: BaseVoucherKind,
    /// Short form for lists.
    pub abbreviation: String,
    /// Numbering.
    pub numbering: NumberingConfig,
    /// Whether vouchers of this type can be entered.
    pub active: bool,
    /// Identity in Tally, when imported.
    pub tally: Option<TallyRef>,
}

/// The closed set of voucher behaviours in v1 (spec 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BaseVoucherKind {
    /// Cash/bank transfers.
    Contra,
    /// Payment.
    Payment,
    /// Receipt.
    Receipt,
    /// Journal.
    Journal,
    /// Sales.
    Sales,
    /// Purchase.
    Purchase,
    /// Credit note.
    CreditNote,
    /// Debit note.
    DebitNote,
    /// Memorandum (never in the books).
    Memorandum,
    /// Reversing journal (only in scenarios).
    ReversingJournal,
    /// Goods received.
    ReceiptNote,
    /// Goods delivered.
    DeliveryNote,
    /// Goods returned by a customer.
    RejectionIn,
    /// Goods returned to a supplier.
    RejectionOut,
    /// Stock transfer or adjustment.
    StockJournal,
    /// Physical stock count.
    PhysicalStock,
    /// Sales order.
    SalesOrder,
    /// Purchase order.
    PurchaseOrder,
}

/// Numbering of a voucher type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberingConfig {
    /// How numbers are assigned.
    pub method: NumberingMethod,
    /// When `true`, numbers never change after save (required for sales-side and GST series).
    pub prevent_renumbering: bool,
    /// Text before the number, such as `INV/26-27/`.
    pub prefix: String,
    /// Text after the number.
    pub suffix: String,
    /// Minimum digits, zero-padded.
    pub width: u8,
    /// When the counter restarts.
    pub restart: RestartPeriod,
    /// Named series, if the type uses more than one.
    pub series: Vec<String>,
}

/// How voucher numbers are assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NumberingMethod {
    /// Allocated by the engine.
    Automatic,
    /// Typed by the user, checked for uniqueness.
    Manual,
    /// No number.
    None,
}

/// When an automatic counter restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RestartPeriod {
    /// Never.
    Never,
    /// Each financial year.
    Yearly,
    /// Each month.
    Monthly,
}
