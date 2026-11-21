//! Accounting voucher entry. **Card U4** implements this module.
//!
//! Frozen interface (plan card U4):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `VoucherEntry`. The screen enters the vouchers that are plain debit and credit
//! lines: Contra, Payment, Receipt, Journal, Memorandum and Reversing journal. Other voucher kinds
//! answer [`Outcome::Unsupported`] from [`VoucherEntryScreen::open`] (sales, purchase and notes
//! belong to the invoice screen, card U5; stock and order kinds hold no accounting lines).
//!
//! What spec 7 asks of an entry form, and where it lives:
//!
//! * Enter advances, Shift+Enter goes back ([`VoucherEntryScreen::submit`],
//!   [`VoucherEntryScreen::step_back`]).
//! * Type-ahead with fuzzy and alias matching, recents first, balance and GSTIN shown, and a
//!   "create new" item ([`search_ledgers`], [`PickerItem`]).
//! * Inline arithmetic such as `1200*18%` ([`eval_amount`]).
//! * The balancing line is prefilled ([`VoucherDraft::push_prefilled_row`]).
//! * Alt+C creates a ledger inline ([`VoucherEntryScreen::begin_create`]). The screen never writes
//!   to the store: it records a [`CreateLedgerRequest`] for the host and keeps the line
//!   [`LedgerRef::Pending`] until the host calls [`VoucherEntryScreen::resolve_created_ledger`].
//! * A fresh voucher after save ([`VoucherEntryScreen::confirm_saved`]).
//!
//! Rule C4 (the UI never sums anything): the only arithmetic here is on what the user typed. The
//! difference shown under the lines and the prefilled balancing line are an entry aid; the engine
//! recomputes and checks every voucher when it posts it (spec 5 invariant 1, spec 6.1). Ledger
//! balances shown in the type-ahead are handed in by the host through
//! [`VoucherEntryScreen::set_balances`]; a ledger without one shows no balance, never zero.

use std::collections::BTreeMap;
use std::str::FromStr;

use egui::{Id, Key, Modifiers, Ui};
use ledgerly_core::{
    Date, Decimal, DrCr, FinancialYear, GroupId, LedgerId, MoneyError, Month, Outcome, Paise,
    RoundingRule, SignedAmount, VoucherId, VoucherTypeId, round_rupees_to_paise,
};
use ledgerly_model::{
    BaseVoucherKind, Books, Ledger, LedgerLine, Masters, Voucher, VoucherError, VoucherMode,
    VoucherStatus,
};
use thiserror::Error;

use crate::{AppCtx, Screen, ScreenAction};

/// How many type-ahead items are listed at most.
const PICKER_LIMIT: usize = 50;
/// How many recently used ledgers are remembered.
const RECENTS_LIMIT: usize = 20;
/// Deepest nesting of parentheses and unary minus signs an amount may have.
const MAX_AMOUNT_DEPTH: usize = 64;
/// Longest amount text, in characters, that is evaluated.
const MAX_AMOUNT_CHARS: usize = 256;
/// The id a voucher carries until the store assigns one on commit.
pub const UNASSIGNED_VOUCHER_ID: VoucherId = VoucherId(0);

// ---------------------------------------------------------------------------------------------
// Inline arithmetic
// ---------------------------------------------------------------------------------------------

/// Why an amount expression could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AmountError {
    /// Nothing was typed.
    #[error("type an amount")]
    Empty,
    /// A character that cannot be part of an amount.
    #[error("'{ch}' is not allowed in an amount; use digits, + - * / ( ) and %")]
    BadChar {
        /// The offending character.
        ch: char,
    },
    /// A run of digits and dots that is not a number.
    #[error("'{text}' is not a number")]
    BadNumber {
        /// The text.
        text: String,
    },
    /// The expression is incomplete or has a symbol in the wrong place.
    #[error("the amount is incomplete or has a stray symbol at character {at}")]
    Syntax {
        /// 1-based character position.
        at: usize,
    },
    /// `a+b%` or `a-b%`: it is unclear whether b% means b/100 or b percent of a.
    #[error(
        "'+ x%' and '- x%' are ambiguous (character {at}); write the base explicitly, for example 200+200*10%"
    )]
    AmbiguousPercent {
        /// 1-based character position.
        at: usize,
    },
    /// A percentage on its own (`18%`) or at the start of a sum (`10%+5`): it is unclear what it
    /// is a percentage of.
    #[error(
        "a percentage needs something to be a percentage of (character {at}); write for example 1200*18%"
    )]
    LonePercent {
        /// 1-based character position.
        at: usize,
    },
    /// Parentheses or minus signs nested too deeply.
    #[error("the amount nests parentheses or minus signs too deeply (character {at})")]
    TooDeep {
        /// 1-based character position.
        at: usize,
    },
    /// More text than any amount needs.
    #[error("the amount is longer than {max} characters")]
    TooLong {
        /// The limit.
        max: usize,
    },
    /// Division by zero.
    #[error("division by zero")]
    DivideByZero,
    /// An intermediate or final value is too large.
    #[error("the amount is too large to calculate")]
    Overflow,
    /// The result is below zero.
    #[error("the amount is negative; enter it on the other side (Dr or Cr) instead")]
    Negative,
    /// The result does not fit in paise.
    #[error(transparent)]
    Money(#[from] MoneyError),
}

/// One side of an operator while parsing: a value, and whether it is just `x%` (see
/// [`AmountError::AmbiguousPercent`]).
struct Operand {
    value: Decimal,
    bare_percent: bool,
}

impl Operand {
    fn plain(value: Decimal) -> Self {
        Operand {
            value,
            bare_percent: false,
        }
    }
}

/// Recursive-descent parser over the typed characters.
struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos = self.pos.saturating_add(1);
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    /// 1-based position of the next character, for messages.
    fn at(&self) -> usize {
        self.pos.saturating_add(1)
    }

    /// Enters one more nesting level, or fails when the text nests too deeply.
    fn descend(&mut self) -> Result<(), AmountError> {
        self.depth = self.depth.saturating_add(1);
        if self.depth > MAX_AMOUNT_DEPTH {
            return Err(AmountError::TooDeep { at: self.at() });
        }
        Ok(())
    }

    fn ascend(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// `expr := term (('+' | '-') term)*`
    fn expr(&mut self) -> Result<Operand, AmountError> {
        let mut acc = self.term()?;
        loop {
            self.skip_space();
            let op = self.peek();
            let adding = op == Some('+');
            if !adding && op != Some('-') {
                return Ok(acc);
            }
            if acc.bare_percent {
                return Err(AmountError::LonePercent { at: self.at() });
            }
            self.bump();
            let at = self.at();
            let rhs = self.term()?;
            if rhs.bare_percent {
                return Err(AmountError::AmbiguousPercent { at });
            }
            let next = if adding {
                acc.value.checked_add(rhs.value)
            } else {
                acc.value.checked_sub(rhs.value)
            };
            acc = Operand::plain(next.ok_or(AmountError::Overflow)?);
        }
    }

    /// `term := unary (('*' | '/') unary)*`
    fn term(&mut self) -> Result<Operand, AmountError> {
        let mut acc = self.unary()?;
        loop {
            self.skip_space();
            let op = self.peek();
            let multiplying = op == Some('*');
            if !multiplying && op != Some('/') {
                return Ok(acc);
            }
            self.bump();
            let rhs = self.unary()?;
            let next = if multiplying {
                acc.value.checked_mul(rhs.value)
            } else if rhs.value.is_zero() {
                return Err(AmountError::DivideByZero);
            } else {
                acc.value.checked_div(rhs.value)
            };
            acc = Operand::plain(next.ok_or(AmountError::Overflow)?);
        }
    }

    /// `unary := '-' unary | primary '%'*`
    fn unary(&mut self) -> Result<Operand, AmountError> {
        self.skip_space();
        if self.peek() == Some('-') {
            self.bump();
            self.descend()?;
            let inner = self.unary()?;
            self.ascend();
            return Ok(Operand {
                value: -inner.value,
                bare_percent: inner.bare_percent,
            });
        }
        let mut operand = self.primary()?;
        loop {
            self.skip_space();
            if self.peek() != Some('%') {
                return Ok(operand);
            }
            self.bump();
            let value = operand
                .value
                .checked_div(Decimal::ONE_HUNDRED)
                .ok_or(AmountError::Overflow)?;
            operand = Operand {
                value,
                bare_percent: true,
            };
        }
    }

    /// `primary := number | '(' expr ')'`
    fn primary(&mut self) -> Result<Operand, AmountError> {
        self.skip_space();
        let Some(c) = self.peek() else {
            return Err(AmountError::Syntax { at: self.at() });
        };
        if c.is_ascii_digit() || c == '.' {
            let mut text = String::new();
            while let Some(d) = self.peek().filter(|d| d.is_ascii_digit() || *d == '.') {
                text.push(d);
                self.bump();
            }
            return match Decimal::from_str(&text) {
                Ok(value) => Ok(Operand::plain(value)),
                Err(_) => Err(AmountError::BadNumber { text }),
            };
        }
        if c == '(' {
            self.bump();
            self.descend()?;
            let inner = self.expr()?;
            self.ascend();
            self.skip_space();
            if self.peek() != Some(')') {
                return Err(AmountError::Syntax { at: self.at() });
            }
            self.bump();
            return Ok(inner);
        }
        Err(AmountError::Syntax { at: self.at() })
    }
}

