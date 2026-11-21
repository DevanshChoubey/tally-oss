//! Card RP3: balance sheet tests. Hand-built books; every expected figure is derived in a comment.
//!
//! Chart used throughout (all names fictional):
//!
//! ```text
//! 1 Capital Account (primary, liability)      ledger 20 Proprietor Capital
//! 2 Current Liabilities (primary, liability)
//!   3 Sundry Creditors                        ledger 21 Supplier One
//! 4 Current Assets (primary, asset)
//!   5 Cash-in-Hand                            ledger 22 Cash
//!   6 Sundry Debtors                          ledger 23 Customer One
//! 7 Fixed Assets (primary, asset)             ledger 24 Furniture
//! 8 Sales Accounts (primary, income)          ledger 25 Sales
//! 9 Purchase Accounts (primary, expense)      ledger 26 Purchases
//! 10 Indirect Expenses (primary, expense)     ledger 27 Rent
//! 11 Unclassified (primary, no nature)        ledger 28 Odd Ledger
//! ```
//!
//! `rs(n)` is `n` rupees. Books begin on 1 April 2026; reports are as of 30 September 2026.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::wildcard_enum_match_arm,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, DrCr, GodownId, GroupId, LedgerId, MoneyError, Month, Outcome, Paise, Period,
    Qty, Rate, SignedAmount, StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, Masters, Nature,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, OpeningStock, Posting, ReservedGroup,
    RestartPeriod, Scenario, StockItem, StockMove, ValuationMethod, VoucherStatus, VoucherType,
};
use ledgerly_reports::balance_sheet::balance_sheet;
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, ReportRow, RowKind,
};
use ledgerly_stock::{ProvisionalReason, StockValuation, ValuationStatus};
use proptest::prelude::*;
use rust_decimal::Decimal;

const CAPITAL: u64 = 20;
const CREDITOR: u64 = 21;
const CASH: u64 = 22;
const DEBTOR: u64 = 23;
const FURNITURE: u64 = 24;
const SALES: u64 = 25;
const PURCHASES: u64 = 26;
const RENT: u64 = 27;
const ODD: u64 = 28;

const JOURNAL_TYPE: u64 = 1;
const MEMO_TYPE: u64 = 2;

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

fn as_of() -> Date {
    d(2026, Month::September, 30)
}

fn june() -> Date {
    d(2026, Month::June, 15)
}

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn group(id: u64, name: &str, parent: Option<u64>, nature: Option<Nature>) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: name.into(),
        aliases: vec![],
        parent: parent.map(GroupId),
        reserved: None,
        nature,
        affects_gross_profit: false,
        tally: None,
    }
}

fn ledger(id: u64, name: &str, group: u64) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.into(),
        aliases: vec![],
        group: GroupId(group),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn voucher_type(id: u64, name: &str, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id: VoucherTypeId(id),
        name: name.to_string(),
        base,
        abbreviation: name.to_string(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 0,
            restart: RestartPeriod::Never,
            series: vec![],
        },
        active: true,
        tally: None,
    }
}

