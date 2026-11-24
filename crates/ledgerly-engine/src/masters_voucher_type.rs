//! Altering a voucher type: its name, abbreviation and numbering (prefix, suffix, width,
//! restart, method).
//!
//! Model: TallyPrime gives a voucher type a numbering method, Prefix Details and Suffix Details,
//! a Starting Number and a Width of Numerical Part with "Prefill with zero"
//! (<https://help.tallysolutions.com/use-voucher-numbering-methods/>, fetched 2026-11-25). Voucher
//! types are seeded with the company, so there is no create or delete: only an alter, which keeps
//! the id and the base kind (the base kind is the type's behaviour). Vouchers already posted keep
//! their numbers; a number that no longer fits the new prefix is simply not counted by the
//! allocator (see [`crate::numbering`]).

use ledgerly_core::Outcome;
use ledgerly_model::{BaseVoucherKind, Books, MasterSnapshot, VoucherType};
use ledgerly_store::Event;

use crate::EngineError;

/// The longest number a GST invoice may carry (CGST Rules 2017, rule 46(b)).
const GST_NUMBER_MAX: usize = 16;

fn invalid(field: &str, reason: impl Into<String>) -> EngineError {
    EngineError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

/// Alter a voucher type.
///
/// # Errors
/// [`EngineError::NotFound`] when the type does not exist; [`EngineError::Invalid`] when the base
/// kind changes, the name is empty, or a sales-side type's prefix, width and suffix could make a
/// number longer than 16 characters or use a character rule 46(b) forbids.
pub fn alter_voucher_type(
    vt: VoucherType,
    books: &Books,
) -> Result<Outcome<Vec<Event>>, EngineError> {
    let before = books
        .masters
        .voucher_types
        .get(&vt.id)
        .ok_or_else(|| EngineError::NotFound {
            what: format!("voucher type {}", vt.id.0),
        })?;
    if before.base != vt.base {
        return Err(invalid(
            "base",
            "the base kind is the behaviour of a voucher type and cannot change",
        ));
    }
    if vt.name.trim().is_empty() {
        return Err(invalid("name", "a voucher type needs a name"));
    }
    if gst_series(vt.base) {
        let cfg = &vt.numbering;
        let affixes = format!("{}{}", cfg.prefix, cfg.suffix);
        if !affixes
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '/')
        {
            return Err(invalid(
                "numbering",
                "a GST invoice number may use only letters, digits, hyphen and slash \
                 (CGST Rules, rule 46(b))",
            ));
        }
        if affixes.chars().count() + usize::from(cfg.width) > GST_NUMBER_MAX {
            return Err(invalid(
                "numbering",
                "prefix, number and suffix together may not exceed 16 characters \
                 (CGST Rules, rule 46(b))",
            ));
        }
    }
    if *before == vt {
        return Ok(Outcome::Value(Vec::new()));
    }
    Ok(Outcome::Value(vec![Event::MasterAltered {
        before: MasterSnapshot::VoucherType(before.clone()),
        after: MasterSnapshot::VoucherType(vt),
    }]))
}

/// Whether numbers of this base kind are GST document numbers (sales-side).
const fn gst_series(kind: BaseVoucherKind) -> bool {
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
