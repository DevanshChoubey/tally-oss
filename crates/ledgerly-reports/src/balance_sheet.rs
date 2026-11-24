//! Balance sheet. **Card RP3** implements this module.
//!
//! Frozen interface (plan card RP3):
//!
//! ```text
//! pub fn balance_sheet(books: &Books, params: &ReportParams, closing_stock: &Outcome<Vec<StockValuation>>) -> Result<Outcome<Report>, ReportError>;
//! ```
//!
//! # What is computed
//!
//! The statement as of `params.as_of` (the period is not used). Two columns: `Liabilities` and
//! `Assets`. Every ledger under a primary group of nature Liability goes to the first column and
//! every ledger under a primary group of nature Asset to the second. A ledger's closing balance
//! is its opening balance plus the included postings dated on or before `as_of`, the same rule
//! as the trial balance, so the two reports agree ledger by ledger. A balance on the unnatural
//! side (a debit balance in a liability group) is shown with its own side, never netted away.
//!
//! The ledgers of Income and Expense primary groups are not listed. Their net, plus the closing
//! stock, is folded into one `Profit and Loss A/c` row (spec 6.1: prior-year profit folds into
//! the profit and loss ledger at query time, there are no closing entries):
//!
//! ```text
//! profit = - (net debit of every income and expense ledger up to as_of) + closing stock
//! ```
//!
//! That is the cumulative profit from the beginning of the books, not only the current year. The
//! `Profit and Loss A/c` row is a credit for a profit and shows on the assets side as a debit for
//! a loss. A ledger carrying `ReservedLedger::ProfitAndLoss` is an ordinary ledger of its group:
//! do not also enter prior profit as an opening balance on an income or expense ledger, or it is
//! counted twice.
//!
//! Opening balances may be unbalanced (spec invariant 4). The difference is an explicit
//! [`RowKind::Difference`] row on the short side, so both totals agree. Opening stock is not
//! absorbed there: the profit row would be overstated by it (the frozen interface has no opening
//! stock input to deduct), so books with opening stock are `Unsupported`. Postings that do not
//! balance among themselves are corrupt input and an error, never absorbed.
//!
//! Closing stock is injected, never computed here: the valuation must be dated `params.as_of`.
//! A provisional valuation makes the stock row, the profit row and both totals provisional.
//!
//! # What is not computed (returns `Unsupported`)
//!
//! * comparative or monthly columns, filters and `exceptions_only`;
//! * a named scenario over memorandum or reversing-journal postings;
//! * a primary group with no nature that holds a non-zero balance;
//! * closing stock being `Unsupported`;
//! * books with opening stock (an item with a non-zero opening quantity and rate), a stock
//!   movement dated before `books_begin`, or a non-zero balance on a ledger under the
//!   Stock-in-Hand group: the profit would be overstated, or the stock counted twice, with no
//!   visible sign because the opening difference row absorbs the gap.
//!
//! # Layout (s9)
//!
//! Primary groups are listed in the chart's order, not id order: Capital Account, Loans
//! (Liability), Current Liabilities, Fixed Assets, Investments, Current Assets, then the rest
//! (`GROUP_ORDER`, which orders sub-groups too); groups that are not reserved keep their id order
//! after those. The closing
//! stock is the value of the Stock-in-Hand group: when the chart has that group, the report lists
//! one `Closing stock` row where the group stands (a child of Current Assets in the reserved
//! chart), the stock is part of its parent's total, and a provisional stock makes every group
//! total above it provisional. A chart without a Stock-in-Hand group keeps a top-level
//! `Closing stock` row.
//!
//! `Detail::Condensed` lists primary groups only; `Detail::Detailed` goes down to ledgers and
//! leaves out ledgers and groups whose balance is zero.

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{
    Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, StockItemId, VoucherTypeId,
};
use ledgerly_model::{
    BaseVoucherKind, Books, Group, Ledger, Nature, Posting, ReservedGroup, Scenario, VoucherStatus,
    VoucherType,
};
use ledgerly_stock::{ProvisionalReason, StockValuation, ValuationStatus};

