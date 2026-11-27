//! Bills outstanding and ageing. **Card RP7** implements this module.
//!
//! Frozen interface (plan card RP7):
//!
//! ```text
//! pub fn outstanding(books: &Books, params: &ReportParams, receivable: bool) -> Result<Outcome<Report>, ReportError>;
//! ```
//!
//! # What the report shows (spec 6.2, "Outstandings")
//!
//! One block per party ledger: a ledger that sits under the reserved group Sundry Debtors
//! (receivable) or Sundry Creditors (payable) and has bill-wise tracking on. A ledger row carries
//! the totals; in [`Detail::Detailed`] it is followed by one row per open bill and, when the party
//! has any, one separate `On account` row. Parties with nothing open are not listed.
//!
//! Open bills come from [`ledgerly_engine::bills::open_bills`] (card E7), so this report and the
//! voucher-entry checks agree on what is open. Amounts keep their own side (Dr/Cr): an unsettled
//! credit note on a debtor shows as a credit, it is never flipped.
//!
//! Columns: `Bill date`, `Due date`, `Pending`, `Not yet due`, `0-30 days`, `31-60 days`,
//! `61-90 days`, `Over 90 days`, `No due date`, `On account`, `Net outstanding`.
//!
//! # Ageing
//!
//! Ageing is by **due date**, as of `params.as_of`. Overdue days = `as_of - due` in whole days.
//! A bill due on `as_of` is 0 days overdue. Buckets: 0 to 30, 31 to 60, 61 to 90 and over 90 days
//! overdue; a bill whose due date is after `as_of` is `Not yet due`. A bill without a due date
//! (no due date and no credit days on its opening allocation) is not aged from some other date, it
//! goes to `No due date`. Bucket edges are the card's definition; they are not observed from any
//! accounting package.
//!
//! # On account
//!
//! `OnAccount` allocations are not linked to a bill, so they are never aged. They are summed per
//! party (counting the same vouchers as the bill projection: regular and post-dated vouchers dated
//! on or before `as_of`, never memorandum or reversing journals) and shown in their own row and
//! column. `Net outstanding` is `Pending` plus `On account`.
//!
//! # Unsupported (never guessed)
//!
//! - comparative or monthly columns, `exceptions_only`, filters by voucher type or item;
//! - a named scenario that includes optional or other statuses (open bills are only defined over
//!   regular and post-dated vouchers);
//! - a bill-wise party ledger with a non-zero opening balance: the model has no opening bill
//!   breakup, so the list would omit it (same limit as card E7).

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{Date, GroupId, LedgerId, MoneyError, Outcome, Paise, SignedAmount};
use ledgerly_engine::EngineError;
use ledgerly_engine::bills::{OpenBill, open_bills};
use ledgerly_model::{
    BaseVoucherKind, BillRefKind, Books, Group, Ledger, ReservedGroup, Scenario, Voucher,
    VoucherStatus,
};

use crate::{
    Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};

/// Label of the separate on-account row.
const ON_ACCOUNT_LABEL: &str = "On account";

/// Deepest group nesting followed when looking for a reserved ancestor.
const MAX_GROUP_DEPTH: usize = 256;

/// Column labels, in cell order.
const COLUMN_LABELS: [&str; 11] = [
    "Bill date",
    "Due date",
    "Pending",
    "Not yet due",
    "0-30 days",
    "31-60 days",
    "61-90 days",
    "Over 90 days",
    "No due date",
    "On account",
    "Net outstanding",
];

