//! s9: the balance sheet's layout over a chart with the reserved groups. Hand-built books; every
//! expected figure is worked out in a comment.
//!
//! Chart (ids differ from the presentation order on purpose):
//!
//! ```text
//! 1 Current Assets (reserved, asset)       2 Current Liabilities (reserved, liability)
//! 3 Fixed Assets (reserved, asset)         4 Loans (reserved, liability)
//! 5 Capital Account (reserved, liability)  6 Stock-in-Hand (under 1)   7 Cash-in-Hand (under 1)
//! 8 Sales Accounts (income)
//! ledgers: 20 Cash (7), 21 Furniture (3), 22 Term loan (4), 23 Partner capital (5), 24 Sales (8)
//! ```
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, DrCr, GroupId, LedgerId, Month, Outcome, Paise, Period, Qty, SignedAmount,
    StateCode, StockItemId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, Masters, Nature,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, Posting, ReservedGroup, RestartPeriod,
    Scenario, VoucherStatus, VoucherType,
};
use ledgerly_reports::balance_sheet::balance_sheet;
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams, RowKind,
};
use ledgerly_stock::{ProvisionalReason, StockValuation, ValuationStatus};

fn d(day: u8) -> Date {
    Date::from_calendar_date(2026, Month::September, day).unwrap()
}

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn group(
    id: u64,
    name: &str,
    parent: Option<u64>,
    reserved: Option<ReservedGroup>,
    nature: Option<Nature>,
) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: name.into(),
        aliases: vec![],
        parent: parent.map(GroupId),
        reserved,
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

fn books() -> Books {
    use ReservedGroup as R;
    let mut m = Masters::default();
    for g in [
        group(1, "Current Assets", None, Some(R::CurrentAssets), Some(Nature::Asset)),
        group(2, "Current Liabilities", None, Some(R::CurrentLiabilities), Some(Nature::Liability)),
        group(3, "Fixed Assets", None, Some(R::FixedAssets), Some(Nature::Asset)),
        group(4, "Loans (Liability)", None, Some(R::Loans), Some(Nature::Liability)),
        group(5, "Capital Account", None, Some(R::CapitalAccount), Some(Nature::Liability)),
        group(6, "Stock-in-Hand", Some(1), Some(R::StockInHand), None),
        group(7, "Cash-in-Hand", Some(1), Some(R::CashInHand), None),
        group(8, "Sales Accounts", None, Some(R::SalesAccounts), Some(Nature::Income)),
    ] {
        m.groups.insert(g.id, g);
    }
    for l in [
        ledger(20, "Cash", 7),
        ledger(21, "Furniture", 3),
        ledger(22, "Term loan", 4),
        ledger(23, "Partner capital", 5),
        ledger(24, "Sales", 8),
    ] {
        m.ledgers.insert(l.id, l);
    }
    m.voucher_types.insert(
        VoucherTypeId(1),
        VoucherType {
            id: VoucherTypeId(1),
            name: "Journal".into(),
            base: BaseVoucherKind::Journal,
            abbreviation: "Jrnl".into(),
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
        },
    );
    let date = Date::from_calendar_date(2026, Month::April, 1).unwrap();
    // 1: Dr Cash 10,00,000 Cr Capital. 2: Dr Furniture 2,00,000 Cr Loan. 3: Dr Cash 50,000 Cr Sales.
    let pair = |n: u64, dr: u64, cr: u64, amount: Paise| {
        vec![
            Posting {
                voucher: VoucherId(n),
                date,
                ledger: LedgerId(dr),
                amount: SignedAmount::dr(amount),
                status: VoucherStatus::Regular,
                type_id: VoucherTypeId(1),
            },
            Posting {
                voucher: VoucherId(n),
                date,
                ledger: LedgerId(cr),
                amount: SignedAmount::cr(amount),
                status: VoucherStatus::Regular,
                type_id: VoucherTypeId(1),
            },
        ]
    };
    let mut postings = pair(1, 20, 23, rs(1_000_000));
    postings.extend(pair(2, 21, 22, rs(200_000)));
    postings.extend(pair(3, 20, 24, rs(50_000)));
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Fictional Traders".into(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: vec![],
            fy_start: date,
            books_begin: date,
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Warn,
        },
        masters: m,
        vouchers: BTreeMap::new(),
        postings,
        stock_moves: vec![],
    }
}