use crate::{
    Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};

/// Label of the explicit opening-difference row.
const DIFFERENCE_LABEL: &str = "Difference in opening balances";

/// Label of the folded profit and loss row.
const PROFIT_LABEL: &str = "Profit and Loss A/c";

/// Label of the closing stock row.
const STOCK_LABEL: &str = "Closing stock";

/// The order groups are listed in, at every level: the design mock's (design/mock-v2/index.html,
/// `balanceSheet`: Capital, Loans, Current liabilities with its Sundry creditors, Duties and
/// taxes, Provisions; Fixed assets, Current assets with Closing stock, Sundry debtors,
/// Cash-in-hand, Bank accounts, Loans and advances), with Investments between the two asset
/// groups. TallyPrime's Balance Sheet help says the order of groups is a setting (an index per
/// group; its example puts Investments above Fixed Assets, help.tallysolutions.com Balance Sheet
/// page, read 2026-11-25); this fixed order is the default until groups carry an index.
const GROUP_ORDER: [ReservedGroup; 17] = [
    ReservedGroup::CapitalAccount,
    ReservedGroup::ReservesAndSurplus,
    ReservedGroup::Loans,
    ReservedGroup::BankOd,
    ReservedGroup::SecuredLoans,
    ReservedGroup::UnsecuredLoans,
    ReservedGroup::CurrentLiabilities,
    ReservedGroup::SundryCreditors,
    ReservedGroup::DutiesAndTaxes,
    ReservedGroup::Provisions,
    ReservedGroup::FixedAssets,
    ReservedGroup::Investments,
    ReservedGroup::CurrentAssets,
    ReservedGroup::StockInHand,
    ReservedGroup::SundryDebtors,
    ReservedGroup::CashInHand,
    ReservedGroup::BankAccounts,
];

/// Sort key of a group: its place in [`GROUP_ORDER`], the other reserved groups after it, then
/// groups that are not reserved. Ties keep id order (the sort is stable).
fn group_rank(group: &Group) -> usize {
    match group.reserved {
        Some(r) => GROUP_ORDER
            .iter()
            .position(|p| *p == r)
            .unwrap_or(GROUP_ORDER.len()),
        None => GROUP_ORDER.len().saturating_add(1),
    }
}

/// Deepest group nesting the report follows. `ReportRow::depth` is a `u8`.
const MAX_DEPTH: u8 = 200;

/// Unwraps an [`Outcome`] inside a function returning `Result<Outcome<_>, _>`, passing
/// `Unsupported` straight out.
macro_rules! supported {
    ($e:expr) => {
        match $e {
            Outcome::Value(v) => v,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        }
    };
}

/// Which column a row sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Liabilities,
    Assets,
}

/// What a primary group is for in this statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// Listed in the liabilities column.
    Liability,
    /// Listed in the assets column.
    Asset,
    /// Folded into the profit and loss row.
    Profit,
    /// No nature set: unsupported when it holds a balance.
    NoNature,
}

fn role_of(group: &Group) -> Role {
    match group.nature {
        Some(Nature::Liability) => Role::Liability,
        Some(Nature::Asset) => Role::Asset,
        Some(Nature::Income | Nature::Expense) => Role::Profit,
        None => Role::NoNature,
    }
}

/// Total of the closing stock input.
#[derive(Clone)]
struct StockTotal {
    value: Paise,
    provisional: Option<String>,
}