fn masters() -> Masters {
    let mut m = Masters::default();
    for g in [
        group(1, "Capital Account", None, Some(Nature::Liability)),
        group(2, "Current Liabilities", None, Some(Nature::Liability)),
        group(3, "Sundry Creditors", Some(2), None),
        group(4, "Current Assets", None, Some(Nature::Asset)),
        group(5, "Cash-in-Hand", Some(4), None),
        group(6, "Sundry Debtors", Some(4), None),
        group(7, "Fixed Assets", None, Some(Nature::Asset)),
        group(8, "Sales Accounts", None, Some(Nature::Income)),
        group(9, "Purchase Accounts", None, Some(Nature::Expense)),
        group(10, "Indirect Expenses", None, Some(Nature::Expense)),
        group(11, "Unclassified", None, None),
    ] {
        m.groups.insert(g.id, g);
    }
    for l in [
        ledger(CAPITAL, "Proprietor Capital", 1),
        ledger(CREDITOR, "Supplier One", 3),
        ledger(CASH, "Cash", 5),
        ledger(DEBTOR, "Customer One", 6),
        ledger(FURNITURE, "Furniture", 7),
        ledger(SALES, "Sales", 8),
        ledger(PURCHASES, "Purchases", 9),
        ledger(RENT, "Rent", 10),
        ledger(ODD, "Odd Ledger", 11),
    ] {
        m.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(JOURNAL_TYPE, "Journal", BaseVoucherKind::Journal),
        voucher_type(MEMO_TYPE, "Memorandum", BaseVoucherKind::Memorandum),
    ] {
        m.voucher_types.insert(t.id, t);
    }
    m
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".into(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: vec![],
        fy_start: d(2026, Month::April, 1),
        books_begin: d(2026, Month::April, 1),
        entity_kind: EntityKind::Proprietorship,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn books_with(postings: Vec<Posting>) -> Books {
    Books {
        company: company(),
        masters: masters(),
        vouchers: BTreeMap::new(),
        postings,
        stock_moves: vec![],
    }
}

fn books_with_openings(postings: Vec<Posting>, openings: &[(u64, SignedAmount)]) -> Books {
    let mut books = books_with(postings);
    for (id, amount) in openings {
        books
            .masters
            .ledgers
            .get_mut(&LedgerId(*id))
            .unwrap()
            .opening = *amount;
    }
    books
}

fn one_posting(
    n: u64,
    date: Date,
    ledger: u64,
    amount: SignedAmount,
    status: VoucherStatus,
    type_id: u64,
) -> Posting {
    Posting {
        voucher: VoucherId(n),
        date,
        ledger: LedgerId(ledger),
        amount,
        status,
        type_id: VoucherTypeId(type_id),
    }
}

/// A balanced two-line regular journal voucher: debit `dr_ledger`, credit `cr_ledger`.
fn pair(n: u64, date: Date, dr_ledger: u64, cr_ledger: u64, amount: Paise) -> Vec<Posting> {
    pair_with(
        n,
        date,
        dr_ledger,
        cr_ledger,
        amount,
        VoucherStatus::Regular,
        JOURNAL_TYPE,
    )
}

fn pair_with(
    n: u64,
    date: Date,
    dr_ledger: u64,
    cr_ledger: u64,
    amount: Paise,
    status: VoucherStatus,
    type_id: u64,
) -> Vec<Posting> {
    vec![
        one_posting(
            n,
            date,
            dr_ledger,
            SignedAmount::dr(amount),
            status,
            type_id,
        ),
        one_posting(
            n,
            date,
            cr_ledger,
            SignedAmount::cr(amount),
            status,
            type_id,
        ),
    ]
}

fn params(detail: Detail) -> ReportParams {
    ReportParams {
        kind: ReportKind::BalanceSheet,
        period: Period::new(d(2026, Month::April, 1), as_of()).unwrap(),
        as_of: as_of(),
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn valuation(item: u64, on: Date, value: Paise, status: ValuationStatus) -> StockValuation {
    StockValuation {
        item: StockItemId(item),
        as_of: on,
        qty: Qty::ZERO,
        value,
        rate: None,
        status,
    }
}

fn stock(value: Paise) -> Outcome<Vec<StockValuation>> {
    Outcome::Value(vec![valuation(1, as_of(), value, ValuationStatus::Final)])
}

fn no_stock() -> Outcome<Vec<StockValuation>> {
    Outcome::Value(vec![])
}

fn run(books: &Books, p: &ReportParams, closing: &Outcome<Vec<StockValuation>>) -> Report {
    balance_sheet(books, p, closing)
        .unwrap()
        .value()
        .expect("report should be supported")
}

fn row<'a>(r: &'a Report, label: &str) -> &'a ReportRow {
    r.rows
        .iter()
        .find(|x| x.label == label)
        .unwrap_or_else(|| panic!("no row {label:?} in {:?}", r.rows))
}

fn dr(p: Paise) -> Cell {
    Cell::Amount(SignedAmount::dr(p))
}

fn cr(p: Paise) -> Cell {
    Cell::Amount(SignedAmount::cr(p))
}

/// The two total amounts, ignoring side and provisional marking.
fn total_amounts(r: &Report) -> (Paise, Paise) {
    let amount = |c: &Cell| match c {
        Cell::Amount(a) => a.amount,
        Cell::Provisional { amount, .. } => amount.amount,
        other => panic!("total is not an amount: {other:?}"),
    };
    (amount(&r.totals[0]), amount(&r.totals[1]))
}

/// The hand set (see `hand_set_*` tests for the arithmetic).
fn hand_postings() -> Vec<Posting> {
    let apr = d(2026, Month::April, 1);
    let mut p = Vec::new();
    p.extend(pair(1, apr, CASH, CAPITAL, rs(500_000)));
    p.extend(pair(2, june(), FURNITURE, CASH, rs(100_000)));
    p.extend(pair(3, june(), PURCHASES, CREDITOR, rs(200_000)));
    p.extend(pair(4, june(), DEBTOR, SALES, rs(300_000)));
    p.extend(pair(5, june(), RENT, CASH, rs(20_000)));
    p
}

#[test]
fn hand_set_totals_are_equal() {
    // provenance: derived — accounting identity, assets = liabilities + capital + profit.
    // Ledger balances (Dr positive): Cash 500,000 - 100,000 - 20,000 = 380,000 Dr;
    // Furniture 100,000 Dr; Customer One 300,000 Dr => assets 780,000.
    // Capital 500,000 Cr; Supplier One 200,000 Cr => liabilities 700,000.
    // Profit = Sales 300,000 + closing stock 50,000 - purchases 200,000 - rent 20,000 = 130,000.
    // Liabilities side 700,000 + 130,000 = 830,000. Assets side 780,000 + 50,000 = 830,000.
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        &stock(rs(50_000)),
    );
    assert_eq!(
        r.totals,
        vec![cr(rs(830_000)), dr(rs(830_000))],
        "liabilities total and assets total"
    );
    let (l, a) = total_amounts(&r);
    assert_eq!(l, a);
}

