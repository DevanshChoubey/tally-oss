//! Card RP5: ledger vouchers and group summary tests. Hand-built books; every expected figure is
//! derived in a comment (provenance: derived, plain double-entry arithmetic from the postings).
//!
//! Chart (all names fictional):
//!
//! ```text
//! 2 Current Assets (primary, Asset)
//!   3 Cash-in-Hand        ledger 11 Cash        opening Dr 1,000
//!   4 Sundry Debtors      ledger 12 Customer One  opening Dr 100
//!                         ledger 14 Customer Two  opening nil
//! 5 Sales Accounts (primary) ledger 13 Sales
//! ```
//!
//! `rs(n)` is `n` rupees.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, DrCr, GroupId, LedgerId, Outcome, Paise, Period, SignedAmount, StateCode,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, Masters, Nature,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, Posting, ReservedLedger, RestartPeriod,
    Scenario, VoucherStatus, VoucherType,
};
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportKind, ReportParams, RowKind,
    ledger::{group_summary, ledger_vouchers},
};
use time::macros::date;

const CASH: u64 = 11;
const CUST1: u64 = 12;
const SALES: u64 = 13;
const CUST2: u64 = 14;
const JOURNAL: u64 = 1;
const MEMO: u64 = 2;

fn rs(n: i64) -> Paise {
    Paise::new(n * 100)
}

fn group(id: u64, name: &str, parent: Option<u64>, nature: Option<Nature>) -> Group {
    Group {
        id: GroupId(id),
        name: name.to_string(),
        aliases: vec![],
        parent: parent.map(GroupId),
        reserved: None,
        nature,
        affects_gross_profit: false,
        tally: None,
    }
}

