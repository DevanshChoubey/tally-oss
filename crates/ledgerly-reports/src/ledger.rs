//! Group summaries and ledger vouchers. **Card RP5** implements this module.
//!
//! Frozen interface (plan card RP5):
//!
//! ```text
//! pub fn group_summary(books: &Books, params: &ReportParams, group: GroupId) -> Result<Outcome<Report>, ReportError>;
//! pub fn ledger_vouchers(books: &Books, params: &ReportParams, ledger: LedgerId) -> Result<Outcome<Report>, ReportError>;
//! ```
//!
//! # Ledger vouchers
//!
//! Columns: Date, Particulars, Voucher Type, Voucher No, Debit, Credit, Balance. Rows, in order:
//! an `Opening Balance` row (the ledger's opening balance plus every included posting dated before
//! `params.period.from()`), one voucher row per included posting dated inside the period, sorted by
//! `(date, voucher id)`, and a `Closing Balance` row. Each voucher row carries the running balance
//! after that posting, as a side-tagged amount: the side switches when the balance crosses zero
//! (a zero balance is shown as a debit of nil). Particulars name the other ledgers of the same
//! voucher. The voucher number is empty when the voucher record is not in `books.vouchers` or has
//! no number yet. `params.as_of` and `params.detail` play no part: the period decides the rows.
//!
//! # Group summary
//!
//! Columns: Debit, Credit. One row per direct sub-group (rolled up through its whole subtree) and
//! per ledger directly in the group, with the closing balance as of `params.as_of` (opening plus
//! included postings dated on or before it). Rows with a nil balance are left out; a final
//! `Subtotal` row shows the net balance of the group. The totals are the sums of the Debit and
//! Credit columns. `params.detail` plays no part: the summary is always one level down; drill
//! into a row for more.
//!
//! # Unsupported (never guessed)
//!
//! Comparative and monthly columns, filters, `exceptions_only`, a named scenario over memorandum
//! or reversing-journal postings, and the reserved Profit and Loss ledger (its balance folds in
//! prior-year profit at query time, spec 6.1, which no card computes yet).

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{
    Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    BaseVoucherKind, Books, Ledger, Posting, ReservedLedger, Scenario, VoucherStatus, VoucherType,
};

use crate::{
    Cell, ColumnHeader, ColumnSpec, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, ReportRow, RowKind,
};

/// Text of the particulars cell when the voucher touches no other ledger.
/// Report name used in `Unsupported` messages.
const LEDGER_NAME: &str = "ledger vouchers";

/// Report name used in `Unsupported` messages.
const GROUP_NAME: &str = "group summary";

const NO_OTHER_LEDGER: &str = "(no other ledger)";

/// One posting that counts under the scenario, with its debit-positive value.
struct Included<'a> {
    posting: &'a Posting,
    signed: Paise,
}

/// Result of selecting postings.
enum Collected<'a> {
    Postings(Vec<Included<'a>>),
    Unsupported,
}

/// What a posting contributes under the scenario.
enum Inclusion {
    Include,
    Exclude,
    Unsupported,
}

fn inclusion(posting: &Posting, voucher_type: &VoucherType, scenario: &Scenario) -> Inclusion {
    // Spec invariant 3: memorandum and reversing vouchers never touch base balances; only a named
    // scenario could include them, and no rule says how, so a named scenario is unsupported.
    match voucher_type.base {
        BaseVoucherKind::Memorandum | BaseVoucherKind::ReversingJournal => {
            return match scenario {
                Scenario::Actual => Inclusion::Exclude,
                Scenario::Including(statuses) if statuses.is_empty() => Inclusion::Exclude,
                Scenario::Including(_) => Inclusion::Unsupported,
            };
        }
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
    let included = match posting.status {
        // Post-dated vouchers enter the books on their date; the caller cuts at the end date.
        VoucherStatus::Regular | VoucherStatus::PostDated => true,
        // A cancelled voucher keeps its number but has no amounts.
        VoucherStatus::Cancelled => false,
        VoucherStatus::Optional => match scenario {
            Scenario::Actual => false,
            Scenario::Including(statuses) => statuses.contains(&VoucherStatus::Optional),
        },
    };
    if included {
        Inclusion::Include
    } else {
        Inclusion::Exclude
    }
}

/// Postings selected by `keep`, dated on or before `cut`, that count under `scenario`.
fn collect<'a>(
    books: &'a Books,
    scenario: &Scenario,
    cut: Date,
    keep: impl Fn(&Posting) -> bool,
) -> Result<Collected<'a>, ReportError> {
    let mut out = Vec::new();
    for posting in &books.postings {
        if posting.date > cut || !keep(posting) {
            continue;
        }
        let voucher_type = voucher_type_of(books, posting.type_id)?;
        match inclusion(posting, voucher_type, scenario) {
            Inclusion::Exclude => continue,
            Inclusion::Unsupported => return Ok(Collected::Unsupported),
            Inclusion::Include => {}
        }
        if posting.date < books.company.books_begin {
            return Err(ReportError::Invalid {
                field: "postings".to_string(),
                reason: format!(
                    "voucher {} is dated {} before the books begin on {}",
                    posting.voucher.0, posting.date, books.company.books_begin
                ),
            });
        }
        let signed = signed_amount(posting.amount, "postings")?;
        out.push(Included { posting, signed });
    }
    Ok(Collected::Postings(out))
}