#[test]
fn profit_and_loss_ledger_carries_current_year_profit() {
    // provenance: derived — same hand set: profit 130,000 = Sales 300,000 + closing stock 50,000
    // - purchases 200,000 - rent 20,000 (trading form, spec 6.2). A profit is a credit balance on
    // the liabilities side.
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        &stock(rs(50_000)),
    );
    let pl = row(&r, "Profit and Loss A/c");
    assert_eq!(pl.cells, vec![cr(rs(130_000)), Cell::Empty]);
    assert_eq!(pl.kind, RowKind::Item);
}

#[test]
fn condensed_shows_primary_groups_only() {
    // provenance: derived — spec 6.2 "Condensed: primary groups only". Primaries with a non-zero
    // balance: Capital Account (500,000 Cr), Current Liabilities (200,000 Cr), Current Assets
    // (680,000 Dr = Cash 380,000 + Customer 300,000), Fixed Assets (100,000 Dr). Sales, purchase
    // and expense groups fold into the profit row; closing stock has its own row.
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        &stock(rs(50_000)),
    );
    let labels: Vec<&str> = r.rows.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Capital Account",
            "Current Liabilities",
            "Profit and Loss A/c",
            "Current Assets",
            "Fixed Assets",
            "Closing stock",
        ]
    );
    assert!(r.rows.iter().all(|x| x.depth == 0));
    assert!(r.rows.iter().all(|x| x.kind != RowKind::Ledger));
    assert_eq!(
        row(&r, "Capital Account").cells,
        vec![cr(rs(500_000)), Cell::Empty]
    );
    assert_eq!(
        row(&r, "Current Liabilities").cells,
        vec![cr(rs(200_000)), Cell::Empty]
    );
    assert_eq!(
        row(&r, "Current Assets").cells,
        vec![Cell::Empty, dr(rs(680_000))]
    );
    assert_eq!(
        row(&r, "Fixed Assets").cells,
        vec![Cell::Empty, dr(rs(100_000))]
    );
    assert_eq!(
        row(&r, "Closing stock").cells,
        vec![Cell::Empty, dr(rs(50_000))]
    );
}

#[test]
fn detailed_goes_down_to_ledgers_and_hides_zero_balances() {
    // provenance: derived — same balances as the hand set. Sub-groups: Sundry Creditors 200,000
    // Cr; Cash-in-Hand 380,000 Dr; Sundry Debtors 300,000 Dr. Rent, Sales and Purchases are
    // profit-and-loss ledgers and do not appear. The Odd Ledger has a zero balance and is hidden.
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Detailed),
        &stock(rs(50_000)),
    );
    let labels: Vec<&str> = r.rows.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Capital Account",
            "Proprietor Capital",
            "Current Liabilities",
            "Sundry Creditors",
            "Supplier One",
            "Profit and Loss A/c",
            "Current Assets",
            "Cash-in-Hand",
            "Cash",
            "Sundry Debtors",
            "Customer One",
            "Fixed Assets",
            "Furniture",
            "Closing stock",
        ]
    );
    let cash = row(&r, "Cash");
    assert_eq!(cash.depth, 2);
    assert_eq!(cash.kind, RowKind::Ledger);
    assert_eq!(cash.key, Some(DrillKey::Ledger(LedgerId(CASH))));
    assert_eq!(cash.cells, vec![Cell::Empty, dr(rs(380_000))]);
    assert_eq!(
        row(&r, "Supplier One").cells,
        vec![cr(rs(200_000)), Cell::Empty]
    );
    assert_eq!(
        row(&r, "Sundry Debtors").key,
        Some(DrillKey::Group(ledgerly_core::GroupId(6)))
    );
}