/// Builds the balance sheet as of `params.as_of` (spec 6.2).
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not a balance sheet, `as_of` precedes the
/// company's `books_begin`, a stored amount is negative, a posting precedes `books_begin`, the
/// included postings do not balance, a stock valuation is dated other than `as_of` or lists an
/// item twice, or a ledger cannot be reached from a primary group; [`ReportError::NotFound`]
/// when a posting names a ledger or voucher type that does not exist; [`ReportError::Money`] on
/// overflow.
pub fn balance_sheet(
    books: &Books,
    params: &ReportParams,
    closing_stock: &Outcome<Vec<StockValuation>>,
) -> Result<Outcome<Report>, ReportError> {
    check_kind(params)?;
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
    let stock = supported!(stock_total(books, closing_stock, params.as_of)?);
    let movement = supported!(movement_by_ledger(books, params)?);
    let (balances, opening_net) = closing_balances(books, &movement)?;
    if let Some(unsupported) = unsupported_stock_books(books, &balances) {
        return Ok(unsupported);
    }
    build_report(books, params, &balances, opening_net, &stock)
}

fn check_kind(params: &ReportParams) -> Result<(), ReportError> {
    match params.kind {
        ReportKind::BalanceSheet => Ok(()),
        ReportKind::TrialBalance
        | ReportKind::ProfitAndLoss
        | ReportKind::GroupSummary(_)
        | ReportKind::LedgerVouchers(_)
        | ReportKind::DayBook
        | ReportKind::Register(_)
        | ReportKind::Outstanding { .. }
        | ReportKind::StockSummary
        | ReportKind::Audit(_) => Err(ReportError::Invalid {
            field: "kind".to_owned(),
            reason: "balance_sheet needs ReportKind::BalanceSheet".to_owned(),
        }),
    }
}

/// Parameters this card does not compute.
fn unsupported_params(params: &ReportParams) -> Option<Outcome<Report>> {
    match &params.columns {
        ColumnSpec::Single => {}
        ColumnSpec::Comparative(_) | ColumnSpec::Monthly => {
            return Some(Outcome::unsupported(
                "balance sheet: comparative or monthly columns",
                "only the single liabilities and assets column pair is computed; run one report per date",
            ));
        }
    }
    if params.filter != ReportFilter::default() {
        return Some(Outcome::unsupported(
            "balance sheet: filters",
            "a filtered balance sheet would not balance; clear the filter",
        ));
    }
    if params.exceptions_only {
        return Some(Outcome::unsupported(
            "balance sheet: exceptions only",
            "no exception rules are defined for the balance sheet",
        ));
    }
    None
}

/// Whether `group` is the Stock-in-Hand group or sits under it.
fn under_stock_in_hand(books: &Books, group: GroupId) -> bool {
    let mut current = Some(group);
    // The hop bound stops a cyclic chart; `check_all_placed` reports the cycle itself.
    for _ in 0..=books.masters.groups.len() {
        let Some(id) = current else {
            return false;
        };
        let Some(g) = books.masters.groups.get(&id) else {
            return false;
        };
        if g.reserved == Some(ReservedGroup::StockInHand) {
            return true;
        }
        current = g.parent;
    }
    false
}

/// Books whose stock cannot be folded into the profit correctly.
///
/// The profit row is `-(income and expense net) + closing stock`. Opening stock, stock moved
/// before the books begin, or a Stock-in-Hand ledger carrying a balance would each need an
/// opening stock deduction (or would count the stock twice against the closing stock row); the
/// opening difference row would hide the error, so the report says `Unsupported` instead.
fn unsupported_stock_books(
    books: &Books,
    balances: &BTreeMap<LedgerId, Paise>,
) -> Option<Outcome<Report>> {
    for item in books.masters.stock_items.values() {
        let valued = item
            .opening
            .iter()
            .any(|o| !o.qty.0.is_zero() && !o.rate.0.is_zero());
        if valued {
            return Some(Outcome::unsupported(
                "balance sheet: opening stock",
                format!(
                    "item '{}' has opening stock; the profit row would be overstated by its value",
                    item.name
                ),
            ));
        }
    }
    if let Some(m) = books
        .stock_moves
        .iter()
        .find(|m| m.date < books.company.books_begin)
    {
        return Some(Outcome::unsupported(
            "balance sheet: stock moved before the books begin",
            format!(
                "a stock movement is dated {}, before the books begin on {}",
                m.date, books.company.books_begin
            ),
        ));
    }
    for (id, ledger) in &books.masters.ledgers {
        let balance = balances.get(id).copied().unwrap_or(Paise::ZERO);
        if !balance.is_zero() && under_stock_in_hand(books, ledger.group) {
            return Some(Outcome::unsupported(
                "balance sheet: balance on a Stock-in-Hand ledger",
                format!(
                    "ledger '{}' under Stock-in-Hand has a balance; closing stock is injected, so it would be counted twice",
                    ledger.name
                ),
            ));
        }
    }
    None
}

