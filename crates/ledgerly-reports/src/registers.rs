//! Day book and registers. **Card RP6** implements this module.
//!
//! Frozen interface (plan card RP6):
//!
//! ```text
//! pub fn day_book(books: &Books, params: &ReportParams) -> Result<Outcome<Report>, ReportError>;
//! pub fn register(books: &Books, params: &ReportParams, kind: BaseVoucherKind) -> Result<Outcome<Report>, ReportError>;
//! ```
//!
//! Both are voucher lists (spec 6.2: "Voucher list: day book, ledger vouchers, sales/purchase/
//! journal registers"). They read [`Books::vouchers`], not the postings, because a cancelled
//! voucher keeps its number but has no lines and so no postings.
//!
//! # Which vouchers are listed
//!
//! - Dated inside `params.period`.
//! - `Regular` always; `PostDated` when dated on or before `params.as_of` or when the scenario
//!   names it; `Optional` only when the scenario names it (spec invariant 3, the same meaning as
//!   the profit and loss report).
//! - `Cancelled` always: the list shows the number was used. A cancelled voucher has amount zero
//!   and the status cell reads `Cancelled`. A cancelled voucher that still carries lines is corrupt
//!   input and an error, not a zero.
//! - The day book leaves out Memorandum and Reversing Journal vouchers (they never touch the
//!   books); a named scenario that adds statuses over such a voucher is `Unsupported`, as in the
//!   trial balance. A register of exactly those kinds lists them, because the user asked for them.
//!
//! # Order
//!
//! By voucher date, then by entry sequence. [`Voucher`] carries no sequence field; voucher ids are
//! allocated in entry order, so the id is the entry sequence.
//!
//! # Columns and rows
//!
//! Single column layout: one row per voucher, label = party (else the first ledger line's
//! ledger), key = [`DrillKey::Voucher`], columns `Date`, `Voucher type`, `Voucher no.`, `Amount`,
//! `Status`. The amount is the voucher total: the sum of its debit lines, which must equal the sum
//! of its credit lines. The total row sums the amounts of the listed vouchers.
//!
//! Monthly column layout (registers only): one row per calendar month of the period (months with
//! no voucher show zero), columns `Vouchers` (not cancelled), `Cancelled`, `Amount`, and a total
//! row.
//!
//! # Unsupported (never guessed)
//!
//! Comparative columns, monthly columns on the day book, `exceptions_only`, the `groups` filter,
//! and the amount of voucher kinds that have no accounting lines (notes, orders, stock journal,
//! physical stock): those amount cells are `Cell::Unsupported`, and so is the total over them.
//! A monthly register of such a kind is `Unsupported` as a whole.

use ledgerly_core::{
    Decimal, DrCr, FinancialYear, LedgerId, Outcome, Paise, Qty, SignedAmount, Unsupported,
};
use ledgerly_model::{BaseVoucherKind, Books, Scenario, Voucher, VoucherStatus};

use crate::params::expand_columns;
use crate::{
    Cell, ColumnHeader, ColumnSpec, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, ReportRow, RowKind,
};

/// Builds the day book: every listed voucher of the period, in date then entry order (spec 6.2).
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not the day book, a voucher is dated before the
/// books begin, a voucher is unbalanced, has no lines, or a cancelled voucher has lines;
/// [`ReportError::NotFound`] when a voucher names a voucher type or ledger that does not exist;
/// [`ReportError::Money`] on overflow.
pub fn day_book(books: &Books, params: &ReportParams) -> Result<Outcome<Report>, ReportError> {
    if params.kind != ReportKind::DayBook {
        return Err(ReportError::Invalid {
            field: "kind".to_string(),
            reason: "the day book engine only runs the day book".to_string(),
        });
    }
    build(books, params, None)
}

