//! Card RP4: profit and loss in trading form.
//!
//! All figures are fictional. Every expected value carries its provenance next to it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::wildcard_enum_match_arm,
    reason = "test helpers outside #[test] functions; tests may unwrap and panic (engineering rules B2)"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, DrCr, GroupId, LedgerId, Month, Outcome, Paise, Period, Qty, SignedAmount,
    StateCode, StockItemId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, Group, Ledger, Masters, Nature,
    NegativeStockPolicy, Posting, ReservedGroup, Scenario, VoucherStatus,
};
use ledgerly_reports::profit_loss::profit_loss;
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams, ReportRow, RowKind,
};
use ledgerly_stock::{ProvisionalReason, StockValuation, ValuationStatus};
use proptest::prelude::*;

const SALES_G: u64 = 1;
const PURCHASE_G: u64 = 2;
const DIRECT_EXP_G: u64 = 3;
const INDIRECT_EXP_G: u64 = 4;
const INDIRECT_INC_G: u64 = 5;
const CURRENT_ASSETS_G: u64 = 6;
const DIRECT_INC_G: u64 = 7;
const SALES_EXPORT_G: u64 = 8;

const SALES_L: u64 = 10;
const PURCHASES_L: u64 = 11;
const FREIGHT_L: u64 = 12;
const RENT_L: u64 = 13;
const INTEREST_L: u64 = 14;
const CASH_L: u64 = 15;
const COMMISSION_L: u64 = 16;
const EXPORT_SALES_L: u64 = 17;

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

fn period() -> Period {
    Period::new(d(2026, Month::April, 1), d(2026, Month::September, 30)).unwrap()
}

fn group(id: u64, name: &str, parent: Option<u64>, nature: Option<Nature>, gp: bool) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: name.into(),
        aliases: vec![],
        parent: parent.map(GroupId),
        reserved: None,
        nature,
        affects_gross_profit: gp,
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