/// Builds bills receivable (`receivable == true`) or payable as of `params.as_of` (spec 6.2).
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not `Outstanding { receivable }` with the same
/// flag, `as_of` precedes the company's `books_begin`, a group chain is cyclic, or the bill
/// projection rejects the books (for example an over-settled bill);
/// [`ReportError::NotFound`] when a group or voucher type referenced does not exist;
/// [`ReportError::Money`] on overflow.
pub fn outstanding(
    books: &Books,
    params: &ReportParams,
    receivable: bool,
) -> Result<Outcome<Report>, ReportError> {
    if params.kind != (ReportKind::Outstanding { receivable }) {
        return Err(ReportError::Invalid {
            field: "kind".to_owned(),
            reason: "the outstanding engine only runs the matching bills outstanding report"
                .to_owned(),
        });
    }
    if let Some(unsupported) = unsupported_params(params) {
        return Ok(unsupported);
    }
    if params.as_of < books.company.books_begin {
        return Err(ReportError::Invalid {
            field: "as_of".to_owned(),
            reason: format!(
                "{} is before the books begin on {}",
                params.as_of, books.company.books_begin
            ),
        });
    }
    let reserved = if receivable {
        ReservedGroup::SundryDebtors
    } else {
        ReservedGroup::SundryCreditors
    };
    let parties = select_parties(books, &params.filter, reserved)?;
    if let Some(ledger) = parties
        .bill_wise
        .iter()
        .find(|l| !l.opening.amount.is_zero())
    {
        return Ok(Outcome::unsupported(
            "outstanding: opening balance on a bill-wise ledger",
            format!(
                "ledger '{}' has an opening balance but the model has no opening bill breakup, \
                 so its bills would be incomplete; bill-by-bill opening balances are not \
                 supported yet",
                ledger.name
            ),
        ));
    }
    let wanted: BTreeSet<LedgerId> = parties.bill_wise.iter().map(|l| l.id).collect();
    let on_account = on_account_by_ledger(books, params.as_of, &wanted)?;
    let mut blocks = Vec::new();
    for &ledger in &parties.bill_wise {
        let bills = open_bills(books, ledger.id, params.as_of).map_err(|e| map_engine(&e))?;
        let account = on_account.get(&ledger.id).copied().unwrap_or(Paise::ZERO);
        if bills.is_empty() && account.is_zero() {
            continue;
        }
        blocks.push(Block {
            ledger,
            bills,
            on_account: account,
        });
    }
    build_report(params, receivable, &blocks, &parties.not_bill_wise).map(Outcome::Value)
}

/// Parameters this card does not compute.
fn unsupported_params(params: &ReportParams) -> Option<Outcome<Report>> {
    match &params.columns {
        ColumnSpec::Single => {}
        ColumnSpec::Comparative(_) | ColumnSpec::Monthly => {
            return Some(Outcome::unsupported(
                "outstanding: comparative or monthly columns",
                "the ageing columns are the comparison; run one report per as-of date",
            ));
        }
    }
    match &params.scenario {
        Scenario::Actual => {}
        Scenario::Including(statuses) => {
            if !statuses.is_empty() {
                return Some(Outcome::unsupported(
                    "outstanding: scenario with optional or other vouchers",
                    "open bills are defined over regular and post-dated vouchers only",
                ));
            }
        }
    }
    let ReportFilter {
        ledgers: _,
        groups: _,
        voucher_types,
        items,
    } = &params.filter;
    if !(voucher_types.is_empty() && items.is_empty()) {
        return Some(Outcome::unsupported(
            "outstanding: filter by voucher type or item",
            "only ledger and group filters apply to bills",
        ));
    }
    if params.exceptions_only {
        return Some(Outcome::unsupported(
            "outstanding: exceptions only",
            "no exception rules are defined for bills outstanding",
        ));
    }
    None
}

/// Party ledgers of one side, split by whether bills are tracked.
struct Parties<'a> {
    /// Bill-wise ledgers in display order (name, then id).
    bill_wise: Vec<&'a Ledger>,
    /// Names of party ledgers without bill-wise tracking, sorted.
    not_bill_wise: Vec<String>,
}

fn select_parties<'a>(
    books: &'a Books,
    filter: &ReportFilter,
    reserved: ReservedGroup,
) -> Result<Parties<'a>, ReportError> {
    let mut bill_wise = Vec::new();
    let mut not_bill_wise = Vec::new();
    for ledger in books.masters.ledgers.values() {
        if !filter.ledgers.is_empty() && !filter.ledgers.contains(&ledger.id) {
            continue;
        }
        // A ledger under Primary (no group) is not under the debtors or creditors group.
        let Some(group) = ledger.group else {
            continue;
        };
        let chain = group_chain(books, group)?;
        if !chain.iter().any(|g| g.reserved == Some(reserved)) {
            continue;
        }
        if !filter.groups.is_empty() && !chain.iter().any(|g| filter.groups.contains(&g.id)) {
            continue;
        }
        if ledger.bill_wise.is_some() {
            bill_wise.push(ledger);
        } else {
            not_bill_wise.push(ledger.name.clone());
        }
    }
    bill_wise.sort_by(|a, b| (&a.name, a.id).cmp(&(&b.name, b.id)));
    not_bill_wise.sort();
    Ok(Parties {
        bill_wise,
        not_bill_wise,
    })
}