/// The reason line for a stock item that went below zero: the item's name, or its number when
/// the masters do not hold it (never the Rust debug text of its id).
pub(crate) fn negative_stock_reason(books: &Books, item: StockItemId, date: Date) -> String {
    let who = books
        .masters
        .stock_items
        .get(&item)
        .map_or_else(|| item.0.to_string(), |i| i.name.clone());
    format!("negative stock: item {who} first went below zero on {date}")
}

fn stock_total(
    books: &Books,
    input: &Outcome<Vec<StockValuation>>,
    expected: Date,
) -> Result<Outcome<StockTotal>, ReportError> {
    let valuations = match input {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => {
            return Ok(Outcome::unsupported(
                format!("balance sheet: closing stock ({})", u.what),
                format!("closing stock cannot be valued: {}", u.reason),
            ));
        }
    };
    let mut seen = BTreeSet::new();
    let mut value = Paise::ZERO;
    let mut reasons = Vec::new();
    for v in valuations {
        if v.as_of != expected {
            return Err(ReportError::Invalid {
                field: "closing stock".to_owned(),
                reason: format!(
                    "valuation of item {:?} is dated {} but the balance sheet needs {expected}",
                    v.item, v.as_of
                ),
            });
        }
        if !seen.insert(v.item) {
            return Err(ReportError::Invalid {
                field: "closing stock".to_owned(),
                reason: format!("item {:?} is valued twice", v.item),
            });
        }
        value = value.checked_add(v.value)?;
        match &v.status {
            ValuationStatus::Final => {}
            ValuationStatus::Provisional(ProvisionalReason::NegativeStock { item, date }) => {
                reasons.push(negative_stock_reason(books, *item, *date));
            }
        }
    }
    let provisional = if reasons.is_empty() {
        None
    } else {
        Some(reasons.join("; "))
    };
    Ok(Outcome::Value(StockTotal { value, provisional }))
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
            field: field.to_owned(),
            reason: "a stored amount is negative".to_owned(),
        });
    }
    Ok(amount.to_signed()?)
}

/// Net movement (debit positive) per ledger up to `as_of`, or why it cannot be computed.
fn movement_by_ledger(
    books: &Books,
    params: &ReportParams,
) -> Result<Outcome<BTreeMap<LedgerId, Paise>>, ReportError> {
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
                return Ok(Outcome::unsupported(
                    "balance sheet: memorandum or reversing journal vouchers in a named scenario",
                    "no rule defines whether a named scenario includes them; use the actual scenario",
                ));
            }
            Inclusion::Include => {}
        }
        if posting.date < books.company.books_begin {
            return Err(ReportError::Invalid {
                field: "postings".to_owned(),
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
            field: "postings".to_owned(),
            reason: format!(
                "debits and credits of the included postings differ by {} paise",
                net.get()
            ),
        });
    }
    Ok(Outcome::Value(by_ledger))
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

/// A cell for a debit-positive amount: debit when positive, credit when negative.
fn amount_cell(net_debit: Paise) -> Result<Cell, ReportError> {
    Ok(Cell::Amount(SignedAmount::from_signed(net_debit)?))
}

