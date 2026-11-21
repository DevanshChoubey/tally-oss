//! Card E7 tests: bill-wise projection and validation (spec section 5, invariant 2).
//!
//! Every expected value carries a provenance comment. All are `derived` by hand from the
//! definition of a bill reference (research R1 "BillAlloc"/"Bill"): a bill is opened by a `New`
//! (or `Advance`) allocation, reduced by `Against` allocations on the opposite side, and its open
//! amount is the signed sum (Dr positive, Cr negative) of those allocations.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may unwrap and index (engineering rule B2)"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    Date, DrCr, LedgerId, Paise, SignedAmount, StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_engine::EngineError;
use ledgerly_engine::bills::{OpenBill, open_bills, validate_bill_refs};
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Books, Company,
    EntityKind, Features, Ledger, LedgerLine, Masters, NegativeStockPolicy, NumberingConfig,
    NumberingMethod, RestartPeriod, Voucher, VoucherError, VoucherMode, VoucherStatus, VoucherType,
};
use time::macros::date;

const DEBTOR: LedgerId = LedgerId(10);
const OTHER_DEBTOR: LedgerId = LedgerId(11);
const SALES: LedgerId = LedgerId(20);
const BANK: LedgerId = LedgerId(30);

const T_SALES: VoucherTypeId = VoucherTypeId(1);
const T_RECEIPT: VoucherTypeId = VoucherTypeId(2);
const T_JOURNAL: VoucherTypeId = VoucherTypeId(3);
const T_MEMO: VoucherTypeId = VoucherTypeId(4);
const T_REVERSING: VoucherTypeId = VoucherTypeId(5);
const T_SALES_ORDER: VoucherTypeId = VoucherTypeId(6);

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn ledger(id: LedgerId, name: &str, bill_wise: bool) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_string(),
        aliases: Vec::new(),
        group: ledgerly_core::GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: bill_wise.then_some(BillWiseConfig {
            default_credit_days: Some(45),
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

fn vtype(id: VoucherTypeId, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: format!("{base:?}"),
        base,
        abbreviation: String::new(),
        numbering: NumberingConfig {
            method: NumberingMethod::Manual,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 0,
            restart: RestartPeriod::Never,
            series: Vec::new(),
        },
        active: true,
        tally: None,
    }
}

fn empty_books() -> Books {
    let mut masters = Masters::default();
    for l in [
        ledger(DEBTOR, "Debtor A", true),
        ledger(OTHER_DEBTOR, "Debtor B", true),
        ledger(SALES, "Sales", false),
        ledger(BANK, "Bank", false),
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for (id, base) in [
        (T_SALES, BaseVoucherKind::Sales),
        (T_RECEIPT, BaseVoucherKind::Receipt),
        (T_JOURNAL, BaseVoucherKind::Journal),
        (T_MEMO, BaseVoucherKind::Memorandum),
        (T_REVERSING, BaseVoucherKind::ReversingJournal),
        (T_SALES_ORDER, BaseVoucherKind::SalesOrder),
    ] {
        masters.voucher_types.insert(id, vtype(id, base));
    }
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: ledgerly_core::CompanyId(1),
            name: "Test Traders".to_string(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features {
                bill_wise: true,
                ..Features::default()
            },
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn alloc(kind: BillRefKind, name: &str, amount: SignedAmount) -> BillAllocation {
    BillAllocation {
        kind,
        name: name.to_string(),
        due_date: None,
        credit_days: None,
        amount,
    }
}

fn line(ledger: LedgerId, amount: SignedAmount, bills: Vec<BillAllocation>) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills,
        cost: Vec::new(),
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
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

/// Sales invoice: Dr party, with one allocation of the given kind; Cr sales (no bills).
fn invoice(
    id: u64,
    date: Date,
    party: LedgerId,
    kind: BillRefKind,
    name: &str,
    a: Paise,
) -> Voucher {
    voucher(
        id,
        T_SALES,
        date,
        VoucherStatus::Regular,
        vec![
            line(
                party,
                SignedAmount::dr(a),
                vec![alloc(kind, name, SignedAmount::dr(a))],
            ),
            line(SALES, SignedAmount::cr(a), Vec::new()),
        ],
    )
}

/// Receipt: Cr party with one allocation of the given kind; Dr bank (no bills).
fn receipt(
    id: u64,
    date: Date,
    party: LedgerId,
    kind: BillRefKind,
    name: &str,
    a: Paise,
) -> Voucher {
    voucher(
        id,
        T_RECEIPT,
        date,
        VoucherStatus::Regular,
        vec![
            line(
                party,
                SignedAmount::cr(a),
                vec![alloc(kind, name, SignedAmount::cr(a))],
            ),
            line(BANK, SignedAmount::dr(a), Vec::new()),
        ],
    )
}

fn with(mut books: Books, vs: Vec<Voucher>) -> Books {
    for v in vs {
        books.vouchers.insert(v.id, v);
    }
    books
}

fn bills(books: &Books, as_of: Date) -> Vec<OpenBill> {
    open_bills(books, DEBTOR, as_of).unwrap()
}

#[test]
fn receipt_against_10000_bill_for_12000_errors_over_settled() {
    // provenance: derived — R1 invariant 2 / spec section 5 inv. 2 ("Against refs ... not
    // over-settle"). Open = Rs 10,000 = 1,000,000 paise; settling Rs 12,000 = 1,200,000 paise.
    let books = with(
        empty_books(),
        vec![invoice(
            1,
            date!(2026 - 04 - 10),
            DEBTOR,
            BillRefKind::New,
            "INV-1",
            rs(10_000),
        )],
    );
    let r = receipt(
        2,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(12_000),
    );
    assert_eq!(
        validate_bill_refs(&r, &books),
        Err(EngineError::OverSettled {
            bill: "INV-1".to_string(),
            open: Paise::new(1_000_000),
            settling: Paise::new(1_200_000),
        })
    );
}

#[test]
fn partial_receipt_leaves_balance_open_and_full_receipt_closes() {
    // provenance: derived — signed sum: +10,000 (New, Dr) - 4,000 (Against, Cr) = 6,000 Dr open
    // = 600,000 paise; then a further 6,000 Against closes it: 6,000 - 6,000 = 0, so no open bill.
    let invoice_v = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let part = receipt(
        2,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(4_000),
    );
    let books = with(empty_books(), vec![invoice_v.clone(), part.clone()]);
    let got = bills(&books, date!(2026 - 04 - 30));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].name, "INV-1");
    assert_eq!(got[0].date, date!(2026 - 04 - 10));
    assert_eq!(got[0].open, SignedAmount::dr(Paise::new(600_000)));

    let rest = receipt(
        3,
        date!(2026 - 04 - 25),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(6_000),
    );
    assert_eq!(validate_bill_refs(&rest, &books), Ok(()));
    let books = with(books, vec![rest]);
    assert_eq!(bills(&books, date!(2026 - 04 - 30)), Vec::new());
}

#[test]
fn advance_then_invoice_then_against_closes_both_bills() {
    // provenance: derived — three vouchers on one party ledger:
    //  v1 receipt Cr 3,000 Advance "ADV-1"  -> bill ADV-1 open 3,000 Cr
    //  v2 sales   Dr 10,000 New "INV-1"     -> bill INV-1 open 10,000 Dr
    //  v3 journal Dr 3,000 Against ADV-1 (3,000 Cr - 3,000 Dr = 0) and
    //             Cr 3,000 Against INV-1 (10,000 Dr - 3,000 Cr = 7,000 Dr).
    //  v4 receipt Cr 7,000 Against INV-1 (7,000 - 7,000 = 0). Nothing is left open.
    let v1 = receipt(
        1,
        date!(2026 - 04 - 05),
        DEBTOR,
        BillRefKind::Advance,
        "ADV-1",
        rs(3_000),
    );
    let v2 = invoice(
        2,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let books = with(empty_books(), vec![v1, v2]);
    let mid = bills(&books, date!(2026 - 04 - 30));
    assert_eq!(mid.len(), 2);
    assert_eq!(
        (mid[0].name.as_str(), mid[0].open),
        ("ADV-1", SignedAmount::cr(rs(3_000)))
    );
    assert_eq!(
        (mid[1].name.as_str(), mid[1].open),
        ("INV-1", SignedAmount::dr(rs(10_000)))
    );

    let v3 = voucher(
        3,
        T_JOURNAL,
        date!(2026 - 04 - 15),
        VoucherStatus::Regular,
        vec![
            line(
                DEBTOR,
                SignedAmount::dr(rs(3_000)),
                vec![alloc(
                    BillRefKind::Against,
                    "ADV-1",
                    SignedAmount::dr(rs(3_000)),
                )],
            ),
            line(
                DEBTOR,
                SignedAmount::cr(rs(3_000)),
                vec![alloc(
                    BillRefKind::Against,
                    "INV-1",
                    SignedAmount::cr(rs(3_000)),
                )],
            ),
        ],
    );
    assert_eq!(validate_bill_refs(&v3, &books), Ok(()));
    let books = with(books, vec![v3]);
    let after = bills(&books, date!(2026 - 04 - 30));
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].open, SignedAmount::dr(rs(7_000)));

    let v4 = receipt(
        4,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(7_000),
    );
    assert_eq!(validate_bill_refs(&v4, &books), Ok(()));
    let books = with(books, vec![v4]);
    assert_eq!(bills(&books, date!(2026 - 04 - 30)), Vec::new());
}

#[test]
fn advance_then_invoice_with_same_name_nets_to_one_bill() {
    // provenance: derived — an Advance and a New with the same name on one ledger are one bill:
    // -3,000 (Advance, Cr) + 10,000 (New, Dr) = 7,000 Dr open; an Against Cr 7,000 then closes it.
    let v1 = receipt(
        1,
        date!(2026 - 04 - 05),
        DEBTOR,
        BillRefKind::Advance,
        "INV-9",
        rs(3_000),
    );
    let v2 = invoice(
        2,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-9",
        rs(10_000),
    );
    let books = with(empty_books(), vec![v1, v2]);
    let got = bills(&books, date!(2026 - 04 - 30));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].open, SignedAmount::dr(rs(7_000)));
    // The bill is dated by the invoice (the New), not by the earlier advance.
    assert_eq!(got[0].date, date!(2026 - 04 - 10));
    let v3 = receipt(
        3,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-9",
        rs(7_000),
    );
    let books = with(books, vec![v3]);
    assert_eq!(bills(&books, date!(2026 - 04 - 30)), Vec::new());
}