/// `start` and all its ancestors, nearest first.
fn group_chain(books: &Books, start: GroupId) -> Result<Vec<&Group>, ReportError> {
    let mut chain = Vec::new();
    let mut next = Some(start);
    while let Some(id) = next {
        if chain.len() >= MAX_GROUP_DEPTH {
            return Err(ReportError::Invalid {
                field: "groups".to_owned(),
                reason: format!(
                    "group {} is nested too deeply or loops back on itself",
                    id.0
                ),
            });
        }
        let group = books
            .masters
            .groups
            .get(&id)
            .ok_or_else(|| ReportError::NotFound {
                what: format!("group {}", id.0),
            })?;
        chain.push(group);
        next = group.parent;
    }
    Ok(chain)
}

fn map_engine(error: &EngineError) -> ReportError {
    if let EngineError::NotFound { what } = error {
        return ReportError::NotFound { what: what.clone() };
    }
    if let EngineError::Money(money) = error {
        return ReportError::Money(money.clone());
    }
    ReportError::Invalid {
        field: "bills".to_owned(),
        reason: error.to_string(),
    }
}

/// Whether a voucher kind posts to ledgers (mirrors the bill projection of card E7).
enum KindEffect {
    Posts,
    Excluded,
    NoBills,
}

fn kind_effect(kind: BaseVoucherKind) -> KindEffect {
    match kind {
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Receipt
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote => KindEffect::Posts,
        BaseVoucherKind::Memorandum | BaseVoucherKind::ReversingJournal => KindEffect::Excluded,
        BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => KindEffect::NoBills,
    }
}

/// Net on-account amount per ledger (Dr positive), over the vouchers the bill projection counts.
fn on_account_by_ledger(
    books: &Books,
    as_of: Date,
    wanted: &BTreeSet<LedgerId>,
) -> Result<BTreeMap<LedgerId, Paise>, ReportError> {
    let mut totals: BTreeMap<LedgerId, Paise> = BTreeMap::new();
    for voucher in books.vouchers.values() {
        let in_time = match voucher.status {
            VoucherStatus::Regular | VoucherStatus::PostDated => voucher.date <= as_of,
            VoucherStatus::Optional | VoucherStatus::Cancelled => false,
        };
        if !in_time {
            continue;
        }
        let voucher_type = books
            .masters
            .voucher_types
            .get(&voucher.type_id)
            .ok_or_else(|| ReportError::NotFound {
                what: format!("voucher type {}", voucher.type_id.0),
            })?;
        match kind_effect(voucher_type.base) {
            KindEffect::Posts => {}
            KindEffect::Excluded => continue,
            KindEffect::NoBills => {
                if voucher.ledger_lines.iter().any(|l| !l.bills.is_empty()) {
                    return Err(ReportError::Invalid {
                        field: format!("voucher type {}", voucher_type.name),
                        reason: "this voucher type does not post to ledgers, so bill allocations \
                                 cannot be applied; remove them"
                            .to_owned(),
                    });
                }
                continue;
            }
        }
        add_on_account(&mut totals, voucher, wanted)?;
    }
    Ok(totals)
}

/// One party with something outstanding.
struct Block<'a> {
    ledger: &'a Ledger,
    bills: Vec<OpenBill>,
    /// Net on-account amount, Dr positive.
    on_account: Paise,
}

/// Ageing bucket of a bill.
#[derive(Clone, Copy)]
enum Bucket {
    NotDue,
    Days0To30,
    Days31To60,
    Days61To90,
    Over90,
    NoDue,
}

/// Whole days a bill is overdue as of `as_of`: `as_of - due`. A bill due on `as_of` is 0 days
/// overdue. `None` when the bill is not yet due (its due date is after `as_of`) or has no due
/// date, so it is not aged. This is the count the ageing buckets are cut from.
#[must_use]
pub fn days_overdue(as_of: Date, due: Option<Date>) -> Option<i64> {
    let overdue = (as_of - due?).whole_days();
    (overdue >= 0).then_some(overdue)
}

fn bucket_of(as_of: Date, due: Option<Date>) -> Bucket {
    let Some(due) = due else {
        return Bucket::NoDue;
    };
    let Some(overdue) = days_overdue(as_of, Some(due)) else {
        return Bucket::NotDue;
    };
    if overdue <= 30 {
        Bucket::Days0To30
    } else if overdue <= 60 {
        Bucket::Days31To60
    } else if overdue <= 90 {
        Bucket::Days61To90
    } else {
        Bucket::Over90
    }
}

