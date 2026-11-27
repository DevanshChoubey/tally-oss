//! Trial balance. **Card RP2** implements this module.
//!
//! Frozen interface (plan card RP2):
//!
//! ```text
//! pub fn trial_balance(books: &Books, params: &ReportParams) -> Result<Outcome<Report>, ReportError>;
//! ```
//!
//! The report shows the closing balance of every ledger as of `params.as_of`, rolled up through the
//! group tree, in a Debit and a Credit column. A ledger's closing balance is its opening balance
//! plus the included postings dated on or before `as_of`. Opening balances may be unbalanced
//! (spec invariant 4); the difference is an explicit [`RowKind::Difference`] row on the short side
//! so both columns total the same. Postings that do not balance among themselves are corrupt input
//! and are a [`ReportError::Invalid`], never absorbed into that row.
//!
//! TallyPrime keeps the predefined Profit & Loss A/c under Primary, not under a group, and carries
//! the previous year's profit or loss as its opening balance
//! (<https://help.tallysolutions.com/tally-prime/accounting/ledgers-and-groups-in-tallyprime/>, read
//! 2026-11-27). Its balance is a depth-0 ledger row after the primary groups, shown when it is not
//! zero, and it counts in both totals. Any other ledger without a group is corrupt input.
//!
//! Unsupported (never guessed): comparative and monthly columns, filters, `exceptions_only`, and a
//! named scenario over memorandum or reversing-journal postings.

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, VoucherTypeId};
use ledgerly_model::{
    BaseVoucherKind, Books, Group, Ledger, Posting, ReservedLedger, Scenario, VoucherStatus,
    VoucherType,
};

use crate::{
    Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};

/// Label of the explicit opening-difference row.
const DIFFERENCE_LABEL: &str = "Difference in opening balances";

/// Deepest group nesting the report follows. `ReportRow::depth` is a `u8`.
const MAX_DEPTH: u8 = 200;