fn voucher_type_of(books: &Books, id: VoucherTypeId) -> Result<&VoucherType, ReportError> {
    books
        .masters
        .voucher_types
        .get(&id)
        .ok_or_else(|| ReportError::NotFound {
            what: format!("voucher type {}", id.0),
        })
}

/// Debit-positive value of a side-tagged amount, rejecting negative stored amounts.
fn signed_amount(amount: SignedAmount, field: &str) -> Result<Paise, ReportError> {
    if amount.amount.is_negative() {
        return Err(ReportError::Invalid {
            field: field.to_string(),
            reason: "a stored amount is negative".to_string(),
        });
    }
    Ok(amount.to_signed()?)
}

fn wrong_kind(engine: &str) -> ReportError {
    ReportError::Invalid {
        field: "kind".to_string(),
        reason: format!("{engine} needs a report kind naming the same object"),
    }
}

/// Parameters this card does not compute.
fn unsupported_params(params: &ReportParams, report: &str) -> Option<Outcome<Report>> {
    match &params.columns {
        ColumnSpec::Single => {}
        ColumnSpec::Comparative(_) | ColumnSpec::Monthly => {
            return Some(Outcome::unsupported(
                format!("{report}: comparative or monthly columns"),
                "only the single-column layout is computed; run one report per period",
            ));
        }
    }
    let ReportFilter {
        ledgers,
        groups,
        voucher_types,
        items,
    } = &params.filter;
    if !(ledgers.is_empty() && groups.is_empty() && voucher_types.is_empty() && items.is_empty()) {
        return Some(Outcome::unsupported(
            format!("{report}: filters"),
            "a filtered report would not add up to the balance; clear the filter",
        ));
    }
    if params.exceptions_only {
        return Some(Outcome::unsupported(
            format!("{report}: exceptions only"),
            "no exception rules are defined for this report",
        ));
    }
    None
}

fn unsupported_scenario(report: &str) -> Outcome<Report> {
    Outcome::unsupported(
        format!("{report}: memorandum or reversing journal vouchers in a named scenario"),
        "no rule defines whether a named scenario includes them; use the actual scenario",
    )
}

fn unsupported_profit_and_loss(report: &str) -> Outcome<Report> {
    Outcome::unsupported(
        format!("{report}: the Profit and Loss ledger"),
        "its balance folds in prior-year profit at query time, which is not computed yet",
    )
}

/// `true` for the reserved Profit and Loss ledger.
fn is_profit_and_loss(ledger: &Ledger) -> bool {
    match ledger.reserved {
        Some(ReservedLedger::ProfitAndLoss) => true,
        Some(ReservedLedger::Cash) | None => false,
    }
}

fn status_name(status: VoucherStatus) -> &'static str {
    match status {
        VoucherStatus::Regular => "Regular",
        VoucherStatus::Optional => "Optional",
        VoucherStatus::PostDated => "PostDated",
        VoucherStatus::Cancelled => "Cancelled",
    }
}

fn scenario_note(scenario: &Scenario) -> String {
    match scenario {
        Scenario::Actual => "Scenario: Actual.".to_string(),
        Scenario::Including(statuses) => {
            let names: Vec<&str> = statuses.iter().map(|s| status_name(*s)).collect();
            format!("Scenario: Actual plus {}.", names.join(", "))
        }
    }
}

fn column(label: &str) -> ColumnHeader {
    ColumnHeader {
        label: label.to_string(),
        period: None,
    }
}