/// Builds the register of one voucher kind (sales register, purchase register, ...).
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not `Register(kind)` or for the data problems
/// listed on [`day_book`]; [`ReportError::NotFound`] and [`ReportError::Money`] as there;
/// [`ReportError::Period`] when monthly columns reach the edge of the calendar.
pub fn register(
    books: &Books,
    params: &ReportParams,
    kind: BaseVoucherKind,
) -> Result<Outcome<Report>, ReportError> {
    if params.kind != ReportKind::Register(kind) {
        return Err(ReportError::Invalid {
            field: "kind".to_string(),
            reason: "the register engine runs only the register of the kind it is given"
                .to_string(),
        });
    }
    build(books, params, Some(kind))
}

/// Total of a voucher, or the reason it has none.
enum Amount {
    Value(Paise),
    Unsupported(Unsupported),
}

/// One voucher that made the list.
struct Entry<'a> {
    voucher: &'a Voucher,
    type_name: &'a str,
    amount: Amount,
}

/// Outcome of looking at the books for listable vouchers.
enum Collected<'a> {
    Entries(Vec<Entry<'a>>),
    Unsupported(Unsupported),
}

fn build(
    books: &Books,
    params: &ReportParams,
    only: Option<BaseVoucherKind>,
) -> Result<Outcome<Report>, ReportError> {
    if let Some(unsupported) = unsupported_params(params, only) {
        return Ok(unsupported);
    }
    let entries = match collect(books, params, only)? {
        Collected::Entries(entries) => entries,
        Collected::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
    };
    let title = match only {
        Some(kind) => format!("{} Register", kind_name(kind)),
        None => "Day Book".to_string(),
    };
    match &params.columns {
        ColumnSpec::Single => list_report(books, params, title, &entries).map(Outcome::Value),
        ColumnSpec::Monthly => monthly_report(params, title, &entries),
        ColumnSpec::Comparative(_) => Ok(comparative_unsupported()),
    }
}

fn comparative_unsupported() -> Outcome<Report> {
    Outcome::unsupported(
        "voucher list: comparative columns",
        "a voucher list has one column per figure, not per period; run one report per period",
    )
}

/// Parameters this card does not compute.
fn unsupported_params(
    params: &ReportParams,
    only: Option<BaseVoucherKind>,
) -> Option<Outcome<Report>> {
    match &params.columns {
        ColumnSpec::Single => {}
        ColumnSpec::Comparative(_) => return Some(comparative_unsupported()),
        ColumnSpec::Monthly => match only {
            None => {
                return Some(Outcome::unsupported(
                    "day book: monthly columns",
                    "the day book lists vouchers; ask for a register to get monthly totals",
                ));
            }
            Some(kind) if !has_accounting_amount(kind) => {
                return Some(Outcome::unsupported(
                    "register: monthly totals of a kind without accounting lines",
                    "notes, orders, stock journals and physical stock have no voucher amount in the books",
                ));
            }
            Some(_) => {}
        },
    }
    if !params.filter.groups.is_empty() {
        return Some(Outcome::unsupported(
            "voucher list: group filter",
            "filtering by account group is not computed; filter by ledger or voucher type",
        ));
    }
    if params.exceptions_only {
        return Some(Outcome::unsupported(
            "voucher list: exceptions only",
            "no exception rules are defined for the day book or registers",
        ));
    }
    None
}

