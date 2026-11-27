//! Card RP6: day book and register tests. Hand-built books; every expected figure is derived in a
//! comment. Ledgers: 11 Cash, 12 Customer One, 13 Sales, 14 GST Payable. Voucher types: 1 Sales,
//! 2 Journal, 3 Memorandum, 4 Sales Order.
//!
//! Ordering rule (derived from the card: "day book ordered by date then entry seq"): a voucher
//! has no sequence field, voucher ids are allocated in entry order, so the id is the sequence.
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
    CompanyId, Date, Decimal, LedgerId, Outcome, Paise, Period, Qty, SignedAmount, StateCode,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Ledger, LedgerLine, Masters,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, RestartPeriod, Scenario, Voucher,
    VoucherMode, VoucherStatus, VoucherType,
};
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, RowKind,
    registers::{day_book, register},
};
use time::macros::date;

const CASH: u64 = 11;
const CUSTOMER: u64 = 12;
const SALES: u64 = 13;
const GST: u64 = 14;

const SALES_TYPE: u64 = 1;
const JOURNAL_TYPE: u64 = 2;
const MEMO_TYPE: u64 = 3;
const ORDER_TYPE: u64 = 4;

fn rs(n: i64) -> Paise {
    Paise::new(n * 100)
}

fn ledger(id: u64, name: &str) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_string(),
        aliases: vec![],
        group: Some(ledgerly_core::GroupId(1)),
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

