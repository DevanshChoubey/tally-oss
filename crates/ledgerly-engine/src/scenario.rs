//! Which postings a scenario includes. **Card E6** implements this module.
//!
//! Interface (plan card E6, changed by review fix round 2 to return an error and an
//! `Unsupported` outcome, because a silent exclusion is a wrong number; rule A1):
//!
//! ```text
//! pub fn effective_postings<'a>(books: &'a Books, scenario: &Scenario, as_of: Date)
//!     -> Result<Outcome<Vec<&'a Posting>>, EngineError>;
//! ```
//!
//! Spec section 5, invariant 3: optional, memorandum, reversing and future post-dated vouchers
//! never affect base balances; only a named scenario includes them.
//!
//! Rules applied, in order, to each posting of [`Books::postings`]:
//!
//! 1. A posting dated after `as_of` is out (every status, so balances are "as of" a date). Such a
//!    posting is not examined further, so its voucher type is not checked either.
//! 2. A posting whose voucher type is missing from the masters is an error
//!    ([`EngineError::NotFound`]). It is never silently dropped, because it might carry any
//!    amount under any type.
//! 3. A posting whose voucher type is `Memorandum` or `ReversingJournal`:
//!    - under [`Scenario::Actual`], or a named scenario that lists no statuses, it is out
//!      (invariant 3);
//!    - under a named scenario that lists statuses, no rule says how it would be included, so the
//!      whole result is [`Outcome::Unsupported`]. The same rule applies in the reports.
//! 4. By status: `Regular` is in; `PostDated` is in (it enters the books on its date, and rule 1
//!    already cut later ones); `Cancelled` is out (no amounts); `Optional` is in only when the
//!    scenario is [`Scenario::Including`] and lists [`VoucherStatus::Optional`].
//!
//! The result keeps the order of [`Books::postings`] (sorted by `(date, voucher)`).

use ledgerly_core::{Date, Outcome};
use ledgerly_model::{BaseVoucherKind, Books, Posting, Scenario, VoucherStatus};

use crate::EngineError;

/// What a voucher type's base kind means for a scenario, before the status is looked at.
enum TypeInclusion {
    /// The type can reach the books; the status decides.
    Consider,
    /// The type never reaches the books under this scenario.
    Exclude,
    /// The type might reach the books, but no rule says how under this scenario.
    Unsupported,
}

/// The postings of `books` that `scenario` includes as of `as_of`, in book order.
///
/// # Errors
///
/// Returns [`EngineError::NotFound`] when a posting dated on or before `as_of` names a voucher
/// type that is not in the masters.
pub fn effective_postings<'a>(
    books: &'a Books,
    scenario: &Scenario,
    as_of: Date,
) -> Result<Outcome<Vec<&'a Posting>>, EngineError> {
    let mut included = Vec::new();
    for posting in &books.postings {
        if posting.date > as_of {
            continue;
        }
        let Some(voucher_type) = books.masters.voucher_types.get(&posting.type_id) else {
            return Err(EngineError::NotFound {
                what: format!("voucher type {}", posting.type_id.0),
            });
        };
        match type_inclusion(voucher_type.base, scenario) {
            TypeInclusion::Exclude => continue,
            TypeInclusion::Unsupported => {
                return Ok(Outcome::unsupported(
                    format!(
                        "voucher {} of type {:?} in a named scenario",
                        posting.voucher.0, voucher_type.base
                    ),
                    "Ledgerly has no rule for including memorandum or reversing-journal \
                     vouchers in a named scenario; use the actual scenario or remove them",
                ));
            }
            TypeInclusion::Consider => {}
        }
        if status_included(posting.status, scenario) {
            included.push(posting);
        }
    }
    Ok(Outcome::Value(included))
}

/// Whether a voucher type's base kind reaches the books under `scenario`.
fn type_inclusion(base: BaseVoucherKind, scenario: &Scenario) -> TypeInclusion {
    match base {
        BaseVoucherKind::Memorandum | BaseVoucherKind::ReversingJournal => match scenario {
            Scenario::Actual => TypeInclusion::Exclude,
            Scenario::Including(statuses) if statuses.is_empty() => TypeInclusion::Exclude,
            Scenario::Including(_) => TypeInclusion::Unsupported,
        },
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
        | BaseVoucherKind::PurchaseOrder => TypeInclusion::Consider,
    }
}

/// Whether a posting with `status` is included under `scenario`.
fn status_included(status: VoucherStatus, scenario: &Scenario) -> bool {
    match status {
        VoucherStatus::Regular | VoucherStatus::PostDated => true,
        VoucherStatus::Cancelled => false,
        VoucherStatus::Optional => match scenario {
            Scenario::Actual => false,
            Scenario::Including(statuses) => statuses.contains(&VoucherStatus::Optional),
        },
    }
}