#[test]
fn credit_days_30_sets_due_date() {
    // provenance: derived — credit period of 30 days counted from the bill date (voucher date)
    // 10 Apr 2026: April has 30 days, so 10 Apr + 20 days = 30 Apr and + 10 more = 10 May 2026.
    let mut v = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(1_000),
    );
    v.ledger_lines[0].bills[0].credit_days = Some(30);
    let books = with(empty_books(), vec![v]);
    let got = bills(&books, date!(2026 - 04 - 30));
    assert_eq!(got[0].due, Some(date!(2026 - 05 - 10)));
}

#[test]
fn explicit_due_date_is_kept_and_absent_terms_give_no_due_date() {
    // provenance: derived — a due date typed on the allocation is the due date; with neither a
    // due date nor credit days the bill has none (the ledger default is an entry-time default).
    let mut v1 = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "A",
        rs(1_000),
    );
    v1.ledger_lines[0].bills[0].due_date = Some(date!(2026 - 06 - 01));
    let v2 = invoice(
        2,
        date!(2026 - 04 - 11),
        DEBTOR,
        BillRefKind::New,
        "B",
        rs(1_000),
    );
    let books = with(empty_books(), vec![v1, v2]);
    let got = bills(&books, date!(2026 - 04 - 30));
    assert_eq!(got[0].due, Some(date!(2026 - 06 - 01)));
    assert_eq!(got[1].due, None);
}

