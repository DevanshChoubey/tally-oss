//! Bank reconciliation statement (spec 6.5). **Card K3.**
//!
//! Frozen interface (plan card K3):
//!
//! ```text
//! pub fn brs(books: &Books, ledger: LedgerId, as_of: Date, statements: &[BankStatement]) -> Result<Outcome<Brs>, BankError>;
//! ```
//!
//! The statement lists the balance as per books on the as-of date, the entries in the books that
//! the bank has not yet cleared on that date, and the balance as per the bank statements:
//!
//! ```text
//! balance as per books
//!   - deposits recorded in the books but not yet credited by the bank (debits to the bank ledger)
//!   + payments recorded in the books but not yet debited by the bank (credits to the bank ledger)
//!   = balance as per bank
//! ```
//!
//! Conventions:
//!
//! - Books balance is debit-positive (an asset); statement balances are credit-positive from the
//!   account holder's view (see `ledgerly_bankfmt::RawLine`). Both are positive when the business
//!   holds money, so they are directly comparable.
//! - Which vouchers count: [`VoucherStatus::Regular`] and [`VoucherStatus::PostDated`] vouchers
//!   dated on or before `as_of` (spec invariant 3, scenario `Actual`). Optional and cancelled
//!   vouchers are left out.
//! - A bank line is cleared on `as_of` only when it carries a bank date on or before `as_of`; the
//!   bank date is set only by a user-confirmed match (spec 6.5). A bank line with no bank
//!   allocation has no bank date and is reported as not cleared.
//! - The ledger's opening balance is treated as cleared. If it contains items the bank had not
//!   cleared at the start of the books, they surface in [`Brs::difference`], not hidden.
//! - [`Brs::difference`] is `bank balance - (books balance - deposits + payments)`. It is zero
//!   exactly when every statement line up to `as_of` is accounted for in the books. A non-zero
//!   difference (a bank charge not yet booked, for example) is reported, never absorbed.
//! - The bank balance at `as_of` is the first statement's opening balance plus every statement
//!   line dated on or before `as_of`. Statements must chain (each opening equals the previous
//!   closing) and must not overlap; otherwise the result is `Unsupported`. A statement that ends
//!   before `as_of` is taken to be the latest the bank has given us; nothing is assumed after it.

use std::collections::BTreeSet;

use ledgerly_core::{Date, DrCr, LedgerId, MoneyError, Outcome, Paise, VoucherId};
use ledgerly_model::{Books, Ledger, VoucherStatus};

use crate::BankError;
use crate::statement::BankStatement;

/// One entry in the books on the bank ledger that the bank had not cleared on the as-of date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrsItem {
    /// The voucher holding the entry.
    pub voucher: VoucherId,
    /// Voucher date.
    pub date: Date,
    /// Cheque or reference number from the bank allocation, if any.
    pub instrument_number: Option<String>,
    /// Instrument date from the bank allocation, if any.
    pub instrument_date: Option<Date>,
    /// The bank date, when one is set but falls after the as-of date.
    pub bank_date: Option<Date>,
    /// The amount, never negative.
    pub amount: Paise,
}

/// A bank reconciliation statement as of one date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brs {
    /// The bank ledger.
    pub ledger: LedgerId,
    /// The as-of date.
    pub as_of: Date,
    /// Balance as per books, debit-positive (positive when the business holds money).
    pub books_balance: Paise,
    /// Debits to the bank ledger the bank had not credited by `as_of`, to be deducted.
    pub uncleared_deposits: Vec<BrsItem>,
    /// Credits to the bank ledger the bank had not debited by `as_of`, to be added back.
    pub uncleared_payments: Vec<BrsItem>,
    /// Total of [`Brs::uncleared_deposits`].
    pub uncleared_deposits_total: Paise,
    /// Total of [`Brs::uncleared_payments`].
    pub uncleared_payments_total: Paise,
    /// `books_balance - uncleared_deposits_total + uncleared_payments_total`.
    pub expected_bank_balance: Paise,
    /// Balance as per the bank statements at `as_of`, credit-positive.
    pub bank_balance: Paise,
    /// `bank_balance - expected_bank_balance`; zero when the statement reconciles.
    pub difference: Paise,
}

impl Brs {
    /// `true` when the books, the uncleared items and the bank statements agree exactly.
    pub fn is_reconciled(&self) -> bool {
        self.difference.is_zero()
    }
}

