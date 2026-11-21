use std::collections::BTreeMap;

use ledgerly_core::{
    BatchId, CostCategoryId, CostCentreId, Date, GodownId, LedgerId, Paise, Qty, Rate,
    SignedAmount, StateCode, StockItemId, TallyRef, UnitId, VoucherId, VoucherTypeId,
};
use serde::{Deserialize, Serialize};

use crate::ledger::TaxLedgerKind;

/// A voucher (transaction).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Voucher {
    /// Identifier.
    pub id: VoucherId,
    /// Voucher type.
    pub type_id: VoucherTypeId,
    /// Number, once allocated.
    pub number: Option<String>,
    /// Numbering series, when the type has several.
    pub series: Option<String>,
    /// Voucher date.
    pub date: Date,
    /// Date the goods or services were supplied, when different (GST time of supply).
    pub supply_date: Option<Date>,
    /// Date payment was received or made, when known (GST time of supply).
    pub payment_date: Option<Date>,
    /// Status.
    pub status: VoucherStatus,
    /// Entry mode.
    pub mode: VoucherMode,
    /// Party ledger.
    pub party: Option<LedgerId>,
    /// GST place of supply, when set or overridden.
    pub place_of_supply: Option<StateCode>,
    /// Whether tax is payable under reverse charge.
    pub reverse_charge: bool,
    /// Supplier's or other external reference number.
    pub reference: Option<String>,
    /// Date of the external reference.
    pub reference_date: Option<Date>,
    /// Narration.
    pub narration: String,
    /// Accounting lines.
    pub ledger_lines: Vec<LedgerLine>,
    /// Inventory lines.
    pub inventory_lines: Vec<InventoryLine>,
    /// GST computed by the engine, with the rule pack version used.
    pub gst: Option<StoredGst>,
    /// Identity in Tally, when imported.
    pub tally: Option<TallyRef>,
    /// Unknown imported fields, kept unchanged (rule A9).
    pub extra: BTreeMap<String, String>,
}

/// Status of a voucher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum VoucherStatus {
    /// In the books.
    Regular,
    /// Optional: kept out of the books.
    Optional,
    /// Post-dated: enters the books on its date.
    PostDated,
    /// Cancelled: number kept, no amounts.
    Cancelled,
}

/// How the voucher was entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoucherMode {
    /// Plain Dr/Cr lines.
    AsVoucher,
    /// Invoice with ledger lines only.
    AccountingInvoice,
    /// Invoice with stock items.
    ItemInvoice,
}

/// One accounting line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerLine {
    /// Ledger.
    pub ledger: LedgerId,
    /// Amount and side.
    pub amount: SignedAmount,
    /// Bill allocations; must sum to the line on bill-wise ledgers.
    pub bills: Vec<BillAllocation>,
    /// Cost centre allocations.
    pub cost: Vec<CostAllocation>,
    /// Bank instrument details on bank lines.
    pub bank: Option<BankAllocation>,
    /// Invoice-level "ineligible for input credit" for this line, as the user set it on the
    /// invoice. `None` means the master value applies. Only purchase lines use it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ineligible_itc: Option<bool>,
}

/// A bill reference on a line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillAllocation {
    /// Kind of reference.
    pub kind: BillRefKind,
    /// Bill name (usually the invoice number).
    pub name: String,
    /// Due date.
    pub due_date: Option<Date>,
    /// Credit period in days.
    pub credit_days: Option<u32>,
    /// Amount and side.
    pub amount: SignedAmount,
}

/// Kind of bill reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BillRefKind {
    /// Opens a new bill.
    New,
    /// Settles an existing bill.
    Against,
    /// Advance against a future bill.
    Advance,
    /// Not linked to a bill.
    OnAccount,
}

/// A cost centre allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostAllocation {
    /// Category.
    pub category: CostCategoryId,
    /// Centre.
    pub centre: CostCentreId,
    /// Amount.
    pub amount: Paise,
}

/// Bank instrument on a bank line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankAllocation {
    /// Instrument.
    pub instrument: InstrumentKind,
    /// Cheque or reference number.
    pub instrument_number: Option<String>,
    /// Instrument date.
    pub instrument_date: Option<Date>,
    /// Date cleared by the bank; set only by a user-confirmed match (spec 6.5).
    pub bank_date: Option<Date>,
    /// Payee name for cheques.
    pub favouring: Option<String>,
}

/// Payment instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InstrumentKind {
    /// Cheque.
    Cheque,
    /// NEFT.
    Neft,
    /// RTGS.
    Rtgs,
    /// IMPS.
    Imps,
    /// UPI.
    Upi,
    /// Card.
    Card,
    /// Other electronic transfer.
    Other,
}

/// One inventory line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryLine {
    /// Item.
    pub item: StockItemId,
    /// Quantity in `unit`.
    pub qty: Qty,
    /// Unit the quantity is in.
    pub unit: UnitId,
    /// Rate per `unit`, in rupees.
    pub rate: Rate,
    /// Discount in percent.
    pub discount_percent: Option<Rate>,
    /// Line amount.
    pub amount: Paise,
    /// Sales/purchase ledger the amount is posted to.
    pub ledger: Option<LedgerId>,
    /// Godown and batch allocations; quantities sum to `qty`.
    pub godowns: Vec<GodownAllocation>,
}

/// Godown/batch allocation of an inventory line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GodownAllocation {
    /// Godown.
    pub godown: GodownId,
    /// Batch.
    pub batch: Option<BatchId>,
    /// Quantity.
    pub qty: Qty,
    /// Amount.
    pub amount: Paise,
}

/// GST on a voucher as computed by the engine (rule: user overrides are logged separately).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredGst {
    /// Rule pack version used.
    pub pack_version: String,
    /// Tax lines.
    pub lines: Vec<GstLine>,
    /// Explicit round-off amount.
    pub round_off: Paise,
}

/// One tax line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GstLine {
    /// Tax ledger posted to.
    pub ledger: LedgerId,
    /// Which tax.
    pub kind: TaxLedgerKind,
    /// Taxable value.
    pub taxable: Paise,
    /// Rate in percent.
    pub rate_percent: Rate,
    /// Tax amount.
    pub tax: Paise,
}