#[test]
fn due_date_disagreeing_with_credit_days_is_an_error() {
    // provenance: derived — 10 Apr + 30 days = 10 May, not 1 Jun; two answers for one field is
    // an error, never a silent pick.
    let mut v = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(1_000),
    );
    v.ledger_lines[0].bills[0].credit_days = Some(30);
    v.ledger_lines[0].bills[0].due_date = Some(date!(2026 - 06 - 01));
    let books = with(empty_books(), vec![v]);
    assert!(matches!(
        open_bills(&books, DEBTOR, date!(2026 - 04 - 30)),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn consistent_due_date_and_credit_days_are_accepted() {
    // provenance: derived — 10 Apr + 30 days = 10 May 2026 (see credit_days_30_sets_due_date).
    let mut v = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(1_000),
    );
    v.ledger_lines[0].bills[0].credit_days = Some(30);
    v.ledger_lines[0].bills[0].due_date = Some(date!(2026 - 05 - 10));
    let books = with(empty_books(), vec![v]);
    assert_eq!(
        bills(&books, date!(2026 - 04 - 30))[0].due,
        Some(date!(2026 - 05 - 10))
    );
}

#[test]
fn re_saving_an_existing_receipt_is_not_over_settled() {
    // provenance: derived — bill 10,000; the stored receipt settles 10,000 (open 0). Re-validating
    // that same voucher (an alteration) must ignore its own stored allocation: open is 10,000 again
    // and settling 10,000 fits. An altered amount of 10,000.01 would not fit (1,000,001 > 1,000,000).
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let rec = receipt(
        2,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(10_000),
    );
    let books = with(empty_books(), vec![inv, rec.clone()]);
    assert_eq!(validate_bill_refs(&rec, &books), Ok(()));
    let altered = receipt(
        2,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        Paise::new(1_000_001),
    );
    assert_eq!(
        validate_bill_refs(&altered, &books),
        Err(EngineError::OverSettled {
            bill: "INV-1".to_string(),
            open: Paise::new(1_000_000),
            settling: Paise::new(1_000_001),
        })
    );
}

#[test]
fn two_settlements_in_one_voucher_accumulate() {
    // provenance: derived — bill 10,000; two Against lines of 6,000 each in one voucher: the
    // first leaves 4,000 open, the second (6,000) exceeds it.
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let books = with(empty_books(), vec![inv]);
    let v = voucher(
        2,
        T_RECEIPT,
        date!(2026 - 04 - 20),
        VoucherStatus::Regular,
        vec![
            line(
                DEBTOR,
                SignedAmount::cr(rs(6_000)),
                vec![alloc(
                    BillRefKind::Against,
                    "INV-1",
                    SignedAmount::cr(rs(6_000)),
                )],
            ),
            line(
                DEBTOR,
                SignedAmount::cr(rs(6_000)),
                vec![alloc(
                    BillRefKind::Against,
                    "INV-1",
                    SignedAmount::cr(rs(6_000)),
                )],
            ),
            line(BANK, SignedAmount::dr(rs(12_000)), Vec::new()),
        ],
    );
    assert_eq!(
        validate_bill_refs(&v, &books),
        Err(EngineError::OverSettled {
            bill: "INV-1".to_string(),
            open: rs(4_000),
            settling: rs(6_000),
        })
    );
}

#[test]
fn back_dated_receipt_cannot_over_settle_a_bill_a_later_receipt_settled() {
    // provenance: derived — bill 10,000 (10 Apr); a receipt dated 30 Apr settles 8,000 (2,000
    // open). A new receipt dated 20 Apr for 5,000 is back-dated but the total 8,000 + 5,000 = 13,000
    // exceeds 10,000, so it fails with open = 2,000, settling = 5,000.
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let late = receipt(
        2,
        date!(2026 - 04 - 30),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(8_000),
    );
    let books = with(empty_books(), vec![inv, late]);
    let back = receipt(
        3,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(5_000),
    );
    assert_eq!(
        validate_bill_refs(&back, &books),
        Err(EngineError::OverSettled {
            bill: "INV-1".to_string(),
            open: rs(2_000),
            settling: rs(5_000),
        })
    );
}

#[test]
fn settling_a_closed_bill_is_over_settled_with_nothing_open() {
    // provenance: derived — 10,000 - 10,000 = 0 open; settling any further amount (1) exceeds 0.
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let rec = receipt(
        2,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(10_000),
    );
    let books = with(empty_books(), vec![inv, rec]);
    let again = receipt(
        3,
        date!(2026 - 04 - 25),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        Paise::new(1),
    );
    assert_eq!(
        validate_bill_refs(&again, &books),
        Err(EngineError::OverSettled {
            bill: "INV-1".to_string(),
            open: Paise::ZERO,
            settling: Paise::new(1),
        })
    );
}

#[test]
fn against_a_bill_of_another_ledger_is_rejected() {
    // provenance: derived — spec section 5 inv. 2: Against refs must be on the same ledger. INV-1
    // lives on Debtor A; Debtor B's receipt naming it must fail with BillOnWrongLedger.
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let books = with(empty_books(), vec![inv]);
    let r = receipt(
        2,
        date!(2026 - 04 - 20),
        OTHER_DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(1_000),
    );
    assert_eq!(
        validate_bill_refs(&r, &books),
        Err(EngineError::Voucher(VoucherError::BillOnWrongLedger {
            bill: "INV-1".to_string(),
            ledger: OTHER_DEBTOR,
        }))
    );
}

#[test]
fn against_a_bill_that_does_not_exist_is_not_found() {
    // provenance: derived — spec section 5 inv. 2: Against refs must exist.
    let books = empty_books();
    let r = receipt(
        1,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "NOPE",
        rs(1_000),
    );
    assert!(matches!(
        validate_bill_refs(&r, &books),
        Err(EngineError::NotFound { .. })
    ));
}

#[test]
fn against_on_the_same_side_as_the_bill_is_rejected() {
    // provenance: derived — a Dr allocation on a Dr bill raises it; it does not settle it.
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let books = with(empty_books(), vec![inv]);
    let v = voucher(
        2,
        T_JOURNAL,
        date!(2026 - 04 - 20),
        VoucherStatus::Regular,
        vec![line(
            DEBTOR,
            SignedAmount::dr(rs(1_000)),
            vec![alloc(
                BillRefKind::Against,
                "INV-1",
                SignedAmount::dr(rs(1_000)),
            )],
        )],
    );
    assert!(matches!(
        validate_bill_refs(&v, &books),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn allocations_on_a_ledger_that_is_not_bill_wise_are_rejected() {
    // provenance: derived — spec section 5 inv. 2 applies to "a bill-wise line"; Sales is not.
    let v = voucher(
        1,
        T_SALES,
        date!(2026 - 04 - 10),
        VoucherStatus::Regular,
        vec![line(
            SALES,
            SignedAmount::cr(rs(1_000)),
            vec![alloc(BillRefKind::New, "X", SignedAmount::cr(rs(1_000)))],
        )],
    );
    assert!(matches!(
        validate_bill_refs(&v, &empty_books()),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn unknown_ledger_with_allocations_is_a_voucher_error() {
    // provenance: derived — a line on a ledger that does not exist cannot be checked.
    let v = invoice(
        1,
        date!(2026 - 04 - 10),
        LedgerId(999),
        BillRefKind::New,
        "X",
        rs(1),
    );
    assert_eq!(
        validate_bill_refs(&v, &empty_books()),
        Err(EngineError::Voucher(VoucherError::UnknownLedger(LedgerId(
            999
        ))))
    );
}

#[test]
fn a_second_new_with_the_same_name_is_a_duplicate() {
    // provenance: derived — merging two New refs of one name would hide a bill; it is rejected.
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let books = with(empty_books(), vec![inv]);
    let again = invoice(
        2,
        date!(2026 - 04 - 11),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(5_000),
    );
    assert!(matches!(
        validate_bill_refs(&again, &books),
        Err(EngineError::Duplicate { .. })
    ));
    let both = with(books, vec![again]);
    assert!(matches!(
        open_bills(&both, DEBTOR, date!(2026 - 04 - 30)),
        Err(EngineError::Duplicate { .. })
    ));
}

#[test]
fn optional_cancelled_memorandum_and_reversing_vouchers_never_open_or_settle_bills() {
    // provenance: derived — spec section 5 invariant 3: optional, memorandum and reversing
    // vouchers (and cancelled ones, which carry no amounts) never affect base balances. Only the
    // Regular invoice of 10,000 counts, so exactly 10,000 Dr is open.
    let reg = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let mut optional = receipt(
        2,
        date!(2026 - 04 - 11),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(3_000),
    );
    optional.status = VoucherStatus::Optional;
    let mut cancelled = receipt(
        3,
        date!(2026 - 04 - 12),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(3_000),
    );
    cancelled.status = VoucherStatus::Cancelled;
    let mut memo = receipt(
        4,
        date!(2026 - 04 - 13),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(3_000),
    );
    memo.type_id = T_MEMO;
    let mut rev = receipt(
        5,
        date!(2026 - 04 - 14),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(3_000),
    );
    rev.type_id = T_REVERSING;
    let books = with(empty_books(), vec![reg, optional, cancelled, memo, rev]);
    let got = bills(&books, date!(2026 - 04 - 30));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].open, SignedAmount::dr(rs(10_000)));
}

#[test]
fn post_dated_voucher_counts_only_on_or_before_as_of() {
    // provenance: derived — spec section 5 invariant 3 / Scenario::Actual: post-dated vouchers
    // enter the books on their date. Receipt (post-dated 25 Apr) of 4,000 against 10,000:
    // as of 20 Apr open is 10,000; as of 25 Apr it is 6,000.
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let mut pd = receipt(
        2,
        date!(2026 - 04 - 25),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(4_000),
    );
    pd.status = VoucherStatus::PostDated;
    let books = with(empty_books(), vec![inv, pd]);
    assert_eq!(
        bills(&books, date!(2026 - 04 - 20))[0].open,
        SignedAmount::dr(rs(10_000))
    );
    assert_eq!(
        bills(&books, date!(2026 - 04 - 25))[0].open,
        SignedAmount::dr(rs(6_000))
    );
}

#[test]
fn as_of_excludes_later_vouchers() {
    // provenance: derived — as of 15 Apr the 20 Apr receipt has not happened (10,000 open); as of
    // 9 Apr the 10 Apr invoice has not happened either (no bills).
    let inv = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(10_000),
    );
    let rec = receipt(
        2,
        date!(2026 - 04 - 20),
        DEBTOR,
        BillRefKind::Against,
        "INV-1",
        rs(4_000),
    );
    let books = with(empty_books(), vec![inv, rec]);
    assert_eq!(
        bills(&books, date!(2026 - 04 - 15))[0].open,
        SignedAmount::dr(rs(10_000))
    );
    assert_eq!(bills(&books, date!(2026 - 04 - 09)), Vec::new());
}

#[test]
fn on_account_allocations_are_not_bills() {
    // provenance: derived — R1: OnAccount is "not linked to a bill"; it opens nothing.
    let v = receipt(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::OnAccount,
        "",
        rs(500),
    );
    let books = with(empty_books(), vec![v]);
    assert_eq!(bills(&books, date!(2026 - 04 - 30)), Vec::new());
}

#[test]
fn open_bills_are_scoped_to_the_ledger_and_sorted_by_date_then_name() {
    // provenance: derived — Debtor B's bill is not Debtor A's; Debtor A's bills are ordered by
    // (date, name): B-late (5 Apr, "Z"), then 10 Apr "A", then 10 Apr "B".
    let v1 = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "B",
        rs(1),
    );
    let v2 = invoice(
        2,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "A",
        rs(2),
    );
    let v3 = invoice(
        3,
        date!(2026 - 04 - 05),
        DEBTOR,
        BillRefKind::New,
        "Z",
        rs(3),
    );
    let v4 = invoice(
        4,
        date!(2026 - 04 - 01),
        OTHER_DEBTOR,
        BillRefKind::New,
        "Q",
        rs(4),
    );
    let books = with(empty_books(), vec![v1, v2, v3, v4]);
    let names: Vec<String> = bills(&books, date!(2026 - 04 - 30))
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert_eq!(names, ["Z", "A", "B"]);
}

#[test]
fn bill_wise_ledger_with_an_opening_balance_cannot_be_projected() {
    // provenance: derived — the model has no opening bill breakup, so a bill-wise ledger with a
    // non-zero opening would show open bills that omit it; that must be an error, not a short list.
    let mut books = empty_books();
    if let Some(l) = books.masters.ledgers.get_mut(&DEBTOR) {
        l.opening = SignedAmount::dr(rs(500));
    }
    assert!(matches!(
        open_bills(&books, DEBTOR, date!(2026 - 04 - 30)),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn unknown_ledger_is_not_found_and_non_bill_wise_ledger_has_no_bills() {
    // provenance: derived — a missing ledger cannot be answered; a ledger without bill-wise
    // tracking has no bills by definition.
    let books = empty_books();
    assert!(matches!(
        open_bills(&books, LedgerId(999), date!(2026 - 04 - 30)),
        Err(EngineError::NotFound { .. })
    ));
    assert_eq!(
        open_bills(&books, SALES, date!(2026 - 04 - 30)),
        Ok(Vec::new())
    );
}

#[test]
fn voucher_of_unknown_type_is_an_error_not_a_skip() {
    // provenance: derived — rule A1: a voucher whose type cannot be resolved cannot be classified.
    let mut v = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(1),
    );
    v.type_id = VoucherTypeId(777);
    let books = with(empty_books(), vec![v]);
    assert_eq!(
        open_bills(&books, DEBTOR, date!(2026 - 04 - 30)),
        Err(EngineError::Voucher(VoucherError::UnknownVoucherType(
            VoucherTypeId(777)
        )))
    );
}

#[test]
fn order_voucher_carrying_bill_allocations_is_rejected() {
    // provenance: derived — a Sales Order does not post to ledgers, so bill allocations on it have
    // no meaning; refusing beats silently ignoring them.
    let mut v = invoice(
        1,
        date!(2026 - 04 - 10),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(1),
    );
    v.type_id = T_SALES_ORDER;
    let books = with(empty_books(), vec![v]);
    assert!(matches!(
        open_bills(&books, DEBTOR, date!(2026 - 04 - 30)),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn credit_days_overflowing_the_calendar_is_an_error() {
    // provenance: derived — time::Date tops out at year 9999; 31 Dec 9999 + 1 day is unrepresentable.
    let mut v = invoice(
        1,
        date!(9999 - 12 - 31),
        DEBTOR,
        BillRefKind::New,
        "INV-1",
        rs(1),
    );
    v.ledger_lines[0].bills[0].credit_days = Some(1);
    let books = with(empty_books(), vec![v]);
    assert!(matches!(
        open_bills(&books, DEBTOR, date!(9999 - 12 - 31)),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn allocation_sign_follows_dr_cr_side() {
    // provenance: derived — a credit-note style Cr New bill on a creditor is open on the Cr side.
    let v = voucher(
        1,
        T_JOURNAL,
        date!(2026 - 04 - 10),
        VoucherStatus::Regular,
        vec![line(
            DEBTOR,
            SignedAmount {
                amount: rs(250),
                side: DrCr::Cr,
            },
            vec![alloc(BillRefKind::New, "CN-1", SignedAmount::cr(rs(250)))],
        )],
    );
    let books = with(empty_books(), vec![v]);
    assert_eq!(
        bills(&books, date!(2026 - 04 - 30))[0].open,
        SignedAmount::cr(rs(250))
    );
}

mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn open_is_invoice_minus_settlements(
            invoice_paise in 1i64..1_000_000_000,
            cuts in proptest::collection::vec(0u32..=100, 0..8),
        ) {
            // provenance: derived — oracle is plain subtraction in the test: each receipt settles
            // `cut`% (integer division, floor) of what remains, so remaining_i+1 = remaining_i -
            // remaining_i * cut / 100; a bill with remaining 0 is not listed.
            let mut books = with(
                empty_books(),
                vec![invoice(1, date!(2026 - 04 - 10), DEBTOR, BillRefKind::New, "INV", Paise::new(invoice_paise))],
            );
            let mut remaining = invoice_paise;
            for (i, cut) in cuts.iter().enumerate() {
                let settle = remaining * i64::from(*cut) / 100;
                if settle == 0 { continue; }
                let id = 2 + i as u64;
                let r = receipt(id, date!(2026 - 04 - 20), DEBTOR, BillRefKind::Against, "INV", Paise::new(settle));
                prop_assert_eq!(validate_bill_refs(&r, &books), Ok(()));
                books = with(books, vec![r]);
                remaining -= settle;
            }
            let got = bills(&books, date!(2026 - 04 - 30));
            if remaining == 0 {
                prop_assert!(got.is_empty());
            } else {
                prop_assert_eq!(got.len(), 1);
                prop_assert_eq!(got[0].open, SignedAmount::dr(Paise::new(remaining)));
                // one paisa more than remaining always over-settles
                let over = receipt(999, date!(2026 - 04 - 29), DEBTOR, BillRefKind::Against, "INV", Paise::new(remaining + 1));
                let is_over_settled = matches!(
                    validate_bill_refs(&over, &books),
                    Err(EngineError::OverSettled { .. })
                );
                prop_assert!(is_over_settled);
            }
        }
    }
}
