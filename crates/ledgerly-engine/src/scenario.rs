//! Which postings a scenario includes. **Card E6** implements this module.
//!
//! Frozen interface (plan card E6):
//!
//! ```text
//! pub fn effective_postings<'a>(books: &'a Books, scenario: &Scenario, as_of: Date) -> Vec<&'a Posting>;
//! ```
//!
//! Spec section 5, invariant 3: optional, memorandum, reversing and future post-dated vouchers
//! never affect base balances; only a named scenario includes them.
//!
//! Rules applied, in order, to each posting of [`Books::postings`]:
//!
//! 1. A posting dated after `as_of` is out (every status, so balances are "as of" a date).
//! 2. A posting whose voucher type is `Memorandum` or `ReversingJournal` is out in every scenario.
//!    The frozen return type cannot say "unsupported", and no rule defines how a named scenario
//!    would include them, so they are never returned (callers such as the trial balance report
//!    their own `Unsupported` for a named scenario). A posting whose voucher type is missing from
//!    the masters is out too, because it might be one of those.
//! 3. By status: `Regular` is in; `PostDated` is in (it enters the books on its date, and rule 1
//!    already cut later ones); `Cancelled` is out (no amounts); `Optional` is in only when the
//!    scenario is [`Scenario::Including`] and lists [`VoucherStatus::Optional`].
//!
//! The result keeps the order of [`Books::postings`] (sorted by `(date, voucher)`).

use ledgerly_core::Date;
use ledgerly_model::{BaseVoucherKind, Books, Posting, Scenario, VoucherStatus};

/// The postings of `books` that `scenario` includes as of `as_of`, in book order.
#[must_use]
pub fn effective_postings<'a>(
    books: &'a Books,
    scenario: &Scenario,
    as_of: Date,
) -> Vec<&'a Posting> {
    books
        .postings
        .iter()
        .filter(|posting| is_effective(books, posting, scenario, as_of))
        .collect()
}

fn is_effective(books: &Books, posting: &Posting, scenario: &Scenario, as_of: Date) -> bool {
    if posting.date > as_of {
        return false;
    }
    if !type_can_post(books, posting) {
        return false;
    }
    match posting.status {
        VoucherStatus::Regular | VoucherStatus::PostDated => true,
        VoucherStatus::Cancelled => false,
        VoucherStatus::Optional => match scenario {
            Scenario::Actual => false,
            Scenario::Including(statuses) => statuses.contains(&VoucherStatus::Optional),
        },
    }
}

/// Whether the posting's voucher type may ever reach the books (not memorandum or reversing, and
/// known).
fn type_can_post(books: &Books, posting: &Posting) -> bool {
    let Some(voucher_type) = books.masters.voucher_types.get(&posting.type_id) else {
        return false;
    };
    match voucher_type.base {
        BaseVoucherKind::Memorandum | BaseVoucherKind::ReversingJournal => false,
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => true,
    }
}