fn allowed_in_amount(c: char) -> bool {
    c.is_ascii_digit()
        || c.is_whitespace()
        || matches!(c, '.' | '+' | '-' | '*' | '/' | '(' | ')' | '%')
}

/// Evaluates what the user typed in an amount field to paise.
///
/// Grammar: numbers (digits with one optional `.`), `+ - * /`, parentheses, unary minus, and a
/// postfix `%` meaning "divided by 100", so `1200*18%` is 1200 x 0.18 = 216. `x%` directly after
/// `+` or `-` is refused as ambiguous ([`AmountError::AmbiguousPercent`]), and a percentage that
/// is not the operand of `*` or `/` (a lone `18%`) is refused ([`AmountError::LonePercent`]).
/// Text over 256 characters or nested more than 64 deep is refused. The result is rounded
/// to the nearest paisa, halves away from zero ([`RoundingRule::HalfUpToPaise`]). No floats are
/// involved. A negative result is an error: the side (Dr or Cr) carries the sign.
///
/// # Errors
/// An [`AmountError`] naming what is wrong with the text.
pub fn eval_amount(input: &str) -> Result<Paise, AmountError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(AmountError::Empty);
    }
    if let Some(ch) = text.chars().find(|c| !allowed_in_amount(*c)) {
        return Err(AmountError::BadChar { ch });
    }
    if text
        .chars()
        .take(MAX_AMOUNT_CHARS.saturating_add(1))
        .count()
        > MAX_AMOUNT_CHARS
    {
        return Err(AmountError::TooLong {
            max: MAX_AMOUNT_CHARS,
        });
    }
    let mut parser = Parser {
        chars: text.chars().collect(),
        pos: 0,
        depth: 0,
    };
    let result = parser.expr()?;
    parser.skip_space();
    if parser.pos < parser.chars.len() {
        return Err(AmountError::Syntax { at: parser.at() });
    }
    if result.bare_percent {
        return Err(AmountError::LonePercent { at: 1 });
    }
    let value = result.value;
    if value.is_sign_negative() && !value.is_zero() {
        return Err(AmountError::Negative);
    }
    Ok(round_rupees_to_paise(value, RoundingRule::HalfUpToPaise)?)
}

// ---------------------------------------------------------------------------------------------
// Dates and display
// ---------------------------------------------------------------------------------------------

/// Why a typed date could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EntryDateError {
    /// Nothing was typed.
    #[error("type a date such as 15-4 or 15-04-2026")]
    Empty,
    /// Not day-month, day-month-year or year-month-day.
    #[error("'{0}' is not a date; use day-month, day-month-year or year-month-day")]
    Format(String),
    /// The numbers do not make a calendar date.
    #[error("'{0}' is not a date in the calendar")]
    NotADate(String),
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Reads a date typed in an entry form: `15-4`, `15/4`, `15-04-2026`, `2026-04-15`.
///
/// With no year, the year is the one of the financial year `fy` in which that month falls (months
/// from the month `fy` starts in belong to its first calendar year, the rest to the next).
/// Two-digit years are refused as ambiguous.
///
/// # Errors
/// An [`EntryDateError`].
pub fn parse_entry_date(text: &str, fy: FinancialYear) -> Result<Date, EntryDateError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(EntryDateError::Empty);
    }
    let bad = || EntryDateError::Format(text.to_owned());
    let not_a_date = || EntryDateError::NotADate(text.to_owned());
    let parts: Vec<&str> = text.split(['-', '/']).collect();
    if !parts.iter().all(|p| all_digits(p)) {
        return Err(bad());
    }
    let (day, month, year) = match parts.as_slice() {
        [d, m] => (*d, *m, None),
        [a, b, c] if a.len() == 4 => (*c, *b, Some(*a)),
        [a, b, c] if c.len() == 4 => (*a, *b, Some(*c)),
        _ => return Err(bad()),
    };
    let day: u8 = day.parse().map_err(|_| bad())?;
    let month_number: u8 = month.parse().map_err(|_| bad())?;
    let month = Month::try_from(month_number).map_err(|_| not_a_date())?;
    let year: i32 = match year {
        Some(y) => y.parse().map_err(|_| bad())?,
        None if month >= fy.start.month() => fy.start.year(),
        None => fy.start.year().checked_add(1).ok_or_else(not_a_date)?,
    };
    Date::from_calendar_date(year, month, day).map_err(|_| not_a_date())
}

/// `dd-mm-yyyy`, the form the date field shows after Enter.
#[must_use]
pub fn format_entry_date(date: Date) -> String {
    format!(
        "{:02}-{:02}-{}",
        date.day(),
        u8::from(date.month()),
        date.year()
    )
}

/// Digits grouped the Indian way (`1234567` -> `12,34,567`).
fn group_indian(rupees: u64) -> String {
    let mut out: Vec<char> = Vec::new();
    for (i, digit) in rupees.to_string().chars().rev().enumerate() {
        if i == 3 || (i > 3 && i.saturating_sub(3) % 2 == 0) {
            out.push(',');
        }
        out.push(digit);
    }
    out.iter().rev().collect()
}

/// `12,34,567.00` (a leading `-` for negative amounts). Card C1 owns the shared formatter
/// (`ledgerly_core::format::format_inr`); this private copy exists only so this card does not
/// depend on a card that is not merged yet, and is to be replaced by it in Wave 2.
fn format_amount(amount: Paise) -> String {
    let abs = amount.get().unsigned_abs();
    let sign = if amount.is_negative() { "-" } else { "" };
    format!("{sign}{}.{:02}", group_indian(abs / 100), abs % 100)
}

fn side_label(side: DrCr) -> &'static str {
    match side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    }
}

fn format_signed(amount: SignedAmount) -> String {
    format!(
        "{} {}",
        format_amount(amount.amount),
        side_label(amount.side)
    )
}

// ---------------------------------------------------------------------------------------------
// Voucher kinds
// ---------------------------------------------------------------------------------------------

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

    /// The side the first line starts on: a payment credits the paying account first, the other
    /// kinds start on the debit. It is only a starting point; Alt+S switches it.
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