/// `[Debit, Credit]` cells for a debit-positive balance.
fn balance_cells(balance: Paise) -> Result<Vec<Cell>, ReportError> {
    if balance.is_zero() {
        Ok(vec![Cell::Empty, Cell::Empty])
    } else if balance.is_negative() {
        Ok(vec![
            Cell::Empty,
            Cell::Amount(SignedAmount::cr(balance.checked_neg()?)),
        ])
    } else {
        Ok(vec![Cell::Amount(SignedAmount::dr(balance)), Cell::Empty])
    }
}

// ---------------------------------------------------------------------------------------------
// Ledger vouchers
// ---------------------------------------------------------------------------------------------

/// Vouchers of one ledger over `params.period`, with opening, running and closing balance.
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not `LedgerVouchers(ledger)`, the period ends
/// before the books begin, a stored amount is negative, or an included posting precedes
/// `books_begin`; [`ReportError::NotFound`] when the ledger, a counter ledger or a voucher type
/// does not exist; [`ReportError::Money`] on overflow.
pub fn ledger_vouchers(
    books: &Books,
    params: &ReportParams,
    ledger: LedgerId,
) -> Result<Outcome<Report>, ReportError> {
    if params.kind != ReportKind::LedgerVouchers(ledger) {
        return Err(wrong_kind("the ledger vouchers engine"));
    }
    let master = books
        .masters
        .ledgers
        .get(&ledger)
        .ok_or_else(|| ReportError::NotFound {
            what: format!("ledger {}", ledger.0),
        })?;
    if let Some(unsupported) = unsupported_params(params, LEDGER_NAME) {
        return Ok(unsupported);
    }
    if is_profit_and_loss(master) {
        return Ok(unsupported_profit_and_loss(LEDGER_NAME));
    }
    if params.period.to() < books.company.books_begin {
        return Err(ReportError::Invalid {
            field: "period".to_string(),
            reason: format!(
                "the period ends {} before the books begin on {}",
                params.period.to(),
                books.company.books_begin
            ),
        });
    }
    let mut included = match collect(books, &params.scenario, params.period.to(), |p| {
        p.ledger == ledger
    })? {
        Collected::Postings(v) => v,
        Collected::Unsupported => return Ok(unsupported_scenario(LEDGER_NAME)),
    };
    // Stable sort: lines of one voucher keep their stored order.
    included.sort_by_key(|i| (i.posting.date, i.posting.voucher));
    build_ledger_report(books, params, master, &included).map(Outcome::Value)
}

fn build_ledger_report(
    books: &Books,
    params: &ReportParams,
    master: &Ledger,
    included: &[Included<'_>],
) -> Result<Report, ReportError> {
    let from = params.period.from();
    let mut balance = signed_amount(master.opening, "opening")?;
    let mut in_period = Vec::new();
    for item in included {
        if item.posting.date < from {
            balance = balance.checked_add(item.signed)?;
        } else {
            in_period.push(item);
        }
    }
    let counters = counter_ledgers(books, master.id, &in_period)?;
    let mut rows = vec![balance_row("Opening Balance", RowKind::Text, balance)?];
    let mut dr_total = Paise::ZERO;
    let mut cr_total = Paise::ZERO;
    for item in in_period {
        balance = balance.checked_add(item.signed)?;
        match item.posting.amount.side {
            DrCr::Dr => dr_total = dr_total.checked_add(item.posting.amount.amount)?,
            DrCr::Cr => cr_total = cr_total.checked_add(item.posting.amount.amount)?,
        }
        let particulars = counters
            .get(&item.posting.voucher)
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| NO_OTHER_LEDGER.to_string());
        rows.push(voucher_row(books, item.posting, particulars, balance)?);
    }
    rows.push(balance_row("Closing Balance", RowKind::Subtotal, balance)?);
    Ok(Report {
        title: format!("Ledger Vouchers: {}", master.name),
        params: params.clone(),
        columns: [
            "Date",
            "Particulars",
            "Voucher Type",
            "Voucher No",
            "Debit",
            "Credit",
            "Balance",
        ]
        .into_iter()
        .map(column)
        .collect(),
        rows,
        totals: vec![
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            Cell::Amount(SignedAmount::dr(dr_total)),
            Cell::Amount(SignedAmount::cr(cr_total)),
            Cell::Amount(SignedAmount::from_signed(balance)?),
        ],
        notes: ledger_notes(books, params),
    })
}

