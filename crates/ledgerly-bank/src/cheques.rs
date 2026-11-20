//! Cheque books, post-dated cheques and bounced cheques. **Card K4** implements this module.
//!
//! Frozen interface (plan card K4):
//!
//! ```text
//! pub struct ChequeBook { .. }
//! pub fn check_leaf(book: &ChequeBook, number: &str, books: &Books) -> Result<(), BankError>;
//! pub fn bounce_proposal(voucher: &Voucher, charges: Paise, charges_ledger: LedgerId, date: Date) -> Result<Voucher, BankError>;   // a reversing draft, not posted
//! ```
//!
//! # Leaves
//!
//! A [`ChequeBook`] is a run of consecutive leaf numbers on one bank ledger. A leaf is *used* when
//! any voucher in [`Books`] (cancelled ones included, because the paper leaf is gone) carries a
//! credit line on the book's bank ledger whose bank allocation is a cheque with that number.
//! Numbers are compared as integers, so `000123` and `123` are the same leaf. Only credit
//! (money-out) lines count: a debit line holds a cheque *received* from a customer, whose number
//! belongs to the customer's book, not ours.
//!
//! # Bounce proposal
//!
//! [`bounce_proposal`] builds a draft that reverses every line of the original voucher (each line
//! on the opposite side) and adds the bank charges as a debit to the charges ledger with a credit to
//! the bank ledger. The draft has status [`VoucherStatus::Optional`], so it is kept out of the
//! books until the user accepts it: the function proposes, it never posts (spec 6.5, rule A10).
//!
//! The draft's bill allocations reverse the original's: a reversed line carries the opposite
//! side, so each bill's net effect returns to what it was before the original voucher (a new
//! bill is settled against by name; an `Against`/`Advance` reference is re-opened by an
//! `Against` of the opposite side). The bank instrument is not copied to the reversing lines, so
//! the cheque number is not counted a second time by [`check_leaf`].
use std::collections::BTreeMap;

use ledgerly_core::{Date, DrCr, LedgerId, Paise, SignedAmount, VoucherId};
use ledgerly_model::{
    BillAllocation, BillRefKind, Books, InstrumentKind, LedgerLine, ReservedGroup, Voucher,
    VoucherMode, VoucherStatus,
};
use serde::{Deserialize, Serialize};

use crate::error::BankError;

/// A cheque book: consecutive leaf numbers `first..=last` on one bank ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChequeBook {
    /// The bank ledger the book belongs to.
    pub bank: LedgerId,
    /// First leaf number.
    pub first: u64,
    /// Last leaf number (inclusive).
    pub last: u64,
    /// Date the book was issued by the bank.
    pub issued: Date,
}

impl ChequeBook {
    /// Builds a book.
    ///
    /// # Errors
    /// [`BankError::Invalid`] when `first` is greater than `last`.
    pub fn new(bank: LedgerId, first: u64, last: u64, issued: Date) -> Result<Self, BankError> {
        if first > last {
            return Err(BankError::Invalid {
                field: "cheque book".to_owned(),
                reason: format!("first leaf {first} is after last leaf {last}"),
            });
        }
        Ok(ChequeBook {
            bank,
            first,
            last,
            issued,
        })
    }

    /// `true` when `leaf` lies inside the book's range.
    pub fn contains(&self, leaf: u64) -> bool {
        (self.first..=self.last).contains(&leaf)
    }
}

fn leaf_error(number: &str, reason: impl Into<String>) -> BankError {
    BankError::ChequeLeaf {
        number: number.to_owned(),
        reason: reason.into(),
    }
}