fn collect<'a>(
    books: &'a Books,
    params: &ReportParams,
    only: Option<BaseVoucherKind>,
) -> Result<Collected<'a>, ReportError> {
    let mut entries = Vec::new();
    for voucher in books.vouchers.values() {
        if !params.period.contains(voucher.date) || !passes_filter(voucher, &params.filter) {
            continue;
        }
        let voucher_type = books
            .masters
            .voucher_types
            .get(&voucher.type_id)
            .ok_or_else(|| ReportError::NotFound {
                what: format!(
                    "voucher type {} of voucher {}",
                    voucher.type_id.0, voucher.id.0
                ),
            })?;
        let base = voucher_type.base;
        if only.is_some_and(|kind| kind != base) {
            continue;
        }
        if only.is_none() && is_off_books(base) {
            match &params.scenario {
                Scenario::Actual => continue,
                Scenario::Including(statuses) if statuses.is_empty() => continue,
                Scenario::Including(_) => {
                    return Ok(Collected::Unsupported(Unsupported {
                        what: "day book: Memorandum or reversing journal vouchers in a named scenario"
                            .to_string(),
                        reason: "no rule defines whether a named scenario includes them; use the actual scenario"
                            .to_string(),
                    }));
                }
            }
        }
        if !listed(voucher, params) {
            continue;
        }
        if voucher.date < books.company.books_begin {
            return Err(ReportError::Invalid {
                field: "vouchers".to_string(),
                reason: format!(
                    "voucher {} is dated {} before the books begin on {}",
                    voucher.id.0, voucher.date, books.company.books_begin
                ),
            });
        }
        entries.push(Entry {
            voucher,
            type_name: &voucher_type.name,
            amount: amount_of(voucher, base)?,
        });
    }
    entries.sort_by_key(|e| (e.voucher.date, e.voucher.id));
    Ok(Collected::Entries(entries))
}

/// Whether the voucher's status puts it on the list (spec invariant 3).
fn listed(voucher: &Voucher, params: &ReportParams) -> bool {
    let named = |status: VoucherStatus| match &params.scenario {
        Scenario::Actual => false,
        Scenario::Including(statuses) => statuses.contains(&status),
    };
    match voucher.status {
        VoucherStatus::Regular | VoucherStatus::Cancelled => true,
        VoucherStatus::PostDated => voucher.date <= params.as_of || named(VoucherStatus::PostDated),
        VoucherStatus::Optional => named(VoucherStatus::Optional),
    }
}

fn passes_filter(voucher: &Voucher, filter: &ReportFilter) -> bool {
    let ReportFilter {
        ledgers,
        groups: _,
        voucher_types,
        items,
    } = filter;
    let type_ok = voucher_types.is_empty() || voucher_types.contains(&voucher.type_id);
    let ledger_ok = ledgers.is_empty()
        || voucher
            .ledger_lines
            .iter()
            .any(|line| ledgers.contains(&line.ledger));
    let item_ok = items.is_empty()
        || voucher
            .inventory_lines
            .iter()
            .any(|line| items.contains(&line.item));
    type_ok && ledger_ok && item_ok
}

/// Kinds that never touch the books (spec invariant 3).
fn is_off_books(base: BaseVoucherKind) -> bool {
    match base {
        BaseVoucherKind::Memorandum | BaseVoucherKind::ReversingJournal => true,
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
        | BaseVoucherKind::PurchaseOrder => false,
    }
}

/// Kinds whose vouchers carry Dr/Cr ledger lines and so have a total amount.
fn has_accounting_amount(base: BaseVoucherKind) -> bool {
    match base {
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal => true,
        BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => false,
    }
}

fn kind_name(base: BaseVoucherKind) -> &'static str {
    match base {
        BaseVoucherKind::Contra => "Contra",
        BaseVoucherKind::Payment => "Payment",
        BaseVoucherKind::Receipt => "Receipt",
        BaseVoucherKind::Journal => "Journal",
        BaseVoucherKind::Sales => "Sales",
        BaseVoucherKind::Purchase => "Purchase",
        BaseVoucherKind::CreditNote => "Credit Note",
        BaseVoucherKind::DebitNote => "Debit Note",
        BaseVoucherKind::Memorandum => "Memorandum",
        BaseVoucherKind::ReversingJournal => "Reversing Journal",
        BaseVoucherKind::ReceiptNote => "Receipt Note",
        BaseVoucherKind::DeliveryNote => "Delivery Note",
        BaseVoucherKind::RejectionIn => "Rejection In",
        BaseVoucherKind::RejectionOut => "Rejection Out",
        BaseVoucherKind::StockJournal => "Stock Journal",
        BaseVoucherKind::PhysicalStock => "Physical Stock",
        BaseVoucherKind::SalesOrder => "Sales Order",
        BaseVoucherKind::PurchaseOrder => "Purchase Order",
    }
}

