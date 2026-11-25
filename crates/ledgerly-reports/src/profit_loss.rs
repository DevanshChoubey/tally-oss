//! Profit and loss in trading form. **Card RP4** implements this module.
//!
//! Frozen interface (plan card RP4):
//!
//! ```text
//! pub fn profit_loss(books: &Books, params: &ReportParams, opening_stock: &Outcome<Vec<StockValuation>>, closing_stock: &Outcome<Vec<StockValuation>>) -> Result<Outcome<Report>, ReportError>;
//! ```
//!
//! # What is computed
//!
//! Trading section (spec 6.2): `GP = Sales + Closing stock - Opening stock - Purchases - Direct
//! expenses`, generalised the way a trading account is: every ledger under a primary group of
//! nature Income or Expense whose group has `affects_gross_profit` set belongs to the trading
//! account (so direct incomes are credits like sales). Profit and loss section: the ledgers of
//! the remaining Income and Expense primary groups; `NP = GP + indirect incomes - indirect
//! expenses`.
//!
//! Two-sided rows (session 12): the account is also given as the debit and credit sides of a
//! horizontal account. Every amount carries the side it stands on; `Gross profit c/o` (or `Gross
//! loss c/o` on the credit side) balances the trading account, `Gross profit b/f` (or `Gross loss
//! b/f` on the debit side) opens the profit and loss account, `Net profit c/o` (or `Net loss c/o`)
//! balances it, and `Trading total` and `Profit and loss total` are what both sides come to. The
//! sides must agree or the report is an error. The signed `Gross profit` and `Net profit` rows
//! stay.
//!
//! Stock values are injected, never computed here. Opening stock must be the valuation at the
//! day before the period starts and closing stock the valuation at the period end; a valuation
//! dated otherwise is an error. A provisional valuation makes every figure derived from it
//! provisional, with the reason.
//!
//! Opening stock entered by hand is the opening balance of a ledger under Stock-in-Hand (TallyPrime
//! help "Stock Items - FAQ", <https://help.tallysolutions.com/stock-items-faq/>, read
//! 2026-11-25: create a Stock-in-Hand ledger and enter the opening stock amount; the manually
//! entered opening and closing stock are displayed in the Profit & Loss account). It is stock on
//! hand when the books began, so it is added to the opening stock of a period that starts on
//! `books_begin`. For a later period the opening stock is what is on hand then, which the ledger
//! does not say, so such a report is `Unsupported` while a Stock-in-Hand ledger holds an opening
//! balance.
//!
//! # What is not computed (returns `Unsupported`)
//!
//! * comparative or monthly columns, filters and `exceptions_only` (not implemented yet);
//! * a non-zero opening balance on an Income/Expense ledger (no defined period split);
//! * a group of nature Asset or Liability flagged `affects_gross_profit`, or a primary group
//!   with no nature;
//! * a Stock-in-Hand ledger that is posted to or has a credit opening balance, and a period that
//!   starts after `books_begin` while a Stock-in-Hand ledger holds an opening balance;
//! * either stock input being `Unsupported`.
//!
//! The scenario filter is applied here from the documented meaning of [`Scenario`] because the
//! engine's `effective_postings` (card E6) is not built yet. Once it is, this module should call
//! it instead (see the interface request in the card report).

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{Date, GroupId, LedgerId, Outcome, Paise, SignedAmount};
use ledgerly_model::{Books, Group, Ledger, Nature, Posting, Scenario, VoucherStatus};
use ledgerly_stock::{ProvisionalReason, StockValuation, ValuationStatus};

use crate::balance_sheet::{negative_stock_reason, stock_in_hand_opening};
use crate::{
    Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};

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

/// Which block of the statement a ledger lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Bucket {
    TradingExpense,
    TradingIncome,
    IndirectExpense,
    IndirectIncome,
}

/// A ledger that belongs to the P&L, with its group chain (own group first, primary last).
struct Classified {
    chain: Vec<GroupId>,
    bucket: Bucket,
}

/// Totals of one stock input.
struct StockTotal {
    value: Paise,
    provisional: Option<String>,
}

