//! The draft of a voucher being entered: text as typed, and its conversion to a voucher.

use std::collections::BTreeMap;

use ledgerly_core::{
    Date, DrCr, FinancialYear, LedgerId, MoneyError, Outcome, Paise, SignedAmount, VoucherId,
    VoucherTypeId,
};
use ledgerly_model::{
    Ledger, LedgerLine, Masters, Voucher, VoucherError, VoucherMode, VoucherStatus,
};
use thiserror::Error;

use super::amount::{AmountError, eval_amount};
use super::date::{EntryDateError, parse_entry_date};
use super::kind::EntryKind;

/// The id a voucher carries until the store assigns one on commit.
pub const UNASSIGNED_VOUCHER_ID: VoucherId = VoucherId(0);

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
pub(super) fn entry_support(ledger: &Ledger) -> Outcome<()> {
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
    /// A draft with one empty line on the kind's first side. A single-entry draft (payment or
    /// receipt) starts with its account line and one particulars line on the other side.
    #[must_use]
    pub fn new(type_id: VoucherTypeId, kind: EntryKind, date_text: String) -> Self {
        let rows = if kind.is_single_entry() {
            vec![
                DraftRow::empty(kind.first_side()),
                DraftRow::empty(kind.first_side().opposite()),
            ]
        } else {
            vec![DraftRow::empty(kind.first_side())]
        };
        VoucherDraft {
            type_id,
            kind,
            date_text,
            narration: String::new(),
            rows,
        }
    }

    /// `true` when the draft is in single-entry style: line 0 is the cash or bank account, which
    /// the user picks once, and the lines after it are the ledgers paid or received. The account's
    /// amount is the total of those lines and is never typed.
    #[must_use]
    pub fn is_single_entry(&self) -> bool {
        self.kind.is_single_entry()
    }

    /// Adds an empty particulars line on the side opposite the account (single entry only).
    pub fn push_particular(&mut self) {
        self.rows
            .push(DraftRow::empty(self.kind.first_side().opposite()));
    }

    /// The total of the particulars lines (every line after the account in single entry), or
    /// `None` while none of them has an amount. An entry aid: the engine checks the voucher when
    /// it posts.
    ///
    /// # Errors
    /// The first particulars line whose amount is invalid, or an overflow.
    pub fn particulars_total(&self) -> Result<Option<Paise>, DraftError> {
        let mut total: Option<Paise> = None;
        for (index, _) in self.rows.iter().enumerate().skip(1) {
            if let Some(amount) = self.line_amount(index)? {
                total = Some(match total {
                    Some(sum) => sum.checked_add(amount)?,
                    None => amount,
                });
            }
        }
        Ok(total)
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
        if index == 0 && self.is_single_entry() {
            return self.particulars_total();
        }
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
        for (i, row) in self
            .rows
            .iter()
            .enumerate()
            .filter(|(i, r)| !r.is_blank() || (*i == 0 && self.is_single_entry()))
        {
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