#[test]
fn loss_shows_on_the_assets_side() {
    // provenance: derived — Capital 100,000 Cr / Cash 100,000 Dr, then rent 30,000 paid from cash.
    // Cash = 100,000 - 30,000 = 70,000 Dr. Profit = -30,000 (a loss), no stock.
    // The loss is a debit, so the Profit and Loss A/c row is on the assets side:
    // assets 70,000 + 30,000 = 100,000; liabilities = capital 100,000.
    let mut p = pair(1, d(2026, Month::April, 1), CASH, CAPITAL, rs(100_000));
    p.extend(pair(2, june(), RENT, CASH, rs(30_000)));
    let r = run(&books_with(p), &params(Detail::Condensed), &no_stock());
    assert_eq!(
        row(&r, "Profit and Loss A/c").cells,
        vec![Cell::Empty, dr(rs(30_000))]
    );
    assert_eq!(r.totals, vec![cr(rs(100_000)), dr(rs(100_000))]);
}

#[test]
fn no_activity_gives_zero_totals_and_no_rows() {
    // provenance: derived — nothing posted, nothing owned: both sides are 0.
    let r = run(&books_with(vec![]), &params(Detail::Detailed), &no_stock());
    assert_eq!(r.rows, [] as [ledgerly_reports::ReportRow; 0]);
    let (l, a) = total_amounts(&r);
    assert_eq!(l, Paise::ZERO);
    assert_eq!(a, Paise::ZERO);
}

#[test]
fn debit_balance_in_a_liability_group_keeps_its_side() {
    // provenance: derived — Dr Supplier One 5,000 / Cr Capital 5,000. Supplier One is a debit
    // balance of 5,000 in a liability group: it is shown as a debit in the liabilities column,
    // not netted into a smaller credit. Liabilities net = 5,000 Cr (capital) - 5,000 Dr = 0.
    let p = pair(1, june(), CREDITOR, CAPITAL, rs(5_000));
    let r = run(&books_with(p), &params(Detail::Condensed), &no_stock());
    assert_eq!(
        row(&r, "Current Liabilities").cells,
        vec![dr(rs(5_000)), Cell::Empty]
    );
    assert_eq!(
        row(&r, "Capital Account").cells,
        vec![cr(rs(5_000)), Cell::Empty]
    );
    let (l, a) = total_amounts(&r);
    assert_eq!(l, Paise::ZERO);
    assert_eq!(a, Paise::ZERO);
}

#[test]
fn unbalanced_opening_is_an_explicit_difference_row() {
    // provenance: derived — spec invariant 4. Opening: Capital 100,000 Cr, Cash 60,000 Dr.
    // Debits 60,000 < credits 100,000, so the difference is a debit of 40,000 on the assets side
    // (it stands for an opening asset with no ledger, such as opening stock).
    // Assets = Cash 60,000 + difference 40,000 = 100,000; liabilities = 100,000.
    let books = books_with_openings(
        vec![],
        &[
            (CAPITAL, SignedAmount::cr(rs(100_000))),
            (CASH, SignedAmount::dr(rs(60_000))),
        ],
    );
    let r = run(&books, &params(Detail::Condensed), &no_stock());
    let diff = row(&r, "Difference in opening balances");
    assert_eq!(diff.kind, RowKind::Difference);
    assert_eq!(diff.cells, vec![Cell::Empty, dr(rs(40_000))]);
    assert_eq!(r.totals, vec![cr(rs(100_000)), dr(rs(100_000))]);
}

#[test]
fn opening_difference_on_the_credit_side() {
    // provenance: derived — Opening: Capital 30,000 Cr, Cash 50,000 Dr. Debits exceed credits by
    // 20,000, so the difference is a credit of 20,000 on the liabilities side.
    // Liabilities = 30,000 + 20,000 = 50,000; assets = Cash 50,000.
    let books = books_with_openings(
        vec![],
        &[
            (CAPITAL, SignedAmount::cr(rs(30_000))),
            (CASH, SignedAmount::dr(rs(50_000))),
        ],
    );
    let r = run(&books, &params(Detail::Condensed), &no_stock());
    assert_eq!(
        row(&r, "Difference in opening balances").cells,
        vec![cr(rs(20_000)), Cell::Empty]
    );
    assert_eq!(r.totals, vec![cr(rs(50_000)), dr(rs(50_000))]);
}

#[test]
fn income_ledger_opening_balance_is_carried_profit() {
    // provenance: derived — spec 6.1 (prior profit folds into the profit ledger). Opening: Sales
    // 10,000 Cr (profit brought forward), Cash 10,000 Dr. Profit = 10,000, a credit.
    // Assets = Cash 10,000; liabilities = Profit and Loss A/c 10,000.
    let books = books_with_openings(
        vec![],
        &[
            (SALES, SignedAmount::cr(rs(10_000))),
            (CASH, SignedAmount::dr(rs(10_000))),
        ],
    );
    let r = run(&books, &params(Detail::Condensed), &no_stock());
    assert_eq!(
        row(&r, "Profit and Loss A/c").cells,
        vec![cr(rs(10_000)), Cell::Empty]
    );
    assert_eq!(r.totals, vec![cr(rs(10_000)), dr(rs(10_000))]);
}