/// Signed (Dr positive) amounts per ageing bucket.
#[derive(Clone, Copy, Default)]
struct Buckets {
    not_due: Paise,
    d0_30: Paise,
    d31_60: Paise,
    d61_90: Paise,
    over_90: Paise,
    no_due: Paise,
}

impl Buckets {
    fn add(&mut self, bucket: Bucket, amount: Paise) -> Result<(), MoneyError> {
        let slot = match bucket {
            Bucket::NotDue => &mut self.not_due,
            Bucket::Days0To30 => &mut self.d0_30,
            Bucket::Days31To60 => &mut self.d31_60,
            Bucket::Days61To90 => &mut self.d61_90,
            Bucket::Over90 => &mut self.over_90,
            Bucket::NoDue => &mut self.no_due,
        };
        *slot = slot.checked_add(amount)?;
        Ok(())
    }

    fn merge(&mut self, other: &Buckets) -> Result<(), MoneyError> {
        self.not_due = self.not_due.checked_add(other.not_due)?;
        self.d0_30 = self.d0_30.checked_add(other.d0_30)?;
        self.d31_60 = self.d31_60.checked_add(other.d31_60)?;
        self.d61_90 = self.d61_90.checked_add(other.d61_90)?;
        self.over_90 = self.over_90.checked_add(other.over_90)?;
        self.no_due = self.no_due.checked_add(other.no_due)?;
        Ok(())
    }

    fn cells(&self) -> Result<Vec<Cell>, MoneyError> {
        [
            self.not_due,
            self.d0_30,
            self.d31_60,
            self.d61_90,
            self.over_90,
            self.no_due,
        ]
        .into_iter()
        .map(amount_cell)
        .collect()
    }
}

/// An amount cell with its side; zero is shown empty.
fn amount_cell(amount: Paise) -> Result<Cell, MoneyError> {
    if amount.is_zero() {
        Ok(Cell::Empty)
    } else {
        SignedAmount::from_signed(amount).map(Cell::Amount)
    }
}

/// "7 open bills", "1 open bill": the count the total line carries.
fn bills_label(count: usize) -> String {
    if count == 1 {
        "1 open bill".to_owned()
    } else {
        format!("{count} open bills")
    }
}

/// Running totals of the whole report.
#[derive(Default)]
struct Totals {
    /// How many open bills the report lists, on every party.
    bills: usize,
    pending: Paise,
    buckets: Buckets,
    on_account: Paise,
}

impl Totals {
    fn cells(&self) -> Result<Vec<Cell>, MoneyError> {
        summary_cells(
            &if self.bills == 0 {
                Cell::Empty
            } else {
                Cell::Text(bills_label(self.bills))
            },
            &Cell::Empty,
            self.pending,
            &self.buckets,
            self.on_account,
        )
    }
}

/// `[bill date, due date, pending, six buckets, on account, net]` for a ledger or total line.
fn summary_cells(
    date: &Cell,
    due: &Cell,
    pending: Paise,
    buckets: &Buckets,
    on_account: Paise,
) -> Result<Vec<Cell>, MoneyError> {
    let mut cells = vec![date.clone(), due.clone(), amount_cell(pending)?];
    cells.extend(buckets.cells()?);
    cells.push(amount_cell(on_account)?);
    cells.push(amount_cell(pending.checked_add(on_account)?)?);
    Ok(cells)
}