fn empty_books() -> Books {
    let mut masters = Masters::default();
    for l in [
        ledger(CASH, "Cash"),
        ledger(CUSTOMER, "Customer One"),
        ledger(SALES, "Sales"),
        ledger(GST, "GST Payable"),
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(SALES_TYPE, "Sales", BaseVoucherKind::Sales),
        voucher_type(JOURNAL_TYPE, "Journal", BaseVoucherKind::Journal),
        voucher_type(MEMO_TYPE, "Memorandum", BaseVoucherKind::Memorandum),
        voucher_type(ORDER_TYPE, "Sales Order", BaseVoucherKind::SalesOrder),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
    Books {
        company: Company {
            ineligible_itc_default: None,
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

fn line(ledger: u64, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger: LedgerId(ledger),
        amount,
        bills: vec![],
        cost: vec![],
        bank: None,
    }
}

fn voucher(
    id: u64,
    type_id: u64,
    on: Date,
    status: VoucherStatus,
    lines: Vec<LedgerLine>,
) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(type_id),
        number: Some(format!("V{id}")),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: lines,
        inventory_lines: vec![],
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

/// Sales invoice: Dr Customer One `total`, Cr Sales `total - tax`, Cr GST Payable `tax`.
fn sale(id: u64, on: Date, total: i64, tax: i64) -> Voucher {
    let mut v = voucher(
        id,
        SALES_TYPE,
        on,
        VoucherStatus::Regular,
        vec![
            line(CUSTOMER, SignedAmount::dr(rs(total))),
            line(SALES, SignedAmount::cr(rs(total - tax))),
            line(GST, SignedAmount::cr(rs(tax))),
        ],
    );
    v.party = Some(LedgerId(CUSTOMER));
    v
}

/// Journal: Dr Cash, Cr Sales, same amount.
fn journal(id: u64, on: Date, amount: i64) -> Voucher {
    voucher(
        id,
        JOURNAL_TYPE,
        on,
        VoucherStatus::Regular,
        vec![
            line(CASH, SignedAmount::dr(rs(amount))),
            line(SALES, SignedAmount::cr(rs(amount))),
        ],
    )
}

fn with(mut b: Books, vouchers: Vec<Voucher>) -> Books {
    for v in vouchers {
        b.vouchers.insert(v.id, v);
    }
    b
}

fn params(kind: ReportKind, columns: ColumnSpec) -> ReportParams {
    ReportParams {
        kind,
        period: Period::new(date!(2026 - 04 - 01), date!(2026 - 06 - 30)).unwrap(),
        as_of: date!(2026 - 06 - 30),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn day_params() -> ReportParams {
    params(ReportKind::DayBook, ColumnSpec::Single)
}

fn sales_params(columns: ColumnSpec) -> ReportParams {
    params(ReportKind::Register(BaseVoucherKind::Sales), columns)
}

fn value(outcome: Result<Outcome<Report>, ReportError>) -> Report {
    match outcome.unwrap() {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn voucher_ids(report: &Report) -> Vec<u64> {
    report
        .rows
        .iter()
        .map(|r| match r.key {
            Some(DrillKey::Voucher(id)) => id.0,
            Some(_) | None => panic!("row without a voucher key: {}", r.label),
        })
        .collect()
}

fn amount(cell: &Cell) -> Paise {
    match cell {
        Cell::Amount(a) => a.amount,
        Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Empty
        | Cell::Provisional { .. }
        | Cell::Unsupported(_) => panic!("not an amount: {cell:?}"),
    }
}

fn count(cell: &Cell) -> Decimal {
    match cell {
        Cell::Qty(Qty(d)) => *d,
        Cell::Amount(_)
        | Cell::Signed(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Empty
        | Cell::Provisional { .. }
        | Cell::Unsupported(_) => panic!("not a count: {cell:?}"),
    }
}

#[test]
fn day_book_ordered_by_date_then_entry_seq() {
    // provenance: derived — card RP6 rule "date then entry seq"; ids are entry order.
    // Vouchers: id 3 on 5 Apr, id 1 on 10 Apr, id 2 on 5 Apr, id 4 on 1 Apr.
    // Sorted by (date, id): 1 Apr -> 4; 5 Apr -> 2, 3; 10 Apr -> 1.
    let b = with(
        empty_books(),
        vec![
            journal(3, date!(2026 - 04 - 05), 100),
            journal(1, date!(2026 - 04 - 10), 200),
            journal(2, date!(2026 - 04 - 05), 300),
            journal(4, date!(2026 - 04 - 01), 400),
        ],
    );
    let r = value(day_book(&b, &day_params()));
    assert_eq!(voucher_ids(&r), vec![4, 2, 3, 1]);
    // provenance: derived — total = 100 + 200 + 300 + 400 = 1,000 rupees.
    assert_eq!(amount(&r.totals[3]), rs(1_000));
    assert_eq!(r.rows[0].kind, RowKind::Voucher);
    assert_eq!(r.rows[0].cells.len(), r.columns.len());
    assert_eq!(r.totals.len(), r.columns.len());
}

#[test]
fn day_book_excludes_vouchers_outside_the_period() {
    // provenance: derived — period is 1 Apr to 30 Jun 2026 inclusive; 31 Mar and 1 Jul are outside.
    let mut b = with(
        empty_books(),
        vec![
            journal(1, date!(2026 - 04 - 01), 100),
            journal(2, date!(2026 - 06 - 30), 200),
            journal(3, date!(2026 - 07 - 01), 300),
        ],
    );
    b.company.books_begin = date!(2026 - 03 - 01);
    b.vouchers
        .insert(VoucherId(4), journal(4, date!(2026 - 03 - 31), 400));
    let r = value(day_book(&b, &day_params()));
    assert_eq!(voucher_ids(&r), vec![1, 2]);
    // provenance: derived — 100 + 200 = 300 rupees.
    assert_eq!(amount(&r.totals[3]), rs(300));
}

#[test]
fn row_label_is_party_else_first_ledger_and_amount_is_debit_total() {
    // provenance: derived — sale: Dr 11,800 = Cr 10,000 + Cr 1,800; amount is the debit sum 11,800.
    // The journal has no party; its first line is Cash.
    let b = with(
        empty_books(),
        vec![
            sale(1, date!(2026 - 04 - 02), 11_800, 1_800),
            journal(2, date!(2026 - 04 - 03), 500),
        ],
    );
    let r = value(day_book(&b, &day_params()));
    assert_eq!(r.rows[0].label, "Customer One");
    assert_eq!(amount(&r.rows[0].cells[3]), rs(11_800));
    assert_eq!(r.rows[0].cells[1], Cell::Text("Sales".to_string()));
    assert_eq!(r.rows[0].cells[2], Cell::Text("V1".to_string()));
    assert_eq!(r.rows[1].label, "Cash");
    assert_eq!(r.rows[1].cells[0], Cell::Date(date!(2026 - 04 - 03)));
}

#[test]
fn cancelled_vouchers_listed_with_zero_amount_and_flagged() {
    // provenance: derived — spec section 5: a cancelled voucher keeps its number, carries no
    // amounts. The cancelled voucher 2 has no lines; the total is the regular sale only.
    let cancelled = voucher(
        2,
        SALES_TYPE,
        date!(2026 - 04 - 05),
        VoucherStatus::Cancelled,
        vec![],
    );
    let b = with(
        empty_books(),
        vec![sale(1, date!(2026 - 04 - 02), 1_180, 180), cancelled],
    );
    let r = value(day_book(&b, &day_params()));
    assert_eq!(voucher_ids(&r), vec![1, 2]);
    assert_eq!(amount(&r.rows[1].cells[3]), Paise::ZERO);
    assert_eq!(r.rows[1].cells[4], Cell::Text("Cancelled".to_string()));
    assert_eq!(r.rows[1].cells[2], Cell::Text("V2".to_string()));
    assert_eq!(r.rows[0].cells[4], Cell::Empty);
    assert_eq!(amount(&r.totals[3]), rs(1_180));
}

#[test]
fn cancelled_voucher_with_lines_is_an_error() {
    // provenance: derived — a cancelled voucher has no amounts (spec section 5); lines mean the
    // stored data is corrupt, so the report refuses rather than show zero.
    let bad = voucher(
        1,
        SALES_TYPE,
        date!(2026 - 04 - 05),
        VoucherStatus::Cancelled,
        vec![line(CASH, SignedAmount::dr(rs(1)))],
    );
    let b = with(empty_books(), vec![bad]);
    assert!(matches!(
        day_book(&b, &day_params()),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn sales_register_monthly_totals() {
    // provenance: derived — sales vouchers: Apr 11,800 and 5,900; May 2,360; June none.
    // April = 11,800 + 5,900 = 17,700 over 2 vouchers; May = 2,360 over 1; June = 0 over 0.
    // Total = 17,700 + 2,360 = 20,060 over 3 vouchers. A journal of 999 in April, an optional
    // sale of 777 in May and a sale of 555 on 1 Jul are not in the register. One cancelled
    // sale in May counts in the Cancelled column only.
    let optional = {
        let mut v = sale(5, date!(2026 - 05 - 20), 777, 0);
        v.status = VoucherStatus::Optional;
        v
    };
    let cancelled = voucher(
        6,
        SALES_TYPE,
        date!(2026 - 05 - 21),
        VoucherStatus::Cancelled,
        vec![],
    );
    let b = with(
        empty_books(),
        vec![
            sale(1, date!(2026 - 04 - 10), 11_800, 1_800),
            sale(2, date!(2026 - 04 - 30), 5_900, 900),
            sale(3, date!(2026 - 05 - 01), 2_360, 360),
            journal(4, date!(2026 - 04 - 15), 999),
            optional,
            cancelled,
            sale(7, date!(2026 - 07 - 01), 555, 0),
        ],
    );
    let r = value(register(
        &b,
        &sales_params(ColumnSpec::Monthly),
        BaseVoucherKind::Sales,
    ));
    let labels: Vec<&str> = r.rows.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(labels, vec!["Apr 2026", "May 2026", "Jun 2026"]);
    assert_eq!(count(&r.rows[0].cells[0]), Decimal::from(2));
    assert_eq!(amount(&r.rows[0].cells[2]), rs(17_700));
    assert_eq!(count(&r.rows[1].cells[0]), Decimal::from(1));
    assert_eq!(count(&r.rows[1].cells[1]), Decimal::from(1));
    assert_eq!(amount(&r.rows[1].cells[2]), rs(2_360));
    assert_eq!(count(&r.rows[2].cells[0]), Decimal::from(0));
    assert_eq!(amount(&r.rows[2].cells[2]), Paise::ZERO);
    assert_eq!(count(&r.totals[0]), Decimal::from(3));
    assert_eq!(count(&r.totals[1]), Decimal::from(1));
    assert_eq!(amount(&r.totals[2]), rs(20_060));
    assert_eq!(r.columns.len(), 3);
    assert_eq!(r.title, "Sales Register");
}

#[test]
fn register_lists_only_its_kind_in_single_layout() {
    // provenance: derived — the sales register lists vouchers whose type's base is Sales.
    let b = with(
        empty_books(),
        vec![
            sale(1, date!(2026 - 04 - 10), 118, 18),
            journal(2, date!(2026 - 04 - 11), 50),
            sale(3, date!(2026 - 04 - 12), 236, 36),
        ],
    );
    let r = value(register(
        &b,
        &sales_params(ColumnSpec::Single),
        BaseVoucherKind::Sales,
    ));
    assert_eq!(voucher_ids(&r), vec![1, 3]);
    // provenance: derived — 118 + 236 = 354 rupees.
    assert_eq!(amount(&r.totals[3]), rs(354));
}

#[test]
fn memorandum_left_out_of_day_book_but_in_its_own_register() {
    // provenance: derived — spec invariant 3: memorandum vouchers never touch the books, so the
    // actual-scenario day book skips them; the memorandum register lists them on request.
    let memo = voucher(
        2,
        MEMO_TYPE,
        date!(2026 - 04 - 05),
        VoucherStatus::Regular,
        vec![
            line(CASH, SignedAmount::dr(rs(70))),
            line(SALES, SignedAmount::cr(rs(70))),
        ],
    );
    let b = with(
        empty_books(),
        vec![journal(1, date!(2026 - 04 - 04), 10), memo],
    );
    let day = value(day_book(&b, &day_params()));
    assert_eq!(voucher_ids(&day), vec![1]);
    let reg = value(register(
        &b,
        &params(
            ReportKind::Register(BaseVoucherKind::Memorandum),
            ColumnSpec::Single,
        ),
        BaseVoucherKind::Memorandum,
    ));
    assert_eq!(voucher_ids(&reg), vec![2]);
    assert_eq!(amount(&reg.totals[3]), rs(70));
}

#[test]
fn named_scenario_over_memorandum_is_unsupported() {
    // provenance: derived — no rule says how a named scenario treats memorandum vouchers; the
    // trial balance (RP2) returns Unsupported for the same input and so does this report.
    let memo = voucher(
        2,
        MEMO_TYPE,
        date!(2026 - 04 - 05),
        VoucherStatus::Regular,
        vec![
            line(CASH, SignedAmount::dr(rs(70))),
            line(SALES, SignedAmount::cr(rs(70))),
        ],
    );
    let b = with(empty_books(), vec![memo]);
    let mut p = day_params();
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    assert!(day_book(&b, &p).unwrap().is_unsupported());
}

#[test]
fn optional_and_post_dated_follow_the_scenario() {
    // provenance: derived — spec invariant 3 as implemented by the profit and loss report:
    // optional only when the scenario names it; post-dated when on/before as_of or named.
    let mut optional = journal(1, date!(2026 - 04 - 05), 10);
    optional.status = VoucherStatus::Optional;
    let mut post_early = journal(2, date!(2026 - 06 - 30), 20);
    post_early.status = VoucherStatus::PostDated;
    let mut post_late = journal(3, date!(2026 - 06 - 30), 40);
    post_late.status = VoucherStatus::PostDated;
    let b = with(empty_books(), vec![optional, post_early, post_late]);

    let mut p = day_params();
    p.as_of = date!(2026 - 06 - 29);
    let actual = value(day_book(&b, &p));
    assert_eq!(actual.rows, [] as [ledgerly_reports::ReportRow; 0]);

    p.as_of = date!(2026 - 06 - 30);
    let on_date = value(day_book(&b, &p));
    assert_eq!(voucher_ids(&on_date), vec![2, 3]);
    assert_eq!(
        on_date.rows[0].cells[4],
        Cell::Text("Post-dated".to_string())
    );
    // provenance: derived — 20 + 40 = 60 rupees.
    assert_eq!(amount(&on_date.totals[3]), rs(60));

    p.as_of = date!(2026 - 06 - 29);
    p.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let with_optional = value(day_book(&b, &p));
    assert_eq!(voucher_ids(&with_optional), vec![1]);
    assert_eq!(
        with_optional.rows[0].cells[4],
        Cell::Text("Optional".to_string())
    );
}

#[test]
fn voucher_type_and_ledger_filters_narrow_the_list() {
    // provenance: derived — filter on voucher type 1 keeps vouchers 1 and 3; filter on ledger
    // Customer One keeps the vouchers with a line on it (1 and 3, not the journal 2).
    let b = with(
        empty_books(),
        vec![
            sale(1, date!(2026 - 04 - 10), 118, 18),
            journal(2, date!(2026 - 04 - 11), 50),
            sale(3, date!(2026 - 04 - 12), 236, 36),
        ],
    );
    let mut p = day_params();
    p.filter.voucher_types = vec![VoucherTypeId(SALES_TYPE)];
    assert_eq!(voucher_ids(&value(day_book(&b, &p))), vec![1, 3]);
    let mut p = day_params();
    p.filter.ledgers = vec![LedgerId(CUSTOMER)];
    assert_eq!(voucher_ids(&value(day_book(&b, &p))), vec![1, 3]);
    let mut p = day_params();
    p.filter.ledgers = vec![LedgerId(CASH)];
    assert_eq!(voucher_ids(&value(day_book(&b, &p))), vec![2]);
}

#[test]
fn order_voucher_amount_is_unsupported_not_zero() {
    // provenance: derived — a sales order has no accounting lines, so the books hold no amount for
    // it; showing 0 would be a wrong number. Its cell and the total are Unsupported.
    let order = voucher(
        2,
        ORDER_TYPE,
        date!(2026 - 04 - 05),
        VoucherStatus::Regular,
        vec![],
    );
    let b = with(
        empty_books(),
        vec![journal(1, date!(2026 - 04 - 04), 10), order],
    );
    let r = value(day_book(&b, &day_params()));
    assert_eq!(amount(&r.rows[0].cells[3]), rs(10));
    assert!(matches!(r.rows[1].cells[3], Cell::Unsupported(_)));
    assert!(matches!(r.totals[3], Cell::Unsupported(_)));
    // A monthly register of that kind is unsupported as a whole.
    let p = params(
        ReportKind::Register(BaseVoucherKind::SalesOrder),
        ColumnSpec::Monthly,
    );
    assert!(
        register(&b, &p, BaseVoucherKind::SalesOrder)
            .unwrap()
            .is_unsupported()
    );
}

#[test]
fn unsupported_parameters_are_reported_not_guessed() {
    // provenance: derived — the module doc lists these as not computed.
    let b = with(empty_books(), vec![journal(1, date!(2026 - 04 - 04), 10)]);
    let comparative = params(
        ReportKind::DayBook,
        ColumnSpec::Comparative(vec![day_params().period]),
    );
    assert!(day_book(&b, &comparative).unwrap().is_unsupported());
    let monthly = params(ReportKind::DayBook, ColumnSpec::Monthly);
    assert!(day_book(&b, &monthly).unwrap().is_unsupported());
    let mut exceptions = day_params();
    exceptions.exceptions_only = true;
    assert!(day_book(&b, &exceptions).unwrap().is_unsupported());
    let mut groups = day_params();
    groups.filter.groups = vec![ledgerly_core::GroupId(1)];
    assert!(day_book(&b, &groups).unwrap().is_unsupported());
}

#[test]
fn bad_input_is_an_error() {
    let b = with(empty_books(), vec![journal(1, date!(2026 - 04 - 04), 10)]);
    // Wrong report kind for the engine.
    assert!(matches!(
        day_book(&b, &sales_params(ColumnSpec::Single)),
        Err(ReportError::Invalid { .. })
    ));
    assert!(matches!(
        register(&b, &day_params(), BaseVoucherKind::Sales),
        Err(ReportError::Invalid { .. })
    ));
    // provenance: derived — Dr 10 vs Cr 9 does not balance.
    let unbalanced = voucher(
        2,
        JOURNAL_TYPE,
        date!(2026 - 04 - 05),
        VoucherStatus::Regular,
        vec![
            line(CASH, SignedAmount::dr(rs(10))),
            line(SALES, SignedAmount::cr(rs(9))),
        ],
    );
    let bad = with(empty_books(), vec![unbalanced]);
    assert!(matches!(
        day_book(&bad, &day_params()),
        Err(ReportError::Invalid { .. })
    ));
    // Unknown voucher type.
    let mut orphan = journal(3, date!(2026 - 04 - 05), 1);
    orphan.type_id = VoucherTypeId(99);
    let bad = with(empty_books(), vec![orphan]);
    assert!(matches!(
        day_book(&bad, &day_params()),
        Err(ReportError::NotFound { .. })
    ));
    // Voucher before the books begin.
    let mut early = empty_books();
    early.company.books_begin = date!(2026 - 04 - 10);
    let early = with(early, vec![journal(4, date!(2026 - 04 - 05), 1)]);
    assert!(matches!(
        day_book(&early, &day_params()),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn monthly_register_over_two_financial_years_is_an_error() {
    // provenance: derived — monthly columns need a period inside one financial year (RP1).
    let b = with(empty_books(), vec![]);
    let mut p = sales_params(ColumnSpec::Monthly);
    p.period = Period::new(date!(2026 - 03 - 01), date!(2026 - 06 - 30)).unwrap();
    assert!(matches!(
        register(&b, &p, BaseVoucherKind::Sales),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn rows_drill_to_their_voucher() {
    // provenance: derived — spec 6.2: every figure drills down; a voucher row keys its voucher id.
    let b = with(empty_books(), vec![journal(7, date!(2026 - 04 - 04), 10)]);
    let r = value(day_book(&b, &day_params()));
    assert_eq!(r.rows[0].key, Some(DrillKey::Voucher(VoucherId(7))));
}
