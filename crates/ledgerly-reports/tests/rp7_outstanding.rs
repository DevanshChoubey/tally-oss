//! Card RP7: bills outstanding and ageing.
//!
//! All figures are fictional. Every expected value carries its provenance next to it. Bucket
//! edges are the card's definition (0-30, 31-60, 61-90, over 90 days past the due date, as of the
//! report date); the day counts below are worked out by hand from calendar month lengths.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::wildcard_enum_match_arm,
    clippy::needless_range_loop,
    clippy::too_many_lines,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, GroupId, LedgerId, Month, Outcome, Paise, Period, SignedAmount, StateCode,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Books, Company,
    EntityKind, Features, Group, Ledger, LedgerLine, Masters, Nature, NegativeStockPolicy,
    NumberingConfig, NumberingMethod, ReservedGroup, RestartPeriod, Scenario, Voucher, VoucherMode,
    VoucherStatus, VoucherType,
};
use ledgerly_reports::outstanding::outstanding;
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter, ReportKind,
    ReportParams, ReportRow, RowKind,
};
use proptest::prelude::*;

const G_DEBTORS: GroupId = GroupId(1);
const G_CREDITORS: GroupId = GroupId(2);
const G_SALES: GroupId = GroupId(3);
const G_BANK: GroupId = GroupId(4);
const G_NORTH: GroupId = GroupId(5);

const ASHA: LedgerId = LedgerId(10);
const BHARAT: LedgerId = LedgerId(11);
const CASH_CUSTOMER: LedgerId = LedgerId(12);
const ZED: LedgerId = LedgerId(13);
const DORMANT: LedgerId = LedgerId(14);
const SALES: LedgerId = LedgerId(20);
const BANK: LedgerId = LedgerId(30);

const T_SALES: VoucherTypeId = VoucherTypeId(1);
const T_RECEIPT: VoucherTypeId = VoucherTypeId(2);
const T_PURCHASE: VoucherTypeId = VoucherTypeId(3);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(4);
const T_MEMO: VoucherTypeId = VoucherTypeId(5);

// Column positions in every row and in the totals.
const C_BILL_DATE: usize = 0;
const C_DUE: usize = 1;
const C_PENDING: usize = 2;
const C_NOT_DUE: usize = 3;
const C_0_30: usize = 4;
const C_31_60: usize = 5;
const C_61_90: usize = 6;
const C_OVER_90: usize = 7;
const C_NO_DUE: usize = 8;
const C_ON_ACCOUNT: usize = 9;
const C_NET: usize = 10;

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

fn as_of() -> Date {
    d(2026, Month::September, 30)
}

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn dr(rupees: i64) -> Cell {
    Cell::Amount(SignedAmount::dr(rs(rupees)))
}

fn cr(rupees: i64) -> Cell {
    Cell::Amount(SignedAmount::cr(rs(rupees)))
}

fn group(
    id: GroupId,
    name: &str,
    parent: Option<GroupId>,
    reserved: Option<ReservedGroup>,
) -> Group {
    Group {
        ineligible_itc: None,
        id,
        name: name.into(),
        aliases: vec![],
        parent,
        reserved,
        nature: parent.is_none().then_some(Nature::Asset),
        affects_gross_profit: false,
        tally: None,
    }
}

fn ledger(id: LedgerId, name: &str, group: GroupId, bill_wise: bool) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.into(),
        aliases: vec![],
        group,
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: bill_wise.then_some(BillWiseConfig {
            default_credit_days: None,
        }),
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn voucher_type(id: VoucherTypeId, name: &str, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: name.into(),
        base,
        abbreviation: name.into(),
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
        group(
            G_DEBTORS,
            "Sundry Debtors",
            None,
            Some(ReservedGroup::SundryDebtors),
        ),
        group(
            G_CREDITORS,
            "Sundry Creditors",
            None,
            Some(ReservedGroup::SundryCreditors),
        ),
        group(
            G_SALES,
            "Sales Accounts",
            None,
            Some(ReservedGroup::SalesAccounts),
        ),
        group(
            G_BANK,
            "Bank Accounts",
            None,
            Some(ReservedGroup::BankAccounts),
        ),
        group(G_NORTH, "Northern Debtors", Some(G_DEBTORS), None),
    ] {
        m.groups.insert(g.id, g);
    }
    for l in [
        ledger(ASHA, "Asha Traders", G_DEBTORS, true),
        ledger(BHARAT, "Bharat Stores", G_NORTH, true),
        ledger(CASH_CUSTOMER, "Cash Customer", G_DEBTORS, false),
        ledger(ZED, "Zed Supplies", G_CREDITORS, true),
        ledger(DORMANT, "Dormant Debtor", G_DEBTORS, true),
        ledger(SALES, "Sales", G_SALES, false),
        ledger(BANK, "Bank", G_BANK, false),
    ] {
        m.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(T_SALES, "Sales", BaseVoucherKind::Sales),
        voucher_type(T_RECEIPT, "Receipt", BaseVoucherKind::Receipt),
        voucher_type(T_PURCHASE, "Purchase", BaseVoucherKind::Purchase),
        voucher_type(T_PAYMENT, "Payment", BaseVoucherKind::Payment),
        voucher_type(T_MEMO, "Memorandum", BaseVoucherKind::Memorandum),
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

fn alloc(
    kind: BillRefKind,
    name: &str,
    due: Option<Date>,
    credit_days: Option<u32>,
    amount: SignedAmount,
) -> BillAllocation {
    BillAllocation {
        kind,
        name: name.into(),
        due_date: due,
        credit_days,
        amount,
    }
}

fn line(ledger: LedgerId, amount: SignedAmount, bills: Vec<BillAllocation>) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills,
        cost: vec![],
        bank: None,
    }
}