/// Builds the trial balance as of `params.as_of` (spec 6.2).
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not a trial balance, `as_of` precedes the
/// company's `books_begin`, a stored amount is negative, a posting precedes `books_begin`, the
/// included postings do not balance, or a ledger cannot be reached from a primary group;
/// [`ReportError::NotFound`] when a posting names a ledger or voucher type that does not exist;
/// [`ReportError::Money`] on overflow.
pub fn trial_balance(books: &Books, params: &ReportParams) -> Result<Outcome<Report>, ReportError> {
    if params.kind != ReportKind::TrialBalance {
        return Err(ReportError::Invalid {
            field: "kind".to_string(),
            reason: "the trial balance engine only runs the trial balance".to_string(),
        });
    }
    if let Some(unsupported) = unsupported_params(params) {
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
    let movement = match movement_by_ledger(books, params)? {
        Movement::Totals(m) => m,
        Movement::Unsupported(what, reason) => return Ok(Outcome::unsupported(what, reason)),
    };
    let (balances, opening_net) = closing_balances(books, &movement)?;
    build_report(books, params, &balances, opening_net).map(Outcome::Value)
}

/// Parameters this card does not compute.
fn unsupported_params(params: &ReportParams) -> Option<Outcome<Report>> {
    match &params.columns {
        ColumnSpec::Single => {}
        ColumnSpec::Comparative(_) | ColumnSpec::Monthly => {
            return Some(Outcome::unsupported(
                "trial balance: comparative or monthly columns",
                "only the single closing-balance column pair is computed; run one report per period",
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
            "trial balance: filters",
            "a filtered trial balance would not balance; clear the filter",
        ));
    }
    if params.exceptions_only {
        return Some(Outcome::unsupported(
            "trial balance: exceptions only",
            "no exception rules are defined for the trial balance",
        ));
    }
    None
}

/// Net movement (debit positive) per ledger, or the reason it cannot be computed.
enum Movement {
    Totals(BTreeMap<LedgerId, Paise>),
    Unsupported(&'static str, &'static str),
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
        // Post-dated vouchers enter the books on their date; the caller has already cut at as_of.
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

fn movement_by_ledger(books: &Books, params: &ReportParams) -> Result<Movement, ReportError> {
    let mut by_ledger: BTreeMap<LedgerId, Paise> = BTreeMap::new();
    let mut net = Paise::ZERO;
    for posting in &books.postings {
        if posting.date > params.as_of {
            continue;
        }
        let voucher_type = voucher_type_of(books, posting.type_id)?;
        match inclusion(posting, voucher_type, &params.scenario) {
            Inclusion::Exclude => continue,
            Inclusion::Unsupported => {
                return Ok(Movement::Unsupported(
                    "trial balance: Memorandum or reversing journal vouchers in a named scenario",
                    "no rule defines whether a named scenario includes them; use the actual scenario",
                ));
            }
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
        if !books.masters.ledgers.contains_key(&posting.ledger) {
            return Err(ReportError::NotFound {
                what: format!(
                    "ledger {} named by voucher {}",
                    posting.ledger.0, posting.voucher.0
                ),
            });
        }
        let signed = signed_amount(posting.amount, "postings")?;
        let entry = by_ledger.entry(posting.ledger).or_insert(Paise::ZERO);
        *entry = entry.checked_add(signed)?;
        net = net.checked_add(signed)?;
    }
    if !net.is_zero() {
        return Err(ReportError::Invalid {
            field: "postings".to_string(),
            reason: format!(
                "debits and credits of the included postings differ by {} paise",
                net.get()
            ),
        });
    }
    Ok(Movement::Totals(by_ledger))
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

/// Closing balance (debit positive) of every ledger, and the net of all opening balances.
fn closing_balances(
    books: &Books,
    movement: &BTreeMap<LedgerId, Paise>,
) -> Result<(BTreeMap<LedgerId, Paise>, Paise), ReportError> {
    let mut balances = BTreeMap::new();
    let mut opening_net = Paise::ZERO;
    for (id, ledger) in &books.masters.ledgers {
        let opening = signed_amount(ledger.opening, "opening")?;
        opening_net = opening_net.checked_add(opening)?;
        let moved = movement.get(id).copied().unwrap_or(Paise::ZERO);
        balances.insert(*id, opening.checked_add(moved)?);
    }
    Ok((balances, opening_net))
}

/// Walks the group tree and assembles rows, totals and the difference row.
struct Builder<'a> {
    children: BTreeMap<Option<GroupId>, Vec<&'a Group>>,
    ledgers: BTreeMap<GroupId, Vec<&'a Ledger>>,
    balances: &'a BTreeMap<LedgerId, Paise>,
    detail: Detail,
    placed_ledgers: BTreeSet<LedgerId>,
}

/// One group's roll-up. Tally does not net a group whose ledgers sit on both sides: its row shows
/// the total of its debit-balance ledgers and the total of its credit-balance ledgers, and the
/// grand total adds those columns. Source: the Trial Balance screenshots in
/// <https://help.tallysolutions.com/article/Tally.ERP9/Reports/Display_Financial_Statements/Display_Trial_Balance.htm>,
/// read 2026-11-21 (see the rp2 test for the arithmetic).
struct GroupOut {
    /// Sum of the debit balances of every ledger in the group and its sub-groups.
    dr: Paise,
    /// Sum of the credit balances (as positive amounts) of every ledger in the group and its
    /// sub-groups.
    cr: Paise,
    rows: Vec<ReportRow>,
    shown: bool,
}

impl<'a> Builder<'a> {
    fn new(books: &'a Books, balances: &'a BTreeMap<LedgerId, Paise>, detail: Detail) -> Self {
        let mut children: BTreeMap<Option<GroupId>, Vec<&Group>> = BTreeMap::new();
        for g in books.masters.groups.values() {
            children.entry(g.parent).or_default().push(g);
        }
        let mut ledgers: BTreeMap<GroupId, Vec<&Ledger>> = BTreeMap::new();
        for l in books.masters.ledgers.values() {
            if let Some(group) = l.group {
                ledgers.entry(group).or_default().push(l);
            }
        }
        Builder {
            children,
            ledgers,
            balances,
            detail,
            placed_ledgers: BTreeSet::new(),
        }
    }

    fn group(&mut self, group: &Group, depth: u8) -> Result<GroupOut, ReportError> {
        if depth > MAX_DEPTH {
            return Err(ReportError::Invalid {
                field: "groups".to_string(),
                reason: format!("group {} is nested too deeply", group.id.0),
            });
        }
        let detailed = match self.detail {
            Detail::Detailed => true,
            Detail::Condensed => false,
        };
        let mut dr = Paise::ZERO;
        let mut cr = Paise::ZERO;
        let mut shown = false;
        let mut body = Vec::new();

        let own_ledgers = self.ledgers.get(&group.id).cloned().unwrap_or_default();
        for l in own_ledgers {
            self.placed_ledgers.insert(l.id);
            let balance = self.balances.get(&l.id).copied().unwrap_or(Paise::ZERO);
            if balance.is_zero() {
                continue;
            }
            if balance.is_negative() {
                cr = cr.checked_add(balance.checked_neg()?)?;
            } else {
                dr = dr.checked_add(balance)?;
            }
            shown = true;
            if detailed {
                body.push(ReportRow {
                    key: Some(DrillKey::Ledger(l.id)),
                    label: l.name.clone(),
                    depth: depth.saturating_add(1),
                    kind: RowKind::Ledger,
                    cells: balance_cells(balance)?,
                });
            }
        }
        let subgroups = self
            .children
            .get(&Some(group.id))
            .cloned()
            .unwrap_or_default();
        for sub in subgroups {
            let out = self.group(sub, depth.saturating_add(1))?;
            dr = dr.checked_add(out.dr)?;
            cr = cr.checked_add(out.cr)?;
            if out.shown {
                shown = true;
                if detailed {
                    body.extend(out.rows);
                }
            }
        }
        let mut rows = Vec::with_capacity(body.len().saturating_add(1));
        rows.push(ReportRow {
            key: Some(DrillKey::Group(group.id)),
            label: group.name.clone(),
            depth,
            kind: RowKind::Group,
            cells: side_cells(dr, cr),
        });
        rows.extend(body);
        Ok(GroupOut {
            dr,
            cr,
            rows,
            shown,
        })
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

/// `[Debit, Credit]` cells for a group: each side shows its own total, so a group with ledgers on
/// both sides shows both.
fn side_cells(dr: Paise, cr: Paise) -> Vec<Cell> {
    let side = |amount: Paise, cell: fn(Paise) -> SignedAmount| {
        if amount.is_zero() {
            Cell::Empty
        } else {
            Cell::Amount(cell(amount))
        }
    };
    vec![side(dr, SignedAmount::dr), side(cr, SignedAmount::cr)]
}

/// The explicit opening-difference row.
fn difference_row(cells: Vec<Cell>) -> ReportRow {
    ReportRow {
        key: None,
        label: DIFFERENCE_LABEL.to_string(),
        depth: 0,
        kind: RowKind::Difference,
        cells,
    }
}

/// Every ledger must sit under a primary group, or its balance would be missing from the report.
fn check_all_placed(books: &Books, placed: &BTreeSet<LedgerId>) -> Result<(), ReportError> {
    match books
        .masters
        .ledgers
        .values()
        .find(|l| !placed.contains(&l.id))
    {
        Some(orphan) => Err(ReportError::Invalid {
            field: "groups".to_string(),
            reason: match orphan.group {
                Some(_) => format!(
                    "ledger {} is in {}, which is missing or not reachable from a primary group",
                    orphan.name,
                    crate::group_text(orphan.group)
                ),
                None => format!(
                    "ledger {} has no group; only the predefined Profit & Loss A/c sits under Primary",
                    orphan.name
                ),
            },
        }),
        None => Ok(()),
    }
}

fn build_report(
    books: &Books,
    params: &ReportParams,
    balances: &BTreeMap<LedgerId, Paise>,
    opening_net: Paise,
) -> Result<Report, ReportError> {
    let mut builder = Builder::new(books, balances, params.detail);
    let primaries = builder.children.get(&None).cloned().unwrap_or_default();

    let mut rows = Vec::new();
    let mut dr_total = Paise::ZERO;
    let mut cr_total = Paise::ZERO;
    for primary in primaries {
        let out = builder.group(primary, 0)?;
        dr_total = dr_total.checked_add(out.dr)?;
        cr_total = cr_total.checked_add(out.cr)?;
        if out.shown {
            rows.extend(out.rows);
        }
    }
    // The predefined Profit & Loss A/c sits under Primary itself (see the module doc).
    for l in books.masters.ledgers.values() {
        if l.group.is_some() || l.reserved != Some(ReservedLedger::ProfitAndLoss) {
            continue;
        }
        builder.placed_ledgers.insert(l.id);
        let balance = balances.get(&l.id).copied().unwrap_or(Paise::ZERO);
        if balance.is_zero() {
            continue;
        }
        if balance.is_negative() {
            cr_total = cr_total.checked_add(balance.checked_neg()?)?;
        } else {
            dr_total = dr_total.checked_add(balance)?;
        }
        rows.push(ReportRow {
            key: Some(DrillKey::Ledger(l.id)),
            label: l.name.clone(),
            depth: 0,
            kind: RowKind::Ledger,
            cells: balance_cells(balance)?,
        });
    }
    check_all_placed(books, &builder.placed_ledgers)?;

    // Spec invariant 4: an unbalanced opening is shown as an explicit line on the short side.
    if !opening_net.is_zero() {
        let amount = opening_net.checked_abs()?;
        let cells = if opening_net.is_negative() {
            // Opening credits exceed debits: the difference is a debit.
            dr_total = dr_total.checked_add(amount)?;
            vec![Cell::Amount(SignedAmount::dr(amount)), Cell::Empty]
        } else {
            cr_total = cr_total.checked_add(amount)?;
            vec![Cell::Empty, Cell::Amount(SignedAmount::cr(amount))]
        };
        rows.push(difference_row(cells));
    }
    if dr_total != cr_total {
        return Err(ReportError::Invalid {
            field: "totals".to_string(),
            reason: "debit and credit totals differ after the opening difference".to_string(),
        });
    }

    Ok(Report {
        title: "Trial Balance".to_string(),
        params: params.clone(),
        columns: vec![
            ColumnHeader {
                label: "Debit".to_string(),
                period: None,
            },
            ColumnHeader {
                label: "Credit".to_string(),
                period: None,
            },
        ],
        rows,
        totals: vec![
            Cell::Amount(SignedAmount::dr(dr_total)),
            Cell::Amount(SignedAmount::cr(cr_total)),
        ],
        notes: vec![scenario_note(&params.scenario, params.as_of)],
    })
}

fn scenario_note(scenario: &Scenario, as_of: Date) -> String {
    match scenario {
        Scenario::Actual => format!("Scenario: Actual. Balances as of {as_of}."),
        Scenario::Including(statuses) => {
            let names: Vec<&str> = statuses.iter().map(|s| status_name(*s)).collect();
            format!(
                "Scenario: Actual plus {}. Balances as of {as_of}.",
                names.join(", ")
            )
        }
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