/// Net movements per ledger and group, debit positive.
#[derive(Default)]
struct Tree {
    ledger_net: BTreeMap<LedgerId, Paise>,
    ledger_bucket: BTreeMap<LedgerId, Bucket>,
    ledger_chain: BTreeMap<LedgerId, Vec<GroupId>>,
    group_net: BTreeMap<GroupId, Paise>,
    sub_groups: BTreeMap<GroupId, BTreeSet<GroupId>>,
    ledgers_in: BTreeMap<GroupId, BTreeSet<LedgerId>>,
    primaries: BTreeMap<Bucket, BTreeSet<GroupId>>,
    bucket_net: BTreeMap<Bucket, Paise>,
}

/// Builds the profit and loss account in trading form.
///
/// `opening_stock` is the valuation at the day before `params.period` starts and `closing_stock`
/// the valuation at `params.period.to()`.
///
/// # Errors
/// [`ReportError::Invalid`] for parameters of another report or a stock valuation dated wrongly
/// or listed twice for one item, [`ReportError::NotFound`] for a posting or group that references
/// a missing master, [`ReportError::Money`] on overflow.
pub fn profit_loss(
    books: &Books,
    params: &ReportParams,
    opening_stock: &Outcome<Vec<StockValuation>>,
    closing_stock: &Outcome<Vec<StockValuation>>,
) -> Result<Outcome<Report>, ReportError> {
    check_kind(params)?;
    if let Some(u) = unsupported_params(params) {
        return Ok(u);
    }
    let from = params.period.from();
    let day_before = from.previous_day().ok_or_else(|| ReportError::Invalid {
        field: "period".into(),
        reason: format!("{from} has no previous day to value opening stock at"),
    })?;
    let opening = supported!(stock_total(
        books,
        opening_stock,
        day_before,
        "opening stock"
    )?);
    let mut opening = opening;
    let held = supported!(stock_in_hand_opening(books)?);
    if !held.is_zero() {
        if from != books.company.books_begin {
            return Ok(Outcome::unsupported(
                "profit and loss: opening stock after the books begin",
                format!(
                    "a Stock-in-Hand ledger holds the stock on hand at {}; the opening stock of a period starting {from} is not stated by it",
                    books.company.books_begin
                ),
            ));
        }
        opening.value = opening.value.checked_add(held)?;
    }
    let closing = supported!(stock_total(
        books,
        closing_stock,
        params.period.to(),
        "closing stock"
    )?);
    let mut tree = Tree::default();
    supported!(classify_ledgers(books, &mut tree)?);
    accumulate(books, params, &mut tree)?;
    Ok(Outcome::Value(render(
        books, params, &tree, &opening, &closing,
    )?))
}

fn check_kind(params: &ReportParams) -> Result<(), ReportError> {
    match params.kind {
        ReportKind::ProfitAndLoss => Ok(()),
        ReportKind::TrialBalance
        | ReportKind::BalanceSheet
        | ReportKind::GroupSummary(_)
        | ReportKind::GroupLedgers(_)
        | ReportKind::LedgerVouchers(_)
        | ReportKind::DayBook
        | ReportKind::Register(_)
        | ReportKind::Outstanding { .. }
        | ReportKind::StockSummary
        | ReportKind::Audit(_) => Err(ReportError::Invalid {
            field: "kind".into(),
            reason: "profit_loss needs ReportKind::ProfitAndLoss".into(),
        }),
    }
}

fn unsupported_params(params: &ReportParams) -> Option<Outcome<Report>> {
    let what = match &params.columns {
        ColumnSpec::Single => None,
        ColumnSpec::Comparative(_) => Some("profit and loss: comparative columns"),
        ColumnSpec::Monthly => Some("profit and loss: monthly columns"),
    };
    if let Some(what) = what {
        return Some(Outcome::unsupported(
            what,
            "only a single column for the report period is implemented",
        ));
    }
    if params.filter != ReportFilter::default() {
        return Some(Outcome::unsupported(
            "profit and loss: filters",
            "filtering by ledger, group, voucher type or item is not implemented; clear the filter",
        ));
    }
    if params.exceptions_only {
        return Some(Outcome::unsupported(
            "profit and loss: exceptions only",
            "an exceptions view of this report is not implemented",
        ));
    }
    None
}