/// A figure that is provisional when the stock behind it is.
fn figure(net_debit: Paise, provisional: Option<&str>) -> Result<Cell, ReportError> {
    let amount = SignedAmount::from_signed(net_debit)?;
    Ok(match provisional {
        None => Cell::Amount(amount),
        Some(reason) => Cell::Provisional {
            amount,
            reason: reason.to_owned(),
        },
    })
}

/// Puts `cell` in the column of `side`, the other column empty.
fn place(side: Side, cell: Cell) -> Vec<Cell> {
    match side {
        Side::Liabilities => vec![cell, Cell::Empty],
        Side::Assets => vec![Cell::Empty, cell],
    }
}

/// Walks the group tree below the primary groups.
struct Walker<'a> {
    children: BTreeMap<Option<GroupId>, Vec<&'a Group>>,
    ledgers: BTreeMap<GroupId, Vec<&'a Ledger>>,
    balances: &'a BTreeMap<LedgerId, Paise>,
    detailed: bool,
    placed: BTreeSet<LedgerId>,
    stock: StockTotal,
    /// Whether the closing stock row was put where the Stock-in-Hand group stands.
    stock_placed: bool,
}

/// One group's roll-up.
struct GroupOut {
    net: Paise,
    rows: Vec<ReportRow>,
    shown: bool,
    /// Whether the net includes a provisional stock value.
    provisional: bool,
}

impl<'a> Walker<'a> {
    fn new(
        books: &'a Books,
        balances: &'a BTreeMap<LedgerId, Paise>,
        detail: Detail,
        stock: StockTotal,
    ) -> Self {
        let mut children: BTreeMap<Option<GroupId>, Vec<&Group>> = BTreeMap::new();
        for g in books.masters.groups.values() {
            children.entry(g.parent).or_default().push(g);
        }
        for list in children.values_mut() {
            list.sort_by_key(|g| group_rank(g));
        }
        let mut ledgers: BTreeMap<GroupId, Vec<&Ledger>> = BTreeMap::new();
        for l in books.masters.ledgers.values() {
            ledgers.entry(l.group).or_default().push(l);
        }
        let detailed = match detail {
            Detail::Detailed => true,
            Detail::Condensed => false,
        };
        Walker {
            children,
            ledgers,
            balances,
            detailed,
            placed: BTreeSet::new(),
            stock,
            stock_placed: false,
        }
    }

    /// The Stock-in-Hand group is the closing stock: one row where the group stands.
    fn stock_group(&mut self, net: Paise, depth: u8, side: Side) -> Result<GroupOut, ReportError> {
        self.stock_placed = true;
        let provisional = self.stock.provisional.as_deref();
        let row = ReportRow {
            key: None,
            label: STOCK_LABEL.to_owned(),
            depth,
            kind: RowKind::Item,
            cells: place(side, figure(self.stock.value, provisional)?),
        };
        Ok(GroupOut {
            net: net.checked_add(self.stock.value)?,
            rows: vec![row],
            shown: true,
            provisional: provisional.is_some(),
        })
    }

