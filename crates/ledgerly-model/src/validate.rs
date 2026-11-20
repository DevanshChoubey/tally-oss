//! Store-free voucher invariants (spec 5, invariants 1, 2, 6). **Card M4** implements this module.
//!
//! Frozen interface:
//!
//! ```text
//! pub fn validate_voucher(v: &Voucher, masters: &Masters, company: &Company) -> Result<(), VoucherError>;
//! ```
//!
//! What is checked here needs only the voucher, the masters and the company:
//!
//! 1. the voucher type exists, and the date is on or after `books_begin` (invariant 6);
//! 2. every ledger the voucher names exists, and no line amount is negative;
//! 3. the kinds that address a party (`Sales`, `Purchase`, `CreditNote`, `DebitNote`) have one;
//! 4. posting lines balance exactly in paise (invariant 1);
//! 5. on a bill-wise ledger the allocations sum to the line, on any other ledger there are none,
//!    and an `Against` reference does not name a bill that the same voucher opens on another
//!    ledger (invariant 2).
//!
//! What needs the store and belongs to the engine: an `Against` reference naming a bill of an
//! earlier voucher, that bill being open, and over-settlement.

use ledgerly_core::{Paise, SignedAmount};

use crate::company::Company;
use crate::error::VoucherError;
use crate::ledger::Ledger;
use crate::masters::Masters;
use crate::voucher::{BillAllocation, BillRefKind, LedgerLine, Voucher, VoucherStatus};
use crate::voucher_type::BaseVoucherKind;

/// Checks the invariants that need no store.
///
/// Checks run in a fixed order and the first failure is returned: voucher type, date, lines
/// present, ledgers and amounts, party, balance, bill allocations. A `Cancelled` voucher keeps its
/// number but carries no amounts, so it is checked for type and date only and must have no lines.
///
/// # Errors
/// The [`VoucherError`] naming the first invariant the voucher breaks. Amount arithmetic that
/// overflows is [`VoucherError::Money`], never a wrapped total.
pub fn validate_voucher(
    v: &Voucher,
    masters: &Masters,
    company: &Company,
) -> Result<(), VoucherError> {
    let vtype = masters
        .voucher_types
        .get(&v.type_id)
        .ok_or(VoucherError::UnknownVoucherType(v.type_id))?;
    if v.date < company.books_begin {
        return Err(VoucherError::BeforeBooksBegin {
            date: v.date,
            books_begin: company.books_begin,
        });
    }
    match v.status {
        VoucherStatus::Cancelled => return check_cancelled(v),
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    if v.ledger_lines.is_empty() && v.inventory_lines.is_empty() {
        return Err(VoucherError::Empty);
    }
    check_ledgers_and_amounts(v, masters)?;
    check_party(v, vtype.base, masters)?;
    check_balance(v)?;
    check_bills(v, masters)
}

fn check_cancelled(v: &Voucher) -> Result<(), VoucherError> {
    if v.ledger_lines.is_empty() && v.inventory_lines.is_empty() {
        Ok(())
    } else {
        Err(VoucherError::Invalid {
            field: "status".to_string(),
            reason: "a cancelled voucher keeps its number but carries no amounts; remove the \
                     lines or change the status"
                .to_string(),
        })
    }
}

fn check_ledgers_and_amounts(v: &Voucher, masters: &Masters) -> Result<(), VoucherError> {
    for (index, line) in v.ledger_lines.iter().enumerate() {
        if !masters.ledgers.contains_key(&line.ledger) {
            return Err(VoucherError::UnknownLedger(line.ledger));
        }
        let negative_bill = line.bills.iter().any(|b| b.amount.amount.is_negative());
        if line.amount.amount.is_negative() || negative_bill {
            return Err(VoucherError::NegativeAmount { line: index });
        }
    }
    for inv in &v.inventory_lines {
        if let Some(ledger) = inv.ledger
            && !masters.ledgers.contains_key(&ledger)
        {
            return Err(VoucherError::UnknownLedger(ledger));
        }
    }
    Ok(())
}

fn check_party(v: &Voucher, base: BaseVoucherKind, masters: &Masters) -> Result<(), VoucherError> {
    if let Some(party) = v.party
        && !masters.ledgers.contains_key(&party)
    {
        return Err(VoucherError::UnknownLedger(party));
    }
    let needs_party = match base {
        BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote => true,
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
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
    };
    if needs_party && v.party.is_none() {
        return Err(VoucherError::MissingParty);
    }
    Ok(())
}

fn check_balance(v: &Voucher) -> Result<(), VoucherError> {
    let mut dr = Paise::ZERO;
    let mut cr = Paise::ZERO;
    for line in &v.ledger_lines {
        match line.amount.side {
            ledgerly_core::DrCr::Dr => dr = dr.checked_add(line.amount.amount)?,
            ledgerly_core::DrCr::Cr => cr = cr.checked_add(line.amount.amount)?,
        }
    }
    if dr == cr {
        Ok(())
    } else {
        Err(VoucherError::Unbalanced { dr, cr })
    }
}

fn check_bills(v: &Voucher, masters: &Masters) -> Result<(), VoucherError> {
    for (index, line) in v.ledger_lines.iter().enumerate() {
        let ledger = masters
            .ledgers
            .get(&line.ledger)
            .ok_or(VoucherError::UnknownLedger(line.ledger))?;
        check_line_bills(index, line, ledger)?;
    }
    check_against_ledgers(v)
}

/// Bill-wise ledgers: allocations sum to the line (signed, debit positive). Other ledgers: none.
fn check_line_bills(index: usize, line: &LedgerLine, ledger: &Ledger) -> Result<(), VoucherError> {
    if ledger.bill_wise.is_none() {
        if line.bills.is_empty() {
            return Ok(());
        }
        return Err(VoucherError::Invalid {
            field: format!("line {index}"),
            reason: "bill allocations are only allowed on bill-wise ledgers; remove them or \
                     turn on bill-wise tracking for the ledger"
                .to_string(),
        });
    }
    let expected = line.amount.to_signed()?;
    let allocated = sum_signed(&line.bills)?;
    if expected == allocated {
        Ok(())
    } else {
        Err(VoucherError::BillAllocationMismatch {
            line: index,
            expected,
            allocated,
        })
    }
}

fn sum_signed(bills: &[BillAllocation]) -> Result<Paise, VoucherError> {
    let mut total = Paise::ZERO;
    for bill in bills {
        let signed = SignedAmount::to_signed(bill.amount)?;
        total = total.checked_add(signed)?;
    }
    Ok(total)
}

/// An `Against` reference must not name a bill this voucher opens on a different ledger.
fn check_against_ledgers(v: &Voucher) -> Result<(), VoucherError> {
    for line in &v.ledger_lines {
        for bill in &line.bills {
            match bill.kind {
                BillRefKind::Against => {}
                BillRefKind::New | BillRefKind::Advance | BillRefKind::OnAccount => continue,
            }
            let mut opened_elsewhere = false;
            let mut opened_here = false;
            for other in &v.ledger_lines {
                for ob in &other.bills {
                    let opens = match ob.kind {
                        BillRefKind::New | BillRefKind::Advance => true,
                        BillRefKind::Against | BillRefKind::OnAccount => false,
                    };
                    if opens && ob.name == bill.name {
                        if other.ledger == line.ledger {
                            opened_here = true;
                        } else {
                            opened_elsewhere = true;
                        }
                    }
                }
            }
            if opened_elsewhere && !opened_here {
                return Err(VoucherError::BillOnWrongLedger {
                    bill: bill.name.clone(),
                    ledger: line.ledger,
                });
            }
        }
    }
    Ok(())
}