#[test]
fn postings_after_as_of_are_left_out() {
    // provenance: derived — a balance sheet "as of" 30 Sep 2026 holds only postings dated on or
    // before it. Capital 100,000 on 1 Apr counts; a 40,000 sale on 1 Oct does not.
    // Assets = Cash 100,000; liabilities = Capital 100,000; no profit.
    let mut p = pair(1, d(2026, Month::April, 1), CASH, CAPITAL, rs(100_000));
    p.extend(pair(2, d(2026, Month::October, 1), CASH, SALES, rs(40_000)));
    let r = run(&books_with(p), &params(Detail::Condensed), &no_stock());
    assert_eq!(r.totals, vec![cr(rs(100_000)), dr(rs(100_000))]);
    assert!(r.rows.iter().all(|x| x.label != "Profit and Loss A/c"));
}

#[test]
fn optional_and_cancelled_vouchers_follow_the_scenario() {
    // provenance: derived — spec invariant 3: optional vouchers are outside the Actual scenario;
    // a scenario naming Optional includes them; a cancelled voucher has no amounts in any scenario.
    // Base: Capital 100,000 Dr Cash. Optional: Dr Cash 1,000 / Cr Sales 1,000.
    // Cancelled: Dr Cash 7,000 / Cr Sales 7,000.
    let mut p = pair(1, d(2026, Month::April, 1), CASH, CAPITAL, rs(100_000));
    p.extend(pair_with(
        2,
        june(),
        CASH,
        SALES,
        rs(1_000),
        VoucherStatus::Optional,
        JOURNAL_TYPE,
    ));
    p.extend(pair_with(
        3,
        june(),
        CASH,
        SALES,
        rs(7_000),
        VoucherStatus::Cancelled,
        JOURNAL_TYPE,
    ));
    let books = books_with(p);

    let actual = run(&books, &params(Detail::Condensed), &no_stock());
    assert_eq!(actual.totals, vec![cr(rs(100_000)), dr(rs(100_000))]);

    let mut with_optional = params(Detail::Condensed);
    with_optional.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let r = run(&books, &with_optional, &no_stock());
    // Assets = Cash 101,000; liabilities = Capital 100,000 + profit 1,000.
    assert_eq!(r.totals, vec![cr(rs(101_000)), dr(rs(101_000))]);
    assert_eq!(
        row(&r, "Profit and Loss A/c").cells,
        vec![cr(rs(1_000)), Cell::Empty]
    );
}

#[test]
fn post_dated_voucher_enters_on_its_date() {
    // provenance: derived — a post-dated voucher dated on or before as_of is in the books; one
    // dated after as_of is not. Dr Cash 2,000 / Cr Sales 2,000 dated 20 Sep (in) and
    // Dr Cash 9,000 / Cr Sales 9,000 dated 5 Oct (out). Profit 2,000 on top of capital 10,000.
    let mut p = pair(1, d(2026, Month::April, 1), CASH, CAPITAL, rs(10_000));
    p.extend(pair_with(
        2,
        d(2026, Month::September, 20),
        CASH,
        SALES,
        rs(2_000),
        VoucherStatus::PostDated,
        JOURNAL_TYPE,
    ));
    p.extend(pair_with(
        3,
        d(2026, Month::October, 5),
        CASH,
        SALES,
        rs(9_000),
        VoucherStatus::PostDated,
        JOURNAL_TYPE,
    ));
    let r = run(&books_with(p), &params(Detail::Condensed), &no_stock());
    assert_eq!(r.totals, vec![cr(rs(12_000)), dr(rs(12_000))]);
}

#[test]
fn memorandum_vouchers_are_outside_the_books() {
    // provenance: derived — spec invariant 3: a memorandum voucher never touches base balances.
    // A 50,000 memorandum posting changes nothing; the capital voucher gives 10,000 each side.
    let mut p = pair(1, d(2026, Month::April, 1), CASH, CAPITAL, rs(10_000));
    p.extend(pair_with(
        2,
        june(),
        CASH,
        SALES,
        rs(50_000),
        VoucherStatus::Regular,
        MEMO_TYPE,
    ));
    let r = run(&books_with(p), &params(Detail::Condensed), &no_stock());
    assert_eq!(r.totals, vec![cr(rs(10_000)), dr(rs(10_000))]);
}

