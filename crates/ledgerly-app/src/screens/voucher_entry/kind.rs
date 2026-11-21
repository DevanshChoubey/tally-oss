//! The voucher kinds the entry screen enters.

use ledgerly_core::{DrCr, Outcome};
use ledgerly_model::BaseVoucherKind;

/// The voucher kinds this screen enters (plain debit and credit lines).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Cash and bank transfers.
    Contra,
    /// Payment.
    Payment,
    /// Receipt.
    Receipt,
    /// Journal.
    Journal,
    /// Memorandum.
    Memorandum,
    /// Reversing journal.
    ReversingJournal,
}

impl EntryKind {
    /// The entry kind of a voucher type's base, or `Unsupported` when this screen does not enter it.
    #[must_use]
    pub fn from_base(base: BaseVoucherKind) -> Outcome<EntryKind> {
        match base {
            BaseVoucherKind::Contra => Outcome::Value(EntryKind::Contra),
            BaseVoucherKind::Payment => Outcome::Value(EntryKind::Payment),
            BaseVoucherKind::Receipt => Outcome::Value(EntryKind::Receipt),
            BaseVoucherKind::Journal => Outcome::Value(EntryKind::Journal),
            BaseVoucherKind::Memorandum => Outcome::Value(EntryKind::Memorandum),
            BaseVoucherKind::ReversingJournal => Outcome::Value(EntryKind::ReversingJournal),
            BaseVoucherKind::Sales
            | BaseVoucherKind::Purchase
            | BaseVoucherKind::CreditNote
            | BaseVoucherKind::DebitNote => Outcome::unsupported(
                format!("{base:?} voucher on the accounting entry screen"),
                "sales, purchase and note vouchers carry party, GST and invoice details and are entered on the invoice screen",
            ),
            BaseVoucherKind::ReceiptNote
            | BaseVoucherKind::DeliveryNote
            | BaseVoucherKind::RejectionIn
            | BaseVoucherKind::RejectionOut
            | BaseVoucherKind::StockJournal
            | BaseVoucherKind::PhysicalStock
            | BaseVoucherKind::SalesOrder
            | BaseVoucherKind::PurchaseOrder => Outcome::unsupported(
                format!("{base:?} voucher on the accounting entry screen"),
                "this voucher type holds stock or order lines, not accounting lines",
            ),
        }
    }

    /// `true` for the kinds entered in single-entry style: payment and receipt. Their first line
    /// is the cash or bank account, and no line has its own Dr / Cr switch.
    #[must_use]
    pub fn is_single_entry(self) -> bool {
        matches!(self, EntryKind::Payment | EntryKind::Receipt)
    }

    /// The side the first line starts on: a payment credits the paying account first, the other
    /// kinds start on the debit. It is only a starting point; the Dr / Cr button switches it.
    #[must_use]
    pub fn first_side(self) -> DrCr {
        match self {
            EntryKind::Payment => DrCr::Cr,
            EntryKind::Contra
            | EntryKind::Receipt
            | EntryKind::Journal
            | EntryKind::Memorandum
            | EntryKind::ReversingJournal => DrCr::Dr,
        }
    }
}
