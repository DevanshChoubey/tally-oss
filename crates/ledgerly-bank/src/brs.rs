//! Bank reconciliation statement (spec 6.5). **Card K3.**
//!
//! Frozen interface (plan card K3, signature kept in round 3):
//!
//! ```text
//! pub fn brs(books: &Books, ledger: LedgerId, as_of: Date, statements: &[BankStatement]) -> Result<Outcome<Brs>, BankError>;
//! ```
//!
//! Round 3 (owner decision 2026-11-21: copy TallyPrime's documented model). Source:
//! <https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm>
//! (accessed 2026-11-21). It replaces the round-2 policy, under which any missing, short or
//! overlapping statement made the whole BRS `Unsupported`.
//!
//! The book-derived part is always a value:
//!
//! - *Balance as per company books* is the ledger's book balance on `as_of` (opening balance plus
//!   the postings dated on or before `as_of`). Balance is debit-positive: a bank ledger that holds
//!   money has a positive balance.
//! - *Amounts not reflected in bank* are the voucher lines on the bank ledger, dated on or before
//!   `as_of`, whose bank date is blank or later than `as_of`. They are split into deposits (debit
//!   lines to the bank ledger) and payments (credit lines). Which vouchers count: `Regular` and
//!   `PostDated` only (optional and cancelled vouchers are left out; spec invariant 3).
//! - *Balance as per bank* = books balance - uncleared deposits + uncleared payments. Sign rule
//!   for a debit-balance bank ledger: a deposit is debited in the books but not yet credited by
//!   the bank, so the bank's balance is still lower; it is subtracted. A payment is credited in
//!   the books but not yet debited by the bank, so the bank's balance is still higher; it is
//!   added back. The page states the rule in words only, so this follows from the identity.
//! - The ledger's opening balance is treated as cleared. Items the bank had not cleared at the
//!   start of the books surface in [`Brs::difference`], not hidden.
//!
//! The statement-side figure is different. A statement is an imported fact that lists its own
//! opening balance, closing balance and dated lines ([`BankStatement`]). The file does not store
//! its own start and end dates, only the first and last line dates. The balance at `as_of` is
//! exactly known from one statement only when:
//!
//! - `as_of` is on or after the statement's first line date and strictly before its last line
//!   date (the file lists every transaction on dates up to a later line, so the lines on `as_of`
//!   are complete); and
//! - the balance is the statement's opening balance plus its lines dated on or before `as_of`
//!   (the opening is the balance at the start of the file, checked by K1).
//!
//! Consequences:
//!
//! - Gaps between statements matter only for dates inside a gap (no statement covers them, so
//!   `Unsupported`). A covered date does not care about other statements.
//! - Overlap: when two statements cover `as_of`, the figure is given only if they agree on the
//!   balance at `as_of`; otherwise `Unsupported`.
//! - A statement's last line date is a covered date only when the file states its end date
//!   ([`BankStatement::end_date`], the `:62F:` date for MT940, card C3); then every date from the
//!   first line up to and including the end date is covered. A statement with no stated end date
//!   (CSV, XLSX) covers dates strictly before its last line only.
//! - No statement, no lines, or `as_of` outside every statement: `Unsupported` for the statement
//!   side only.
//!
//! [`Brs::difference`] is `balance as per statement - balance as per bank`. The page shows the
//! difference as the gap between the two figures but does not say which one is subtracted, so the
//! direction is our convention and is not Tally-documented.
//!
//! The statement-side figure and the difference are [`Outcome`]s, so an `Unsupported` statement
//! side never hides the books-derived figures.

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
    /// Balance as per company books on `as_of`, debit-positive (positive when the business holds
    /// money). Always a value.
    pub books_balance: Paise,
    /// Debits to the bank ledger the bank had not credited by `as_of`, to be deducted.
    pub uncleared_deposits: Vec<BrsItem>,
    /// Credits to the bank ledger the bank had not debited by `as_of`, to be added back.
    pub uncleared_payments: Vec<BrsItem>,
    /// Total of [`Brs::uncleared_deposits`].
    pub uncleared_deposits_total: Paise,
    /// Total of [`Brs::uncleared_payments`].
    pub uncleared_payments_total: Paise,
    /// Balance as per bank, computed from the books:
    /// `books_balance - uncleared_deposits_total + uncleared_payments_total`. Always a value.
    pub balance_as_per_bank: Paise,
    /// Balance as per the imported statements at `as_of`, credit-positive. `Unsupported` unless
    /// exactly known from the statements (see the module docs for the rule).
    pub statement_balance: Outcome<Paise>,
    /// `statement_balance - balance_as_per_bank`; `Unsupported` when `statement_balance` is.
    pub difference: Outcome<Paise>,
}