fn stock_total(
    books: &Books,
    input: &Outcome<Vec<StockValuation>>,
    expected: Date,
    which: &str,
) -> Result<Outcome<StockTotal>, ReportError> {
    let valuations = match input {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => {
            return Ok(Outcome::unsupported(
                format!("profit and loss: {which} ({})", u.what),
                format!("{which} cannot be valued: {}", u.reason),
            ));
        }
    };
    let mut seen = BTreeSet::new();
    let mut value = Paise::ZERO;
    let mut reasons = Vec::new();
    for v in valuations {
        if v.as_of != expected {
            return Err(ReportError::Invalid {
                field: which.into(),
                reason: format!(
                    "valuation of item {:?} is dated {} but {which} needs {expected}",
                    v.item, v.as_of
                ),
            });
        }
        if !seen.insert(v.item) {
            return Err(ReportError::Invalid {
                field: which.into(),
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

/// Group ids from `start` up to its primary group, own group first.
fn group_chain(books: &Books, start: GroupId) -> Result<Vec<GroupId>, ReportError> {
    let limit = books.masters.groups.len();
    let mut chain = Vec::new();
    let mut at = start;
    for _ in 0..=limit {
        let group = group_of(books, at)?;
        chain.push(at);
        match group.parent {
            Some(parent) => at = parent,
            None => return Ok(chain),
        }
    }
    Err(ReportError::Invalid {
        field: "group".into(),
        reason: format!("group {start:?} is part of a parent cycle"),
    })
}

fn group_of(books: &Books, id: GroupId) -> Result<&Group, ReportError> {
    books
        .masters
        .groups
        .get(&id)
        .ok_or_else(|| ReportError::NotFound {
            what: format!("group {id:?}"),
        })
}

fn bucket_for(nature: Nature, trading: bool) -> Option<Bucket> {
    match (nature, trading) {
        (Nature::Expense, true) => Some(Bucket::TradingExpense),
        (Nature::Income, true) => Some(Bucket::TradingIncome),
        (Nature::Expense, false) => Some(Bucket::IndirectExpense),
        (Nature::Income, false) => Some(Bucket::IndirectIncome),
        (Nature::Asset | Nature::Liability, _) => None,
    }
}

/// Decides, for one ledger, whether it is part of the P&L and where.
fn classify(books: &Books, ledger: &Ledger) -> Result<Outcome<Option<Classified>>, ReportError> {
    let chain = group_chain(books, ledger.group)?;
    let primary_id = chain.last().copied().unwrap_or(ledger.group);
    let primary = group_of(books, primary_id)?;
    let Some(nature) = primary.nature else {
        return Ok(Outcome::unsupported(
            "profit and loss: group without nature",
            format!(
                "primary group '{}' has no nature (asset, liability, income or expense)",
                primary.name
            ),
        ));
    };
    let Some(bucket) = bucket_for(nature, primary.affects_gross_profit) else {
        if primary.affects_gross_profit {
            return Ok(Outcome::unsupported(
                "profit and loss: gross profit flag on a balance sheet group",
                format!(
                    "group '{}' is an asset or liability but affects gross profit; fix the group",
                    primary.name
                ),
            ));
        }
        return Ok(Outcome::Value(None));
    };
    if !ledger.opening.amount.is_zero() {
        return Ok(Outcome::unsupported(
            "profit and loss: opening balance on an income or expense ledger",
            format!(
                "ledger '{}' has an opening balance; how it splits across periods is not defined",
                ledger.name
            ),
        ));
    }
    Ok(Outcome::Value(Some(Classified { chain, bucket })))
}

fn classify_ledgers(books: &Books, tree: &mut Tree) -> Result<Outcome<()>, ReportError> {
    for ledger in books.masters.ledgers.values() {
        let Some(c) = supported!(classify(books, ledger)?) else {
            continue;
        };
        tree.ledger_net.insert(ledger.id, Paise::ZERO);
        tree.ledger_bucket.insert(ledger.id, c.bucket);
        tree.ledgers_in
            .entry(ledger.group)
            .or_default()
            .insert(ledger.id);
        let mut child: Option<GroupId> = None;
        for gid in &c.chain {
            tree.group_net.entry(*gid).or_insert(Paise::ZERO);
            if let Some(child) = child {
                tree.sub_groups.entry(*gid).or_default().insert(child);
            }
            child = Some(*gid);
        }
        if let Some(primary) = c.chain.last() {
            tree.primaries.entry(c.bucket).or_default().insert(*primary);
        }
        tree.ledger_chain.insert(ledger.id, c.chain);
    }
    Ok(Outcome::Value(()))
}

fn listed(scenario: &Scenario, status: VoucherStatus) -> bool {
    match scenario {
        Scenario::Actual => false,
        Scenario::Including(statuses) => statuses.contains(&status),
    }
}

/// Meaning of [`Scenario`] (spec invariant 3): regular always; post-dated on or before the
/// as-of date; anything else only when the scenario names its status.
fn included(p: &Posting, scenario: &Scenario, as_of: Date) -> bool {
    match p.status {
        VoucherStatus::Regular => true,
        VoucherStatus::PostDated => p.date <= as_of || listed(scenario, p.status),
        VoucherStatus::Optional | VoucherStatus::Cancelled => listed(scenario, p.status),
    }
}

fn accumulate(books: &Books, params: &ReportParams, tree: &mut Tree) -> Result<(), ReportError> {
    for p in &books.postings {
        if !params.period.contains(p.date) || !included(p, &params.scenario, params.as_of) {
            continue;
        }
        if !books.masters.ledgers.contains_key(&p.ledger) {
            return Err(ReportError::NotFound {
                what: format!("ledger {:?} (posting of voucher {:?})", p.ledger, p.voucher),
            });
        }
        let (Some(bucket), Some(chain)) = (
            tree.ledger_bucket.get(&p.ledger).copied(),
            tree.ledger_chain.get(&p.ledger),
        ) else {
            continue; // balance sheet ledger
        };
        let delta = p.amount.to_signed()?;
        for gid in chain {
            add_to(&mut tree.group_net, *gid, delta)?;
        }
        add_to(&mut tree.ledger_net, p.ledger, delta)?;
        add_to(&mut tree.bucket_net, bucket, delta)?;
    }
    Ok(())
}

fn add_to<K: Ord>(map: &mut BTreeMap<K, Paise>, key: K, delta: Paise) -> Result<(), ReportError> {
    let slot = map.entry(key).or_insert(Paise::ZERO);
    *slot = slot.checked_add(delta)?;
    Ok(())
}

fn amount_cell(net_debit: Paise) -> Result<Cell, ReportError> {
    Ok(Cell::Amount(SignedAmount::from_signed(net_debit)?))
}

fn row(label: &str, depth: u8, kind: RowKind, key: Option<DrillKey>, cell: Cell) -> ReportRow {
    ReportRow {
        key,
        label: label.to_owned(),
        depth,
        kind,
        cells: vec![cell],
    }
}

fn text_row(label: &str) -> ReportRow {
    row(label, 0, RowKind::Text, None, Cell::Empty)
}

fn render_group(
    books: &Books,
    tree: &Tree,
    gid: GroupId,
    depth: u8,
    detailed: bool,
    rows: &mut Vec<ReportRow>,
) -> Result<(), ReportError> {
    let group = group_of(books, gid)?;
    let net = tree.group_net.get(&gid).copied().unwrap_or(Paise::ZERO);
    rows.push(row(
        &group.name,
        depth,
        RowKind::Group,
        Some(DrillKey::Group(gid)),
        amount_cell(net)?,
    ));
    if !detailed {
        return Ok(());
    }
    let child_depth = depth.saturating_add(1);
    if let Some(subs) = tree.sub_groups.get(&gid) {
        for sub in subs {
            render_group(books, tree, *sub, child_depth, detailed, rows)?;
        }
    }
    if let Some(ledgers) = tree.ledgers_in.get(&gid) {
        for lid in ledgers {
            let name = books
                .masters
                .ledgers
                .get(lid)
                .map(|l| l.name.as_str())
                .ok_or_else(|| ReportError::NotFound {
                    what: format!("ledger {lid:?}"),
                })?;
            let net = tree.ledger_net.get(lid).copied().unwrap_or(Paise::ZERO);
            rows.push(row(
                name,
                child_depth,
                RowKind::Ledger,
                Some(DrillKey::Ledger(*lid)),
                amount_cell(net)?,
            ));
        }
    }
    Ok(())
}

fn render_bucket(
    books: &Books,
    tree: &Tree,
    bucket: Bucket,
    detailed: bool,
    rows: &mut Vec<ReportRow>,
) -> Result<(), ReportError> {
    if let Some(primaries) = tree.primaries.get(&bucket) {
        for gid in primaries {
            render_group(books, tree, *gid, 0, detailed, rows)?;
        }
    }
    Ok(())
}

/// The total both sides of a two-sided account come to once the balancing figure is on its side:
/// `balancing` is debit positive, so a profit is added to the debit side and a loss to the credit
/// side. The sides must agree; they always do when the nets are right, and a difference is an
/// error, never a shown total.
fn balanced_total(
    debit: Paise,
    credit: Paise,
    balancing: Paise,
    account: &str,
) -> Result<Paise, ReportError> {
    let (to_debit, to_credit) = if balancing.is_negative() {
        (Paise::ZERO, balancing.checked_neg()?)
    } else {
        (balancing, Paise::ZERO)
    };
    let debit_total = debit.checked_add(to_debit)?;
    let credit_total = credit.checked_add(to_credit)?;
    if debit_total != credit_total {
        return Err(ReportError::Invalid {
            field: "totals".into(),
            reason: format!(
                "the two sides of the {account} differ by {} paise",
                debit_total.checked_sub(credit_total)?.get()
            ),
        });
    }
    Ok(debit_total)
}

/// A profit figure: `Signed` when final, `Provisional` (profit shown as a credit balance) when
/// stock is provisional.
fn profit_cell(profit: Paise, provisional: Option<&str>) -> Result<Cell, ReportError> {
    match provisional {
        None => Ok(Cell::Signed(profit)),
        Some(reason) => Ok(Cell::Provisional {
            amount: SignedAmount::from_signed(profit.checked_neg()?)?,
            reason: reason.to_owned(),
        }),
    }
}

fn stock_cell(value: Paise, debit: bool, provisional: Option<&str>) -> Result<Cell, ReportError> {
    let amount = if debit {
        SignedAmount::from_signed(value)?
    } else {
        SignedAmount::from_signed(value.checked_neg()?)?
    };
    Ok(match provisional {
        None => Cell::Amount(amount),
        Some(reason) => Cell::Provisional {
            amount,
            reason: reason.to_owned(),
        },
    })
}

/// Joins the provisional reasons of both stock inputs, `None` when both are final.
fn combined_reason(opening: &StockTotal, closing: &StockTotal) -> Option<String> {
    let joined = [
        opening.provisional.as_deref(),
        closing.provisional.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("; ");
    (!joined.is_empty()).then_some(joined)
}

fn report_notes(provisional: Option<&str>) -> Vec<String> {
    let mut notes =
        vec!["Profit figures are positive for a profit and negative for a loss.".to_owned()];
    if let Some(reason) = provisional {
        notes.push(format!(
            "Gross and net profit are provisional because stock is provisional ({reason})."
        ));
    }
    notes
}

#[allow(
    clippy::too_many_lines,
    reason = "one linear pass that lays out the two-sided rows in order"
)]
fn render(
    books: &Books,
    params: &ReportParams,
    tree: &Tree,
    opening: &StockTotal,
    closing: &StockTotal,
) -> Result<Report, ReportError> {
    let detailed = params.detail == Detail::Detailed;
    let net = |b: Bucket| tree.bucket_net.get(&b).copied().unwrap_or(Paise::ZERO);
    // Debit-positive net of the trading account: Dr items minus Cr items, stock included.
    let trading_dr = net(Bucket::TradingExpense)
        .checked_add(net(Bucket::TradingIncome))?
        .checked_add(opening.value)?
        .checked_sub(closing.value)?;
    let gross = trading_dr.checked_neg()?;
    let indirect_dr = net(Bucket::IndirectExpense).checked_add(net(Bucket::IndirectIncome))?;
    let net_profit = gross.checked_sub(indirect_dr)?;
    let provisional = combined_reason(opening, closing);

    let mut rows = vec![text_row("Trading account")];
    rows.push(row(
        "Opening stock",
        0,
        RowKind::Item,
        None,
        stock_cell(opening.value, true, opening.provisional.as_deref())?,
    ));
    render_bucket(books, tree, Bucket::TradingExpense, detailed, &mut rows)?;
    render_bucket(books, tree, Bucket::TradingIncome, detailed, &mut rows)?;
    rows.push(row(
        "Closing stock",
        0,
        RowKind::Item,
        None,
        stock_cell(closing.value, false, closing.provisional.as_deref())?,
    ));
    rows.push(row(
        "Gross profit",
        0,
        RowKind::Subtotal,
        None,
        profit_cell(gross, provisional.as_deref())?,
    ));
    let prov = provisional.as_deref();
    // The two-sided form: each amount carries the side it stands on. Debit side of the trading
    // account is opening stock and the debit groups, credit side the credit groups and closing
    // stock; the gross profit (or loss) balances the two.
    let trading_debit = net(Bucket::TradingExpense).checked_add(opening.value)?;
    let trading_credit = net(Bucket::TradingIncome)
        .checked_neg()?
        .checked_add(closing.value)?;
    let (gross_label, gross_back_label) = if gross.is_negative() {
        ("Gross loss c/o", "Gross loss b/f")
    } else {
        ("Gross profit c/o", "Gross profit b/f")
    };
    rows.push(row(
        gross_label,
        0,
        RowKind::Item,
        None,
        stock_cell(gross, true, prov)?,
    ));
    let trading_total = balanced_total(trading_debit, trading_credit, gross, "trading account")?;
    rows.push(row(
        "Trading total",
        0,
        RowKind::Subtotal,
        None,
        stock_cell(trading_total, true, prov)?,
    ));
    rows.push(text_row("Profit and loss account"));
    rows.push(row(
        gross_back_label,
        0,
        RowKind::Item,
        None,
        stock_cell(gross, false, prov)?,
    ));
    render_bucket(books, tree, Bucket::IndirectExpense, detailed, &mut rows)?;
    render_bucket(books, tree, Bucket::IndirectIncome, detailed, &mut rows)?;
    let net_cell = profit_cell(net_profit, provisional.as_deref())?;
    rows.push(row(
        "Net profit",
        0,
        RowKind::Subtotal,
        None,
        net_cell.clone(),
    ));
    let net_label = if net_profit.is_negative() {
        "Net loss c/o"
    } else {
        "Net profit c/o"
    };
    rows.push(row(
        net_label,
        0,
        RowKind::Item,
        None,
        stock_cell(net_profit, true, prov)?,
    ));
    let account_debit = net(Bucket::IndirectExpense);
    let account_credit = net(Bucket::IndirectIncome).checked_neg()?;
    let carried_in = if gross.is_negative() {
        (gross.checked_neg()?, Paise::ZERO)
    } else {
        (Paise::ZERO, gross)
    };
    let account_total = balanced_total(
        account_debit.checked_add(carried_in.0)?,
        account_credit.checked_add(carried_in.1)?,
        net_profit,
        "profit and loss account",
    )?;
    rows.push(row(
        "Profit and loss total",
        0,
        RowKind::Subtotal,
        None,
        stock_cell(account_total, true, prov)?,
    ));

    let notes = report_notes(provisional.as_deref());
    Ok(Report {
        title: "Profit and loss account".into(),
        params: params.clone(),
        columns: vec![ColumnHeader {
            label: format!("{} to {}", params.period.from(), params.period.to()),
            period: Some(params.period),
        }],
        rows,
        totals: vec![net_cell],
        notes,
    })
}