fn masters() -> Masters {
    let mut m = Masters::default();
    for g in [
        group(SALES_G, "Sales Accounts", None, Some(Nature::Income), true),
        group(
            PURCHASE_G,
            "Purchase Accounts",
            None,
            Some(Nature::Expense),
            true,
        ),
        group(
            DIRECT_EXP_G,
            "Direct Expenses",
            None,
            Some(Nature::Expense),
            true,
        ),
        group(
            INDIRECT_EXP_G,
            "Indirect Expenses",
            None,
            Some(Nature::Expense),
            false,
        ),
        group(
            INDIRECT_INC_G,
            "Indirect Incomes",
            None,
            Some(Nature::Income),
            false,
        ),
        group(
            CURRENT_ASSETS_G,
            "Current Assets",
            None,
            Some(Nature::Asset),
            false,
        ),
        group(
            DIRECT_INC_G,
            "Direct Incomes",
            None,
            Some(Nature::Income),
            true,
        ),
        group(SALES_EXPORT_G, "Export Sales", Some(SALES_G), None, true),
    ] {
        m.groups.insert(g.id, g);
    }
    for l in [
        ledger(SALES_L, "Sales", SALES_G),
        ledger(PURCHASES_L, "Purchases", PURCHASE_G),
        ledger(FREIGHT_L, "Freight Inward", DIRECT_EXP_G),
        ledger(RENT_L, "Rent", INDIRECT_EXP_G),
        ledger(INTEREST_L, "Interest Received", INDIRECT_INC_G),
        ledger(CASH_L, "Cash", CURRENT_ASSETS_G),
        ledger(COMMISSION_L, "Commission Received", DIRECT_INC_G),
        ledger(EXPORT_SALES_L, "Export Sales A/c", SALES_EXPORT_G),
    ] {
        m.ledgers.insert(l.id, l);
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

fn posting(
    n: u64,
    ledger: u64,
    date: Date,
    amount: SignedAmount,
    status: VoucherStatus,
) -> Posting {
    Posting {
        voucher: VoucherId(n),
        date,
        ledger: LedgerId(ledger),
        amount,
        status,
        type_id: VoucherTypeId(1),
    }
}

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

/// The hand set used by most tests (see `hand_set_*` tests for the arithmetic).
fn hand_postings() -> Vec<Posting> {
    let day = d(2026, Month::June, 15);
    let reg = VoucherStatus::Regular;
    vec![
        posting(1, SALES_L, day, SignedAmount::cr(rs(60_000)), reg),
        posting(2, SALES_L, day, SignedAmount::cr(rs(40_000)), reg),
        posting(3, PURCHASES_L, day, SignedAmount::dr(rs(60_000)), reg),
        posting(4, FREIGHT_L, day, SignedAmount::dr(rs(5_000)), reg),
        posting(5, RENT_L, day, SignedAmount::dr(rs(10_000)), reg),
        posting(6, INTEREST_L, day, SignedAmount::cr(rs(2_000)), reg),
        // A balance sheet posting: must not appear in the P&L.
        posting(7, CASH_L, day, SignedAmount::dr(rs(99_000)), reg),
    ]
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

fn params(detail: Detail) -> ReportParams {
    ReportParams {
        kind: ReportKind::ProfitAndLoss,
        period: period(),
        as_of: d(2026, Month::September, 30),
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn valuation(item: u64, as_of: Date, value: Paise, status: ValuationStatus) -> StockValuation {
    StockValuation {
        item: StockItemId(item),
        as_of,
        qty: Qty::ZERO,
        value,
        rate: None,
        status,
    }
}

fn opening_stock(value: Paise) -> Outcome<Vec<StockValuation>> {
    Outcome::Value(vec![valuation(
        1,
        d(2026, Month::March, 31),
        value,
        ValuationStatus::Final,
    )])
}

fn closing_stock(value: Paise) -> Outcome<Vec<StockValuation>> {
    Outcome::Value(vec![valuation(
        1,
        d(2026, Month::September, 30),
        value,
        ValuationStatus::Final,
    )])
}

fn run(books: &Books, p: &ReportParams, open: Paise, close: Paise) -> Report {
    profit_loss(books, p, &opening_stock(open), &closing_stock(close))
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

fn cell(r: &Report, label: &str) -> Cell {
    row(r, label).cells[0].clone()
}

#[test]
fn hand_set_gross_profit_follows_spec_formula() {
    // provenance: derived — spec 6.2: GP = Sales + Closing stock - Opening stock - Purchases
    //   - Direct expenses.
    // Sales 60,000 + 40,000 = 1,00,000; Closing 30,000; Opening 20,000; Purchases 60,000;
    // Direct expenses (freight) 5,000.
    // GP = 100,000 + 30,000 - 20,000 - 60,000 - 5,000 = 45,000.00 rupees = 4,500,000 paise.
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        rs(20_000),
        rs(30_000),
    );
    assert_eq!(
        cell(&r, "Gross profit"),
        Cell::Signed(Paise::new(4_500_000))
    );
}

#[test]
fn hand_set_net_profit_adds_indirect_income_and_subtracts_indirect_expense() {
    // provenance: derived — net profit = gross profit + indirect incomes - indirect expenses
    // (definition of the profit and loss account; spec 6.2 "P&L (trading form)").
    // 45,000 (GP, see previous test) + 2,000 (interest) - 10,000 (rent) = 37,000.00 rupees
    // = 3,700,000 paise. The 99,000 cash posting is a balance sheet item and is ignored.
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        rs(20_000),
        rs(30_000),
    );
    assert_eq!(cell(&r, "Net profit"), Cell::Signed(Paise::new(3_700_000)));
    assert_eq!(r.totals, vec![Cell::Signed(Paise::new(3_700_000))]);
}

#[test]
fn trading_rows_carry_their_side() {
    // provenance: derived — trading account debit side holds opening stock, purchases and direct
    // expenses; credit side holds sales and closing stock (spec 6.2).
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        rs(20_000),
        rs(30_000),
    );
    let dr = |p| Cell::Amount(SignedAmount::dr(p));
    let cr = |p| Cell::Amount(SignedAmount::cr(p));
    assert_eq!(cell(&r, "Opening stock"), dr(rs(20_000)));
    assert_eq!(cell(&r, "Purchase Accounts"), dr(rs(60_000)));
    assert_eq!(cell(&r, "Direct Expenses"), dr(rs(5_000)));
    assert_eq!(cell(&r, "Sales Accounts"), cr(rs(100_000)));
    assert_eq!(cell(&r, "Closing stock"), cr(rs(30_000)));
    assert_eq!(cell(&r, "Indirect Expenses"), dr(rs(10_000)));
    assert_eq!(cell(&r, "Indirect Incomes"), cr(rs(2_000)));
}

#[test]
fn closing_stock_comes_from_the_injected_valuation() {
    // provenance: derived — closing stock is a credit in the trading account, so raising it by
    // 7,500.00 raises GP by exactly 7,500.00: 45,000 + 7,500 = 52,500 = 5,250,000 paise.
    // Two items are summed: 20,000 + 10,000 = 30,000 gives the base case; 37,500 gives +7,500.
    let books = books_with(hand_postings());
    let close = Outcome::Value(vec![
        valuation(
            1,
            d(2026, Month::September, 30),
            rs(25_000),
            ValuationStatus::Final,
        ),
        valuation(
            2,
            d(2026, Month::September, 30),
            rs(12_500),
            ValuationStatus::Final,
        ),
    ]);
    let r = profit_loss(
        &books,
        &params(Detail::Condensed),
        &opening_stock(rs(20_000)),
        &close,
    )
    .unwrap()
    .value()
    .unwrap();
    assert_eq!(
        cell(&r, "Gross profit"),
        Cell::Signed(Paise::new(5_250_000))
    );
}

#[test]
fn opening_stock_comes_from_the_injected_valuation() {
    // provenance: derived — opening stock is a debit in the trading account; opening 25,000
    // instead of 20,000 lowers GP by 5,000: 45,000 - 5,000 = 40,000 = 4,000,000 paise.
    let r = run(
        &books_with(hand_postings()),
        &params(Detail::Condensed),
        rs(25_000),
        rs(30_000),
    );
    assert_eq!(
        cell(&r, "Gross profit"),
        Cell::Signed(Paise::new(4_000_000))
    );
}

#[test]
fn gross_loss_is_negative() {
    // provenance: derived — Sales 10,000 + Closing 0 - Opening 0 - Purchases 25,000
    // - Direct expenses 0 = -15,000.00 = -1,500,000 paise (a gross loss).
    let day = d(2026, Month::May, 2);
    let reg = VoucherStatus::Regular;
    let books = books_with(vec![
        posting(1, SALES_L, day, SignedAmount::cr(rs(10_000)), reg),
        posting(2, PURCHASES_L, day, SignedAmount::dr(rs(25_000)), reg),
    ]);
    let r = run(&books, &params(Detail::Condensed), rs(0), rs(0));
    assert_eq!(
        cell(&r, "Gross profit"),
        Cell::Signed(Paise::new(-1_500_000))
    );
    // No indirect items: net equals gross.
    assert_eq!(r.totals, vec![Cell::Signed(Paise::new(-1_500_000))]);
}

#[test]
fn direct_incomes_are_on_the_credit_side_of_trading() {
    // provenance: derived — a direct income (group flagged affects-gross-profit, nature Income)
    // is a credit of the trading account, like sales. Sales 50,000 + Direct income 4,000
    // - Purchases 30,000 = 24,000.00 = 2,400,000 paise.
    let day = d(2026, Month::May, 2);
    let reg = VoucherStatus::Regular;
    let books = books_with(vec![
        posting(1, SALES_L, day, SignedAmount::cr(rs(50_000)), reg),
        posting(2, COMMISSION_L, day, SignedAmount::cr(rs(4_000)), reg),
        posting(3, PURCHASES_L, day, SignedAmount::dr(rs(30_000)), reg),
    ]);
    let r = run(&books, &params(Detail::Condensed), rs(0), rs(0));
    assert_eq!(
        cell(&r, "Gross profit"),
        Cell::Signed(Paise::new(2_400_000))
    );
}

#[test]
fn postings_outside_the_period_are_excluded() {
    // provenance: derived — flows are net movements within the period (inclusive dates).
    // In period: sales 1,000 on 1 Apr and 2,000 on 30 Sep. Outside: 5,000 on 31 Mar and
    // 7,000 on 1 Oct. GP = 1,000 + 2,000 = 3,000.00 = 300,000 paise.
    let reg = VoucherStatus::Regular;
    let books = books_with(vec![
        posting(
            1,
            SALES_L,
            d(2026, Month::April, 1),
            SignedAmount::cr(rs(1_000)),
            reg,
        ),
        posting(
            2,
            SALES_L,
            d(2026, Month::September, 30),
            SignedAmount::cr(rs(2_000)),
            reg,
        ),
        posting(
            3,
            SALES_L,
            d(2026, Month::March, 31),
            SignedAmount::cr(rs(5_000)),
            reg,
        ),
        posting(
            4,
            SALES_L,
            d(2026, Month::October, 1),
            SignedAmount::cr(rs(7_000)),
            reg,
        ),
    ]);
    let r = run(&books, &params(Detail::Condensed), rs(0), rs(0));
    assert_eq!(cell(&r, "Gross profit"), Cell::Signed(Paise::new(300_000)));
}

#[test]
fn scenario_actual_excludes_optional_cancelled_and_future_post_dated() {
    // provenance: derived — spec invariant 3 and Scenario::Actual doc: regular vouchers plus
    // post-dated ones on or before the as-of date (30 Sep 2026). Included: regular 1,000,
    // post-dated 10 Sep 2,000 and post-dated 30 Sep 500. Excluded: optional 4,000 and
    // cancelled 8,000. GP = 1,000 + 2,000 + 500 = 3,500.00 = 350,000 paise.
    let day = d(2026, Month::August, 1);
    let books = books_with(vec![
        posting(
            1,
            SALES_L,
            day,
            SignedAmount::cr(rs(1_000)),
            VoucherStatus::Regular,
        ),
        posting(
            2,
            SALES_L,
            d(2026, Month::September, 10),
            SignedAmount::cr(rs(2_000)),
            VoucherStatus::PostDated,
        ),
        posting(
            3,
            SALES_L,
            day,
            SignedAmount::cr(rs(4_000)),
            VoucherStatus::Optional,
        ),
        posting(
            4,
            SALES_L,
            day,
            SignedAmount::cr(rs(8_000)),
            VoucherStatus::Cancelled,
        ),
        posting(
            5,
            SALES_L,
            d(2026, Month::September, 30),
            SignedAmount::cr(rs(500)),
            VoucherStatus::PostDated,
        ),
    ]);
    let r = run(&books, &params(Detail::Condensed), rs(0), rs(0));
    assert_eq!(cell(&r, "Gross profit"), Cell::Signed(Paise::new(350_000)));
}

#[test]
fn post_dated_after_as_of_is_excluded_even_inside_the_period() {
    // provenance: derived — Scenario::Actual includes post-dated vouchers only on or before the
    // as-of date. Report as of 31 Aug 2026 over the period to 30 Sep: the post-dated 10 Sep
    // sale of 2,000 is excluded; the regular 1,000 stays. GP = 1,000.00 = 100,000 paise.
    let reg = VoucherStatus::Regular;
    let books = books_with(vec![
        posting(
            1,
            SALES_L,
            d(2026, Month::August, 1),
            SignedAmount::cr(rs(1_000)),
            reg,
        ),
        posting(
            2,
            SALES_L,
            d(2026, Month::September, 10),
            SignedAmount::cr(rs(2_000)),
            VoucherStatus::PostDated,
        ),
    ]);
    let mut p = params(Detail::Condensed);
    p.as_of = d(2026, Month::August, 31);
    let r = run(&books, &p, rs(0), rs(0));
    assert_eq!(cell(&r, "Gross profit"), Cell::Signed(Paise::new(100_000)));
}

#[test]
fn scenario_including_optional_adds_optional_postings() {
    // provenance: derived — Scenario::Including(statuses) is Actual plus those statuses.
    // Regular 1,000 + Optional 4,000 = 5,000.00 = 500,000 paise.
    let day = d(2026, Month::August, 1);
    let books = books_with(vec![
        posting(
            1,
            SALES_L,
            day,
            SignedAmount::cr(rs(1_000)),
            VoucherStatus::Regular,
        ),
        posting(
            2,
            SALES_L,
            day,
            SignedAmount::cr(rs(4_000)),
            VoucherStatus::Optional,
        ),
    ]);
    let mut p = params(Detail::Condensed);
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let r = run(&books, &p, rs(0), rs(0));
    assert_eq!(cell(&r, "Gross profit"), Cell::Signed(Paise::new(500_000)));
}

#[test]
fn condensed_shows_primary_groups_only_detailed_shows_ledgers() {
    // provenance: derived — spec 6.2 / Detail docs: Condensed = primary groups only,
    // Detailed = down to ledgers. Export Sales is a sub-group of Sales Accounts.
    let day = d(2026, Month::June, 1);
    let reg = VoucherStatus::Regular;
    let books = books_with(vec![
        posting(1, SALES_L, day, SignedAmount::cr(rs(3_000)), reg),
        posting(2, EXPORT_SALES_L, day, SignedAmount::cr(rs(2_000)), reg),
    ]);
    let cond = run(&books, &params(Detail::Condensed), rs(0), rs(0));
    assert!(cond.rows.iter().all(|x| x.kind != RowKind::Ledger));
    assert!(cond.rows.iter().all(|x| x.label != "Export Sales"));
    // Sales Accounts is Cr 3,000 + 2,000 = 5,000 in both views.
    let cr5k = Cell::Amount(SignedAmount::cr(rs(5_000)));
    assert_eq!(cell(&cond, "Sales Accounts"), cr5k);

    let det = run(&books, &params(Detail::Detailed), rs(0), rs(0));
    assert_eq!(cell(&det, "Sales Accounts"), cr5k);
    assert_eq!(
        cell(&det, "Sales"),
        Cell::Amount(SignedAmount::cr(rs(3_000)))
    );
    assert_eq!(
        cell(&det, "Export Sales"),
        Cell::Amount(SignedAmount::cr(rs(2_000)))
    );
    assert_eq!(
        cell(&det, "Export Sales A/c"),
        Cell::Amount(SignedAmount::cr(rs(2_000)))
    );
    // Depth: group 0, sub-group 1, ledger under the sub-group 2, ledger directly under group 1.
    assert_eq!(row(&det, "Sales Accounts").depth, 0);
    assert_eq!(row(&det, "Export Sales").depth, 1);
    assert_eq!(row(&det, "Export Sales A/c").depth, 2);
    assert_eq!(row(&det, "Sales").depth, 1);
    // Detail does not change the profit.
    assert_eq!(cell(&cond, "Gross profit"), cell(&det, "Gross profit"));
}

#[test]
fn unsupported_stock_makes_the_report_unsupported() {
    // provenance: derived — rule A1: if closing stock cannot be valued, GP cannot be stated;
    // the report must not fall back to zero stock.
    let books = books_with(hand_postings());
    let unsupported: Outcome<Vec<StockValuation>> =
        Outcome::unsupported("valuation: standard cost", "no standard cost entered");
    let out = profit_loss(
        &books,
        &params(Detail::Condensed),
        &opening_stock(rs(0)),
        &unsupported,
    )
    .unwrap();
    assert!(out.is_unsupported());
    let out = profit_loss(
        &books,
        &params(Detail::Condensed),
        &unsupported,
        &closing_stock(rs(0)),
    )
    .unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn provisional_stock_marks_profit_provisional() {
    // provenance: derived — spec 6.4: valuation over negative stock is Provisional and is never
    // silently final. A figure computed from it must carry the reason.
    // Numbers as the hand set: GP 45,000 (profit shown as a credit balance), NP 37,000.
    let books = books_with(hand_postings());
    let reason = ProvisionalReason::NegativeStock {
        item: StockItemId(1),
        date: d(2026, Month::July, 4),
    };
    let close = Outcome::Value(vec![valuation(
        1,
        d(2026, Month::September, 30),
        rs(30_000),
        ValuationStatus::Provisional(reason),
    )]);
    let r = profit_loss(
        &books,
        &params(Detail::Condensed),
        &opening_stock(rs(20_000)),
        &close,
    )
    .unwrap()
    .value()
    .unwrap();
    for (label, expect) in [
        ("Closing stock", rs(30_000)),
        ("Gross profit", rs(45_000)),
        ("Net profit", rs(37_000)),
    ] {
        match cell(&r, label) {
            Cell::Provisional { amount, reason } => {
                assert!(reason.contains("negative stock"), "{reason}");
                assert_eq!(amount.amount, expect, "{label}");
                if label != "Closing stock" {
                    assert_eq!(amount.side, DrCr::Cr, "profit is a credit balance");
                }
            }
            other => panic!("{label}: expected Provisional, got {other:?}"),
        }
    }
    assert!(matches!(&r.totals[0], Cell::Provisional { .. }));
    assert_eq!(r.notes.len(), 2);
    // Opening stock was final, so it stays final.
    assert!(matches!(cell(&r, "Opening stock"), Cell::Amount(_)));
}

#[test]
fn stock_valued_at_the_wrong_date_is_an_error() {
    // provenance: derived — opening stock is the valuation at the day before the period starts
    // (31 Mar 2026) and closing is the valuation at the period end (30 Sep 2026); a valuation
    // dated otherwise would be a wrong number, so it is a typed error.
    let books = books_with(hand_postings());
    let wrong = Outcome::Value(vec![valuation(
        1,
        d(2026, Month::September, 29),
        rs(1),
        ValuationStatus::Final,
    )]);
    assert!(
        profit_loss(
            &books,
            &params(Detail::Condensed),
            &opening_stock(rs(0)),
            &wrong
        )
        .is_err()
    );
    assert!(
        profit_loss(
            &books,
            &params(Detail::Condensed),
            &wrong,
            &closing_stock(rs(0))
        )
        .is_err()
    );
}

#[test]
fn comparative_monthly_filters_and_exceptions_are_unsupported() {
    // provenance: derived — rule A1: a parameter this report does not implement must yield
    // Unsupported, never be ignored (the figures would then answer a different question).
    let books = books_with(hand_postings());
    let call = |p: &ReportParams| {
        profit_loss(&books, p, &opening_stock(rs(0)), &closing_stock(rs(0))).unwrap()
    };
    let mut p = params(Detail::Condensed);
    p.columns = ColumnSpec::Monthly;
    assert!(call(&p).is_unsupported());
    let mut p = params(Detail::Condensed);
    p.columns = ColumnSpec::Comparative(vec![period()]);
    assert!(call(&p).is_unsupported());
    let mut p = params(Detail::Condensed);
    p.filter.ledgers = vec![LedgerId(SALES_L)];
    assert!(call(&p).is_unsupported());
    let mut p = params(Detail::Condensed);
    p.exceptions_only = true;
    assert!(call(&p).is_unsupported());
}

#[test]
fn wrong_report_kind_is_an_error() {
    // provenance: derived — calling the P&L with another report's parameters is a caller bug.
    let books = books_with(hand_postings());
    let mut p = params(Detail::Condensed);
    p.kind = ReportKind::TrialBalance;
    assert!(profit_loss(&books, &p, &opening_stock(rs(0)), &closing_stock(rs(0))).is_err());
}

#[test]
fn opening_balance_on_an_income_or_expense_ledger_is_unsupported() {
    // provenance: derived — spec 6.1: P&L ledgers carry no balance between years, and a non-zero
    // opening on one has no defined period split; we refuse rather than guess.
    let mut books = books_with(hand_postings());
    if let Some(l) = books.masters.ledgers.get_mut(&LedgerId(RENT_L)) {
        l.opening = SignedAmount::dr(rs(1));
    }
    let out = profit_loss(
        &books,
        &params(Detail::Condensed),
        &opening_stock(rs(0)),
        &closing_stock(rs(0)),
    )
    .unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn gross_profit_flag_on_a_balance_sheet_group_is_unsupported() {
    // provenance: derived — a group of nature Asset cannot sit in the trading account; the
    // conflicting flag is reported, not resolved silently (rule A1).
    let mut books = books_with(hand_postings());
    if let Some(g) = books.masters.groups.get_mut(&GroupId(CURRENT_ASSETS_G)) {
        g.affects_gross_profit = true;
    }
    let out = profit_loss(
        &books,
        &params(Detail::Condensed),
        &opening_stock(rs(0)),
        &closing_stock(rs(0)),
    )
    .unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn posting_to_an_unknown_ledger_is_an_error() {
    // provenance: derived — rule A1: a posting whose ledger does not exist is corrupt data and
    // must surface as a typed error.
    let mut postings = hand_postings();
    postings.push(posting(
        99,
        999,
        d(2026, Month::June, 1),
        SignedAmount::cr(rs(1)),
        VoucherStatus::Regular,
    ));
    let books = books_with(postings);
    assert!(
        profit_loss(
            &books,
            &params(Detail::Condensed),
            &opening_stock(rs(0)),
            &closing_stock(rs(0))
        )
        .is_err()
    );
}

#[test]
fn overflow_is_an_error_not_a_wrapped_number() {
    // provenance: derived — rule A3: checked arithmetic. Two credits of i64::MAX paise to Sales
    // cannot be summed.
    let day = d(2026, Month::June, 1);
    let reg = VoucherStatus::Regular;
    let books = books_with(vec![
        posting(1, SALES_L, day, SignedAmount::cr(Paise::new(i64::MAX)), reg),
        posting(2, SALES_L, day, SignedAmount::cr(Paise::new(i64::MAX)), reg),
    ]);
    assert!(
        profit_loss(
            &books,
            &params(Detail::Condensed),
            &opening_stock(rs(0)),
            &closing_stock(rs(0))
        )
        .is_err()
    );
}

#[test]
fn empty_books_give_zero_profit() {
    // provenance: derived — no postings and no stock: every term of the formula is 0, GP = 0.
    let r = run(
        &books_with(vec![]),
        &params(Detail::Condensed),
        rs(0),
        rs(0),
    );
    assert_eq!(cell(&r, "Gross profit"), Cell::Signed(Paise::ZERO));
    assert_eq!(r.totals, vec![Cell::Signed(Paise::ZERO)]);
    assert_eq!(r.columns.len(), 1);
}

proptest! {
    #[test]
    fn profit_obeys_the_formula_for_any_amounts(
        sales in 0i64..1_000_000_000,
        purchases in 0i64..1_000_000_000,
        direct in 0i64..1_000_000_000,
        opening in 0i64..1_000_000_000,
        closing in 0i64..1_000_000_000,
        ind_exp in 0i64..1_000_000_000,
        ind_inc in 0i64..1_000_000_000,
    ) {
        // provenance: derived — spec 6.2 identity, evaluated independently in plain i64:
        // GP = sales + closing - opening - purchases - direct; NP = GP + ind_inc - ind_exp.
        let day = d(2026, Month::June, 1);
        let reg = VoucherStatus::Regular;
        let p = |n, l, a| posting(n, l, day, a, reg);
        let books = books_with(vec![
            p(1, SALES_L, SignedAmount::cr(Paise::new(sales))),
            p(2, PURCHASES_L, SignedAmount::dr(Paise::new(purchases))),
            p(3, FREIGHT_L, SignedAmount::dr(Paise::new(direct))),
            p(4, RENT_L, SignedAmount::dr(Paise::new(ind_exp))),
            p(5, INTEREST_L, SignedAmount::cr(Paise::new(ind_inc))),
        ]);
        let r = run(
            &books,
            &params(Detail::Detailed),
            Paise::new(opening),
            Paise::new(closing),
        );
        let gp = sales + closing - opening - purchases - direct;
        let np = gp + ind_inc - ind_exp;
        prop_assert_eq!(cell(&r, "Gross profit"), Cell::Signed(Paise::new(gp)));
        prop_assert_eq!(cell(&r, "Net profit"), Cell::Signed(Paise::new(np)));
    }
}

// ---- Stock-in-Hand ledger opening balance as opening stock (session 12) ----
//
// Source: TallyPrime help, "Stock Items - FAQ", "How to enter the opening and closing stock
// manually?" (https://help.tallysolutions.com/stock-items-faq/, read 2026-11-25): create a
// Stock-in-Hand ledger and enter the opening stock amount as its opening balance; the manually
// entered opening and closing stock balances are displayed in the Profit & Loss account. And
// "Accounting - FAQ" (https://help.tallysolutions.com/tally-prime/accounting-and-inventory/accounting-faq/,
// read 2026-11-25): accounts under Stock-in-Hand cannot be used in any transactions, so only their
// opening and closing balances are maintained.

const STOCK_G: u64 = 9;
const STOCK_L: u64 = 18;

fn books_with_stock_in_hand(postings: Vec<Posting>, opening: SignedAmount) -> Books {
    let mut books = books_with(postings);
    let mut g = group(
        STOCK_G,
        "Stock-in-Hand",
        Some(CURRENT_ASSETS_G),
        None,
        false,
    );
    g.reserved = Some(ReservedGroup::StockInHand);
    books.masters.groups.insert(g.id, g);
    let mut l = ledger(STOCK_L, "Opening stock", STOCK_G);
    l.opening = opening;
    books.masters.ledgers.insert(l.id, l);
    books
}

#[test]
fn a_stock_in_hand_ledger_opening_is_opening_stock_for_a_period_from_the_books_begin() {
    // provenance: derived — the hand set (GP 45,000 with opening 20,000, closing 30,000) with a
    // Stock-in-Hand ledger opening of 5,000 added to an injected opening valuation of 20,000:
    // opening stock 25,000, so GP = 100,000 + 30,000 - 25,000 - 60,000 - 5,000 = 40,000.00
    // rupees = 4,000,000 paise (the same figure as `opening_stock_comes_from_the_injected_valuation`).
    let books = books_with_stock_in_hand(hand_postings(), SignedAmount::dr(rs(5_000)));
    let r = run(&books, &params(Detail::Condensed), rs(20_000), rs(30_000));
    assert_eq!(
        cell(&r, "Opening stock"),
        Cell::Amount(SignedAmount::dr(rs(25_000)))
    );
    assert_eq!(
        cell(&r, "Gross profit"),
        Cell::Signed(Paise::new(4_000_000))
    );
}

#[test]
fn a_stock_in_hand_ledger_opening_alone_is_the_opening_stock_row() {
    // provenance: derived — opening 9,000 on the ledger, nothing injected: the Opening stock row
    // is 9,000.00 = 900,000 paise and GP = 100,000 + 0 - 9,000 - 60,000 - 5,000 = 26,000.00
    // = 2,600,000 paise.
    let books = books_with_stock_in_hand(hand_postings(), SignedAmount::dr(rs(9_000)));
    let r = run(&books, &params(Detail::Condensed), rs(0), rs(0));
    assert_eq!(
        cell(&r, "Opening stock"),
        Cell::Amount(SignedAmount::dr(rs(9_000)))
    );
    assert_eq!(
        cell(&r, "Gross profit"),
        Cell::Signed(Paise::new(2_600_000))
    );
}

#[test]
fn a_stock_in_hand_opening_is_unsupported_for_a_period_after_the_books_begin() {
    // provenance: derived — the ledger's opening is the stock on hand when the books began. The
    // opening stock of a later period is what is on hand then, which the ledger's start-of-books
    // balance does not say, so the report must not show either figure.
    let books = books_with_stock_in_hand(hand_postings(), SignedAmount::dr(rs(9_000)));
    let mut p = params(Detail::Condensed);
    p.period = Period::new(d(2026, Month::July, 1), d(2026, Month::September, 30)).unwrap();
    let open = Outcome::Value(vec![valuation(
        1,
        d(2026, Month::June, 30),
        rs(0),
        ValuationStatus::Final,
    )]);
    let out = profit_loss(&books, &p, &open, &closing_stock(rs(0))).unwrap();
    assert!(out.is_unsupported(), "{out:?}");
}

#[test]
fn a_zero_stock_in_hand_ledger_does_not_stop_a_later_period() {
    // provenance: derived — a Stock-in-Hand ledger opening at nil adds nothing to opening stock
    // in any period, so a period after the books begin is still computed: Opening 0, GP as the
    // hand set with no opening stock and no closing stock: 100,000 - 60,000 - 5,000 = 35,000.
    let books = books_with_stock_in_hand(hand_postings(), SignedAmount::dr(Paise::ZERO));
    let mut p = params(Detail::Condensed);
    p.period = Period::new(d(2026, Month::April, 1), d(2026, Month::September, 30)).unwrap();
    let r = run(&books, &p, rs(0), rs(0));
    assert_eq!(
        cell(&r, "Gross profit"),
        Cell::Signed(Paise::new(3_500_000))
    );
}

#[test]
fn a_posting_to_a_stock_in_hand_ledger_is_unsupported() {
    // provenance: derived — accounts under Stock-in-Hand cannot be used in transactions (help
    // page above), so a posting there is not a stock figure this report can place.
    let mut postings = hand_postings();
    postings.push(posting(
        8,
        STOCK_L,
        d(2026, Month::June, 15),
        SignedAmount::dr(rs(1_000)),
        VoucherStatus::Regular,
    ));
    let books = books_with_stock_in_hand(postings, SignedAmount::dr(rs(9_000)));
    let out = profit_loss(
        &books,
        &params(Detail::Condensed),
        &opening_stock(rs(0)),
        &closing_stock(rs(0)),
    )
    .unwrap();
    assert!(out.is_unsupported(), "{out:?}");
}

#[test]
fn a_credit_opening_on_a_stock_in_hand_ledger_is_unsupported() {
    // provenance: derived — stock on hand cannot be negative value; a credit opening is not an
    // opening stock figure and must not be shown as one.
    let books = books_with_stock_in_hand(hand_postings(), SignedAmount::cr(rs(9_000)));
    let out = profit_loss(
        &books,
        &params(Detail::Condensed),
        &opening_stock(rs(0)),
        &closing_stock(rs(0)),
    )
    .unwrap();
    assert!(out.is_unsupported(), "{out:?}");
}
