//! Figures for the Home screen: cash and bank, receivables and payables with how many bills are
//! overdue, and sales by month.
//!
//! Every figure is worked out here, from the books, so the screen only draws them (rule C4).
//! A figure the books cannot give is `Unsupported` with the reason; it is never a guess.

use std::collections::BTreeSet;

use ledgerly_core::{
    Date, DrCr, FinancialYear, GroupId, LedgerId, Month, Outcome, Paise, Period, SignedAmount,
    Unsupported,
};
use ledgerly_engine::bills::open_bills;
use ledgerly_model::{BaseVoucherKind, Books, Ledger, ReservedGroup, Scenario, VoucherStatus};
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams, RowKind,
};

use crate::error::ApiError;
use crate::facade::Api;

/// Deepest group nesting followed when looking for a reserved ancestor.
const MAX_GROUP_DEPTH: usize = 256;
/// A customer bill counts as late once it is more than this many days past its due date.
pub const CUSTOMER_LATE_DAYS: i64 = 30;
/// A supplier bill counts as late once it is any days past its due date.
pub const SUPPLIER_LATE_DAYS: i64 = 0;

/// Cash in hand and bank accounts together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashAndBank {
    /// The two groups' closing balances added.
    pub total: Paise,
    /// How many ledgers sit under the Bank Accounts group.
    pub bank_accounts: usize,
}

/// Open bills that are late by more than a number of days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateBills {
    /// How many bills.
    pub count: usize,
    /// The amount still open on them.
    pub open: Paise,
}

/// Sales of one calendar month, before GST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthSales {
    /// The first day of the month.
    pub month_start: Date,
    /// Net credit to the Sales Accounts group in the month, up to the as-of date.
    pub sales: Paise,
    /// `true` when the as-of date falls inside the month, so the figure is month to date.
    pub to_date: bool,
}

/// The Home screen's figures as of one date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeFigures {
    /// Cash and bank, one figure.
    pub cash_and_bank: Outcome<CashAndBank>,
    /// Closing balance of Sundry Debtors.
    pub receivables: Outcome<Paise>,
    /// Closing balance of Sundry Creditors.
    pub payables: Outcome<Paise>,
    /// Customer bills more than 30 days past their due date.
    pub receivables_overdue: Outcome<LateBills>,
    /// Supplier bills past their due date.
    pub payables_past_due: Outcome<LateBills>,
    /// Sales by month from the start of the financial year to the as-of month.
    pub sales_by_month: Outcome<Vec<MonthSales>>,
}

impl Api {
    /// The Home screen's figures as of `as_of`.
    ///
    /// Balances are the closing balance of one reserved group each, read from the group summary.
    /// Late bills come from the open bills of the party ledgers, aged by due date as of `as_of`
    /// (the Outstanding report's rule); sales by month are the net credit to Sales Accounts.
    ///
    /// # Errors
    /// [`ApiError::Stale`] after a saved change whose reload failed; [`ApiError::Report`] when a
    /// report engine rejects its input.
    pub fn home_figures(&self, as_of: Date) -> Result<HomeFigures, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(all_unsupported(&u)),
        };
        let cash = self.group_balance(
            books,
            (ReservedGroup::CashInHand, "cash in hand"),
            DrCr::Dr,
            as_of,
        )?;
        let bank = self.group_balance(
            books,
            (ReservedGroup::BankAccounts, "bank accounts"),
            DrCr::Dr,
            as_of,
        )?;
        Ok(HomeFigures {
            cash_and_bank: cash_and_bank(books, &cash, &bank),
            receivables: self.group_balance(
                books,
                (ReservedGroup::SundryDebtors, "receivables"),
                DrCr::Dr,
                as_of,
            )?,
            payables: self.group_balance(
                books,
                (ReservedGroup::SundryCreditors, "payables"),
                DrCr::Cr,
                as_of,
            )?,
            receivables_overdue: late_bills(books, as_of, true, CUSTOMER_LATE_DAYS),
            payables_past_due: late_bills(books, as_of, false, SUPPLIER_LATE_DAYS),
            sales_by_month: sales_by_month(books, as_of),
        })
    }

    /// The closing balance of one reserved group as of `as_of`, read from the group summary's net
    /// balance row. A balance on the side opposite to `normal` is `Unsupported`: the figure has no
    /// side marker.
    fn group_balance(
        &self,
        books: &Books,
        (reserved, what): (ReservedGroup, &str),
        normal: DrCr,
        as_of: Date,
    ) -> Result<Outcome<Paise>, ApiError> {
        let Some(group) = books
            .masters
            .groups
            .values()
            .find(|g| g.reserved == Some(reserved))
        else {
            return Ok(Outcome::unsupported(
                what,
                "the reserved group is not in the books",
            ));
        };
        let period = match Period::new(as_of, as_of) {
            Ok(period) => period,
            Err(e) => {
                return Ok(Outcome::unsupported(
                    what,
                    format!("the date is not usable: {e}"),
                ));
            }
        };
        let params = ReportParams {
            kind: ReportKind::GroupSummary(group.id),
            period,
            as_of,
            scenario: Scenario::Actual,
            detail: Detail::Condensed,
            columns: ColumnSpec::Single,
            filter: ReportFilter::default(),
            exceptions_only: false,
        };
        Ok(match self.report(&params)? {
            Outcome::Value(report) => net_balance(&report, normal, what),
            Outcome::Unsupported(u) => Outcome::Unsupported(u),
        })
    }
}

