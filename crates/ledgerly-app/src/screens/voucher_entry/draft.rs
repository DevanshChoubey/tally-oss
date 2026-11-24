//! The draft of a voucher being entered: text as typed, and its conversion to a voucher.

use std::collections::BTreeMap;

use ledgerly_core::{
    Date, DrCr, FinancialYear, LedgerId, MoneyError, Outcome, Paise, SignedAmount, VoucherId,
    VoucherTypeId,
};
use ledgerly_model::{
    BankAllocation, BillAllocation, BillRefKind, InstrumentKind, Ledger, LedgerLine, Masters,
    Voucher, VoucherError, VoucherMode, VoucherStatus,
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

/// The part of a line's amount that settles one open bill, as typed in the bill-wise details
/// (TallyPrime's Bill-wise Details: reference type Agst Ref,
/// <https://help.tallysolutions.com/advances-received/>, read 2026-11-25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillDraft {
    /// The name of the open bill.
    pub name: String,
    /// The amount settled, in rupees as typed.
    pub amount_text: String,
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
    /// What the line settles, for a ledger that tracks bills: the open bills it is set against.
    pub bills: Vec<BillDraft>,
    /// Whether the bill-wise details of the line were accepted. With none typed they mean the
    /// whole amount is On Account (not linked to a bill).
    pub bills_done: bool,
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
            bills: Vec::new(),
            bills_done: false,
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
    /// A line's ledger tracks bills and its bill-wise details were not accepted.
    #[error("line {line}: settle the amount against bills first (Enter on its amount opens them)")]
    BillsNeeded {
        /// 1-based line number.
        line: usize,
    },
    /// The bills settled on a line do not add up to the line's amount.
    #[error("line {line}: the bills settle {} but the line is {}", crate::ui::format::amount(*settled), crate::ui::format::amount(*amount))]
    BillsMismatch {
        /// 1-based line number.
        line: usize,
        /// What the bills settle, in rupees.
        settled: Paise,
        /// The line's amount.
        amount: Paise,
    },
    /// The mode is none of the instruments.
    #[error("the mode '{0}' is none of Cheque, NEFT, RTGS, IMPS, UPI, Card or Other")]
    Mode(String),
    /// A mode is set but the account is not a bank account.
    #[error("a mode applies to a bank account; the account is not one")]
    ModeNeedsBank,
    /// The line does not exist.
    #[error("there is no line {line}")]
    NoSuchLine {
        /// 1-based line number.
        line: usize,
    },
}

/// `Unsupported` when `ledger` needs cost centres, or bill allocations on a screen without the
/// bill-wise details (`bill_details` is true on payment and receipt, which have them): a line
/// without them would be wrong. Checked when the ledger is picked
/// and again when the voucher is built.
pub(super) fn entry_support(ledger: &Ledger, bill_details: bool) -> Outcome<()> {
    if (ledger.bill_wise.is_some() && !bill_details) || ledger.cost_centres_applicable {
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
    /// The mode of a single-entry voucher as typed (NEFT, UPI, ...), kept on the bank line.
    pub mode_text: String,
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
            mode_text: String::new(),
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
        if let Outcome::Unsupported(u) = entry_support(ledger, self.is_single_entry()) {
            return Ok(Outcome::Unsupported(u));
        }
        if amount.is_zero() {
            return Err(DraftError::ZeroAmount { line });
        }
        let bills = if ledger.bill_wise.is_some() && !(index == 0 && self.is_single_entry()) {
            Self::bill_allocations(row, amount, line)?
        } else {
            Vec::new()
        };
        Ok(Outcome::Value(LedgerLine {
            ineligible_itc: None,
            ledger: id,
            amount: SignedAmount {
                amount,
                side: row.side,
            },
            bills,
            cost: Vec::new(),
            bank: self.bank_allocation(ledger, index)?,
        }))
    }

    /// The bill references of a line of a bill-wise ledger: each typed amount against its open
    /// bill, or, with the details accepted and none typed, the whole amount On Account.
    fn bill_allocations(
        row: &DraftRow,
        amount: Paise,
        line: usize,
    ) -> Result<Vec<BillAllocation>, DraftError> {
        if !row.bills_done {
            return Err(DraftError::BillsNeeded { line });
        }
        let side = row.side;
        let mut out = Vec::new();
        let mut settled = Paise::ZERO;
        for bill in row
            .bills
            .iter()
            .filter(|b| !b.amount_text.trim().is_empty())
        {
            let part = eval_amount(&bill.amount_text)
                .map_err(|source| DraftError::Amount { line, source })?;
            settled = settled.checked_add(part)?;
            out.push(BillAllocation {
                kind: BillRefKind::Against,
                name: bill.name.clone(),
                due_date: None,
                credit_days: None,
                amount: SignedAmount { amount: part, side },
            });
        }
        if out.is_empty() {
            return Ok(vec![BillAllocation {
                kind: BillRefKind::OnAccount,
                name: String::new(),
                due_date: None,
                credit_days: None,
                amount: SignedAmount { amount, side },
            }]);
        }
        if settled != amount {
            return Err(DraftError::BillsMismatch {
                line,
                settled,
                amount,
            });
        }
        Ok(out)
    }

    /// The bank instrument of the account line (line 0 of a single-entry voucher), from the mode.
    fn bank_allocation(
        &self,
        ledger: &Ledger,
        index: usize,
    ) -> Result<Option<BankAllocation>, DraftError> {
        if index != 0 || !self.is_single_entry() || self.mode_text.trim().is_empty() {
            return Ok(None);
        }
        let instrument = parse_mode(&self.mode_text)
            .ok_or_else(|| DraftError::Mode(self.mode_text.trim().to_owned()))?;
        if ledger.bank.is_none() {
            return Err(DraftError::ModeNeedsBank);
        }
        Ok(Some(BankAllocation {
            instrument,
            instrument_number: None,
            instrument_date: None,
            bank_date: None,
            favouring: None,
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

/// The words shown for `kind` in the Mode field.
#[must_use]
pub fn mode_label(kind: InstrumentKind) -> &'static str {
    match kind {
        InstrumentKind::Cheque => "Cheque",
        InstrumentKind::Neft => "NEFT",
        InstrumentKind::Rtgs => "RTGS",
        InstrumentKind::Imps => "IMPS",
        InstrumentKind::Upi => "UPI",
        InstrumentKind::Card => "Card",
        InstrumentKind::Other => "Other",
    }
}

/// Reads a typed mode: any start of the seven labels, in any case; `None` when it is empty or
/// names none of them.
#[must_use]
pub fn parse_mode(text: &str) -> Option<InstrumentKind> {
    let t = text.trim().to_lowercase();
    if t.is_empty() {
        return None;
    }
    [
        InstrumentKind::Cheque,
        InstrumentKind::Neft,
        InstrumentKind::Rtgs,
        InstrumentKind::Imps,
        InstrumentKind::Upi,
        InstrumentKind::Card,
        InstrumentKind::Other,
    ]
    .into_iter()
    .find(|k| mode_label(*k).to_lowercase().starts_with(&t))
}