/// Builds the bank reconciliation statement of `ledger` as of `as_of`.
///
/// Returns `Unsupported` when no bank statement covers the date: no statements, a statement with
/// no lines, `as_of` before the first statement line, statements that overlap, or statements that
/// do not chain (a gap).
///
/// # Errors
/// - [`BankError::NotABankLedger`] when the ledger has no bank details.
/// - [`BankError::Invalid`] when the ledger is unknown, a statement belongs to another ledger or
///   appears twice, or the postings on the ledger disagree with the voucher lines.
/// - [`BankError::Money`] on arithmetic overflow.
pub fn brs(
    books: &Books,
    ledger: LedgerId,
    as_of: Date,
    statements: &[BankStatement],
) -> Result<Outcome<Brs>, BankError> {
    let master = books
        .masters
        .ledgers
        .get(&ledger)
        .ok_or_else(|| BankError::Invalid {
            field: "ledger".to_owned(),
            reason: format!("ledger {ledger:?} does not exist in the books"),
        })?;
    if master.bank.is_none() {
        return Err(BankError::NotABankLedger(ledger));
    }
    check_statements(ledger, statements)?;

    let (books_balance, posted) = books_balance(books, master, ledger, as_of)?;
    let items = uncleared_items(books, ledger, as_of)?;
    if posted != items.lines_total {
        return Err(BankError::Invalid {
            field: "books".to_owned(),
            reason: format!(
                "postings to the bank ledger total {} but its voucher lines total {}; the \
                 books read model is out of step with the vouchers",
                posted.to_rupees(),
                items.lines_total.to_rupees()
            ),
        });
    }
    let bank_balance = match bank_balance(statements, as_of)? {
        Outcome::Value(balance) => balance,
        Outcome::Unsupported(unsupported) => return Ok(Outcome::Unsupported(unsupported)),
    };

    let expected_bank_balance = books_balance
        .checked_sub(items.deposits_total)?
        .checked_add(items.payments_total)?;
    let difference = bank_balance.checked_sub(expected_bank_balance)?;
    Ok(Outcome::Value(Brs {
        ledger,
        as_of,
        books_balance,
        uncleared_deposits: items.deposits,
        uncleared_payments: items.payments,
        uncleared_deposits_total: items.deposits_total,
        uncleared_payments_total: items.payments_total,
        expected_bank_balance,
        bank_balance,
        difference,
    }))
}

/// Whether a voucher in this status is in the books for a BRS (scenario `Actual`; the date test
/// is applied separately).
fn counts_in_books(status: VoucherStatus) -> bool {
    match status {
        VoucherStatus::Regular | VoucherStatus::PostDated => true,
        VoucherStatus::Optional | VoucherStatus::Cancelled => false,
    }
}

fn check_statements(ledger: LedgerId, statements: &[BankStatement]) -> Result<(), BankError> {
    let mut seen = BTreeSet::new();
    for statement in statements {
        if statement.ledger() != ledger {
            return Err(BankError::Invalid {
                field: "statements".to_owned(),
                reason: format!(
                    "statement {} was imported for ledger {:?}, not {ledger:?}",
                    statement.file_hash(),
                    statement.ledger()
                ),
            });
        }
        if !seen.insert(statement.file_hash()) {
            return Err(BankError::Invalid {
                field: "statements".to_owned(),
                reason: format!(
                    "statement {} was passed twice; its lines would be counted twice",
                    statement.file_hash()
                ),
            });
        }
    }
    Ok(())
}

/// Opening balance plus postings on or before `as_of`; also returns the postings alone.
fn books_balance(
    books: &Books,
    master: &Ledger,
    ledger: LedgerId,
    as_of: Date,
) -> Result<(Paise, Paise), MoneyError> {
    let mut posted = Paise::ZERO;
    for posting in &books.postings {
        if posting.ledger == ledger && posting.date <= as_of && counts_in_books(posting.status) {
            posted = posted.checked_add(posting.amount.to_signed()?)?;
        }
    }
    let balance = master.opening.to_signed()?.checked_add(posted)?;
    Ok((balance, posted))
}

struct Items {
    deposits: Vec<BrsItem>,
    payments: Vec<BrsItem>,
    deposits_total: Paise,
    payments_total: Paise,
    /// Debit-positive total of every counted line, cleared or not.
    lines_total: Paise,
}

