//! Default voucher types. **Card M2** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub fn default_voucher_types() -> Vec<VoucherType>;   // one per BaseVoucherKind
//! ```
//!
//! Ids follow the order of `ALL` (1-based), which is the declaration order of
//! [`BaseVoucherKind`], so the seed is deterministic. Names follow the reconstructed list in
//! research R1 "Reserved voucher types" (Material In/Out, job-work, payroll and attendance
//! types are v2 and have no base kind yet).
//!
//! Numbering defaults are product choices, not facts about another product:
//!
//! - Every type is `Automatic`, no prefix or suffix, no zero padding, restarting each financial
//!   year (GST rule 46 asks for an invoice number unique in a financial year; a yearly restart
//!   with a series prefix set by the user keeps that true).
//! - `prevent_renumbering` is `true` for the sales-side and GST-document types (Sales, Credit
//!   Note, Debit Note): spec section 5, "Sales-side and GST-document series never renumber".
//!   R1 proposes the same: sales-side series always retain, renumbering only for internal types.
//!   Every other type is internal, so `false`.

use ledgerly_core::VoucherTypeId;

use crate::voucher_type::{
    BaseVoucherKind, NumberingConfig, NumberingMethod, RestartPeriod, VoucherType,
};

/// Every base kind, in id order (id = position + 1).
const ALL: [BaseVoucherKind; 18] = [
    BaseVoucherKind::Contra,
    BaseVoucherKind::Payment,
    BaseVoucherKind::Receipt,
    BaseVoucherKind::Journal,
    BaseVoucherKind::Sales,
    BaseVoucherKind::Purchase,
    BaseVoucherKind::CreditNote,
    BaseVoucherKind::DebitNote,
    BaseVoucherKind::Memorandum,
    BaseVoucherKind::ReversingJournal,
    BaseVoucherKind::ReceiptNote,
    BaseVoucherKind::DeliveryNote,
    BaseVoucherKind::RejectionIn,
    BaseVoucherKind::RejectionOut,
    BaseVoucherKind::StockJournal,
    BaseVoucherKind::PhysicalStock,
    BaseVoucherKind::SalesOrder,
    BaseVoucherKind::PurchaseOrder,
];

/// The seed name and abbreviation of a base kind. Exhaustive, no wildcard arm.
const fn names(kind: BaseVoucherKind) -> (&'static str, &'static str) {
    match kind {
        BaseVoucherKind::Contra => ("Contra", "Ctr"),
        BaseVoucherKind::Payment => ("Payment", "Pmt"),
        BaseVoucherKind::Receipt => ("Receipt", "Rcpt"),
        BaseVoucherKind::Journal => ("Journal", "Jrnl"),
        BaseVoucherKind::Sales => ("Sales", "Sal"),
        BaseVoucherKind::Purchase => ("Purchase", "Pur"),
        BaseVoucherKind::CreditNote => ("Credit Note", "CN"),
        BaseVoucherKind::DebitNote => ("Debit Note", "DN"),
        BaseVoucherKind::Memorandum => ("Memorandum", "Memo"),
        BaseVoucherKind::ReversingJournal => ("Reversing Journal", "RJ"),
        BaseVoucherKind::ReceiptNote => ("Receipt Note", "RN"),
        BaseVoucherKind::DeliveryNote => ("Delivery Note", "DlvN"),
        BaseVoucherKind::RejectionIn => ("Rejection In", "RejIn"),
        BaseVoucherKind::RejectionOut => ("Rejection Out", "RejOut"),
        BaseVoucherKind::StockJournal => ("Stock Journal", "StkJ"),
        BaseVoucherKind::PhysicalStock => ("Physical Stock", "PhyStk"),
        BaseVoucherKind::SalesOrder => ("Sales Order", "SO"),
        BaseVoucherKind::PurchaseOrder => ("Purchase Order", "PO"),
    }
}

/// Whether numbers of this kind never change after save (sales-side and GST documents).
const fn prevents_renumbering(kind: BaseVoucherKind) -> bool {
    match kind {
        BaseVoucherKind::Sales | BaseVoucherKind::CreditNote | BaseVoucherKind::DebitNote => true,
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => false,
    }
}

/// Whether vouchers of `kind` post to ledgers and so change base balances.
///
/// Spec section 5, invariant 3: memorandum and reversing vouchers never affect base balances
/// (only a named scenario includes them). Order, note and stock kinds post no ledger lines (they
/// move stock or record intent), so they do not change ledger balances either; the engine
/// treats them the same way. The other kinds post.
pub const fn affects_books(kind: BaseVoucherKind) -> bool {
    match kind {
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote => true,
        BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => false,
    }
}

/// The seed numbering of `kind`.
fn numbering(kind: BaseVoucherKind) -> NumberingConfig {
    NumberingConfig {
        method: NumberingMethod::Automatic,
        prevent_renumbering: prevents_renumbering(kind),
        prefix: String::new(),
        suffix: String::new(),
        width: 0,
        restart: RestartPeriod::Yearly,
        series: Vec::new(),
    }
}

/// The id of `kind`: its 1-based position in `ALL`.
fn id_of(kind: BaseVoucherKind) -> Option<VoucherTypeId> {
    let pos = ALL.iter().position(|k| *k == kind)?;
    u64::try_from(pos).ok()?.checked_add(1).map(VoucherTypeId)
}

/// The 18 default voucher types, ids `1..=18`, one per [`BaseVoucherKind`] variant.
///
/// Each is named after its base kind, active, automatically numbered, and not imported from
/// anywhere (`tally: None`). Sales, Credit Note and Debit Note set `prevent_renumbering`.
pub fn default_voucher_types() -> Vec<VoucherType> {
    ALL.iter()
        .filter_map(|&kind| {
            let id = id_of(kind)?;
            let (name, abbreviation) = names(kind);
            Some(VoucherType {
                id,
                name: name.to_owned(),
                base: kind,
                abbreviation: abbreviation.to_owned(),
                numbering: numbering(kind),
                active: true,
                tally: None,
            })
        })
        .collect()
}