/// Every figure unsupported, for books that cannot be loaded.
fn all_unsupported(u: &Unsupported) -> HomeFigures {
    HomeFigures {
        cash_and_bank: Outcome::Unsupported(u.clone()),
        receivables: Outcome::Unsupported(u.clone()),
        payables: Outcome::Unsupported(u.clone()),
        receivables_overdue: Outcome::Unsupported(u.clone()),
        payables_past_due: Outcome::Unsupported(u.clone()),
        sales_by_month: Outcome::Unsupported(u.clone()),
    }
}

/// The group's net balance, read from the report's net balance row (never summed here).
fn net_balance(report: &Report, normal: DrCr, what: &str) -> Outcome<Paise> {
    let refuse = |reason: &str| Outcome::unsupported(what, reason);
    let Some(row) = report
        .rows
        .iter()
        .rev()
        .find(|r| r.kind == RowKind::Subtotal)
    else {
        return refuse("the group summary has no net balance row");
    };
    let (Some(dr_cell), Some(cr_cell)) = (row.cells.first(), row.cells.get(1)) else {
        return refuse("the net balance row does not have a debit and a credit cell");
    };
    let (dr, cr) = match (cell_amount(dr_cell, what), cell_amount(cr_cell, what)) {
        (Ok(dr), Ok(cr)) => (dr, cr),
        (Err(u), Ok(_) | Err(_)) | (Ok(_), Err(u)) => return Outcome::Unsupported(u),
    };
    let amount = match (dr, cr) {
        (None, None) => return Outcome::Value(Paise::ZERO),
        (Some(a), None) | (None, Some(a)) => a,
        (Some(_), Some(_)) => return refuse("the net balance row has both a debit and a credit"),
    };
    if amount.side == normal {
        Outcome::Value(amount.amount)
    } else {
        refuse(
            "the balance is on the side opposite to the usual one; open the balance sheet to see it with its side",
        )
    }
}

/// One cell of the net balance row: `None` when empty, the amount when there is one.
fn cell_amount(cell: &Cell, what: &str) -> Result<Option<SignedAmount>, Unsupported> {
    match cell {
        Cell::Empty => Ok(None),
        Cell::Amount(amount) => Ok(Some(*amount)),
        Cell::Unsupported(u) => Err(u.clone()),
        Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Provisional { .. } => Err(Unsupported {
            what: what.to_owned(),
            reason: "the net balance row holds a cell that is not a final amount".to_owned(),
        }),
    }
}