// ---------------------------------------------------------------------------------------------
// Type-ahead
// ---------------------------------------------------------------------------------------------

/// A ledger offered by the type-ahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerHit {
    /// The ledger.
    pub id: LedgerId,
    /// Its name.
    pub name: String,
    /// The alias that matched, when the name itself did not match as well.
    pub via_alias: Option<String>,
    /// GSTIN, for parties that have one.
    pub gstin: Option<String>,
    /// Current balance as supplied by the host; `None` when not supplied.
    pub balance: Option<SignedAmount>,
    /// Whether the ledger is among the recently used.
    pub recent: bool,
}

/// One line of the type-ahead list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerItem {
    /// An existing ledger.
    Ledger(LedgerHit),
    /// "Create a ledger with the typed name".
    Create {
        /// The typed name.
        name: String,
    },
}

/// How well the lowercase `candidate` matches the lowercase `query`: 0 exact, 1 prefix, 2 a word
/// starts with it, 3 contains it, 4 its letters appear in order. `None` for no match.
fn match_rank(candidate: &str, query: &str) -> Option<u8> {
    if candidate == query {
        Some(0)
    } else if candidate.starts_with(query) {
        Some(1)
    } else if candidate.split_whitespace().any(|w| w.starts_with(query)) {
        Some(2)
    } else if candidate.contains(query) {
        Some(3)
    } else {
        let mut rest = candidate.chars();
        query.chars().all(|q| rest.any(|x| x == q)).then_some(4)
    }
}

/// The best rank of a ledger's name (given lowercase as `name_lower`) or aliases for the
/// lowercase `q`, and the alias that gave it when it beat the name. An empty query matches
/// everything with rank 5.
fn best_match(ledger: &Ledger, name_lower: &str, q: &str) -> Option<(u8, Option<String>)> {
    if q.is_empty() {
        return Some((5, None));
    }
    let mut best = match_rank(name_lower, q).map(|r| (r, None));
    for alias in &ledger.aliases {
        if let Some(r) = match_rank(&alias.to_lowercase(), q)
            && best.as_ref().is_none_or(|(b, _)| r < *b)
        {
            best = Some((r, Some(alias.clone())));
        }
    }
    best
}

/// Ledgers matching `query` (name or alias; exact, prefix, word, substring, then in-order
/// letters), at most 50.
///
/// Order: an exact name or alias match first; then ledgers in `recents` (most recent first);
/// then the rest by match quality and name. An empty query lists every ledger, recents first.
/// When the query is not exactly an existing name, a [`PickerItem::Create`] item ends the list.
#[must_use]
pub fn search_ledgers(
    masters: &Masters,
    query: &str,
    recents: &[LedgerId],
    balances: &BTreeMap<LedgerId, SignedAmount>,
) -> Vec<PickerItem> {
    type SortKey = (u8, usize, u8, String, LedgerId);
    let typed = query.trim();
    let q = typed.to_lowercase();
    let mut exists = false;
    let mut scored: Vec<(SortKey, &Ledger, Option<String>)> = Vec::new();
    for ledger in masters.ledgers.values() {
        let name_lower = ledger.name.to_lowercase();
        exists = exists || name_lower == q;
        let Some((rank, via_alias)) = best_match(ledger, &name_lower, &q) else {
            continue;
        };
        let recent_pos = recents.iter().position(|r| *r == ledger.id);
        let key = (
            u8::from(rank != 0),
            recent_pos.unwrap_or(usize::MAX),
            rank,
            name_lower,
            ledger.id,
        );
        scored.push((key, ledger, via_alias));
    }
    // Only the best PICKER_LIMIT are listed: partition them out before sorting, so a company with
    // tens of thousands of ledgers does not pay a full sort on every frame. Keys end in the ledger
    // id, so they are distinct and the order is total.
    if scored.len() > PICKER_LIMIT {
        let _partitioned = scored.select_nth_unstable_by(PICKER_LIMIT, |a, b| a.0.cmp(&b.0));
        scored.truncate(PICKER_LIMIT);
    }
    scored.sort_by_cached_key(|entry| entry.0.clone());
    let mut items: Vec<PickerItem> = scored
        .into_iter()
        .map(|(key, ledger, via_alias)| {
            PickerItem::Ledger(LedgerHit {
                id: ledger.id,
                name: ledger.name.clone(),
                via_alias,
                gstin: ledger
                    .party
                    .as_ref()
                    .and_then(|p| p.gstin.as_ref())
                    .map(|g| g.as_str().to_owned()),
                balance: balances.get(&ledger.id).copied(),
                recent: key.1 != usize::MAX,
            })
        })
        .collect();
    if !q.is_empty() && !exists {
        items.push(PickerItem::Create {
            name: typed.to_owned(),
        });
    }
    items
}

// ---------------------------------------------------------------------------------------------
// The draft
// ---------------------------------------------------------------------------------------------

/// The ledger of a draft line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerRef {
    /// An existing ledger.
    Chosen(LedgerId),
    /// A ledger created inline (Alt+C) that the host has not created yet.
    Pending(String),
}

/// One line of a voucher being entered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRow {
    /// The ledger, once picked.
    pub ledger: Option<LedgerRef>,
    /// The text in the ledger field (the typed query, or the picked ledger's name).
    pub ledger_text: String,
    /// Debit or credit.
    pub side: DrCr,
    /// The text in the amount field; may be an expression such as `1200*18%`.
    pub amount_text: String,
}

impl DraftRow {
    /// An empty line on `side`.
    #[must_use]
    pub fn empty(side: DrCr) -> Self {
        DraftRow {
            ledger: None,
            ledger_text: String::new(),
            side,
            amount_text: String::new(),
        }
    }

    fn is_blank(&self) -> bool {
        self.ledger.is_none()
            && self.ledger_text.trim().is_empty()
            && self.amount_text.trim().is_empty()
    }
}

/// How far the entered lines are from balancing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Balance {
    /// Debits equal credits (also true while nothing is entered).
    Balanced,
    /// One more line of `amount` on `side` would balance the voucher.
    Needs {
        /// Side of the missing line.
        side: DrCr,
        /// Its amount.
        amount: Paise,
    },
}

/// Why a draft cannot be shown or turned into a voucher.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DraftError {
    /// An amount field could not be evaluated.
    #[error("line {line}: {source}")]
    Amount {
        /// 1-based line number.
        line: usize,
        /// What is wrong.
        source: AmountError,
    },
    /// The date could not be read.
    #[error(transparent)]
    Date(#[from] EntryDateError),
    /// An invariant of spec 5 failed.
    #[error(transparent)]
    Voucher(#[from] VoucherError),
    /// Money arithmetic overflowed.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// A line has an amount without a ledger, or a ledger without an amount.
    #[error("line {line} is incomplete: it needs both a ledger and an amount")]
    IncompleteLine {
        /// 1-based line number.
        line: usize,
    },
    /// A line names a ledger created inline that does not exist yet.
    #[error("line {line}: the ledger '{name}' has not been created yet")]
    PendingLedger {
        ineligible_itc: None,
        /// 1-based line number.
        line: usize,
        /// The ledger's name.
        name: String,
    },
    /// A line amount is zero.
    #[error("line {line}: an amount of zero cannot be posted")]
    ZeroAmount {
        /// 1-based line number.
        line: usize,
    },
    /// The line does not exist.
    #[error("there is no line {line}")]
    NoSuchLine {
        /// 1-based line number.
        line: usize,
    },
}

/// `Unsupported` when `ledger` needs bill allocations or cost centres: this screen has no
/// sub-forms for them, and a line without them would be wrong. Checked when the ledger is picked
/// and again when the voucher is built.
fn entry_support(ledger: &Ledger) -> Outcome<()> {
    if ledger.bill_wise.is_some() || ledger.cost_centres_applicable {
        return Outcome::unsupported(
            format!(
                "posting to '{}' on the accounting entry screen",
                ledger.name
            ),
            "the ledger needs bill allocations or cost centres, whose entry sub-forms are not built yet",
        );
    }
    Outcome::Value(())
}

/// A voucher being entered: text fields as typed, turned into a [`Voucher`] only by
/// [`VoucherDraft::build_voucher`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherDraft {
    /// Voucher type.
    pub type_id: VoucherTypeId,
    /// Entry kind of the type.
    pub kind: EntryKind,
    /// The date field as typed.
    pub date_text: String,
    /// Narration.
    pub narration: String,
    /// The lines. There is always at least one.
    pub rows: Vec<DraftRow>,
}