#[test]
fn memorandum_in_a_named_scenario_is_unsupported() {
    // provenance: derived — no rule says what a named scenario does to memorandum vouchers, so the
    // engine must say so rather than pick (rule A1).
    let p = pair_with(
        1,
        june(),
        CASH,
        SALES,
        rs(1_000),
        VoucherStatus::Regular,
        MEMO_TYPE,
    );
    let mut ps = params(Detail::Condensed);
    ps.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let out = balance_sheet(&books_with(p), &ps, &no_stock()).unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn provisional_stock_makes_profit_stock_and_totals_provisional() {
    // provenance: derived — a provisional valuation must never look final. Capital 100,000 Dr Cash;
    // closing stock 25,000 (provisional) => profit 25,000; assets 100,000 + 25,000, liabilities
    // 100,000 + 25,000. The stock row, the profit row and both totals carry the reason.
    let p = pair(1, d(2026, Month::April, 1), CASH, CAPITAL, rs(100_000));
    let closing = Outcome::Value(vec![valuation(
        1,
        as_of(),
        rs(25_000),
        ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: StockItemId(1),
            date: june(),
        }),
    )]);
    let r = run(&books_with(p), &params(Detail::Condensed), &closing);
    let is_provisional = |c: &Cell, side: DrCr, amount: Paise| match c {
        Cell::Provisional { amount: a, reason } => {
            a.side == side && a.amount == amount && reason.contains("negative stock")
        }
        _ => false,
    };
    assert!(is_provisional(
        &row(&r, "Closing stock").cells[1],
        DrCr::Dr,
        rs(25_000)
    ));
    assert!(is_provisional(
        &row(&r, "Profit and Loss A/c").cells[0],
        DrCr::Cr,
        rs(25_000)
    ));
    assert!(is_provisional(&r.totals[0], DrCr::Cr, rs(125_000)));
    assert!(is_provisional(&r.totals[1], DrCr::Dr, rs(125_000)));
    assert!(r.notes.iter().any(|n| n.contains("provisional")));
}

#[test]
fn unsupported_stock_makes_the_report_unsupported() {
    // provenance: derived — profit needs closing stock; without it the number would be wrong.
    let closing: Outcome<Vec<StockValuation>> =
        Outcome::unsupported("valuation: standard cost", "no standard cost set");
    let out = balance_sheet(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        &closing,
    )
    .unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn stock_dated_other_than_as_of_is_an_error() {
    // provenance: derived — closing stock at 31 Aug cannot be the closing stock at 30 Sep.
    let closing = Outcome::Value(vec![valuation(
        1,
        d(2026, Month::August, 31),
        rs(1_000),
        ValuationStatus::Final,
    )]);
    let err = balance_sheet(&books_with(vec![]), &params(Detail::Condensed), &closing).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }), "{err:?}");
}

#[test]
fn item_valued_twice_is_an_error() {
    // provenance: derived — one closing value per item; a duplicate would double count stock.
    let closing = Outcome::Value(vec![
        valuation(1, as_of(), rs(1_000), ValuationStatus::Final),
        valuation(1, as_of(), rs(1_000), ValuationStatus::Final),
    ]);
    let err = balance_sheet(&books_with(vec![]), &params(Detail::Condensed), &closing).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }), "{err:?}");
}

#[test]
fn several_items_add_up() {
    // provenance: derived — closing stock 12,000 + 8,000 = 20,000, no other activity: profit
    // 20,000 (credit), assets 20,000 (stock row).
    let closing = Outcome::Value(vec![
        valuation(1, as_of(), rs(12_000), ValuationStatus::Final),
        valuation(2, as_of(), rs(8_000), ValuationStatus::Final),
    ]);
    let r = run(&books_with(vec![]), &params(Detail::Condensed), &closing);
    assert_eq!(
        row(&r, "Closing stock").cells,
        vec![Cell::Empty, dr(rs(20_000))]
    );
    assert_eq!(r.totals, vec![cr(rs(20_000)), dr(rs(20_000))]);
}

#[test]
fn unsupported_parameters_are_reported() {
    // provenance: derived — rule A1: parameters not implemented give Unsupported, not a guess.
    let books = books_with(hand_postings());
    let closing = no_stock();

    let mut comparative = params(Detail::Condensed);
    comparative.columns = ColumnSpec::Comparative(vec![
        Period::new(d(2026, Month::April, 1), as_of()).unwrap(),
    ]);
    let mut monthly = params(Detail::Condensed);
    monthly.columns = ColumnSpec::Monthly;
    let mut filtered = params(Detail::Condensed);
    filtered.filter.ledgers = vec![LedgerId(CASH)];
    let mut exceptions = params(Detail::Condensed);
    exceptions.exceptions_only = true;

    for p in [comparative, monthly, filtered, exceptions] {
        let out = balance_sheet(&books, &p, &closing).unwrap();
        assert!(out.is_unsupported(), "{p:?}");
    }
}