fn params(detail: Detail) -> ReportParams {
    ReportParams {
        kind: ReportKind::BalanceSheet,
        period: Period::new(Date::from_calendar_date(2026, Month::April, 1).unwrap(), d(30))
            .unwrap(),
        as_of: d(30),
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn closing(value: Paise, status: ValuationStatus) -> Outcome<Vec<StockValuation>> {
    Outcome::Value(vec![StockValuation {
        item: StockItemId(1),
        as_of: d(30),
        qty: Qty::ZERO,
        value,
        rate: None,
        status,
    }])
}

fn run(detail: Detail, stock: &Outcome<Vec<StockValuation>>) -> Report {
    balance_sheet(&books(), &params(detail), stock)
        .unwrap()
        .value()
        .expect("supported")
}

fn labels(r: &Report) -> Vec<(&str, u8)> {
    r.rows.iter().map(|x| (x.label.as_str(), x.depth)).collect()
}

fn amount(c: &Cell) -> (DrCr, Paise) {
    match c {
        Cell::Amount(a) | Cell::Provisional { amount: a, .. } => (a.side, a.amount),
        other => panic!("not an amount: {other:?}"),
    }
}

#[test]
fn closing_stock_is_a_row_inside_current_assets_and_in_its_total() {
    // provenance: derived (arithmetic below); the nesting is TallyPrime's chart, in which
    // Stock-in-Hand is a sub-group of Current Assets (help.tallysolutions.com Balance Sheet page
    // names Stock-in-Hand among the groups, read 2026-11-25; the parent is from the reserved
    // group list in ledgerly-model seed_groups). Cash = 10,00,000 + 50,000 = 10,50,000 Dr.
    // Closing stock 30,000. Current Assets = 10,50,000 + 30,000 = 10,80,000 Dr. Fixed Assets
    // 2,00,000 Dr. Profit = sales 50,000 + closing stock 30,000 = 80,000 Cr. Capital 10,00,000
    // Cr, Loans 2,00,000 Cr. Liabilities: 10,00,000 + 2,00,000 + 80,000 = 12,80,000. Assets:
    // 10,80,000 + 2,00,000 = 12,80,000.
    let r = run(Detail::Detailed, &closing(rs(30_000), ValuationStatus::Final));
    assert_eq!(
        labels(&r),
        vec![
            ("Capital Account", 0),
            ("Partner capital", 1),
            ("Loans (Liability)", 0),
            ("Term loan", 1),
            ("Profit and Loss A/c", 0),
            ("Fixed Assets", 0),
            ("Furniture", 1),
            ("Current Assets", 0),
            ("Closing stock", 1),
            ("Cash-in-Hand", 1),
            ("Cash", 2),
        ]
    );
    let by = |name: &str| r.rows.iter().find(|x| x.label == name).unwrap();
    assert_eq!(amount(&by("Current Assets").cells[1]), (DrCr::Dr, rs(1_080_000)));
    assert_eq!(amount(&by("Closing stock").cells[1]), (DrCr::Dr, rs(30_000)));
    assert_eq!(by("Closing stock").kind, RowKind::Item);
    assert_eq!(amount(&by("Profit and Loss A/c").cells[0]), (DrCr::Cr, rs(80_000)));
    assert_eq!(amount(&r.totals[0]), (DrCr::Cr, rs(1_280_000)));
    assert_eq!(amount(&r.totals[1]), (DrCr::Dr, rs(1_280_000)));
}

#[test]
fn primary_groups_follow_the_chart_order_not_the_id_order() {
    // provenance: derived. The mock's order (index.html:664-681 `balanceSheet`: Capital, Loans,
    // Current liabilities, then Fixed assets, Current assets). TallyPrime makes group order a
    // setting (balance sheet help page, read 2026-11-25), so this is the default. Ids here are
    // Current Assets 1, Fixed Assets 3, Loans 4, Capital 5, the reverse of that order.
    let r = run(Detail::Condensed, &closing(rs(30_000), ValuationStatus::Final));
    assert_eq!(
        labels(&r),
        vec![
            ("Capital Account", 0),
            ("Loans (Liability)", 0),
            ("Profit and Loss A/c", 0),
            ("Fixed Assets", 0),
            ("Current Assets", 0),
        ]
    );
    // The stock is inside Current Assets even when its row is not listed.
    let ca = r.rows.iter().find(|x| x.label == "Current Assets").unwrap();
    assert_eq!(amount(&ca.cells[1]), (DrCr::Dr, rs(1_080_000)));
}

#[test]
fn provisional_stock_marks_current_assets_too() {
    // provenance: derived. Current Assets includes the stock, so a provisional stock makes the
    // group figure provisional (a figure never looks final when part of it is not).
    let stock = closing(
        rs(30_000),
        ValuationStatus::Provisional(ProvisionalReason::NegativeStock {
            item: StockItemId(1),
            date: d(10),
        }),
    );
    let r = run(Detail::Condensed, &stock);
    let ca = r.rows.iter().find(|x| x.label == "Current Assets").unwrap();
    assert!(matches!(ca.cells[1], Cell::Provisional { .. }), "{:?}", ca.cells);
}