fn voucher(
    id: u64,
    type_id: VoucherTypeId,
    date: Date,
    status: VoucherStatus,
    lines: Vec<LedgerLine>,
) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
        series: None,
        date,
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

/// A sales voucher opening bill `name` on `party` for `rupees` Dr.
fn sale(
    id: u64,
    date: Date,
    party: LedgerId,
    name: &str,
    rupees: i64,
    due: Option<Date>,
) -> Voucher {
    let amount = SignedAmount::dr(rs(rupees));
    voucher(
        id,
        T_SALES,
        date,
        VoucherStatus::Regular,
        vec![
            line(
                party,
                amount,
                vec![alloc(BillRefKind::New, name, due, None, amount)],
            ),
            line(SALES, SignedAmount::cr(rs(rupees)), vec![]),
        ],
    )
}

/// The hand set. Report date is 30-Sep-2026.
///
/// Asha Traders (debtor) bills, with the days overdue at 30-Sep-2026 worked out by hand:
/// - A-1 1,000 due 15-Oct-2026: not yet due.
/// - A-2 2,000 due 30-Sep-2026: 0 days overdue.
/// - A-3 4,000 due 31-Aug-2026: 31 Aug to 30 Sep is 30 days.
/// - A-4 8,000 due 30-Aug-2026: 31 days.
/// - A-5 16,000 due 01-Aug-2026: 31 days to 01-Sep plus 29 to 30-Sep is 60 days.
/// - A-6 32,000 due 31-Jul-2026: 31 days to 31-Aug plus 30 is 61 days.
/// - A-7 64,000 due 02-Jul-2026: 31 to 02-Aug, 31 to 02-Sep, 28 to 30-Sep is 90 days.
/// - A-8 128,000 due 01-Jul-2026: 91 days.
/// - A-9 256,000 no due date.
///
/// Settlements: receipt 01-Sep-2026 of 33,000 Cr = 28,000 against A-8 + 5,000 on account.
/// Not counted: a receipt dated 05-Oct-2026 (after the report date) settling A-9 in full plus
/// 7,000 on account; an optional sales bill A-OPT 9,999; an optional receipt with 3,000 on
/// account; a memorandum with 4,000 on account.
fn hand_vouchers() -> Vec<Voucher> {
    let bill_day = d(2026, Month::June, 1);
    let mut v = vec![
        sale(
            1,
            bill_day,
            ASHA,
            "A-1",
            1_000,
            Some(d(2026, Month::October, 15)),
        ),
        sale(
            2,
            bill_day,
            ASHA,
            "A-2",
            2_000,
            Some(d(2026, Month::September, 30)),
        ),
        sale(
            3,
            bill_day,
            ASHA,
            "A-3",
            4_000,
            Some(d(2026, Month::August, 31)),
        ),
        sale(
            4,
            bill_day,
            ASHA,
            "A-4",
            8_000,
            Some(d(2026, Month::August, 30)),
        ),
        sale(
            5,
            bill_day,
            ASHA,
            "A-5",
            16_000,
            Some(d(2026, Month::August, 1)),
        ),
        sale(
            6,
            bill_day,
            ASHA,
            "A-6",
            32_000,
            Some(d(2026, Month::July, 31)),
        ),
        sale(
            7,
            bill_day,
            ASHA,
            "A-7",
            64_000,
            Some(d(2026, Month::July, 2)),
        ),
        sale(
            8,
            bill_day,
            ASHA,
            "A-8",
            128_000,
            Some(d(2026, Month::July, 1)),
        ),
        sale(9, bill_day, ASHA, "A-9", 256_000, None),
    ];
    let receipt_amount = SignedAmount::cr(rs(33_000));
    v.push(voucher(
        10,
        T_RECEIPT,
        d(2026, Month::September, 1),
        VoucherStatus::Regular,
        vec![
            line(
                ASHA,
                receipt_amount,
                vec![
                    alloc(
                        BillRefKind::Against,
                        "A-8",
                        None,
                        None,
                        SignedAmount::cr(rs(28_000)),
                    ),
                    alloc(
                        BillRefKind::OnAccount,
                        "",
                        None,
                        None,
                        SignedAmount::cr(rs(5_000)),
                    ),
                ],
            ),
            line(BANK, SignedAmount::dr(rs(33_000)), vec![]),
        ],
    ));
    // After the report date: must not count.
    v.push(voucher(
        11,
        T_RECEIPT,
        d(2026, Month::October, 5),
        VoucherStatus::Regular,
        vec![
            line(
                ASHA,
                SignedAmount::cr(rs(263_000)),
                vec![
                    alloc(
                        BillRefKind::Against,
                        "A-9",
                        None,
                        None,
                        SignedAmount::cr(rs(256_000)),
                    ),
                    alloc(
                        BillRefKind::OnAccount,
                        "",
                        None,
                        None,
                        SignedAmount::cr(rs(7_000)),
                    ),
                ],
            ),
            line(BANK, SignedAmount::dr(rs(263_000)), vec![]),
        ],
    ));
    // Optional vouchers and memoranda: never counted.
    let opt = SignedAmount::dr(rs(9_999));
    v.push(voucher(
        12,
        T_SALES,
        bill_day,
        VoucherStatus::Optional,
        vec![
            line(
                ASHA,
                opt,
                vec![alloc(
                    BillRefKind::New,
                    "A-OPT",
                    Some(d(2026, Month::July, 1)),
                    None,
                    opt,
                )],
            ),
            line(SALES, SignedAmount::cr(rs(9_999)), vec![]),
        ],
    ));
    v.push(voucher(
        13,
        T_RECEIPT,
        bill_day,
        VoucherStatus::Optional,
        vec![
            line(
                ASHA,
                SignedAmount::cr(rs(3_000)),
                vec![alloc(
                    BillRefKind::OnAccount,
                    "",
                    None,
                    None,
                    SignedAmount::cr(rs(3_000)),
                )],
            ),
            line(BANK, SignedAmount::dr(rs(3_000)), vec![]),
        ],
    ));
    v.push(voucher(
        14,
        T_MEMO,
        bill_day,
        VoucherStatus::Regular,
        vec![line(
            ASHA,
            SignedAmount::cr(rs(4_000)),
            vec![alloc(
                BillRefKind::OnAccount,
                "",
                None,
                None,
                SignedAmount::cr(rs(4_000)),
            )],
        )],
    ));
    // Bharat Stores: B-1 50,000 Dr dated 10-Sep-2026 with 30 credit days (due 10-Oct-2026:
    // 20 days to 30-Sep plus 10), and a credit note CN-1 of 3,000 Cr opened as a bill, no due date.
    let b1 = SignedAmount::dr(rs(50_000));
    v.push(voucher(
        15,
        T_SALES,
        d(2026, Month::September, 10),
        VoucherStatus::Regular,
        vec![
            line(
                BHARAT,
                b1,
                vec![alloc(BillRefKind::New, "B-1", None, Some(30), b1)],
            ),
            line(SALES, SignedAmount::cr(rs(50_000)), vec![]),
        ],
    ));
    let cn = SignedAmount::cr(rs(3_000));
    v.push(voucher(
        16,
        T_SALES,
        d(2026, Month::September, 12),
        VoucherStatus::Regular,
        vec![
            line(
                BHARAT,
                cn,
                vec![alloc(BillRefKind::New, "CN-1", None, None, cn)],
            ),
            line(SALES, SignedAmount::dr(rs(3_000)), vec![]),
        ],
    ));
    // Zed Supplies (creditor): P-1 10,000 Cr due 15-Aug-2026, and 2,000 Dr paid on account.
    let p1 = SignedAmount::cr(rs(10_000));
    v.push(voucher(
        17,
        T_PURCHASE,
        d(2026, Month::May, 1),
        VoucherStatus::Regular,
        vec![
            line(
                ZED,
                p1,
                vec![alloc(
                    BillRefKind::New,
                    "P-1",
                    Some(d(2026, Month::August, 15)),
                    None,
                    p1,
                )],
            ),
            line(SALES, SignedAmount::dr(rs(10_000)), vec![]),
        ],
    ));
    v.push(voucher(
        18,
        T_PAYMENT,
        d(2026, Month::June, 1),
        VoucherStatus::Regular,
        vec![
            line(
                ZED,
                SignedAmount::dr(rs(2_000)),
                vec![alloc(
                    BillRefKind::OnAccount,
                    "",
                    None,
                    None,
                    SignedAmount::dr(rs(2_000)),
                )],
            ),
            line(BANK, SignedAmount::cr(rs(2_000)), vec![]),
        ],
    ));
    v
}