    fn group(&mut self, group: &Group, depth: u8, side: Side) -> Result<GroupOut, ReportError> {
        if depth > MAX_DEPTH {
            return Err(ReportError::Invalid {
                field: "groups".to_owned(),
                reason: format!("group {} is nested too deeply", group.id.0),
            });
        }
        let mut net = Paise::ZERO;
        let mut shown = false;
        let mut provisional = false;
        let mut body = Vec::new();

        let own_ledgers = self.ledgers.get(&group.id).cloned().unwrap_or_default();
        for l in own_ledgers {
            self.placed.insert(l.id);
            let balance = self.balances.get(&l.id).copied().unwrap_or(Paise::ZERO);
            net = net.checked_add(balance)?;
            if balance.is_zero() {
                continue;
            }
            shown = true;
            if self.detailed {
                body.push(ReportRow {
                    key: Some(DrillKey::Ledger(l.id)),
                    label: l.name.clone(),
                    depth: depth.saturating_add(1),
                    kind: RowKind::Ledger,
                    cells: place(side, amount_cell(balance)?),
                });
            }
        }
        let subgroups = self
            .children
            .get(&Some(group.id))
            .cloned()
            .unwrap_or_default();
        for sub in subgroups {
            let out = self.group(sub, depth.saturating_add(1), side)?;
            net = net.checked_add(out.net)?;
            provisional |= out.provisional;
            if out.shown {
                shown = true;
                if self.detailed {
                    body.extend(out.rows);
                }
            }
        }
        if group.reserved == Some(ReservedGroup::StockInHand)
            && !self.stock_placed
            && (!self.stock.value.is_zero() || self.stock.provisional.is_some())
        {
            return self.stock_group(net, depth, side);
        }
        let mut rows = Vec::with_capacity(body.len().saturating_add(1));
        let reason = if provisional {
            self.stock.provisional.as_deref()
        } else {
            None
        };
        rows.push(ReportRow {
            key: Some(DrillKey::Group(group.id)),
            label: group.name.clone(),
            depth,
            kind: RowKind::Group,
            cells: place(side, figure(net, reason)?),
        });
        rows.extend(body);
        Ok(GroupOut {
            net,
            rows,
            shown,
            provisional,
        })
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
            field: "groups".to_owned(),
            reason: format!(
                "ledger {} is in group {}, which is missing or not reachable from a primary group",
                orphan.name, orphan.group.0
            ),
        }),
        None => Ok(()),
    }
}

/// Rows and debit-positive nets of the statement being assembled.
#[derive(Default)]
struct Statement {
    liability_rows: Vec<ReportRow>,
    asset_rows: Vec<ReportRow>,
    liability_net: Paise,
    asset_net: Paise,
    /// Net of the income and expense ledgers (not part of either column).
    profit_net: Paise,
    /// Whether the closing stock is already inside a group's row and total.
    stock_placed: bool,
}

impl Statement {
    /// Adds a row for a debit-positive amount: a credit goes to the liabilities column, a debit
    /// to the assets column.
    fn add_by_sign(
        &mut self,
        label: &str,
        kind: RowKind,
        net_debit: Paise,
        cell: Cell,
    ) -> Result<(), ReportError> {
        let side = if net_debit.is_negative() {
            Side::Liabilities
        } else {
            Side::Assets
        };
        let row = ReportRow {
            key: None,
            label: label.to_owned(),
            depth: 0,
            kind,
            cells: place(side, cell),
        };
        match side {
            Side::Liabilities => {
                self.liability_net = self.liability_net.checked_add(net_debit)?;
                self.liability_rows.push(row);
            }
            Side::Assets => {
                self.asset_net = self.asset_net.checked_add(net_debit)?;
                self.asset_rows.push(row);
            }
        }
        Ok(())
    }
}

fn walk_primaries(
    books: &Books,
    balances: &BTreeMap<LedgerId, Paise>,
    detail: Detail,
    stock: &StockTotal,
) -> Result<Outcome<Statement>, ReportError> {
    let mut walker = Walker::new(books, balances, detail, stock.clone());
    let mut primaries = walker.children.get(&None).cloned().unwrap_or_default();
    primaries.sort_by_key(|g| group_rank(g));
    let mut st = Statement::default();
    for primary in primaries {
        let role = role_of(primary);
        let side = match role {
            Role::Liability | Role::Profit | Role::NoNature => Side::Liabilities,
            Role::Asset => Side::Assets,
        };
        let out = walker.group(primary, 0, side)?;
        match role {
            Role::Liability => {
                st.liability_net = st.liability_net.checked_add(out.net)?;
                if out.shown {
                    st.liability_rows.extend(out.rows);
                }
            }
            Role::Asset => {
                st.asset_net = st.asset_net.checked_add(out.net)?;
                if out.shown {
                    st.asset_rows.extend(out.rows);
                }
            }
            Role::Profit => {
                st.profit_net = st.profit_net.checked_add(out.net)?;
            }
            Role::NoNature => {
                if out.shown {
                    return Ok(Outcome::unsupported(
                        "balance sheet: group without nature",
                        format!(
                            "primary group '{}' has no nature (asset, liability, income or expense)",
                            primary.name
                        ),
                    ));
                }
            }
        }
    }
    check_all_placed(books, &walker.placed)?;
    st.stock_placed = walker.stock_placed;
    Ok(Outcome::Value(st))
}