fn invalid(field: &str, reason: impl Into<String>) -> BankError {
    BankError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

/// Parses a cheque number: ASCII digits only, at least one.
fn parse_leaf(number: &str) -> Option<u64> {
    let t = number.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse::<u64>().ok()
}

/// `true` when `ledger` sits under the reserved Bank Accounts or Bank OD group (at any depth).
fn is_bank_ledger(books: &Books, ledger: LedgerId) -> Result<bool, BankError> {
    let l = books
        .masters
        .ledgers
        .get(&ledger)
        .ok_or_else(|| invalid("bank ledger", format!("ledger {} does not exist", ledger.0)))?;
    let mut next = Some(l.group);
    // Bounded walk: a parent cycle in bad data cannot loop forever.
    for _ in 0..=books.masters.groups.len() {
        let Some(gid) = next else {
            return Ok(false);
        };
        let Some(group) = books.masters.groups.get(&gid) else {
            return Ok(false);
        };
        if group.reserved == Some(ReservedGroup::BankAccounts)
            || group.reserved == Some(ReservedGroup::BankOd)
        {
            return Ok(true);
        }
        next = group.parent;
    }
    Ok(false)
}

/// Checks that `number` is a free leaf of `book`.
///
/// # Errors
/// - [`BankError::Invalid`] when the book's bank ledger does not exist.
/// - [`BankError::NotABankLedger`] when it is not under a bank group.
/// - [`BankError::ChequeLeaf`] when the number is not all digits, lies outside the book, or is
///   already used by a voucher (any status) on the book's bank ledger.
pub fn check_leaf(book: &ChequeBook, number: &str, books: &Books) -> Result<(), BankError> {
    if !is_bank_ledger(books, book.bank)? {
        return Err(BankError::NotABankLedger(book.bank));
    }
    let leaf = parse_leaf(number)
        .ok_or_else(|| leaf_error(number, "a cheque number is digits only and not empty"))?;
    if !book.contains(leaf) {
        return Err(leaf_error(
            number,
            format!(
                "outside the cheque book {} to {} of this bank account",
                book.first, book.last
            ),
        ));
    }
    for voucher in books.vouchers.values() {
        for line in &voucher.ledger_lines {
            if line.ledger != book.bank || line.amount.side != DrCr::Cr {
                continue;
            }
            let Some(bank) = &line.bank else {
                continue;
            };
            if bank.instrument != InstrumentKind::Cheque {
                continue;
            }
            let Some(used) = bank.instrument_number.as_deref() else {
                continue;
            };
            if parse_leaf(used) == Some(leaf) {
                let which = voucher
                    .number
                    .clone()
                    .unwrap_or_else(|| format!("id {}", voucher.id.0));
                return Err(leaf_error(
                    number,
                    format!("already used on voucher {which}"),
                ));
            }
        }
    }
    Ok(())
}

/// The reversing reference for a bill allocation (see the module docs).
fn reverse_bill(bill: &BillAllocation) -> BillAllocation {
    let kind = match bill.kind {
        BillRefKind::New | BillRefKind::Against | BillRefKind::Advance => BillRefKind::Against,
        BillRefKind::OnAccount => BillRefKind::OnAccount,
    };
    BillAllocation {
        kind,
        name: bill.name.clone(),
        due_date: None,
        credit_days: None,
        amount: SignedAmount {
            amount: bill.amount.amount,
            side: bill.amount.side.opposite(),
        },
    }
}

fn reverse_line(line: &LedgerLine) -> LedgerLine {
    LedgerLine {
        ledger: line.ledger,
        amount: SignedAmount {
            amount: line.amount.amount,
            side: line.amount.side.opposite(),
        },
        bills: line.bills.iter().map(reverse_bill).collect(),
        cost: line.cost.clone(),
        bank: None,
    }
}

fn plain_line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

/// Checks the original voucher and the proposal's inputs.
fn validate_original(voucher: &Voucher, charges: Paise, date: Date) -> Result<(), BankError> {
    if voucher.status != VoucherStatus::Regular {
        return Err(invalid(
            "voucher",
            "only a voucher that is in the books can be reversed for a bounce",
        ));
    }
    if voucher.gst.is_some() || !voucher.inventory_lines.is_empty() {
        return Err(invalid(
            "voucher",
            "a voucher with GST or stock lines cannot be reversed here; use a credit note",
        ));
    }
    if charges.is_negative() {
        return Err(invalid("charges", "bank charges cannot be negative"));
    }
    if date < voucher.date {
        return Err(invalid(
            "date",
            "the bounce date is before the voucher date",
        ));
    }
    let mut net = Paise::ZERO;
    for line in &voucher.ledger_lines {
        if line.amount.amount.is_negative() {
            return Err(invalid("voucher", "a line has a negative amount"));
        }
        net = net.checked_add(line.amount.to_signed()?)?;
    }
    if !net.is_zero() {
        return Err(invalid("voucher", "debits and credits do not balance"));
    }

    Ok(())
}

/// The one cheque line of `voucher`.
fn single_cheque_line(voucher: &Voucher) -> Result<&LedgerLine, BankError> {
    let mut cheque_lines = voucher.ledger_lines.iter().filter(|l| {
        l.bank
            .as_ref()
            .is_some_and(|b| b.instrument == InstrumentKind::Cheque)
    });
    let bank_line = match (cheque_lines.next(), cheque_lines.next()) {
        (None, _) => return Err(invalid("voucher", "no cheque line on this voucher")),
        (Some(_), Some(_)) => {
            return Err(invalid(
                "voucher",
                "more than one cheque line; reverse the entries by hand",
            ));
        }
        (Some(line), None) => line,
    };
    Ok(bank_line)
}

/// Proposes the entries for a dishonoured cheque: the original voucher reversed, plus the bank's
/// charges. The result is a draft (status `Optional`, id `VoucherId(0)`, no number); the caller
/// shows it and posts it only when the user accepts.
///
/// `voucher` must be a regular voucher with exactly one cheque line, balanced, without GST or
/// inventory lines. `charges` may be zero (no charges lines are added). `date` is the date of
/// the bounce and may not be before the voucher's date.
///
/// # Errors
/// - [`BankError::Invalid`] for each condition above, a negative `charges`, or `charges_ledger`
///   equal to the cheque's bank ledger.
/// - [`BankError::Money`] when summing the original lines overflows.
pub fn bounce_proposal(
    voucher: &Voucher,
    charges: Paise,
    charges_ledger: LedgerId,
    date: Date,
) -> Result<Voucher, BankError> {
    validate_original(voucher, charges, date)?;
    let bank_line = single_cheque_line(voucher)?;
    if charges_ledger == bank_line.ledger {
        return Err(invalid(
            "charges ledger",
            "the charges ledger is the bank ledger itself",
        ));
    }

    let mut lines: Vec<LedgerLine> = voucher.ledger_lines.iter().map(reverse_line).collect();
    if !charges.is_zero() {
        lines.push(plain_line(charges_ledger, SignedAmount::dr(charges)));
        lines.push(plain_line(bank_line.ledger, SignedAmount::cr(charges)));
    }

    let cheque_no = bank_line
        .bank
        .as_ref()
        .and_then(|b| b.instrument_number.as_deref())
        .unwrap_or("(no number)");
    let voucher_no = voucher.number.as_deref().unwrap_or("(no number)");
    Ok(Voucher {
        id: VoucherId(0),
        type_id: voucher.type_id,
        number: None,
        series: None,
        date,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Optional,
        mode: VoucherMode::AsVoucher,
        party: voucher.party,
        place_of_supply: None,
        reverse_charge: false,
        reference: voucher.number.clone(),
        reference_date: Some(voucher.date),
        narration: format!("Cheque {cheque_no} dishonoured: reversal of voucher {voucher_no}"),
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    })
}