fn books_with(vouchers: Vec<Voucher>) -> Books {
    Books {
        company: company(),
        masters: masters(),
        vouchers: vouchers.into_iter().map(|v| (v.id, v)).collect(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn params(receivable: bool, detail: Detail) -> ReportParams {
    ReportParams {
        kind: ReportKind::Outstanding { receivable },
        period: Period::new(d(2026, Month::April, 1), as_of()).unwrap(),
        as_of: as_of(),
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn run(books: &Books, p: &ReportParams, receivable: bool) -> Report {
    outstanding(books, p, receivable)
        .unwrap()
        .value()
        .expect("report should be supported")
}

fn hand_report(receivable: bool, detail: Detail) -> Report {
    run(
        &books_with(hand_vouchers()),
        &params(receivable, detail),
        receivable,
    )
}

fn party<'a>(r: &'a Report, label: &str) -> &'a ReportRow {
    r.rows
        .iter()
        .find(|x| x.label == label && x.kind == RowKind::Ledger)
        .unwrap_or_else(|| panic!("no party row {label:?} in {:?}", r.rows))
}

fn bill<'a>(r: &'a Report, label: &str) -> &'a ReportRow {
    r.rows
        .iter()
        .find(|x| x.label == label && x.kind == RowKind::Bill)
        .unwrap_or_else(|| panic!("no bill row {label:?} in {:?}", r.rows))
}

fn on_account_row(r: &Report, ledger: LedgerId) -> Option<&ReportRow> {
    r.rows
        .iter()
        .find(|x| x.label == "On account" && x.key == Some(DrillKey::Ledger(ledger)))
}

#[test]
fn asha_buckets_follow_days_past_due_date() {
    // provenance: derived — bucket = whole days from due date to 30-Sep-2026, see the day counts
    // in the `hand_vouchers` doc comment.
    // Not yet due: A-1 1,000.
    // 0-30: A-2 (0 days) 2,000 + A-3 (30 days) 4,000 = 6,000.
    // 31-60: A-4 (31 days) 8,000 + A-5 (60 days) 16,000 = 24,000.
    // 61-90: A-6 (61 days) 32,000 + A-7 (90 days) 64,000 = 96,000.
    // Over 90: A-8 (91 days) 128,000 less the 28,000 settled on 01-Sep-2026 = 100,000.
    // No due date: A-9 256,000.
    let r = hand_report(true, Detail::Condensed);
    let row = party(&r, "Asha Traders");
    assert_eq!(row.cells[C_NOT_DUE], dr(1_000));
    assert_eq!(row.cells[C_0_30], dr(6_000));
    assert_eq!(row.cells[C_31_60], dr(24_000));
    assert_eq!(row.cells[C_61_90], dr(96_000));
    assert_eq!(row.cells[C_OVER_90], dr(100_000));
    assert_eq!(row.cells[C_NO_DUE], dr(256_000));
}

#[test]
fn asha_pending_is_the_sum_of_open_bills() {
    // provenance: derived — bills 1+2+4+8+16+32+64+128+256 = 511 thousand, less the 28,000
    // settled against A-8 = 483,000. The 5,000 on account is not a bill and is not in Pending.
    let r = hand_report(true, Detail::Condensed);
    assert_eq!(party(&r, "Asha Traders").cells[C_PENDING], dr(483_000));
}

#[test]
fn on_account_is_a_separate_row_and_column() {
    // provenance: derived — the 01-Sep-2026 receipt carries 5,000 Cr on account. The receipt dated
    // 05-Oct-2026 (after the report date), the optional receipt (3,000) and the memorandum
    // (4,000) do not count (spec section 5, invariant 3).
    // Net = Pending 483,000 Dr + On account 5,000 Cr = 478,000 Dr.
    let r = hand_report(true, Detail::Detailed);
    let ledger_row = party(&r, "Asha Traders");
    assert_eq!(ledger_row.cells[C_ON_ACCOUNT], cr(5_000));
    assert_eq!(ledger_row.cells[C_NET], dr(478_000));
    let row = on_account_row(&r, ASHA).expect("an on-account row");
    assert_eq!(row.cells[C_ON_ACCOUNT], cr(5_000));
    // Nothing else on that row: it is never aged.
    for (i, c) in row.cells.iter().enumerate() {
        if i != C_ON_ACCOUNT {
            assert_eq!(*c, Cell::Empty, "column {i}");
        }
    }
}

#[test]
fn on_account_is_not_aged_into_any_bucket() {
    // provenance: derived — the bucket columns of Asha sum to the pending figure (483,000):
    // 1,000 + 6,000 + 24,000 + 96,000 + 100,000 + 256,000 = 483,000, so the 5,000 on account is
    // in no bucket.
    let r = hand_report(true, Detail::Condensed);
    let total: i64 = (C_NOT_DUE..=C_NO_DUE)
        .map(|i| match &party(&r, "Asha Traders").cells[i] {
            Cell::Amount(a) => a.amount.get(),
            Cell::Empty => 0,
            other => panic!("unexpected cell {other:?}"),
        })
        .sum();
    assert_eq!(total, rs(483_000).get());
}

#[test]
fn vouchers_after_the_report_date_do_not_count() {
    // provenance: derived — A-9 is settled by a receipt dated 05-Oct-2026, after the report date
    // 30-Sep-2026, so at the report date it is still wholly open (256,000, no due date).
    let r = hand_report(true, Detail::Detailed);
    assert_eq!(bill(&r, "A-9").cells[C_PENDING], dr(256_000));
    assert_eq!(bill(&r, "A-9").cells[C_NO_DUE], dr(256_000));
}

#[test]
fn optional_vouchers_are_not_bills() {
    // provenance: derived — spec section 5 invariant 3: optional vouchers are outside the books.
    let r = hand_report(true, Detail::Detailed);
    assert!(r.rows.iter().all(|x| x.label != "A-OPT"));
}

#[test]
fn bill_rows_carry_dates_drill_key_and_one_bucket() {
    // provenance: derived — A-4 was dated 01-Jun-2026 (the voucher date), due 30-Aug-2026, 8,000
    // open, 31 days overdue so only the 31-60 column is filled.
    let r = hand_report(true, Detail::Detailed);
    let row = bill(&r, "A-4");
    assert_eq!(row.cells[C_BILL_DATE], Cell::Date(d(2026, Month::June, 1)));
    assert_eq!(row.cells[C_DUE], Cell::Date(d(2026, Month::August, 30)));
    assert_eq!(row.cells[C_PENDING], dr(8_000));
    assert_eq!(row.cells[C_31_60], dr(8_000));
    assert_eq!(row.cells[C_0_30], Cell::Empty);
    assert_eq!(
        row.key,
        Some(DrillKey::Bill {
            ledger: ASHA,
            name: "A-4".into()
        })
    );
    assert_eq!(row.depth, 1);
}

#[test]
fn partly_settled_bill_shows_what_is_left() {
    // provenance: derived — A-8 128,000 less 28,000 against it = 100,000, still 91 days overdue.
    let r = hand_report(true, Detail::Detailed);
    assert_eq!(bill(&r, "A-8").cells[C_OVER_90], dr(100_000));
}

#[test]
fn credit_note_bill_keeps_its_side_and_credit_days_set_the_due_date() {
    // provenance: derived — B-1 50,000 Dr dated 10-Sep-2026 with 30 credit days is due
    // 10-Oct-2026 (20 days to 30-Sep + 10), after the report date: not yet due.
    // CN-1 3,000 Cr has no due date: shown as a credit in No due date, never flipped to a debit.
    // Pending = 50,000 Dr - 3,000 Cr = 47,000 Dr.
    let r = hand_report(true, Detail::Detailed);
    assert_eq!(bill(&r, "B-1").cells[C_NOT_DUE], dr(50_000));
    assert_eq!(
        bill(&r, "B-1").cells[C_DUE],
        Cell::Date(d(2026, Month::October, 10))
    );
    assert_eq!(bill(&r, "CN-1").cells[C_NO_DUE], cr(3_000));
    assert_eq!(party(&r, "Bharat Stores").cells[C_PENDING], dr(47_000));
}

#[test]
fn parties_in_subgroups_of_sundry_debtors_are_included() {
    // provenance: derived — Bharat Stores is in "Northern Debtors", a sub-group of Sundry Debtors.
    let r = hand_report(true, Detail::Condensed);
    let labels: Vec<&str> = r.rows.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(labels, vec!["Asha Traders", "Bharat Stores"]);
}

#[test]
fn parties_with_nothing_open_and_untracked_parties_are_not_rows() {
    // provenance: derived — "Dormant Debtor" is bill-wise with no vouchers; "Cash Customer" has no
    // bill-wise tracking. Neither has a row; the second is named in a note so it is not silent.
    let r = hand_report(true, Detail::Detailed);
    assert!(
        r.rows
            .iter()
            .all(|x| x.label != "Dormant Debtor" && x.label != "Cash Customer")
    );
    assert!(r.notes.iter().any(|n| n.contains("Cash Customer")));
}

#[test]
fn totals_add_the_parties() {
    // provenance: derived — Asha pending 483,000 Dr + Bharat 47,000 Dr = 530,000 Dr.
    // Not yet due: 1,000 + 50,000 = 51,000. 0-30: 6,000. 31-60: 24,000. 61-90: 96,000.
    // Over 90: 100,000. No due date: 256,000 Dr + 3,000 Cr = 253,000 Dr.
    // On account: 5,000 Cr. Net: 530,000 Dr - 5,000 Cr = 525,000 Dr.
    let r = hand_report(true, Detail::Condensed);
    assert_eq!(r.totals.len(), r.columns.len());
    assert_eq!(r.totals[C_PENDING], dr(530_000));
    assert_eq!(r.totals[C_NOT_DUE], dr(51_000));
    assert_eq!(r.totals[C_0_30], dr(6_000));
    assert_eq!(r.totals[C_31_60], dr(24_000));
    assert_eq!(r.totals[C_61_90], dr(96_000));
    assert_eq!(r.totals[C_OVER_90], dr(100_000));
    assert_eq!(r.totals[C_NO_DUE], dr(253_000));
    assert_eq!(r.totals[C_ON_ACCOUNT], cr(5_000));
    assert_eq!(r.totals[C_NET], dr(525_000));
}

#[test]
fn every_row_has_one_cell_per_column() {
    // provenance: derived — report model contract (`ReportRow::cells`: one per column).
    let r = hand_report(true, Detail::Detailed);
    assert_eq!(r.columns.len(), 11);
    assert!(r.rows.iter().all(|x| x.cells.len() == r.columns.len()));
}

#[test]
fn condensed_has_party_rows_only() {
    // provenance: derived — `Detail::Condensed` is "primary groups only"; here party rows only.
    let r = hand_report(true, Detail::Condensed);
    assert!(r.rows.iter().all(|x| x.kind == RowKind::Ledger));
}

#[test]
fn payable_lists_creditors_only() {
    // provenance: derived — Zed Supplies: P-1 10,000 Cr due 15-Aug-2026. 15-Aug to 15-Sep is
    // 31 days, plus 15 to 30-Sep is 46 days overdue: 31-60 bucket. On account 2,000 Dr (a payment
    // not linked to a bill). Net = 10,000 Cr - 2,000 Dr = 8,000 Cr.
    let r = hand_report(false, Detail::Detailed);
    let labels: Vec<&str> = r
        .rows
        .iter()
        .filter(|x| x.kind == RowKind::Ledger)
        .map(|x| x.label.as_str())
        .collect();
    assert_eq!(labels, vec!["Zed Supplies"]);
    let row = party(&r, "Zed Supplies");
    assert_eq!(row.cells[C_PENDING], cr(10_000));
    assert_eq!(row.cells[C_31_60], cr(10_000));
    assert_eq!(row.cells[C_ON_ACCOUNT], dr(2_000));
    assert_eq!(row.cells[C_NET], cr(8_000));
    assert_eq!(r.title, "Bills payable");
}

#[test]
fn ledger_filter_limits_the_parties() {
    // provenance: derived — filtering to Bharat Stores leaves only its block.
    let mut p = params(true, Detail::Condensed);
    p.filter.ledgers = vec![BHARAT];
    let r = run(&books_with(hand_vouchers()), &p, true);
    assert_eq!(r.rows.len(), 1);
    assert_eq!(r.rows[0].label, "Bharat Stores");
}

#[test]
fn group_filter_limits_the_parties() {
    // provenance: derived — Bharat Stores is the only ledger under "Northern Debtors".
    let mut p = params(true, Detail::Condensed);
    p.filter.groups = vec![G_NORTH];
    let r = run(&books_with(hand_vouchers()), &p, true);
    assert_eq!(r.rows.len(), 1);
    assert_eq!(r.rows[0].label, "Bharat Stores");
}

#[test]
fn report_is_deterministic() {
    // provenance: derived — rule A7: same input, same output.
    assert_eq!(
        hand_report(true, Detail::Detailed),
        hand_report(true, Detail::Detailed)
    );
}

#[test]
fn bill_wise_ledger_with_opening_balance_is_unsupported() {
    // provenance: derived — the model has no opening bill breakup (card E7 module docs), so the
    // bills of this ledger would be incomplete; the report must say so, not omit the opening.
    let mut books = books_with(hand_vouchers());
    books.masters.ledgers.get_mut(&ASHA).unwrap().opening = SignedAmount::dr(rs(1_000));
    let out = outstanding(&books, &params(true, Detail::Condensed), true).unwrap();
    assert!(out.is_unsupported());
    // The creditor report is not affected by a debtor's opening balance.
    assert!(
        !outstanding(&books, &params(false, Detail::Condensed), false)
            .unwrap()
            .is_unsupported()
    );
}

#[test]
fn unsupported_parameters_are_reported_not_guessed() {
    // provenance: derived — rule A1: parameters the engine does not compute return Unsupported.
    let books = books_with(hand_vouchers());
    let mut monthly = params(true, Detail::Condensed);
    monthly.columns = ColumnSpec::Monthly;
    let mut scenario = params(true, Detail::Condensed);
    scenario.scenario = Scenario::Including(vec![VoucherStatus::Optional]);
    let mut exceptions = params(true, Detail::Condensed);
    exceptions.exceptions_only = true;
    let mut by_type = params(true, Detail::Condensed);
    by_type.filter.voucher_types = vec![T_SALES];
    for p in [monthly, scenario, exceptions, by_type] {
        assert!(outstanding(&books, &p, true).unwrap().is_unsupported());
    }
}

#[test]
fn empty_including_scenario_equals_actual() {
    // provenance: derived — `Scenario::Including([])` adds no statuses, so it is `Actual`.
    let mut p = params(true, Detail::Condensed);
    p.scenario = Scenario::Including(vec![]);
    let r = run(&books_with(hand_vouchers()), &p, true);
    assert_eq!(r.totals[C_PENDING], dr(530_000));
}

#[test]
fn wrong_report_kind_is_an_error() {
    // provenance: derived — a payables request on a receivables report is a caller bug.
    let books = books_with(hand_vouchers());
    let p = params(false, Detail::Condensed);
    assert!(matches!(
        outstanding(&books, &p, true),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn as_of_before_the_books_begin_is_an_error() {
    // provenance: derived — books begin 01-Apr-2026; there is nothing to report before that.
    let mut p = params(true, Detail::Condensed);
    p.as_of = d(2026, Month::March, 31);
    assert!(matches!(
        outstanding(&books_with(hand_vouchers()), &p, true),
        Err(ReportError::Invalid { .. })
    ));
}

#[test]
fn over_settled_books_are_an_error_not_a_number() {
    // provenance: derived — a settlement larger than the bill is corrupt input (E7 rejects it);
    // the report must fail rather than show a negative or clipped figure.
    let mut vouchers = hand_vouchers();
    vouchers.push(voucher(
        30,
        T_RECEIPT,
        d(2026, Month::September, 2),
        VoucherStatus::Regular,
        vec![
            line(
                ASHA,
                SignedAmount::cr(rs(200_000)),
                vec![alloc(
                    BillRefKind::Against,
                    "A-8",
                    None,
                    None,
                    SignedAmount::cr(rs(200_000)),
                )],
            ),
            line(BANK, SignedAmount::dr(rs(200_000)), vec![]),
        ],
    ));
    assert!(
        outstanding(
            &books_with(vouchers),
            &params(true, Detail::Condensed),
            true
        )
        .is_err()
    );
}

#[test]
fn empty_books_give_an_empty_report() {
    // provenance: derived — no bills and nothing on account: no rows, and the totals are empty.
    let r = run(&books_with(vec![]), &params(true, Detail::Detailed), true);
    assert_eq!(r.rows, [] as [ledgerly_reports::ReportRow; 0]);
    assert!(r.totals.iter().all(|c| *c == Cell::Empty));
    assert_eq!(r.totals.len(), r.columns.len());
}

#[test]
fn outcome_type_is_value_for_supported_input() {
    // provenance: derived — `Outcome::Value` for a supported report.
    let out = outstanding(
        &books_with(hand_vouchers()),
        &params(true, Detail::Condensed),
        true,
    )
    .unwrap();
    assert!(matches!(out, Outcome::Value(_)));
}

proptest! {
    // provenance: derived — for any set of Dr bills each bill lands in exactly the one bucket its
    // days overdue name (card definition: <0 not yet due, 0-30, 31-60, 61-90, >90), and the
    // bucket columns add up to Pending.
    #[test]
    fn each_bill_lands_in_exactly_its_bucket(
        bills in proptest::collection::vec((-150i64..150, 1i64..100_000), 1..12)
    ) {
        let bill_day = d(2026, Month::April, 10);
        let vouchers: Vec<Voucher> = bills
            .iter()
            .enumerate()
            .map(|(i, (overdue, rupees))| {
                let due = as_of() - time::Duration::days(*overdue);
                sale(u64::try_from(i).unwrap() + 1, bill_day, ASHA, &format!("P{i}"), *rupees, Some(due))
            })
            .collect();
        let books = books_with(vouchers);
        let r = run(&books, &params(true, Detail::Detailed), true);
        let mut pending = 0i64;
        for (i, (overdue, rupees)) in bills.iter().enumerate() {
            let row = bill(&r, &format!("P{i}"));
            let expected = if *overdue < 0 {
                C_NOT_DUE
            } else if *overdue <= 30 {
                C_0_30
            } else if *overdue <= 60 {
                C_31_60
            } else if *overdue <= 90 {
                C_61_90
            } else {
                C_OVER_90
            };
            for col in C_NOT_DUE..=C_NO_DUE {
                if col == expected {
                    prop_assert_eq!(&row.cells[col], &dr(*rupees));
                } else {
                    prop_assert_eq!(&row.cells[col], &Cell::Empty);
                }
            }
            pending += rupees;
        }
        prop_assert_eq!(&party(&r, "Asha Traders").cells[C_PENDING], &dr(pending));
    }
}

#[test]
fn days_overdue_counts_whole_days_past_the_due_date() {
    // provenance: derived. The ageing rule in the module doc: overdue days = as_of - due in whole
    // days; a bill due on the report date is 0 days overdue; a bill due after it, or with no due
    // date, is not aged. 20 Nov 2026 less a due date of 14 Oct 2026: 17 (rest of October after
    // the 14th: 31 - 14) + 20 (November) = 37 days. 30 Jun 2026 less 20 Apr 2026: 10 + 31 + 30 = 71.
    use ledgerly_reports::outstanding::days_overdue;
    let d = |y, m, day| Date::from_calendar_date(y, m, day).unwrap();
    assert_eq!(
        days_overdue(
            d(2026, Month::November, 20),
            Some(d(2026, Month::October, 14))
        ),
        Some(37)
    );
    assert_eq!(
        days_overdue(d(2026, Month::June, 30), Some(d(2026, Month::April, 20))),
        Some(71)
    );
    assert_eq!(
        days_overdue(d(2026, Month::June, 30), Some(d(2026, Month::June, 30))),
        Some(0)
    );
    assert_eq!(
        days_overdue(d(2026, Month::June, 30), Some(d(2026, Month::July, 1))),
        None
    );
    assert_eq!(days_overdue(d(2026, Month::June, 30), None), None);
}

#[test]
fn the_total_line_counts_the_open_bills() {
    // provenance: derived (round s16, the mock's total line "N open bills", index.html
    // `outstanding`). Asha Traders has nine open bills A-1 to A-9 (A-8 is part settled, so 100,000
    // is still open; A-9's settling receipt is dated after the report date). Bharat Stores has
    // B-1 and the credit note CN-1 opened as a bill: 9 + 2 = 11. The on-account amounts are not
    // bills. The count is the same condensed and detailed.
    for detail in [Detail::Condensed, Detail::Detailed] {
        let r = hand_report(true, detail);
        assert_eq!(
            r.totals[C_BILL_DATE],
            Cell::Text("11 open bills".to_owned()),
            "{detail:?}"
        );
    }
}

#[test]
fn the_total_line_says_one_open_bill_in_the_singular() {
    // provenance: derived: Zed Supplies has one bill, P-1 10,000 Cr (the 2,000 Dr is on account).
    let r = hand_report(false, Detail::Detailed);
    assert_eq!(r.totals[C_BILL_DATE], Cell::Text("1 open bill".to_owned()));
}