fn ledger(id: u64, name: &str, group: u64, opening: SignedAmount) -> Ledger {
    Ledger {
        id: LedgerId(id),
        name: name.to_string(),
        aliases: vec![],
        group: GroupId(group),
        opening,
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

fn books() -> Books {
    let mut masters = Masters::default();
    for g in [
        group(2, "Current Assets", None, Some(Nature::Asset)),
        group(3, "Cash-in-Hand", Some(2), None),
        group(4, "Sundry Debtors", Some(2), None),
        group(5, "Sales Accounts", None, Some(Nature::Income)),
    ] {
        masters.groups.insert(g.id, g);
    }
    for l in [
        ledger(CASH, "Cash", 3, SignedAmount::dr(rs(1_000))),
        ledger(CUST1, "Customer One", 4, SignedAmount::dr(rs(100))),
        ledger(SALES, "Sales", 5, SignedAmount::dr(Paise::ZERO)),
        ledger(CUST2, "Customer Two", 4, SignedAmount::dr(Paise::ZERO)),
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(JOURNAL, "Journal", BaseVoucherKind::Journal),
        voucher_type(MEMO, "Memorandum", BaseVoucherKind::Memorandum),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
    Books {
        company: Company {
            id: CompanyId(1),
            name: "Test Traders".to_string(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: vec![],
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: vec![],
        stock_moves: vec![],
    }
}

/// One two-line voucher: Dr `dr`, Cr `cr`, `amount`.
#[expect(clippy::too_many_arguments, reason = "flat test helper")]
fn voucher(
    b: &mut Books,
    id: u64,
    on: Date,
    dr: u64,
    cr: u64,
    amount: Paise,
    status: VoucherStatus,
    type_id: u64,
) {
    for (ledger, side) in [(dr, DrCr::Dr), (cr, DrCr::Cr)] {
        b.postings.push(Posting {
            voucher: VoucherId(id),
            date: on,
            ledger: LedgerId(ledger),
            amount: SignedAmount { amount, side },
            status,
            type_id: VoucherTypeId(type_id),
        });
    }
}

fn regular(b: &mut Books, id: u64, on: Date, dr: u64, cr: u64, amount: Paise) {
    voucher(b, id, on, dr, cr, amount, VoucherStatus::Regular, JOURNAL);
}

fn params(kind: ReportKind, from: Date, to: Date) -> ReportParams {
    ReportParams {
        kind,
        period: Period::new(from, to).unwrap(),
        as_of: to,
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ledgerly_reports::ReportFilter::default(),
        exceptions_only: false,
    }
}

fn lv_params(ledger: u64, from: Date, to: Date) -> ReportParams {
    params(ReportKind::LedgerVouchers(LedgerId(ledger)), from, to)
}

fn lv(b: &Books, p: &ReportParams, ledger: u64) -> Report {
    match ledger_vouchers(b, p, LedgerId(ledger)).unwrap() {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn gs(b: &Books, p: &ReportParams, group: u64) -> Report {
    match group_summary(b, p, GroupId(group)).unwrap() {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

/// The balance cell (last column) of row `i` as (amount, side).
fn bal(r: &Report, i: usize) -> (Paise, DrCr) {
    match &r.rows[i].cells[6] {
        Cell::Amount(a) => (a.amount, a.side),
        other => panic!("balance cell is {other:?}"),
    }
}

fn amount(c: &Cell) -> Option<SignedAmount> {
    match c {
        Cell::Amount(a) => Some(*a),
        Cell::Empty => None,
        Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Provisional { .. }
        | Cell::Unsupported(_) => panic!("unexpected cell {c:?}"),
    }
}

fn unsupported_what(r: Result<Outcome<Report>, ReportError>) -> String {
    match r.unwrap() {
        Outcome::Unsupported(u) => u.what,
        Outcome::Value(_) => panic!("expected Unsupported"),
    }
}

/// Cash: opening Dr 1,000 (1 Apr). Vouchers on Cash:
/// 5 Apr Dr 500, 10 Apr Cr 200, 20 Apr Dr 50.
fn cash_books() -> Books {
    let mut b = books();
    regular(&mut b, 1, date!(2026 - 04 - 05), CASH, CUST1, rs(500));
    regular(&mut b, 2, date!(2026 - 04 - 10), CUST1, CASH, rs(200));
    regular(&mut b, 3, date!(2026 - 04 - 20), CASH, SALES, rs(50));
    b
}

#[test]
fn running_balance_per_line() {
    // provenance: derived — running balance = opening + sum of signed lines so far (Dr positive).
    // 1,000 Dr; +500 -> 1,500 Dr; -200 -> 1,300 Dr; +50 -> 1,350 Dr.
    let b = cash_books();
    let p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = lv(&b, &p, CASH);
    // Opening, 3 vouchers, closing.
    assert_eq!(r.rows.len(), 5);
    assert_eq!(bal(&r, 0), (rs(1_000), DrCr::Dr));
    assert_eq!(bal(&r, 1), (rs(1_500), DrCr::Dr));
    assert_eq!(bal(&r, 2), (rs(1_300), DrCr::Dr));
    assert_eq!(bal(&r, 3), (rs(1_350), DrCr::Dr));
    assert_eq!(bal(&r, 4), (rs(1_350), DrCr::Dr));
    assert_eq!(r.rows[1].key, Some(DrillKey::Voucher(VoucherId(1))));
    assert_eq!(r.rows[1].kind, RowKind::Voucher);
    // Debit total 500 + 50 = 550; credit total 200.
    assert_eq!(amount(&r.totals[4]), Some(SignedAmount::dr(rs(550))));
    assert_eq!(amount(&r.totals[5]), Some(SignedAmount::cr(rs(200))));
    assert_eq!(amount(&r.totals[6]), Some(SignedAmount::dr(rs(1_350))));
    // Debit and credit columns of a line.
    assert_eq!(amount(&r.rows[1].cells[4]), Some(SignedAmount::dr(rs(500))));
    assert_eq!(amount(&r.rows[1].cells[5]), None);
    assert_eq!(amount(&r.rows[2].cells[5]), Some(SignedAmount::cr(rs(200))));
    assert_eq!(amount(&r.rows[2].cells[4]), None);
}

#[test]
fn opening_row_is_ledger_opening_at_period_start() {
    // provenance: derived — opening of a period = ledger opening + included lines dated before it.
    // Period from 10 Apr: 1,000 + 500 (5 Apr) = 1,500 Dr. Lines in period: -200 -> 1,300; +50 -> 1,350.
    let b = cash_books();
    let p = lv_params(CASH, date!(2026 - 04 - 10), date!(2026 - 04 - 30));
    let r = lv(&b, &p, CASH);
    assert_eq!(r.rows.len(), 4);
    assert_eq!(r.rows[0].label, "Opening Balance");
    assert_eq!(bal(&r, 0), (rs(1_500), DrCr::Dr));
    assert_eq!(bal(&r, 1), (rs(1_300), DrCr::Dr));
    assert_eq!(bal(&r, 2), (rs(1_350), DrCr::Dr));
    assert_eq!(r.rows[3].label, "Closing Balance");
}

#[test]
fn opening_row_with_no_activity_is_the_master_opening() {
    // provenance: derived — no postings: opening = closing = ledger opening (Dr 1,000).
    let b = books();
    let p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = lv(&b, &p, CASH);
    assert_eq!(r.rows.len(), 2);
    assert_eq!(bal(&r, 0), (rs(1_000), DrCr::Dr));
    assert_eq!(bal(&r, 1), (rs(1_000), DrCr::Dr));
}

#[test]
fn side_switches_when_balance_crosses_zero() {
    // provenance: derived — Customer One opening Dr 100.
    // Cr 250 -> 100 - 250 = -150 = 150 Cr; Dr 150 -> 0; Dr 30 -> 30 Dr.
    let mut b = books();
    regular(&mut b, 1, date!(2026 - 04 - 02), SALES, CUST1, rs(250));
    regular(&mut b, 2, date!(2026 - 04 - 03), CUST1, SALES, rs(150));
    regular(&mut b, 3, date!(2026 - 04 - 04), CUST1, SALES, rs(30));
    let p = lv_params(CUST1, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = lv(&b, &p, CUST1);
    assert_eq!(bal(&r, 0), (rs(100), DrCr::Dr));
    assert_eq!(bal(&r, 1), (rs(150), DrCr::Cr));
    assert_eq!(bal(&r, 2).0, Paise::ZERO);
    assert_eq!(bal(&r, 3), (rs(30), DrCr::Dr));
    assert_eq!(bal(&r, 4), (rs(30), DrCr::Dr));
}

#[test]
fn opening_credit_balance_is_shown_on_the_credit_side() {
    // provenance: derived — a credit opening of 40 is -40 signed; Dr 100 -> +60 Dr.
    let mut b = books();
    b.masters.ledgers.get_mut(&LedgerId(CUST2)).unwrap().opening = SignedAmount::cr(rs(40));
    regular(&mut b, 1, date!(2026 - 04 - 02), CUST2, SALES, rs(100));
    let p = lv_params(CUST2, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = lv(&b, &p, CUST2);
    assert_eq!(bal(&r, 0), (rs(40), DrCr::Cr));
    assert_eq!(bal(&r, 1), (rs(60), DrCr::Dr));
}

#[test]
fn particulars_name_the_other_ledgers_and_the_type() {
    let b = cash_books();
    let p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = lv(&b, &p, CASH);
    // Voucher 1 posts Dr Cash / Cr Customer One; voucher 3 posts Dr Cash / Cr Sales.
    assert_eq!(r.rows[1].label, "Customer One");
    assert_eq!(r.rows[3].label, "Sales");
    assert_eq!(r.rows[1].cells[2], Cell::Text("Journal".to_string()));
    assert_eq!(r.rows[1].cells[0], Cell::Date(date!(2026 - 04 - 05)));
    // No voucher record in the books: number is empty, not invented.
    assert_eq!(r.rows[1].cells[3], Cell::Empty);
}

#[test]
fn postings_after_the_period_are_ignored_and_before_it_fold_into_opening() {
    // provenance: derived — 20 Apr line is outside 1-9 Apr, so closing = 1,000 + 500 = 1,500 Dr.
    let b = cash_books();
    let p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 09));
    let r = lv(&b, &p, CASH);
    assert_eq!(r.rows.len(), 3);
    assert_eq!(bal(&r, 2), (rs(1_500), DrCr::Dr));
}

#[test]
fn optional_cancelled_and_memorandum_vouchers_stay_out_of_the_actual_scenario() {
    // provenance: derived — spec invariant 3: only regular and post-dated vouchers are in the
    // books; the cancelled voucher has no amounts. Only the regular +500 counts: 1,500 Dr.
    let mut b = books();
    regular(&mut b, 1, date!(2026 - 04 - 05), CASH, CUST1, rs(500));
    voucher(
        &mut b,
        2,
        date!(2026 - 04 - 06),
        CASH,
        SALES,
        rs(70),
        VoucherStatus::Optional,
        JOURNAL,
    );
    voucher(
        &mut b,
        3,
        date!(2026 - 04 - 07),
        CASH,
        SALES,
        rs(80),
        VoucherStatus::Cancelled,
        JOURNAL,
    );
    voucher(
        &mut b,
        4,
        date!(2026 - 04 - 08),
        CASH,
        SALES,
        rs(90),
        VoucherStatus::Regular,
        MEMO,
    );
    let p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let r = lv(&b, &p, CASH);
    assert_eq!(r.rows.len(), 3);
    assert_eq!(bal(&r, 2), (rs(1_500), DrCr::Dr));
}

#[test]
fn optional_vouchers_count_when_the_scenario_includes_them() {
    // provenance: derived — 1,000 + 500 + 70 (optional) = 1,570 Dr.
    let mut b = books();
    regular(&mut b, 1, date!(2026 - 04 - 05), CASH, CUST1, rs(500));
    voucher(
        &mut b,
        2,
        date!(2026 - 04 - 06),
        CASH,
        SALES,
        rs(70),
        VoucherStatus::Optional,
        JOURNAL,
    );
    let mut p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let r = lv(&b, &p, CASH);
    assert_eq!(bal(&r, 3), (rs(1_570), DrCr::Dr));
}

#[test]
fn named_scenario_over_memorandum_is_unsupported() {
    let mut b = books();
    voucher(
        &mut b,
        4,
        date!(2026 - 04 - 08),
        CASH,
        SALES,
        rs(90),
        VoucherStatus::Regular,
        MEMO,
    );
    let mut p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let what = unsupported_what(ledger_vouchers(&b, &p, LedgerId(CASH)));
    assert!(what.contains("named scenario"), "{what}");
}

#[test]
fn comparative_filter_and_exceptions_are_unsupported() {
    let b = cash_books();
    let base = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let mut comparative = base.clone();
    comparative.columns = ColumnSpec::Monthly;
    assert!(
        unsupported_what(ledger_vouchers(&b, &comparative, LedgerId(CASH))).contains("columns")
    );
    let mut filtered = base.clone();
    filtered.filter.voucher_types.push(VoucherTypeId(JOURNAL));
    assert!(unsupported_what(ledger_vouchers(&b, &filtered, LedgerId(CASH))).contains("filters"));
    let mut exceptions = base;
    exceptions.exceptions_only = true;
    assert!(
        unsupported_what(ledger_vouchers(&b, &exceptions, LedgerId(CASH)))
            .contains("exceptions only")
    );
}

#[test]
fn profit_and_loss_ledger_is_unsupported() {
    let mut b = books();
    b.masters
        .ledgers
        .get_mut(&LedgerId(SALES))
        .unwrap()
        .reserved = Some(ReservedLedger::ProfitAndLoss);
    let p = lv_params(SALES, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    let what = unsupported_what(ledger_vouchers(&b, &p, LedgerId(SALES)));
    assert!(what.contains("Profit and Loss"), "{what}");
}

#[test]
fn errors_for_wrong_kind_missing_ledger_and_period_before_books() {
    let b = cash_books();
    let p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    assert!(matches!(
        ledger_vouchers(&b, &p, LedgerId(CUST1)),
        Err(ReportError::Invalid { .. })
    ));
    let p99 = lv_params(99, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    assert!(matches!(
        ledger_vouchers(&b, &p99, LedgerId(99)),
        Err(ReportError::NotFound { .. })
    ));
    let early = lv_params(CASH, date!(2026 - 01 - 01), date!(2026 - 03 - 31));
    assert!(matches!(
        ledger_vouchers(&b, &early, LedgerId(CASH)),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn posting_before_books_begin_is_an_error() {
    let mut b = books();
    regular(&mut b, 1, date!(2026 - 03 - 15), CASH, SALES, rs(10));
    let p = lv_params(CASH, date!(2026 - 04 - 01), date!(2026 - 04 - 30));
    assert!(matches!(
        ledger_vouchers(&b, &p, LedgerId(CASH)),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn group_summary_rolls_up_one_level_down() {
    // provenance: derived — as of 30 Apr, Current Assets (group 2):
    //   Cash-in-Hand: Cash 1,000 + 500 - 200 + 50 = 1,350 Dr.
    //   Sundry Debtors: Customer One 100 + (Cr 500 on 5 Apr) - 0 ... voucher 1 is Dr Cash / Cr Cust1
    //   so Customer One = 100 - 500 + 200 = -200 = 200 Cr; Customer Two nil. Debtors net 200 Cr.
    //   Group net = 1,350 - 200 = 1,150 Dr. Dr column total 1,350, Cr column total 200.
    let b = cash_books();
    let p = params(
        ReportKind::GroupSummary(GroupId(2)),
        date!(2026 - 04 - 01),
        date!(2026 - 04 - 30),
    );
    let r = gs(&b, &p, 2);
    assert_eq!(r.rows.len(), 3);
    assert_eq!(r.rows[0].label, "Cash-in-Hand");
    assert_eq!(r.rows[0].key, Some(DrillKey::Group(GroupId(3))));
    assert_eq!(
        amount(&r.rows[0].cells[0]),
        Some(SignedAmount::dr(rs(1_350)))
    );
    assert_eq!(r.rows[1].label, "Sundry Debtors");
    assert_eq!(amount(&r.rows[1].cells[1]), Some(SignedAmount::cr(rs(200))));
    assert_eq!(r.rows[2].kind, RowKind::Subtotal);
    assert_eq!(
        amount(&r.rows[2].cells[0]),
        Some(SignedAmount::dr(rs(1_150)))
    );
    assert_eq!(amount(&r.totals[0]), Some(SignedAmount::dr(rs(1_350))));
    assert_eq!(amount(&r.totals[1]), Some(SignedAmount::cr(rs(200))));
}

#[test]
fn group_summary_lists_ledgers_of_a_leaf_group_and_hides_nil_rows() {
    // provenance: derived — group 4 as of 30 Apr: Customer One = 200 Cr (see above),
    // Customer Two nil and left out. Net 200 Cr.
    let b = cash_books();
    let p = params(
        ReportKind::GroupSummary(GroupId(4)),
        date!(2026 - 04 - 01),
        date!(2026 - 04 - 30),
    );
    let r = gs(&b, &p, 4);
    assert_eq!(r.rows.len(), 2);
    assert_eq!(r.rows[0].label, "Customer One");
    assert_eq!(r.rows[0].key, Some(DrillKey::Ledger(LedgerId(CUST1))));
    assert_eq!(amount(&r.rows[0].cells[1]), Some(SignedAmount::cr(rs(200))));
    assert_eq!(amount(&r.rows[1].cells[1]), Some(SignedAmount::cr(rs(200))));
}

#[test]
fn group_summary_respects_as_of() {
    // provenance: derived — as of 7 Apr only voucher 1 (5 Apr) counts: Cash 1,500 Dr.
    let b = cash_books();
    let p = params(
        ReportKind::GroupSummary(GroupId(3)),
        date!(2026 - 04 - 01),
        date!(2026 - 04 - 07),
    );
    let r = gs(&b, &p, 3);
    assert_eq!(
        amount(&r.rows[0].cells[0]),
        Some(SignedAmount::dr(rs(1_500)))
    );
}

#[test]
fn group_summary_errors_and_unsupported() {
    let b = cash_books();
    let p = params(
        ReportKind::GroupSummary(GroupId(2)),
        date!(2026 - 04 - 01),
        date!(2026 - 04 - 30),
    );
    assert!(matches!(
        group_summary(&b, &p, GroupId(3)),
        Err(ReportError::Invalid { .. })
    ));
    let p99 = params(
        ReportKind::GroupSummary(GroupId(99)),
        date!(2026 - 04 - 01),
        date!(2026 - 04 - 30),
    );
    assert!(matches!(
        group_summary(&b, &p99, GroupId(99)),
        Err(ReportError::NotFound { .. })
    ));
    let mut monthly = p.clone();
    monthly.columns = ColumnSpec::Monthly;
    assert!(unsupported_what(group_summary(&b, &monthly, GroupId(2))).contains("columns"));
}