impl Brs {
    /// `Value(true)` when the statement is known and agrees with the books; `Value(false)` when
    /// it is known and differs; `Unsupported` when the statement-side balance is not known.
    pub fn is_reconciled(&self) -> Outcome<bool> {
        self.difference.clone().map(ledgerly_core::Paise::is_zero)
    }
}

/// Builds the bank reconciliation statement of `ledger` as of `as_of`.
///
/// The result is always `Outcome::Value`: the books-derived figures never depend on the
/// statements. The statement side is `Unsupported` inside the [`Brs`] when it is not exactly known.
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

    let balance_as_per_bank = books_balance
        .checked_sub(items.deposits_total)?
        .checked_add(items.payments_total)?;
    let statement_balance = statement_side(statements, as_of)?;
    let difference = match &statement_balance {
        Outcome::Value(statement) => Outcome::Value(statement.checked_sub(balance_as_per_bank)?),
        Outcome::Unsupported(unsupported) => Outcome::Unsupported(unsupported.clone()),
    };
    Ok(Outcome::Value(Brs {
        ledger,
        as_of,
        books_balance,
        uncleared_deposits: items.deposits,
        uncleared_payments: items.payments,
        uncleared_deposits_total: items.deposits_total,
        uncleared_payments_total: items.payments_total,
        balance_as_per_bank,
        statement_balance,
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

/// The balance as per the imported statements at `as_of`, or `Unsupported` when no statement
/// exactly determines it (module docs: the coverage rule and the overlap rule).
fn statement_side(statements: &[BankStatement], as_of: Date) -> Result<Outcome<Paise>, MoneyError> {
    let mut covering = Vec::new();
    for statement in statements {
        let Some((first, last)) = statement.period() else {
            continue;
        };
        // A stated end date means the file lists every transaction up to and including that day,
        // so the last day is covered. Without one only dates before the last line are.
        let covered = match statement.end_date() {
            Some(end) => as_of <= end,
            None => as_of < last,
        };
        if first <= as_of && covered {
            covering.push(balance_at(statement, as_of)?);
        }
    }
    let Some(first) = covering.first().copied() else {
        let reason = if statements.is_empty() {
            "import the bank statement for the period; the balance per statement comes only from an imported statement"
        } else {
            "no imported statement covers the as-of date: a statement covers a date from its first line up to, but not including, its last line, because the file states no end date of its own; import a statement that runs past this date, or one that states its end date (MT940)"
        };
        return Ok(Outcome::unsupported(
            "BRS balance per imported statement",
            reason,
        ));
    };
    if covering.iter().any(|balance| *balance != first) {
        return Ok(Outcome::unsupported(
            "BRS over overlapping statements",
            "two imported statements cover the as-of date and give different balances there; one of them is wrong or incomplete, so neither figure is used",
        ));
    }
    Ok(Outcome::Value(first))
}

/// The statement's balance at `as_of`: its opening balance plus its lines dated on or before it.
fn balance_at(statement: &BankStatement, as_of: Date) -> Result<Paise, MoneyError> {
    let mut balance = statement.opening();
    for line in statement.lines().iter().filter(|l| l.date <= as_of) {
        balance = balance.checked_add(line.credit)?.checked_sub(line.debit)?;
    }
    Ok(balance)
}