#[test]
fn group_without_nature_holding_a_balance_is_unsupported() {
    // provenance: derived — a primary group with no nature has no side on the balance sheet; with
    // a balance in it the report cannot be placed, so it says so.
    let p = pair(1, june(), ODD, CAPITAL, rs(100));
    let out = balance_sheet(&books_with(p), &params(Detail::Condensed), &no_stock()).unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn group_without_nature_and_no_balance_is_ignored() {
    // provenance: derived — the unclassified group is empty, so nothing is lost by skipping it.
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        &no_stock(),
    );
    assert!(r.rows.iter().all(|x| x.label != "Unclassified"));
}

#[test]
fn wrong_report_kind_is_an_error() {
    let mut p = params(Detail::Condensed);
    p.kind = ReportKind::TrialBalance;
    let err = balance_sheet(&books_with(vec![]), &p, &no_stock()).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }), "{err:?}");
}

#[test]
fn as_of_before_books_begin_is_an_error() {
    let mut p = params(Detail::Condensed);
    p.as_of = d(2026, Month::March, 31);
    let err = balance_sheet(&books_with(vec![]), &p, &no_stock()).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }), "{err:?}");
}

#[test]
fn posting_before_books_begin_is_an_error() {
    let p = pair(1, d(2026, Month::March, 31), CASH, CAPITAL, rs(1));
    let err = balance_sheet(&books_with(p), &params(Detail::Condensed), &no_stock()).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }), "{err:?}");
}

#[test]
fn unbalanced_postings_are_an_error_not_absorbed() {
    // provenance: derived — a voucher with Dr 100 and Cr 90 is corrupt (spec invariant 1).
    let p = vec![
        one_posting(
            1,
            june(),
            CASH,
            SignedAmount::dr(rs(100)),
            VoucherStatus::Regular,
            1,
        ),
        one_posting(
            1,
            june(),
            CAPITAL,
            SignedAmount::cr(rs(90)),
            VoucherStatus::Regular,
            1,
        ),
    ];
    let err = balance_sheet(&books_with(p), &params(Detail::Condensed), &no_stock()).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }), "{err:?}");
}

#[test]
fn posting_to_a_missing_ledger_is_not_found() {
    let p = pair(1, june(), 999, CAPITAL, rs(1));
    let err = balance_sheet(&books_with(p), &params(Detail::Condensed), &no_stock()).unwrap_err();
    assert!(matches!(err, ReportError::NotFound { .. }), "{err:?}");
}

#[test]
fn posting_with_a_missing_voucher_type_is_not_found() {
    let p = pair_with(1, june(), CASH, CAPITAL, rs(1), VoucherStatus::Regular, 99);
    let err = balance_sheet(&books_with(p), &params(Detail::Condensed), &no_stock()).unwrap_err();
    assert!(matches!(err, ReportError::NotFound { .. }), "{err:?}");
}

#[test]
fn ledger_in_an_unreachable_group_is_an_error() {
    // provenance: derived — a ledger whose group is missing would drop out of the totals.
    let mut books = books_with(vec![]);
    books
        .masters
        .ledgers
        .insert(LedgerId(40), ledger(40, "Stray", 777));
    let err = balance_sheet(&books, &params(Detail::Condensed), &no_stock()).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }), "{err:?}");
}

#[test]
fn overflow_is_an_error_not_a_wrap() {
    // provenance: derived — Cash would hold i64::MAX + 1 paise, which has no representation.
    let mut p = pair(1, june(), CASH, CAPITAL, Paise::new(i64::MAX));
    p.extend(pair(2, june(), CASH, CAPITAL, Paise::new(1)));
    let err = balance_sheet(&books_with(p), &params(Detail::Condensed), &no_stock()).unwrap_err();
    assert!(
        matches!(err, ReportError::Money(MoneyError::Overflow)),
        "{err:?}"
    );
}

#[test]
fn report_is_titled_and_has_two_columns() {
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        &no_stock(),
    );
    assert_eq!(r.title, "Balance Sheet");
    let labels: Vec<&str> = r.columns.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Liabilities", "Assets"]);
    assert!(r.rows.iter().all(|x| x.cells.len() == 2));
}

fn amount_strategy() -> impl Strategy<Value = i64> {
    1_i64..100_000_000
}