fn build_report(
    books: &Books,
    params: &ReportParams,
    balances: &BTreeMap<LedgerId, Paise>,
    opening_net: Paise,
    stock: &StockTotal,
) -> Result<Outcome<Report>, ReportError> {
    let mut st = supported!(walk_primaries(books, balances, params.detail, stock)?);
    let provisional = stock.provisional.as_deref();

    // Cumulative profit as a debit-positive figure: income and expense net less closing stock.
    let profit_dr = st.profit_net.checked_sub(stock.value)?;
    if !profit_dr.is_zero() {
        let cell = figure(profit_dr, provisional)?;
        st.add_by_sign(PROFIT_LABEL, RowKind::Item, profit_dr, cell)?;
    }
    if !st.stock_placed && (!stock.value.is_zero() || provisional.is_some()) {
        st.asset_net = st.asset_net.checked_add(stock.value)?;
        st.asset_rows.push(ReportRow {
            key: None,
            label: STOCK_LABEL.to_owned(),
            depth: 0,
            kind: RowKind::Item,
            cells: place(Side::Assets, figure(stock.value, provisional)?),
        });
    }
    // Spec invariant 4: an unbalanced opening is shown as an explicit line on the short side.
    if !opening_net.is_zero() {
        let difference_dr = opening_net.checked_neg()?;
        st.add_by_sign(
            DIFFERENCE_LABEL,
            RowKind::Difference,
            difference_dr,
            amount_cell(difference_dr)?,
        )?;
    }
    if st.liability_net.checked_add(st.asset_net)? != Paise::ZERO {
        return Err(ReportError::Invalid {
            field: "totals".to_owned(),
            reason: "liabilities and assets differ after the profit and opening difference rows"
                .to_owned(),
        });
    }
    let totals = vec![
        figure(st.liability_net, provisional)?,
        figure(st.asset_net, provisional)?,
    ];
    let mut rows = st.liability_rows;
    rows.extend(st.asset_rows);
    Ok(Outcome::Value(Report {
        title: "Balance Sheet".to_owned(),
        params: params.clone(),
        columns: vec![
            ColumnHeader {
                label: "Liabilities".to_owned(),
                period: None,
            },
            ColumnHeader {
                label: "Assets".to_owned(),
                period: None,
            },
        ],
        rows,
        totals,
        notes: report_notes(&params.scenario, params.as_of, provisional),
    }))
}

fn report_notes(scenario: &Scenario, as_of: Date, provisional: Option<&str>) -> Vec<String> {
    let scenario_note = match scenario {
        Scenario::Actual => format!("Scenario: Actual. Balances as of {as_of}."),
        Scenario::Including(statuses) => {
            let names: Vec<&str> = statuses.iter().map(|s| status_name(*s)).collect();
            format!(
                "Scenario: Actual plus {}. Balances as of {as_of}.",
                names.join(", ")
            )
        }
    };
    let mut notes = vec![
        scenario_note,
        "Profit and Loss A/c is the profit from the beginning of the books to the as-of date, \
         including closing stock."
            .to_owned(),
    ];
    if let Some(reason) = provisional {
        notes.push(format!(
            "The profit, closing stock and totals are provisional because stock is provisional ({reason})."
        ));
    }
    notes
}

fn status_name(status: VoucherStatus) -> &'static str {
    match status {
        VoucherStatus::Regular => "Regular",
        VoucherStatus::Optional => "Optional",
        VoucherStatus::PostDated => "PostDated",
        VoucherStatus::Cancelled => "Cancelled",
    }
}