fn uncleared_items(books: &Books, ledger: LedgerId, as_of: Date) -> Result<Items, MoneyError> {
    let mut items = Items {
        deposits: Vec::new(),
        payments: Vec::new(),
        deposits_total: Paise::ZERO,
        payments_total: Paise::ZERO,
        lines_total: Paise::ZERO,
    };
    for voucher in books.vouchers.values() {
        if voucher.date > as_of || !counts_in_books(voucher.status) {
            continue;
        }
        for line in voucher.ledger_lines.iter().filter(|l| l.ledger == ledger) {
            items.lines_total = items.lines_total.checked_add(line.amount.to_signed()?)?;
            let bank_date = line.bank.as_ref().and_then(|b| b.bank_date);
            if bank_date.is_some_and(|d| d <= as_of) {
                continue;
            }
            let item = BrsItem {
                voucher: voucher.id,
                date: voucher.date,
                instrument_number: line.bank.as_ref().and_then(|b| b.instrument_number.clone()),
                instrument_date: line.bank.as_ref().and_then(|b| b.instrument_date),
                bank_date,
                amount: line.amount.amount,
            };
            match line.amount.side {
                DrCr::Dr => {
                    items.deposits_total = items.deposits_total.checked_add(item.amount)?;
                    items.deposits.push(item);
                }
                DrCr::Cr => {
                    items.payments_total = items.payments_total.checked_add(item.amount)?;
                    items.payments.push(item);
                }
            }
        }
    }
    items.deposits.sort_by_key(|i| (i.date, i.voucher));
    items.payments.sort_by_key(|i| (i.date, i.voucher));
    Ok(items)
}

/// The statements' balance at `as_of`, or `Unsupported` when they do not cover it.
fn bank_balance(statements: &[BankStatement], as_of: Date) -> Result<Outcome<Paise>, BankError> {
    if statements.is_empty() {
        return Ok(Outcome::unsupported(
            "BRS without a bank statement",
            "import the bank statement for the period; the balance as per bank comes only from a verified statement",
        ));
    }
    let mut dated: Vec<(&BankStatement, Date, Date)> = Vec::new();
    for statement in statements {
        let Some((from, to)) = statement.period() else {
            return Ok(Outcome::unsupported(
                "BRS with a statement that has no lines",
                format!(
                    "statement {} has no lines, so its position in time is unknown",
                    statement.file_hash()
                ),
            ));
        };
        dated.push((statement, from, to));
    }
    dated.sort_by_key(|(_, from, to)| (*from, *to));

    let Some((first, first_from, first_to)) = dated.first() else {
        return Ok(Outcome::unsupported(
            "BRS without a bank statement",
            "no statement to take the bank balance from",
        ));
    };
    if as_of < *first_from {
        return Ok(Outcome::unsupported(
            "BRS before the first statement line",
            "the statements start after the as-of date and their opening balance is not dated; import an earlier statement",
        ));
    }
    let mut balance = first.opening();
    let mut previous_closing = first.closing();
    // The end of the first statement: a later statement that starts before it overlaps.
    let mut previous_to = first_to;
    add_lines(&mut balance, first, as_of)?;
    for (statement, from, to) in dated.iter().skip(1).filter(|(_, from, _)| *from <= as_of) {
        if from < previous_to {
            return Ok(Outcome::unsupported(
                "BRS over overlapping statements",
                format!(
                    "statement {} starts before the previous statement ends",
                    statement.file_hash()
                ),
            ));
        }
        if statement.opening() != previous_closing {
            return Ok(Outcome::unsupported(
                "BRS over statements with a gap",
                format!(
                    "statement {} opens at {} but the previous one closed at {}; import the statement in between",
                    statement.file_hash(),
                    statement.opening().to_rupees(),
                    previous_closing.to_rupees()
                ),
            ));
        }
        add_lines(&mut balance, statement, as_of)?;
        previous_closing = statement.closing();
        previous_to = to;
    }
    Ok(Outcome::Value(balance))
}

fn add_lines(
    balance: &mut Paise,
    statement: &BankStatement,
    as_of: Date,
) -> Result<(), MoneyError> {
    for line in statement.lines().iter().filter(|l| l.date <= as_of) {
        *balance = balance.checked_add(line.credit)?.checked_sub(line.debit)?;
    }
    Ok(())
}