proptest! {
    // provenance: derived — accounting identity. Balanced postings between any ledgers, any
    // closing stock, and any opening balances: the Profit and Loss A/c row and the opening
    // difference row make both totals equal, because every voucher is Dr = Cr and every ledger
    // is placed (Liability, Asset, or folded into profit).
    #[test]
    fn totals_always_equal(
        moves in proptest::collection::vec((20_u64..28, 20_u64..28, amount_strategy()), 0..12),
        stock_paise in 0_i64..100_000_000,
        opening_cash in 0_i64..100_000_000,
        detailed in any::<bool>(),
    ) {
        let mut p = Vec::new();
        for (i, (from, to, amount)) in moves.iter().enumerate() {
            p.extend(pair(
                u64::try_from(i).unwrap() + 1,
                june(),
                *from,
                *to,
                Paise::new(*amount),
            ));
        }
        let books = books_with_openings(p, &[(CASH, SignedAmount::dr(Paise::new(opening_cash)))]);
        let detail = if detailed { Detail::Detailed } else { Detail::Condensed };
        let r = run(&books, &params(detail), &stock(Paise::new(stock_paise)));
        let (l, a) = total_amounts(&r);
        prop_assert_eq!(l, a);
    }
}

fn stock_item(opening: Vec<OpeningStock>) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: StockItemId(1),
        name: "Widget".into(),
        aliases: vec![],
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: None,
        gst_history: vec![],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening,
        standard_costs: vec![],
        tally: None,
    }
}

fn opening_stock(qty: i64, rate: i64) -> OpeningStock {
    OpeningStock {
        godown: GodownId(1),
        batch: None,
        qty: Qty(Decimal::from(qty)),
        rate: Rate(Decimal::from(rate)),
    }
}

#[test]
fn opening_stock_is_unsupported_not_an_overstated_profit() {
    // provenance: derived — Widget opens with 10 units at Rs 100 = Rs 1,000 of stock that has no
    // ledger, and nothing trades afterwards, so closing stock is still Rs 1,000 and the true
    // profit is -(0) + 1,000 closing - 1,000 opening = 0. Without an opening stock input the
    // report would show a profit of 1,000 and absorb the other 1,000 in the opening difference
    // row, so it must say Unsupported instead.
    let mut books = books_with(vec![]);
    let item = stock_item(vec![opening_stock(10, 100)]);
    books.masters.stock_items.insert(item.id, item);
    let out = balance_sheet(&books, &params(Detail::Condensed), &stock(rs(1_000))).unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn zero_valued_opening_stock_is_harmless() {
    // provenance: derived — 10 units at rate 0 are worth Rs 0, so nothing is overstated.
    let mut books = books_with(hand_postings());
    let item = stock_item(vec![opening_stock(10, 0)]);
    books.masters.stock_items.insert(item.id, item);
    let out = balance_sheet(&books, &params(Detail::Condensed), &no_stock()).unwrap();
    assert!(!out.is_unsupported());
}

#[test]
fn stock_moved_before_books_begin_is_unsupported() {
    // provenance: derived — a movement before books_begin has no matching ledger posting, so the
    // stock it leaves in hand would be counted as profit.
    let mut books = books_with(vec![]);
    books.stock_moves.push(StockMove {
        voucher: VoucherId(1),
        date: d(2026, Month::March, 31),
        item: StockItemId(1),
        godown: GodownId(1),
        batch: None,
        qty_in: Qty(Decimal::from(5)),
        qty_out: Qty::ZERO,
        value: rs(500),
    });
    let out = balance_sheet(&books, &params(Detail::Condensed), &stock(rs(500))).unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn balance_on_a_stock_in_hand_ledger_is_unsupported() {
    // provenance: derived — a ledger under Stock-in-Hand holding Rs 1,000 plus an injected closing
    // stock of Rs 1,000 would show the stock twice, so the report says Unsupported.
    let mut books = books_with_openings(vec![], &[(CASH, SignedAmount::dr(rs(1_000)))]);
    let mut g = group(12, "Stock-in-Hand", Some(4), None);
    g.reserved = Some(ReservedGroup::StockInHand);
    books.masters.groups.insert(g.id, g);
    books
        .masters
        .ledgers
        .insert(LedgerId(29), ledger(29, "Opening Stock", 12));
    books
        .masters
        .ledgers
        .get_mut(&LedgerId(29))
        .unwrap()
        .opening = SignedAmount::dr(rs(1_000));
    books
        .masters
        .ledgers
        .get_mut(&LedgerId(CASH))
        .unwrap()
        .opening = SignedAmount::cr(rs(1_000));
    let out = balance_sheet(&books, &params(Detail::Condensed), &stock(rs(1_000))).unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn empty_stock_in_hand_ledger_is_harmless() {
    // provenance: derived — a Stock-in-Hand ledger with a zero balance counts nothing twice.
    let mut books = books_with(hand_postings());
    let mut g = group(12, "Stock-in-Hand", Some(4), None);
    g.reserved = Some(ReservedGroup::StockInHand);
    books.masters.groups.insert(g.id, g);
    books
        .masters
        .ledgers
        .insert(LedgerId(29), ledger(29, "Opening Stock", 12));
    let out = balance_sheet(&books, &params(Detail::Condensed), &no_stock()).unwrap();
    assert!(!out.is_unsupported());
}