/// Cash and bank as one figure: the two balances added with checked arithmetic, and the number of
/// ledgers under Bank Accounts.
fn cash_and_bank(
    books: &Books,
    cash: &Outcome<Paise>,
    bank: &Outcome<Paise>,
) -> Outcome<CashAndBank> {
    let what = "cash and bank";
    let (cash, bank) = match (cash, bank) {
        (Outcome::Value(c), Outcome::Value(b)) => (*c, *b),
        (Outcome::Unsupported(u), Outcome::Value(_) | Outcome::Unsupported(_))
        | (Outcome::Value(_), Outcome::Unsupported(u)) => return Outcome::Unsupported(u.clone()),
    };
    let Ok(total) = cash.checked_add(bank) else {
        return Outcome::unsupported(what, "the two balances are too large to add");
    };
    let mut bank_accounts = 0_usize;
    for ledger in books.masters.ledgers.values() {
        match under(books, ledger.group, ReservedGroup::BankAccounts) {
            Ok(true) => bank_accounts = bank_accounts.saturating_add(1),
            Ok(false) => {}
            Err(reason) => return Outcome::unsupported(what, reason),
        }
    }
    Outcome::Value(CashAndBank {
        total,
        bank_accounts,
    })
}

/// `true` when `start` or one of its ancestors is the reserved group `reserved`.
fn under(books: &Books, start: Option<GroupId>, reserved: ReservedGroup) -> Result<bool, String> {
    let mut next = start;
    for _ in 0..MAX_GROUP_DEPTH {
        let Some(id) = next else {
            return Ok(false);
        };
        let group = books
            .masters
            .groups
            .get(&id)
            .ok_or_else(|| format!("group {} is not in the books", id.0))?;
        if group.reserved == Some(reserved) {
            return Ok(true);
        }
        next = group.parent;
    }
    Err("a group is nested too deeply or loops back on itself".to_owned())
}

/// The open bills of the party ledgers under Sundry Debtors (`receivable`) or Sundry Creditors
/// that are more than `over_days` past their due date as of `as_of`, on the side the party
/// normally stands (a debit bill for a customer, a credit bill for a supplier).
///
/// `Unsupported` when the count could be short: a party ledger without bill tracking that has
/// postings or an opening balance, or a bill-wise ledger with an opening balance (the model has
/// no opening bill breakup), as the Outstanding report refuses.
fn late_bills(books: &Books, as_of: Date, receivable: bool, over_days: i64) -> Outcome<LateBills> {
    let (reserved, what, side) = if receivable {
        (
            ReservedGroup::SundryDebtors,
            "customer bills overdue",
            DrCr::Dr,
        )
    } else {
        (
            ReservedGroup::SundryCreditors,
            "supplier bills past due",
            DrCr::Cr,
        )
    };
    let refuse = |reason: String| Outcome::unsupported(what, reason);
    let mut count = 0_usize;
    let mut open = Paise::ZERO;
    for ledger in books.masters.ledgers.values() {
        match under(books, ledger.group, reserved) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(reason) => return refuse(reason),
        }
        if let Some(reason) = untracked(books, ledger) {
            return refuse(reason);
        }
        if ledger.bill_wise.is_none() {
            continue;
        }
        let bills = match open_bills(books, ledger.id, as_of) {
            Ok(bills) => bills,
            Err(e) => {
                return refuse(format!(
                    "the bills of '{}' cannot be worked out: {e}",
                    ledger.name
                ));
            }
        };
        for bill in bills {
            let late = bill
                .due
                .is_some_and(|due| (as_of - due).whole_days() > over_days);
            if late && bill.open.side == side {
                count = count.saturating_add(1);
                let Ok(sum) = open.checked_add(bill.open.amount) else {
                    return refuse("the open amounts are too large to add".to_owned());
                };
                open = sum;
            }
        }
    }
    Outcome::Value(LateBills { count, open })
}

/// Why `ledger`'s bills cannot be counted, when they cannot.
fn untracked(books: &Books, ledger: &Ledger) -> Option<String> {
    if ledger.bill_wise.is_some() {
        return (!ledger.opening.amount.is_zero()).then(|| {
            format!(
                "ledger '{}' has an opening balance but the model has no opening bill breakup, so its bills would be incomplete",
                ledger.name
            )
        });
    }
    let has_postings = books.postings.iter().any(|p| p.ledger == ledger.id);
    (has_postings || !ledger.opening.amount.is_zero()).then(|| {
        format!(
            "ledger '{}' does not track bills, so its entries cannot be aged",
            ledger.name
        )
    })
}