impl VoucherDraft {
    /// A draft with one empty line on the kind's first side.
    #[must_use]
    pub fn new(type_id: VoucherTypeId, kind: EntryKind, date_text: String) -> Self {
        VoucherDraft {
            type_id,
            kind,
            date_text,
            narration: String::new(),
            rows: vec![DraftRow::empty(kind.first_side())],
        }
    }

    /// `true` when no line holds anything and there is no narration (the date does not count).
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.narration.trim().is_empty() && self.rows.iter().all(DraftRow::is_blank)
    }

    /// The amount of line `index` (0-based): `None` while its amount field is empty.
    ///
    /// # Errors
    /// [`DraftError::Amount`] when the text is not a valid amount, [`DraftError::NoSuchLine`].
    pub fn line_amount(&self, index: usize) -> Result<Option<Paise>, DraftError> {
        let line = index.saturating_add(1);
        let row = self
            .rows
            .get(index)
            .ok_or(DraftError::NoSuchLine { line })?;
        if row.amount_text.trim().is_empty() {
            return Ok(None);
        }
        eval_amount(&row.amount_text)
            .map(Some)
            .map_err(|source| DraftError::Amount { line, source })
    }

    /// How far the lines entered so far are from balancing.
    ///
    /// # Errors
    /// The first line whose amount is invalid, or an overflow.
    pub fn balance(&self) -> Result<Balance, DraftError> {
        let mut dr = Paise::ZERO;
        let mut cr = Paise::ZERO;
        for (i, row) in self.rows.iter().enumerate() {
            let Some(amount) = self.line_amount(i)? else {
                continue;
            };
            match row.side {
                DrCr::Dr => dr = dr.checked_add(amount)?,
                DrCr::Cr => cr = cr.checked_add(amount)?,
            }
        }
        Ok(match dr.cmp(&cr) {
            std::cmp::Ordering::Equal => Balance::Balanced,
            std::cmp::Ordering::Greater => Balance::Needs {
                side: DrCr::Cr,
                amount: dr.checked_sub(cr)?,
            },
            std::cmp::Ordering::Less => Balance::Needs {
                side: DrCr::Dr,
                amount: cr.checked_sub(dr)?,
            },
        })
    }

    /// Appends a line. When the lines are out of balance it is on the side that balances and its
    /// amount is prefilled with the difference; otherwise it is empty, on the last line's side.
    ///
    /// # Errors
    /// The errors of [`VoucherDraft::balance`]; nothing is appended then.
    pub fn push_prefilled_row(&mut self) -> Result<(), DraftError> {
        let row = match self.balance()? {
            Balance::Needs { side, amount } => DraftRow {
                amount_text: amount.to_rupees().to_string(),
                ..DraftRow::empty(side)
            },
            Balance::Balanced => {
                DraftRow::empty(self.rows.last().map_or(self.kind.first_side(), |r| r.side))
            }
        };
        self.rows.push(row);
        Ok(())
    }

    /// Turns the draft into a voucher: no number (the store allocates it), regular status,
    /// as-voucher mode, id [`UNASSIGNED_VOUCHER_ID`]. Blank lines are skipped.
    ///
    /// Returns `Unsupported` when a line uses a ledger that needs bill allocations or cost
    /// centres: this screen has no sub-forms for them, and a voucher without them would be wrong.
    ///
    /// # Errors
    /// A [`DraftError`] naming the line or field that is wrong, including
    /// [`VoucherError::BeforeBooksBegin`], [`VoucherError::UnknownLedger`], [`VoucherError::Empty`]
    /// and [`VoucherError::Unbalanced`]. The engine checks all invariants again when it posts.
    pub fn build_voucher(
        &self,
        masters: &Masters,
        fy: FinancialYear,
        books_begin: Date,
    ) -> Result<Outcome<Voucher>, DraftError> {
        let date = parse_entry_date(&self.date_text, fy)?;
        if date < books_begin {
            return Err(VoucherError::BeforeBooksBegin { date, books_begin }.into());
        }
        let mut lines = Vec::new();
        let mut dr = Paise::ZERO;
        let mut cr = Paise::ZERO;
        for (i, row) in self.rows.iter().enumerate().filter(|(_, r)| !r.is_blank()) {
            let line = match self.line_for(masters, i, row)? {
                Outcome::Value(line) => line,
                Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
            };
            match line.amount.side {
                DrCr::Dr => dr = dr.checked_add(line.amount.amount)?,
                DrCr::Cr => cr = cr.checked_add(line.amount.amount)?,
            }
            lines.push(line);
        }
        if lines.is_empty() {
            return Err(VoucherError::Empty.into());
        }
        // Entry-aid check on the typed lines so the user hears about it before the host is asked
        // to post; `validate_voucher` (ledgerly-model, spec 5 invariant 1) and the engine decide
        // for real when the voucher is posted. To be replaced by `money_ops::sum` and dropped in
        // favour of the engine's answer once those exist (docs/interface-requests.md).
        if dr != cr {
            return Err(VoucherError::Unbalanced { dr, cr }.into());
        }
        Ok(Outcome::Value(self.voucher_with(date, lines)))
    }

    /// The model line for the non-blank row `index`.
    fn line_for(
        &self,
        masters: &Masters,
        index: usize,
        row: &DraftRow,
    ) -> Result<Outcome<LedgerLine>, DraftError> {
        let line = index.saturating_add(1);
        let (Some(ledger_ref), Some(amount)) = (row.ledger.as_ref(), self.line_amount(index)?)
        else {
            return Err(DraftError::IncompleteLine { line });
        };
        let id = match ledger_ref {
            LedgerRef::Chosen(id) => *id,
            LedgerRef::Pending(name) => {
                return Err(DraftError::PendingLedger {
                    ineligible_itc: None,
                    line,
                    name: name.clone(),
                });
            }
        };
        let ledger = masters
            .ledgers
            .get(&id)
            .ok_or(VoucherError::UnknownLedger(id))?;
        if let Outcome::Unsupported(u) = entry_support(ledger) {
            return Ok(Outcome::Unsupported(u));
        }
        if amount.is_zero() {
            return Err(DraftError::ZeroAmount { line });
        }
        Ok(Outcome::Value(LedgerLine {
            ineligible_itc: None,
            ledger: id,
            amount: SignedAmount {
                amount,
                side: row.side,
            },
            bills: Vec::new(),
            cost: Vec::new(),
            bank: None,
        }))
    }

    fn voucher_with(&self, date: Date, ledger_lines: Vec<LedgerLine>) -> Voucher {
        Voucher {
            id: UNASSIGNED_VOUCHER_ID,
            type_id: self.type_id,
            number: None,
            series: None,
            date,
            supply_date: None,
            payment_date: None,
            status: VoucherStatus::Regular,
            mode: VoucherMode::AsVoucher,
            party: None,
            place_of_supply: None,
            reverse_charge: false,
            reference: None,
            reference_date: None,
            narration: self.narration.trim().to_owned(),
            ledger_lines,
            inventory_lines: Vec::new(),
            gst: None,
            tally: None,
            extra: BTreeMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The screen
// ---------------------------------------------------------------------------------------------

/// Which field has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The date.
    Date,
    /// The ledger of line `0` (0-based).
    Ledger(usize),
    /// The amount of line `0` (0-based).
    Amount(usize),
    /// The narration.
    Narration,
}

/// A ledger the user asked to create with Alt+C, for the host to create through the API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateLedgerRequest {
    /// The new ledger's name.
    pub name: String,
    /// The group to create it under.
    pub group: GroupId,
}