fn ledger_notes(books: &Books, params: &ReportParams) -> Vec<String> {
    let mut notes = vec![scenario_note(&params.scenario)];
    if params.period.from() < books.company.books_begin {
        notes.push(format!(
            "The period starts before the books begin on {}; the opening balance is as at that date.",
            books.company.books_begin
        ));
    }
    notes
}

/// One voucher row with the running balance after the posting.
fn voucher_row(
    books: &Books,
    posting: &Posting,
    particulars: String,
    balance: Paise,
) -> Result<ReportRow, ReportError> {
    let (debit, credit) = match posting.amount.side {
        DrCr::Dr => (Cell::Amount(posting.amount), Cell::Empty),
        DrCr::Cr => (Cell::Empty, Cell::Amount(posting.amount)),
    };
    let type_name = voucher_type_of(books, posting.type_id)?.name.clone();
    let number = books
        .vouchers
        .get(&posting.voucher)
        .and_then(|v| v.number.clone())
        .map_or(Cell::Empty, Cell::Text);
    Ok(ReportRow {
        key: Some(DrillKey::Voucher(posting.voucher)),
        label: particulars.clone(),
        depth: 0,
        kind: RowKind::Voucher,
        cells: vec![
            Cell::Date(posting.date),
            Cell::Text(particulars),
            Cell::Text(type_name),
            number,
            debit,
            credit,
            Cell::Amount(SignedAmount::from_signed(balance)?),
        ],
    })
}

/// A row that shows only a balance in the last column.
fn balance_row(label: &str, kind: RowKind, balance: Paise) -> Result<ReportRow, ReportError> {
    Ok(ReportRow {
        key: None,
        label: label.to_string(),
        depth: 0,
        kind,
        cells: vec![
            Cell::Empty,
            Cell::Text(label.to_string()),
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            Cell::Amount(SignedAmount::from_signed(balance)?),
        ],
    })
}