/// Net credit to the Sales Accounts group for each month from the start of the financial year (or
/// the books, if later) to the month of `as_of`. Regular vouchers count, and post-dated ones on
/// or before `as_of`; optional and cancelled vouchers and memorandum or reversing journals do
/// not (spec invariant 3).
fn sales_by_month(books: &Books, as_of: Date) -> Outcome<Vec<MonthSales>> {
    let what = "sales by month";
    let refuse = |reason: String| Outcome::unsupported(what, reason);
    let Ok(year) = FinancialYear::containing(as_of) else {
        return refuse("the date is outside the supported calendar".to_owned());
    };
    let first_day = year.start.max(books.company.books_begin);
    if as_of < first_day {
        return refuse(format!("{as_of} is before the books begin on {first_day}"));
    }
    let mut ledgers: BTreeSet<LedgerId> = BTreeSet::new();
    for ledger in books.masters.ledgers.values() {
        match under(books, ledger.group, ReservedGroup::SalesAccounts) {
            Ok(true) => {
                ledgers.insert(ledger.id);
            }
            Ok(false) => {}
            Err(reason) => return refuse(reason),
        }
    }
    let Some(mut months) = month_starts(first_day, as_of) else {
        return refuse("the months cannot be listed at the edge of the calendar".to_owned());
    };
    let mut net = vec![Paise::ZERO; months.len()];
    for posting in &books.postings {
        if !ledgers.contains(&posting.ledger) || posting.date < first_day || posting.date > as_of {
            continue;
        }
        let Some(voucher_type) = books.masters.voucher_types.get(&posting.type_id) else {
            return refuse(format!(
                "voucher type {} is not in the books",
                posting.type_id.0
            ));
        };
        if !counts(voucher_type.base, posting.status) {
            continue;
        }
        let index = month_index(first_day, posting.date);
        let Some(slot) = usize::try_from(index).ok().and_then(|i| net.get_mut(i)) else {
            return refuse("a posting falls outside the listed months".to_owned());
        };
        // Credit adds to sales, a debit (a return) takes away.
        let total = match posting.amount.side {
            DrCr::Cr => slot.checked_add(posting.amount.amount),
            DrCr::Dr => slot.checked_sub(posting.amount.amount),
        };
        let Ok(total) = total else {
            return refuse("the sales are too large to add".to_owned());
        };
        *slot = total;
    }
    let mut out = Vec::with_capacity(months.len());
    for (month_start, sales) in months.drain(..).zip(net) {
        let to_date = month_end(month_start).is_some_and(|end| as_of < end);
        out.push(MonthSales {
            month_start,
            sales,
            to_date,
        });
    }
    Outcome::Value(out)
}

/// Whether a posting of a voucher of this kind and status is in the books.
fn counts(base: BaseVoucherKind, status: VoucherStatus) -> bool {
    match base {
        BaseVoucherKind::Memorandum | BaseVoucherKind::ReversingJournal => return false,
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
        | BaseVoucherKind::PurchaseOrder => {}
    }
    match status {
        VoucherStatus::Regular | VoucherStatus::PostDated => true,
        VoucherStatus::Optional | VoucherStatus::Cancelled => false,
    }
}

/// The first day of each month from the month of `first` to the month of `last`.
fn month_starts(first: Date, last: Date) -> Option<Vec<Date>> {
    let mut start = first.replace_day(1).ok()?;
    let mut out = Vec::new();
    while start <= last {
        out.push(start);
        start = next_month(start)?;
    }
    Some(out)
}

/// The first day of the month after `start`'s.
fn next_month(start: Date) -> Option<Date> {
    let year = if start.month() == Month::December {
        start.year().checked_add(1)?
    } else {
        start.year()
    };
    Date::from_calendar_date(year, start.month().next(), 1).ok()
}

/// The last day of `start`'s month.
fn month_end(start: Date) -> Option<Date> {
    next_month(start)?.previous_day()
}

/// How many months `date`'s month is after `first`'s (0 for the same month).
fn month_index(first: Date, date: Date) -> i64 {
    let years = i64::from(date.year()) - i64::from(first.year());
    let months = i64::from(u8::from(date.month())) - i64::from(u8::from(first.month()));
    years * 12 + months
}