fn build_report(
    params: &ReportParams,
    receivable: bool,
    blocks: &[Block<'_>],
    not_bill_wise: &[String],
) -> Result<Report, ReportError> {
    let mut rows = Vec::new();
    let mut totals = Totals::default();
    let mut any_without_due = false;
    for block in blocks {
        let mut pending = Paise::ZERO;
        let mut buckets = Buckets::default();
        let mut bill_rows = Vec::new();
        for bill in &block.bills {
            let signed = bill.open.to_signed()?;
            let bucket = bucket_of(params.as_of, bill.due);
            any_without_due |= matches!(bucket, Bucket::NoDue);
            pending = pending.checked_add(signed)?;
            buckets.add(bucket, signed)?;
            bill_rows.push(bill_row(block.ledger.id, bill, bucket, signed)?);
        }
        rows.push(ReportRow {
            key: Some(DrillKey::Ledger(block.ledger.id)),
            label: block.ledger.name.clone(),
            depth: 0,
            kind: RowKind::Ledger,
            cells: summary_cells(
                &Cell::Empty,
                &Cell::Empty,
                pending,
                &buckets,
                block.on_account,
            )?,
        });
        if params.detail == Detail::Detailed {
            rows.append(&mut bill_rows);
            if !block.on_account.is_zero() {
                rows.push(on_account_row(block)?);
            }
        }
        totals.bills = totals.bills.saturating_add(block.bills.len());
        totals.pending = totals.pending.checked_add(pending)?;
        totals.buckets.merge(&buckets)?;
        totals.on_account = totals.on_account.checked_add(block.on_account)?;
    }
    Ok(Report {
        title: if receivable {
            "Bills receivable".to_owned()
        } else {
            "Bills payable".to_owned()
        },
        params: params.clone(),
        columns: COLUMN_LABELS
            .iter()
            .map(|label| ColumnHeader {
                label: (*label).to_owned(),
                period: None,
            })
            .collect(),
        rows,
        totals: totals.cells()?,
        notes: notes(params.as_of, any_without_due, not_bill_wise),
    })
}

/// One bill: its dates, its open amount in its bucket, and no on-account or net figure.
fn bill_row(
    ledger: LedgerId,
    bill: &OpenBill,
    bucket: Bucket,
    signed: Paise,
) -> Result<ReportRow, ReportError> {
    let mut single = Buckets::default();
    single.add(bucket, signed)?;
    let due_cell = bill.due.map_or(Cell::Empty, Cell::Date);
    let mut cells = summary_cells(
        &Cell::Date(bill.date),
        &due_cell,
        signed,
        &single,
        Paise::ZERO,
    )?;
    // The last cell is "Net outstanding", which only ledger and total lines carry.
    cells.pop();
    cells.push(Cell::Empty);
    Ok(ReportRow {
        key: Some(DrillKey::Bill {
            ledger,
            name: bill.name.clone(),
        }),
        label: bill.name.clone(),
        depth: 1,
        kind: RowKind::Bill,
        cells,
    })
}

/// The separate on-account row: only the "On account" cell is filled.
fn on_account_row(block: &Block<'_>) -> Result<ReportRow, ReportError> {
    // Cells before "On account": bill date, due date, pending and the six buckets.
    let mut cells = vec![Cell::Empty; 9];
    cells.push(amount_cell(block.on_account)?);
    cells.push(Cell::Empty);
    Ok(ReportRow {
        key: Some(DrillKey::Ledger(block.ledger.id)),
        label: ON_ACCOUNT_LABEL.to_owned(),
        depth: 1,
        kind: RowKind::Bill,
        cells,
    })
}

fn notes(as_of: Date, any_without_due: bool, not_bill_wise: &[String]) -> Vec<String> {
    let mut notes = vec![format!(
        "Ageing counts whole days past the due date as of {as_of}; a bill due on that date is 0 days overdue."
    )];
    if any_without_due {
        notes.push(
            "Bills with no due date and no credit days are not aged; they are in No due date."
                .to_owned(),
        );
    }
    if !not_bill_wise.is_empty() {
        notes.push(format!(
            "Not listed because bill-wise tracking is off: {}.",
            not_bill_wise.join(", ")
        ));
    }
    notes
}

/// Adds the on-account allocations of `voucher` on the wanted ledgers to `totals`.
fn add_on_account(
    totals: &mut BTreeMap<LedgerId, Paise>,
    voucher: &Voucher,
    wanted: &BTreeSet<LedgerId>,
) -> Result<(), ReportError> {
    for line in voucher
        .ledger_lines
        .iter()
        .filter(|l| wanted.contains(&l.ledger))
    {
        for alloc in line
            .bills
            .iter()
            .filter(|a| a.kind == BillRefKind::OnAccount)
        {
            if alloc.amount.amount.is_negative() {
                return Err(ReportError::Invalid {
                    field: "bill allocation".to_owned(),
                    reason: "an on-account amount is negative; give a positive amount with a \
                             Dr/Cr side"
                        .to_owned(),
                });
            }
            let slot = totals.entry(line.ledger).or_insert(Paise::ZERO);
            *slot = slot.checked_add(alloc.amount.to_signed()?)?;
        }
    }
    Ok(())
}