/// Why a screen could not be opened.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OpenError {
    /// The voucher type is not in the masters.
    #[error("voucher type {0:?} does not exist")]
    UnknownVoucherType(VoucherTypeId),
    /// The voucher type is switched off.
    #[error("voucher type '{0}' is not active; switch it on in its master first")]
    Inactive(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Notice {
    Error(String),
    Info(String),
}

struct CreateForm {
    name: String,
    group: Option<GroupId>,
}

/// What the user did in the widgets this frame, applied after drawing.
enum UiEvent {
    Focus(Field),
    LedgerTyped,
    Submit { field: Field, back: bool },
    Pick(usize),
    Side(usize),
    Leave,
}

/// The keys this screen reads. They are hard-coded here, in one table, until the keymap (card U2,
/// `keymap.rs`) is merged and the screen can ask it for its bindings (spec 7: keys are rebindable;
/// the need is filed in `docs/interface-requests.md`). Ctrl+Enter is deliberately absent: spec 7
/// gives it to "alter master", which this screen does not do, so it is left to the shell.
mod bindings {
    use egui::{Key, Modifiers};

    /// Save: Ctrl+A and Ctrl+S (spec 7 names Ctrl+A; Ctrl+S is the plan's second binding).
    pub(super) const SAVE: [(Modifiers, Key); 2] =
        [(Modifiers::COMMAND, Key::A), (Modifiers::COMMAND, Key::S)];
    /// Create a ledger inline (spec 7: Alt+C).
    pub(super) const CREATE_MASTER: (Modifiers, Key) = (Modifiers::ALT, Key::C);
    /// Switch the line between Dr and Cr. Provisional: no source for this binding yet.
    pub(super) const TOGGLE_SIDE: (Modifiers, Key) = (Modifiers::ALT, Key::S);
    /// Delete the line. Provisional: no source for this binding yet (spec 7 gives Alt+D to
    /// "delete" a voucher, not a line), so it is only a placeholder until the keymap is merged.
    pub(super) const DELETE_LINE: (Modifiers, Key) = (Modifiers::COMMAND, Key::D);
}

/// Keys read before the widgets see them.
enum KeyCmd {
    Save,
    DeleteLine,
    CreateMaster,
    ToggleSide,
    PickUp,
    PickDown,
    Escape,
    DiscardYes,
    DiscardNo,
}

/// The accounting voucher entry screen (card U4).
pub struct VoucherEntryScreen {
    type_name: String,
    fy: FinancialYear,
    books_begin: Date,
    draft: VoucherDraft,
    balances: BTreeMap<LedgerId, SignedAmount>,
    recents: Vec<LedgerId>,
    focus: Field,
    focus_pending: bool,
    pick_index: usize,
    create: Option<CreateForm>,
    confirm_discard: bool,
    notice: Option<Notice>,
    outbox: Option<Voucher>,
    master_requests: Vec<CreateLedgerRequest>,
}

impl VoucherEntryScreen {
    /// Opens an entry screen for voucher type `type_id`, dated `today` (in financial year `fy`).
    ///
    /// Returns `Unsupported` for voucher kinds this screen does not enter.
    ///
    /// # Errors
    /// [`OpenError`] when the voucher type is missing or inactive.
    pub fn open(
        books: &Books,
        type_id: VoucherTypeId,
        fy: FinancialYear,
        today: Date,
    ) -> Result<Outcome<Self>, OpenError> {
        let voucher_type = books
            .masters
            .voucher_types
            .get(&type_id)
            .ok_or(OpenError::UnknownVoucherType(type_id))?;
        if !voucher_type.active {
            return Err(OpenError::Inactive(voucher_type.name.clone()));
        }
        let kind = match EntryKind::from_base(voucher_type.base) {
            Outcome::Value(kind) => kind,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        Ok(Outcome::Value(VoucherEntryScreen {
            type_name: voucher_type.name.clone(),
            fy,
            books_begin: books.company.books_begin,
            draft: VoucherDraft::new(type_id, kind, format_entry_date(today)),
            balances: BTreeMap::new(),
            recents: Vec::new(),
            focus: Field::Ledger(0),
            focus_pending: true,
            pick_index: 0,
            create: None,
            confirm_discard: false,
            notice: None,
            outbox: None,
            master_requests: Vec::new(),
        }))
    }

    /// The voucher being entered.
    #[must_use]
    pub fn draft(&self) -> &VoucherDraft {
        &self.draft
    }

    /// The voucher being entered, for the host to prefill (for example from an AI proposal).
    pub fn draft_mut(&mut self) -> &mut VoucherDraft {
        &mut self.draft
    }

    /// The field that has the keyboard.
    #[must_use]
    pub fn focus(&self) -> Field {
        self.focus
    }

    /// The current error message, if any.
    #[must_use]
    pub fn error_text(&self) -> Option<&str> {
        match &self.notice {
            Some(Notice::Error(s)) => Some(s),
            Some(Notice::Info(_)) | None => None,
        }
    }

    /// The current information message, if any.
    #[must_use]
    pub fn info_text(&self) -> Option<&str> {
        match &self.notice {
            Some(Notice::Info(s)) => Some(s),
            Some(Notice::Error(_)) | None => None,
        }
    }

    /// Ledger balances to show in the type-ahead, from the engine through the API.
    pub fn set_balances(&mut self, balances: BTreeMap<LedgerId, SignedAmount>) {
        self.balances = balances;
    }

    /// Recently used ledgers, most recent first.
    pub fn set_recents(&mut self, recents: Vec<LedgerId>) {
        self.recents = recents;
    }

    fn go(&mut self, field: Field) {
        self.focus = field;
        self.focus_pending = true;
        self.pick_index = 0;
    }

    fn fail(&mut self, message: impl Into<String>) {
        self.notice = Some(Notice::Error(message.into()));
    }

    fn remember(&mut self, id: LedgerId) {
        self.recents.retain(|r| *r != id);
        self.recents.insert(0, id);
        self.recents.truncate(RECENTS_LIMIT);
    }

    /// The type-ahead list for the ledger field that has the keyboard (empty when another field
    /// has it, or its ledger is already picked).
    #[must_use]
    pub fn picker_items(&self, masters: &Masters) -> Vec<PickerItem> {
        let Field::Ledger(i) = self.focus else {
            return Vec::new();
        };
        match self.draft.rows.get(i) {
            Some(row) if row.ledger.is_none() => {
                search_ledgers(masters, &row.ledger_text, &self.recents, &self.balances)
            }
            Some(_) | None => Vec::new(),
        }
    }

    /// The highlighted line of the type-ahead list (0-based).
    #[must_use]
    pub fn pick_index(&self) -> usize {
        self.pick_index
    }

    /// Moves the highlight in the type-ahead list by `delta`, staying inside it.
    pub fn move_pick(&mut self, masters: &Masters, delta: isize) {
        let len = self.picker_items(masters).len();
        self.shift_pick(len, delta);
    }

    /// Moves the highlight by `delta` in a list of `len` items, staying inside it.
    fn shift_pick(&mut self, len: usize, delta: isize) {
        let next = self.pick_index.saturating_add_signed(delta);
        self.pick_index = next.min(len.saturating_sub(1));
    }

    /// Types `text` into the ledger field of line `row` (0-based); a ledger already picked there
    /// is dropped, as the user is searching again.
    pub fn set_ledger_text(&mut self, row: usize, text: &str) {
        if let Some(r) = self.draft.rows.get_mut(row) {
            text.clone_into(&mut r.ledger_text);
            r.ledger = None;
            self.pick_index = 0;
        }
    }

    /// Types `text` into the amount field of line `row` (0-based).
    pub fn set_amount_text(&mut self, row: usize, text: &str) {
        if let Some(r) = self.draft.rows.get_mut(row) {
            text.clone_into(&mut r.amount_text);
        }
    }

    /// Enter: accepts the field that has the keyboard and moves on.
    pub fn submit(&mut self, masters: &Masters) {
        self.notice = None;
        match self.focus {
            Field::Date => self.submit_date(),
            Field::Ledger(i) => self.submit_ledger(masters, i),
            Field::Amount(i) => self.submit_amount(i),
            Field::Narration => self.save(masters),
        }
    }

    /// Shift+Enter: back to the previous field.
    pub fn step_back(&mut self) {
        let previous = match self.focus {
            Field::Date | Field::Ledger(0) => Field::Date,
            Field::Ledger(i) => Field::Amount(i.saturating_sub(1)),
            Field::Amount(i) => Field::Ledger(i),
            Field::Narration => Field::Amount(self.draft.rows.len().saturating_sub(1)),
        };
        self.go(previous);
    }

    fn submit_date(&mut self) {
        match parse_entry_date(&self.draft.date_text, self.fy) {
            Ok(date) => {
                self.draft.date_text = format_entry_date(date);
                self.go(Field::Ledger(0));
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    fn submit_ledger(&mut self, masters: &Masters, i: usize) {
        let Some(row) = self.draft.rows.get(i) else {
            return;
        };
        if row.ledger.is_some() {
            self.go(Field::Amount(i));
            return;
        }
        if row.ledger_text.trim().is_empty() {
            self.end_of_list(i);
            return;
        }
        match self.picker_items(masters).get(self.pick_index).cloned() {
            Some(PickerItem::Ledger(hit)) => {
                if let Some(Outcome::Unsupported(u)) =
                    masters.ledgers.get(&hit.id).map(entry_support)
                {
                    return self.fail(format!("Not supported yet: {} ({})", u.what, u.reason));
                }
                if let Some(r) = self.draft.rows.get_mut(i) {
                    r.ledger = Some(LedgerRef::Chosen(hit.id));
                    r.ledger_text = hit.name;
                }
                self.remember(hit.id);
                self.go(Field::Amount(i));
            }
            Some(PickerItem::Create { name }) => {
                self.create = Some(CreateForm { name, group: None });
            }
            None => self.fail("No ledger matches; press Alt+C to create one."),
        }
    }

    /// Enter on an empty ledger field: ends the list when the lines balance.
    fn end_of_list(&mut self, i: usize) {
        let row_empty = self
            .draft
            .rows
            .get(i)
            .is_some_and(|r| r.amount_text.trim().is_empty());
        match self.draft.balance() {
            Ok(Balance::Balanced) if i > 0 && row_empty => {
                self.draft.rows.truncate(i);
                self.go(Field::Narration);
            }
            Ok(Balance::Balanced | Balance::Needs { .. }) => {
                self.fail("Pick a ledger: type part of its name or alias.");
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    fn submit_amount(&mut self, i: usize) {
        let amount = match self.draft.line_amount(i) {
            Ok(Some(amount)) => amount,
            Ok(None) => return self.fail("Type an amount."),
            Err(e) => return self.fail(e.to_string()),
        };
        if amount.is_zero() {
            return self.fail("An amount of zero cannot be posted.");
        }
        if let Some(r) = self.draft.rows.get_mut(i) {
            r.amount_text = amount.to_rupees().to_string();
        }
        let next = i.saturating_add(1);
        if next < self.draft.rows.len() {
            return self.go(Field::Ledger(next));
        }
        match self.draft.balance() {
            Ok(Balance::Balanced) => self.go(Field::Narration),
            Ok(Balance::Needs { .. }) => match self.draft.push_prefilled_row() {
                Ok(()) => self.go(Field::Ledger(next)),
                Err(e) => self.fail(e.to_string()),
            },
            Err(e) => self.fail(e.to_string()),
        }
    }

    /// Switches line `row` between Dr and Cr.
    pub fn toggle_side(&mut self, row: usize) {
        if let Some(r) = self.draft.rows.get_mut(row) {
            r.side = r.side.opposite();
        }
    }

    /// Deletes the line that has the keyboard (the only line is emptied instead). Bound to a
    /// provisional key, see `bindings::DELETE_LINE`.
    pub fn delete_line(&mut self) {
        let (Field::Ledger(i) | Field::Amount(i)) = self.focus else {
            return;
        };
        if self.draft.rows.len() <= 1 {
            self.draft.rows = vec![DraftRow::empty(self.draft.kind.first_side())];
            return self.go(Field::Ledger(0));
        }
        if i < self.draft.rows.len() {
            self.draft.rows.remove(i);
        }
        self.go(Field::Ledger(
            i.min(self.draft.rows.len().saturating_sub(1)),
        ));
    }

    // ---- creating a ledger inline (Alt+C) ----

    /// Alt+C: opens the "create ledger" form for the ledger field that has the keyboard, with the
    /// typed text as the name.
    pub fn begin_create(&mut self) {
        if let Field::Ledger(i) = self.focus {
            let name = self
                .draft
                .rows
                .get(i)
                .map_or_default(|r| r.ledger_text.trim().to_owned());
            self.create = Some(CreateForm { name, group: None });
        }
    }

    /// `true` while the create-ledger form is open.
    #[must_use]
    pub fn is_creating(&self) -> bool {
        self.create.is_some()
    }

    /// Confirms the create form: records a [`CreateLedgerRequest`] and marks the line pending.
    pub fn confirm_create(&mut self, masters: &Masters, name: &str, group: Option<GroupId>) {
        let name = name.trim();
        let Some(group) = group else {
            return self.fail("Choose a group for the new ledger.");
        };
        if name.is_empty() {
            return self.fail("Type a name for the new ledger.");
        }
        if !masters.groups.contains_key(&group) {
            return self.fail("That group does not exist.");
        }
        let lower = name.to_lowercase();
        let taken = masters
            .ledgers
            .values()
            .any(|l| l.name.to_lowercase() == lower)
            || self
                .master_requests
                .iter()
                .any(|r| r.name.to_lowercase() == lower);
        if taken {
            return self.fail(format!(
                "A ledger called '{name}' already exists; pick it from the list."
            ));
        }
        let Field::Ledger(i) = self.focus else {
            return self.fail("Put the cursor on a ledger field first.");
        };
        if let Some(r) = self.draft.rows.get_mut(i) {
            r.ledger = Some(LedgerRef::Pending(name.to_owned()));
            name.clone_into(&mut r.ledger_text);
        }
        self.master_requests.push(CreateLedgerRequest {
            name: name.to_owned(),
            group,
        });
        self.create = None;
        self.notice = Some(Notice::Info(format!("Creating ledger '{name}'.")));
        self.go(Field::Amount(i));
    }

    /// Ledgers the user asked to create since the last call, for the host to create.
    pub fn take_master_requests(&mut self) -> Vec<CreateLedgerRequest> {
        std::mem::take(&mut self.master_requests)
    }

    /// The host created the requested ledger `name` as `id`: lines waiting for it now use it.
    pub fn resolve_created_ledger(&mut self, name: &str, id: LedgerId) {
        for row in &mut self.draft.rows {
            if row.ledger == Some(LedgerRef::Pending(name.to_owned())) {
                row.ledger = Some(LedgerRef::Chosen(id));
            }
        }
        self.remember(id);
    }

    /// The host could not create the requested ledger `name`: lines waiting for it are cleared.
    pub fn reject_created_ledger(&mut self, name: &str, reason: &str) {
        for row in &mut self.draft.rows {
            if row.ledger == Some(LedgerRef::Pending(name.to_owned())) {
                row.ledger = None;
            }
        }
        self.fail(format!("Ledger '{name}' was not created: {reason}"));
    }

    // ---- saving and leaving ----

    /// Ctrl+A / Ctrl+S: checks the draft and, when it passes, leaves the voucher for the host in
    /// [`VoucherEntryScreen::take_save_request`]. The draft stays on screen until the host calls
    /// [`VoucherEntryScreen::confirm_saved`].
    pub fn save(&mut self, masters: &Masters) {
        match self.draft.build_voucher(masters, self.fy, self.books_begin) {
            Ok(Outcome::Value(voucher)) => {
                self.outbox = Some(voucher);
                self.notice = Some(Notice::Info(
                    "Checked. Waiting for the engine to post it.".to_owned(),
                ));
            }
            Ok(Outcome::Unsupported(u)) => {
                self.fail(format!("Not supported yet: {} ({})", u.what, u.reason));
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    /// The voucher the user asked to save, once.
    pub fn take_save_request(&mut self) -> Option<Voucher> {
        self.outbox.take()
    }

    /// The host posted the voucher: starts a fresh one, keeping type and date.
    pub fn confirm_saved(&mut self) {
        self.draft = VoucherDraft::new(
            self.draft.type_id,
            self.draft.kind,
            self.draft.date_text.clone(),
        );
        self.outbox = None;
        self.notice = Some(Notice::Info("Saved. Next voucher.".to_owned()));
        self.go(Field::Ledger(0));
    }

    /// The host could not post the voucher: the draft stays and the reason is shown.
    pub fn save_failed(&mut self, reason: &str) {
        self.outbox = None;
        self.fail(format!("Not saved: {reason}"));
    }

    /// Esc: returns `true` when the screen can close now; otherwise asks "discard?" first.
    pub fn request_leave(&mut self) -> bool {
        if self.draft.is_blank() {
            true
        } else {
            self.confirm_discard = true;
            false
        }
    }

    /// `true` while the screen asks "discard this voucher?".
    #[must_use]
    pub fn is_confirming_discard(&self) -> bool {
        self.confirm_discard
    }

    /// Answers the "discard?" question; `true` means leave.
    pub fn answer_discard(&mut self, discard: bool) -> bool {
        self.confirm_discard = false;
        discard
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// What `draw_row` needs to know about the screen.
struct RowCtx<'a> {
    base: Id,
    want: Option<Field>,
    focused: Field,
    items: &'a [PickerItem],
    pick: usize,
    balance: Option<SignedAmount>,
}

/// Which text field to draw.
struct FieldSpec {
    id: Id,
    field: Field,
    width: f32,
    hint: &'static str,
}

fn item_label(item: &PickerItem) -> String {
    match item {
        PickerItem::Ledger(hit) => {
            let mut parts = vec![hit.name.clone()];
            if let Some(alias) = &hit.via_alias {
                parts.push(format!("(alias {alias})"));
            }
            if let Some(balance) = hit.balance {
                parts.push(format_signed(balance));
            }
            if let Some(gstin) = &hit.gstin {
                parts.push(gstin.clone());
            }
            parts.join("   ")
        }
        PickerItem::Create { name } => format!("+ Create ledger \"{name}\"   (Alt+C)"),
    }
}

/// A single-line field that can take the keyboard on request. Reports focus and Enter as events
/// and returns whether the text changed.
fn text_field(
    ui: &mut Ui,
    spec: &FieldSpec,
    text: &mut String,
    want: Option<Field>,
    events: &mut Vec<UiEvent>,
) -> bool {
    let response = ui.add(
        egui::TextEdit::singleline(text)
            .id(spec.id)
            .desired_width(spec.width)
            .hint_text(spec.hint),
    );
    if want == Some(spec.field) {
        response.request_focus();
    }
    if response.has_focus() {
        events.push(UiEvent::Focus(spec.field));
    }
    if response.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)) {
        let back = ui.ctx().input(|i| i.modifiers.shift);
        events.push(UiEvent::Submit {
            field: spec.field,
            back,
        });
    }
    response.changed()
}

fn draw_row(ui: &mut Ui, i: usize, row: &mut DraftRow, rc: &RowCtx<'_>, events: &mut Vec<UiEvent>) {
    ui.horizontal(|ui| {
        if ui
            .button(side_label(row.side))
            .on_hover_text("Switch Dr / Cr (Alt+S)")
            .clicked()
        {
            events.push(UiEvent::Side(i));
        }
        let ledger_spec = FieldSpec {
            id: rc.base.with(("ledger", i)),
            field: Field::Ledger(i),
            width: 320.0,
            hint: "Ledger: type to search",
        };
        if text_field(ui, &ledger_spec, &mut row.ledger_text, rc.want, events) {
            row.ledger = None;
            events.push(UiEvent::LedgerTyped);
        }
        let amount_spec = FieldSpec {
            id: rc.base.with(("amount", i)),
            field: Field::Amount(i),
            width: 150.0,
            hint: "Amount, e.g. 1200*18%",
        };
        text_field(ui, &amount_spec, &mut row.amount_text, rc.want, events);
        if let Some(balance) = rc.balance {
            ui.weak(format!("Balance {}", format_signed(balance)));
        }
        if let Some(LedgerRef::Pending(_)) = row.ledger {
            ui.weak("new ledger, not created yet");
        }
        let plain = row
            .amount_text
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.');
        if !plain && let Ok(paise) = eval_amount(&row.amount_text) {
            ui.weak(format!("= {}", format_amount(paise)));
        }
    });
    if rc.focused == Field::Ledger(i) && !rc.items.is_empty() {
        ui.group(|ui| {
            for (j, item) in rc.items.iter().enumerate() {
                if ui
                    .selectable_label(j == rc.pick, item_label(item))
                    .clicked()
                {
                    events.push(UiEvent::Pick(j));
                }
            }
        });
    }
}

fn read_keys(ui: &Ui, picker_open: bool, escape_wanted: bool, confirming: bool) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    ui.ctx().input_mut(|i| {
        if confirming {
            if i.consume_key(Modifiers::NONE, Key::Y) {
                out.push(KeyCmd::DiscardYes);
            }
            if i.consume_key(Modifiers::NONE, Key::N) {
                out.push(KeyCmd::DiscardNo);
            }
        }
        let mut save = false;
        for (modifiers, key) in bindings::SAVE {
            // Consume every binding, so one key press never reaches a widget.
            save |= i.consume_key(modifiers, key);
        }
        if save {
            out.push(KeyCmd::Save);
        }
        if i.consume_key(bindings::DELETE_LINE.0, bindings::DELETE_LINE.1) {
            out.push(KeyCmd::DeleteLine);
        }
        if i.consume_key(bindings::CREATE_MASTER.0, bindings::CREATE_MASTER.1) {
            out.push(KeyCmd::CreateMaster);
        }
        if i.consume_key(bindings::TOGGLE_SIDE.0, bindings::TOGGLE_SIDE.1) {
            out.push(KeyCmd::ToggleSide);
        }
        if picker_open && i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            out.push(KeyCmd::PickDown);
        }
        if picker_open && i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            out.push(KeyCmd::PickUp);
        }
        if escape_wanted && i.consume_key(Modifiers::NONE, Key::Escape) {
            out.push(KeyCmd::Escape);
        }
    });
    out
}

impl VoucherEntryScreen {
    /// `picker_len` is the length of the type-ahead list this frame already computed.
    fn handle_key(&mut self, masters: &Masters, cmd: &KeyCmd, picker_len: usize) -> ScreenAction {
        match cmd {
            KeyCmd::Save => self.save(masters),
            KeyCmd::DeleteLine => self.delete_line(),
            KeyCmd::CreateMaster => self.begin_create(),
            KeyCmd::ToggleSide => {
                if let Field::Ledger(i) | Field::Amount(i) = self.focus {
                    self.toggle_side(i);
                }
            }
            KeyCmd::PickUp => self.shift_pick(picker_len, -1),
            KeyCmd::PickDown => self.shift_pick(picker_len, 1),
            KeyCmd::DiscardYes => {
                if self.answer_discard(true) {
                    return ScreenAction::Pop;
                }
            }
            KeyCmd::DiscardNo => {
                self.answer_discard(false);
            }
            KeyCmd::Escape => return self.handle_escape(),
        }
        ScreenAction::Stay
    }

    fn handle_escape(&mut self) -> ScreenAction {
        if self.create.is_some() {
            self.create = None;
        } else if self.confirm_discard {
            self.confirm_discard = false;
        } else if self.searching() {
            if let Field::Ledger(i) = self.focus {
                self.set_ledger_text(i, "");
            }
        } else if self.request_leave() {
            return ScreenAction::Pop;
        }
        ScreenAction::Stay
    }

    /// `true` while the ledger field that has the keyboard holds an unfinished search.
    fn searching(&self) -> bool {
        let Field::Ledger(i) = self.focus else {
            return false;
        };
        self.draft
            .rows
            .get(i)
            .is_some_and(|r| r.ledger.is_none() && !r.ledger_text.trim().is_empty())
    }

    fn apply_events(&mut self, masters: &Masters, events: Vec<UiEvent>) -> ScreenAction {
        let mut action = ScreenAction::Stay;
        for event in events {
            match event {
                UiEvent::Focus(field) => {
                    if self.focus != field {
                        self.focus = field;
                        self.pick_index = 0;
                    }
                }
                UiEvent::LedgerTyped => self.pick_index = 0,
                UiEvent::Submit { field, back } => {
                    self.focus = field;
                    if back {
                        self.step_back();
                    } else {
                        self.submit(masters);
                    }
                }
                UiEvent::Pick(j) => {
                    self.pick_index = j;
                    self.submit(masters);
                }
                UiEvent::Side(i) => self.toggle_side(i),
                UiEvent::Leave => {
                    if self.request_leave() {
                        action = ScreenAction::Pop;
                    }
                }
            }
        }
        action
    }

    fn balance_text(&self) -> String {
        match self.draft.balance() {
            Ok(Balance::Balanced) => {
                "Lines as typed balance (the engine checks again when it posts).".to_owned()
            }
            Ok(Balance::Needs { side, amount }) => format!(
                "Lines as typed do not balance: a {} line of {} would balance them.",
                side_label(side),
                format_amount(amount)
            ),
            Err(e) => format!("Difference not available: {e}"),
        }
    }

    fn draw_header(&mut self, ui: &mut Ui, base: Id, events: &mut Vec<UiEvent>) {
        let want = self.focus_pending.then_some(self.focus);
        let spec = FieldSpec {
            id: base.with("date"),
            field: Field::Date,
            width: 110.0,
            hint: "dd-mm-yyyy",
        };
        ui.horizontal(|ui| {
            ui.label("Date");
            text_field(ui, &spec, &mut self.draft.date_text, want, events);
            ui.weak(format!("Financial year {}", self.fy.label()));
        });
    }

    fn draw_rows(
        &mut self,
        ui: &mut Ui,
        base: Id,
        items: &[PickerItem],
        events: &mut Vec<UiEvent>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        for (i, row) in self.draft.rows.iter_mut().enumerate() {
            let balance = match &row.ledger {
                Some(LedgerRef::Chosen(id)) => self.balances.get(id).copied(),
                Some(LedgerRef::Pending(_)) | None => None,
            };
            let rc = RowCtx {
                base,
                want,
                focused: self.focus,
                items,
                pick: self.pick_index,
                balance,
            };
            draw_row(ui, i, row, &rc, events);
        }
    }

    fn draw_footer(&mut self, ui: &mut Ui, base: Id, masters: &Masters, events: &mut Vec<UiEvent>) {
        let want = self.focus_pending.then_some(self.focus);
        let spec = FieldSpec {
            id: base.with("narration"),
            field: Field::Narration,
            width: 480.0,
            hint: "Optional",
        };
        ui.horizontal(|ui| {
            ui.label("Narration");
            text_field(ui, &spec, &mut self.draft.narration, want, events);
        });
        ui.label(self.balance_text());
        match &self.notice {
            Some(Notice::Error(s)) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Error: {s}"));
            }
            Some(Notice::Info(s)) => {
                ui.label(format!("Note: {s}"));
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui.button("Save (Ctrl+A)").clicked() {
                self.save(masters);
            }
            if ui.button("Discard (Esc)").clicked() {
                events.push(UiEvent::Leave);
            }
        });
    }

    fn draw_create_form(&mut self, ui: &mut Ui, masters: &Masters) {
        let Some(form) = self.create.as_mut() else {
            return;
        };
        let mut confirmed = false;
        let mut cancelled = false;
        ui.group(|ui| {
            ui.label("Create ledger");
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(280.0));
            });
            let selected = form
                .group
                .and_then(|g| masters.groups.get(&g))
                .map_or("Choose a group", |g| g.name.as_str());
            egui::ComboBox::from_id_salt("u4-create-group")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (id, group) in &masters.groups {
                        ui.selectable_value(&mut form.group, Some(*id), group.name.as_str());
                    }
                });
            ui.horizontal(|ui| {
                confirmed = ui.button("Create").clicked();
                cancelled = ui.button("Cancel (Esc)").clicked();
            });
        });
        let (name, group) = (form.name.clone(), form.group);
        if confirmed {
            self.confirm_create(masters, &name, group);
        } else if cancelled {
            self.create = None;
        }
    }

    fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
        let base = ui.scope_id().with("voucher-entry");
        let escape_wanted = self.create.is_some()
            || self.confirm_discard
            || self.searching()
            || !self.draft.is_blank();
        // The type-ahead list is computed once per frame (twice when a key changed the state it
        // depends on), never once per use: it ranks every ledger (rule D1).
        let mut items = self.picker_items(masters);
        let keys = read_keys(ui, !items.is_empty(), escape_wanted, self.confirm_discard);
        let mut action = ScreenAction::Stay;
        for cmd in &keys {
            if matches!(
                self.handle_key(masters, cmd, items.len()),
                ScreenAction::Pop
            ) {
                action = ScreenAction::Pop;
            }
        }
        if !keys.is_empty() {
            items = self.picker_items(masters);
        }
        ui.heading(self.title());
        if self.confirm_discard {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Warning: discard this voucher? Press Y to discard, N to keep it.",
            );
        }
        let mut events = Vec::new();
        let enabled = !self.confirm_discard && self.create.is_none();
        ui.add_enabled_ui(enabled, |ui| {
            self.draw_header(ui, base, &mut events);
            ui.separator();
            self.draw_rows(ui, base, &items, &mut events);
            ui.separator();
            self.draw_footer(ui, base, masters, &mut events);
        });
        self.draw_create_form(ui, masters);
        self.focus_pending = false;
        if matches!(self.apply_events(masters, events), ScreenAction::Pop) {
            action = ScreenAction::Pop;
        }
        if !keys.is_empty() || self.focus_pending {
            ui.ctx().request_repaint();
        }
        action
    }
}

impl Screen for VoucherEntryScreen {
    fn title(&self) -> String {
        format!("{} voucher", self.type_name)
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if let Some(books) = ctx.books.as_ref() {
            self.draw(ui, &books.masters)
        } else {
            ui.label("No company is open.");
            ScreenAction::Stay
        }
    }
}