/// For each voucher shown, the names of the other ledgers it posts to, joined with `, `.
fn counter_ledgers(
    books: &Books,
    this: LedgerId,
    shown: &[&Included<'_>],
) -> Result<BTreeMap<VoucherId, String>, ReportError> {
    let wanted: BTreeSet<VoucherId> = shown.iter().map(|i| i.posting.voucher).collect();
    let mut seen: BTreeMap<VoucherId, Vec<LedgerId>> = BTreeMap::new();
    for posting in &books.postings {
        if posting.ledger == this || !wanted.contains(&posting.voucher) {
            continue;
        }
        let ids = seen.entry(posting.voucher).or_default();
        if !ids.contains(&posting.ledger) {
            ids.push(posting.ledger);
        }
    }
    let mut out = BTreeMap::new();
    for (voucher, ids) in seen {
        let mut names = Vec::with_capacity(ids.len());
        for id in ids {
            let ledger = books
                .masters
                .ledgers
                .get(&id)
                .ok_or_else(|| ReportError::NotFound {
                    what: format!("ledger {} named by voucher {}", id.0, voucher.0),
                })?;
            names.push(ledger.name.as_str());
        }
        out.insert(voucher, names.join(", "));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Group summary
// ---------------------------------------------------------------------------------------------

/// Summary of one group as of `params.as_of`: its sub-groups and ledgers one level down.
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not `GroupSummary(group)`, `as_of` precedes the
/// books, a stored amount is negative, or an included posting precedes `books_begin`;
/// [`ReportError::NotFound`] when the group or a posting's voucher type does not exist;
/// [`ReportError::Money`] on overflow.
pub fn group_summary(
    books: &Books,
    params: &ReportParams,
    group: GroupId,
) -> Result<Outcome<Report>, ReportError> {
    if params.kind != ReportKind::GroupSummary(group) {
        return Err(wrong_kind("the group summary engine"));
    }
    let master = books
        .masters
        .groups
        .get(&group)
        .ok_or_else(|| ReportError::NotFound {
            what: format!("group {}", group.0),
        })?;
    if let Some(unsupported) = unsupported_params(params, GROUP_NAME) {
        return Ok(unsupported);
    }
    if params.as_of < books.company.books_begin {
        return Err(ReportError::Invalid {
            field: "as_of".to_string(),
            reason: format!(
                "{} is before the books begin on {}",
                params.as_of, books.company.books_begin
            ),
        });
    }
    let subtree = descendants(books, group);
    let members: Vec<&Ledger> = books
        .masters
        .ledgers
        .values()
        .filter(|l| subtree.contains(&l.group))
        .collect();
    if members.iter().any(|l| is_profit_and_loss(l)) {
        return Ok(unsupported_profit_and_loss(GROUP_NAME));
    }
    let member_ids: BTreeSet<LedgerId> = members.iter().map(|l| l.id).collect();
    let included = match collect(books, &params.scenario, params.as_of, |p| {
        member_ids.contains(&p.ledger)
    })? {
        Collected::Postings(v) => v,
        Collected::Unsupported => return Ok(unsupported_scenario(GROUP_NAME)),
    };
    let mut balances: BTreeMap<LedgerId, Paise> = BTreeMap::new();
    for l in &members {
        balances.insert(l.id, signed_amount(l.opening, "opening")?);
    }
    for item in &included {
        if let Some(b) = balances.get_mut(&item.posting.ledger) {
            *b = b.checked_add(item.signed)?;
        }
    }
    build_group_report(
        books,
        params,
        master.name.as_str(),
        group,
        &members,
        &balances,
    )
    .map(Outcome::Value)
}

/// The group and every group below it. Cycles in a corrupt tree are visited once.
fn descendants(books: &Books, root: GroupId) -> BTreeSet<GroupId> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        stack.extend(
            books
                .masters
                .groups
                .values()
                .filter(|g| g.parent == Some(id))
                .map(|g| g.id),
        );
    }
    seen
}

fn build_group_report(
    books: &Books,
    params: &ReportParams,
    group_name: &str,
    group: GroupId,
    members: &[&Ledger],
    balances: &BTreeMap<LedgerId, Paise>,
) -> Result<Report, ReportError> {
    let balance_of = |id: LedgerId| balances.get(&id).copied().unwrap_or(Paise::ZERO);
    let mut shown: Vec<(ReportRow, Paise)> = Vec::new();
    let mut net = Paise::ZERO;

    for child in books
        .masters
        .groups
        .values()
        .filter(|g| g.parent == Some(group))
    {
        let sub = descendants(books, child.id);
        let mut child_net = Paise::ZERO;
        let mut any = false;
        for l in members.iter().filter(|l| sub.contains(&l.group)) {
            let b = balance_of(l.id);
            child_net = child_net.checked_add(b)?;
            any = any || !b.is_zero();
        }
        net = net.checked_add(child_net)?;
        if any {
            let row = summary_row(
                DrillKey::Group(child.id),
                &child.name,
                RowKind::Group,
                child_net,
            )?;
            shown.push((row, child_net));
        }
    }
    for l in members.iter().filter(|l| l.group == group) {
        let b = balance_of(l.id);
        net = net.checked_add(b)?;
        if !b.is_zero() {
            shown.push((
                summary_row(DrillKey::Ledger(l.id), &l.name, RowKind::Ledger, b)?,
                b,
            ));
        }
    }
    let mut dr_total = Paise::ZERO;
    let mut cr_total = Paise::ZERO;
    let mut rows = Vec::with_capacity(shown.len().saturating_add(1));
    for (row, b) in shown {
        if b.is_negative() {
            cr_total = cr_total.checked_add(b.checked_neg()?)?;
        } else {
            dr_total = dr_total.checked_add(b)?;
        }
        rows.push(row);
    }
    rows.push(ReportRow {
        key: None,
        label: format!("Net balance of {group_name}"),
        depth: 0,
        kind: RowKind::Subtotal,
        cells: balance_cells(net)?,
    });
    Ok(Report {
        title: format!("Group Summary: {group_name}"),
        params: params.clone(),
        columns: vec![column("Debit"), column("Credit")],
        rows,
        totals: vec![
            Cell::Amount(SignedAmount::dr(dr_total)),
            Cell::Amount(SignedAmount::cr(cr_total)),
        ],
        notes: vec![
            scenario_note(&params.scenario),
            format!("Closing balances as of {}.", params.as_of),
        ],
    })
}

fn summary_row(
    key: DrillKey,
    label: &str,
    kind: RowKind,
    balance: Paise,
) -> Result<ReportRow, ReportError> {
    Ok(ReportRow {
        key: Some(key),
        label: label.to_string(),
        depth: 0,
        kind,
        cells: balance_cells(balance)?,
    })
}