fn invalid_voucher(voucher: &Voucher, reason: &str) -> ReportError {
    ReportError::Invalid {
        field: "vouchers".to_string(),
        reason: format!("voucher {}: {reason}", voucher.id.0),
    }
}

/// The voucher total: the debit sum, which must equal the credit sum.
fn amount_of(voucher: &Voucher, base: BaseVoucherKind) -> Result<Amount, ReportError> {
    match voucher.status {
        VoucherStatus::Cancelled => {
            if voucher.ledger_lines.is_empty() {
                return Ok(Amount::Value(Paise::ZERO));
            }
            return Err(invalid_voucher(
                voucher,
                "a cancelled voucher carries no amounts but has ledger lines",
            ));
        }
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    if !has_accounting_amount(base) {
        return Ok(Amount::Unsupported(Unsupported {
            what: format!("{} amount", kind_name(base)),
            reason: "this voucher kind has no accounting lines, so it has no amount in the books"
                .to_string(),
        }));
    }
    if voucher.ledger_lines.is_empty() {
        return Err(invalid_voucher(voucher, "it has no ledger lines"));
    }
    let signed = SideTotals::of(voucher)?;
    if signed.debit != signed.credit {
        return Err(invalid_voucher(
            voucher,
            "debit and credit lines do not add up to the same amount",
        ));
    }
    Ok(Amount::Value(signed.debit))
}

struct SideTotals {
    debit: Paise,
    credit: Paise,
}

impl SideTotals {
    fn of(voucher: &Voucher) -> Result<Self, ReportError> {
        let mut totals = SideTotals {
            debit: Paise::ZERO,
            credit: Paise::ZERO,
        };
        for line in &voucher.ledger_lines {
            if line.amount.amount.is_negative() {
                return Err(invalid_voucher(voucher, "a ledger line amount is negative"));
            }
            let side = match line.amount.side {
                DrCr::Dr => &mut totals.debit,
                DrCr::Cr => &mut totals.credit,
            };
            *side = side.checked_add(line.amount.amount)?;
        }
        Ok(totals)
    }
}

fn ledger_name(books: &Books, id: LedgerId, voucher: &Voucher) -> Result<String, ReportError> {
    books
        .masters
        .ledgers
        .get(&id)
        .map(|l| l.name.clone())
        .ok_or_else(|| ReportError::NotFound {
            what: format!("ledger {} named by voucher {}", id.0, voucher.id.0),
        })
}

/// Party, else the first ledger line's ledger, else empty.
fn particulars(books: &Books, voucher: &Voucher) -> Result<String, ReportError> {
    let first = voucher
        .party
        .or_else(|| voucher.ledger_lines.first().map(|l| l.ledger));
    match first {
        Some(id) => ledger_name(books, id, voucher),
        None => Ok(String::new()),
    }
}

fn status_cell(status: VoucherStatus) -> Cell {
    match status {
        VoucherStatus::Regular => Cell::Empty,
        VoucherStatus::Optional => Cell::Text("Optional".to_string()),
        VoucherStatus::PostDated => Cell::Text("Post-dated".to_string()),
        VoucherStatus::Cancelled => Cell::Text("Cancelled".to_string()),
    }
}

fn header(label: &str) -> ColumnHeader {
    ColumnHeader {
        label: label.to_string(),
        period: None,
    }
}

fn amount_cell(amount: &Amount) -> Cell {
    match amount {
        Amount::Value(p) => Cell::Amount(SignedAmount::dr(*p)),
        Amount::Unsupported(u) => Cell::Unsupported(u.clone()),
    }
}

/// Sum of the voucher amounts, or the first unsupported one.
fn total_amount(entries: &[Entry<'_>]) -> Result<Amount, ReportError> {
    let mut total = Paise::ZERO;
    for entry in entries {
        match &entry.amount {
            Amount::Value(p) => total = total.checked_add(*p)?,
            Amount::Unsupported(u) => return Ok(Amount::Unsupported(u.clone())),
        }
    }
    Ok(Amount::Value(total))
}

fn list_report(
    books: &Books,
    params: &ReportParams,
    title: String,
    entries: &[Entry<'_>],
) -> Result<Report, ReportError> {
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        let voucher = entry.voucher;
        rows.push(ReportRow {
            key: Some(DrillKey::Voucher(voucher.id)),
            label: particulars(books, voucher)?,
            depth: 0,
            kind: RowKind::Voucher,
            cells: vec![
                Cell::Date(voucher.date),
                Cell::Text(entry.type_name.to_string()),
                voucher
                    .number
                    .as_ref()
                    .map_or(Cell::Empty, |n| Cell::Text(n.clone())),
                amount_cell(&entry.amount),
                status_cell(voucher.status),
            ],
        });
    }
    let total = total_amount(entries)?;
    Ok(Report {
        title,
        params: params.clone(),
        columns: vec![
            header("Date"),
            header("Voucher type"),
            header("Voucher no."),
            header("Amount"),
            header("Status"),
        ],
        rows,
        totals: vec![
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            amount_cell(&total),
            Cell::Empty,
        ],
        notes: Vec::new(),
    })
}

/// Per-month tally of a monthly register.
#[derive(Default)]
struct MonthTally {
    vouchers: u64,
    cancelled: u64,
    amount: Paise,
}

fn bump(count: u64) -> Result<u64, ReportError> {
    count.checked_add(1).ok_or_else(count_overflow)
}

fn count_cell(count: u64) -> Cell {
    Cell::Qty(Qty(Decimal::from(count)))
}

fn monthly_report(
    params: &ReportParams,
    title: String,
    entries: &[Entry<'_>],
) -> Result<Outcome<Report>, ReportError> {
    let fy = FinancialYear::containing(params.period.from())?;
    let months = expand_columns(params, fy)?;
    let mut tallies: Vec<MonthTally> = months.iter().map(|_| MonthTally::default()).collect();
    for entry in entries {
        let date = entry.voucher.date;
        let index = months
            .iter()
            .position(|m| m.period.is_some_and(|p| p.contains(date)));
        let tally = index.and_then(|i| tallies.get_mut(i)).ok_or_else(|| {
            invalid_voucher(entry.voucher, "its date falls in no month of the report")
        })?;
        match &entry.amount {
            Amount::Value(p) => tally.amount = tally.amount.checked_add(*p)?,
            Amount::Unsupported(u) => {
                return Ok(Outcome::Unsupported(u.clone()));
            }
        }
        match entry.voucher.status {
            VoucherStatus::Cancelled => tally.cancelled = bump(tally.cancelled)?,
            VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {
                tally.vouchers = bump(tally.vouchers)?;
            }
        }
    }
    let mut rows = Vec::with_capacity(months.len());
    let mut total = MonthTally::default();
    for (month, tally) in months.iter().zip(&tallies) {
        total.vouchers = total
            .vouchers
            .checked_add(tally.vouchers)
            .ok_or_else(count_overflow)?;
        total.cancelled = total
            .cancelled
            .checked_add(tally.cancelled)
            .ok_or_else(count_overflow)?;
        total.amount = total.amount.checked_add(tally.amount)?;
        rows.push(ReportRow {
            key: None,
            label: month.label.clone(),
            depth: 0,
            kind: RowKind::Group,
            cells: vec![
                count_cell(tally.vouchers),
                count_cell(tally.cancelled),
                Cell::Amount(SignedAmount::dr(tally.amount)),
            ],
        });
    }
    Ok(Outcome::Value(Report {
        title,
        params: params.clone(),
        columns: vec![header("Vouchers"), header("Cancelled"), header("Amount")],
        rows,
        totals: vec![
            count_cell(total.vouchers),
            count_cell(total.cancelled),
            Cell::Amount(SignedAmount::dr(total.amount)),
        ],
        notes: Vec::new(),
    }))
}

fn count_overflow() -> ReportError {
    ReportError::Invalid {
        field: "vouchers".to_string(),
        reason: "too many vouchers to count".to_string(),
    }
}
