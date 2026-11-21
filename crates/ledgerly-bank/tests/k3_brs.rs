//! Card K3: bank reconciliation statement (spec 6.5).
//!
//! Every expected value is derived by hand from the standard BRS identity, written next to it:
//! balance as per books - deposits not yet credited by the bank + payments not yet debited by the
//! bank = balance as per bank. All parties, amounts and dates are fictional.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_bank::BankError;
use ledgerly_bank::brs::{Brs, brs};
use ledgerly_bank::statement::{BankStatement, check_statement};
use ledgerly_bankfmt::{RawLine, RawStatement};
use ledgerly_core::{
    CompanyId, Date, GroupId, LedgerId, Month, Outcome, Paise, SignedAmount, StateCode, VoucherId,
    VoucherTypeId,
};
use ledgerly_model::{
    Address, BankAllocation, BankDetails, Books, Company, EntityKind, Features, InstrumentKind,
    Ledger, LedgerLine, Masters, NegativeStockPolicy, Posting, Voucher, VoucherMode, VoucherStatus,
};
use proptest::prelude::*;

const BANK: LedgerId = LedgerId(10);
const PARTY: LedgerId = LedgerId(20);

fn d(month: Month, day: u8) -> Date {
    Date::from_calendar_date(2025, month, day).unwrap()
}

fn apr(day: u8) -> Date {
    d(Month::April, day)
}

fn ledger(id: LedgerId, bank: bool, opening: SignedAmount) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: format!("Fictional ledger {}", id.0),
        aliases: Vec::new(),
        group: GroupId(1),
        opening,
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: bank.then(|| BankDetails {
            account_number: "XXXX0001".to_owned(),
            ifsc: "FICT0000001".to_owned(),
            bank_name: "Fictional Bank".to_owned(),
            branch: "Fictional Branch".to_owned(),
        }),
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn bank_line(
    amount: SignedAmount,
    number: Option<&str>,
    bank_date: Option<Date>,
    with_allocation: bool,
) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger: BANK,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: with_allocation.then(|| BankAllocation {
            instrument: InstrumentKind::Cheque,
            instrument_number: number.map(str::to_owned),
            instrument_date: None,
            bank_date,
            favouring: None,
        }),
    }
}

fn counter_line(amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger: PARTY,
        amount: if amount.side == ledgerly_core::DrCr::Dr {
            SignedAmount::cr(amount.amount)
        } else {
            SignedAmount::dr(amount.amount)
        },
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

/// A voucher with the given bank line and the balancing party line.
fn voucher(id: u64, date: Date, status: VoucherStatus, line: LedgerLine) -> Voucher {
    let counter = counter_line(line.amount);
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: Some(id.to_string()),
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
        ledger_lines: vec![line, counter],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

/// Books with the bank ledger (opening as given) and a postings list mirroring the vouchers.
fn books_with(opening: SignedAmount, bank: bool, vouchers: Vec<Voucher>) -> Books {
    let state = StateCode::new(27).unwrap();
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state,
        pan: None,
        registrations: Vec::new(),
        fy_start: apr(1),
        books_begin: apr(1),
        entity_kind: EntityKind::Proprietorship,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Warn,
    };
    let mut masters = Masters::default();
    masters.ledgers.insert(BANK, ledger(BANK, bank, opening));
    masters
        .ledgers
        .insert(PARTY, ledger(PARTY, false, SignedAmount::dr(Paise::ZERO)));
    let mut postings = Vec::new();
    for v in &vouchers {
        for l in &v.ledger_lines {
            postings.push(Posting {
                voucher: v.id,
                date: v.date,
                ledger: l.ledger,
                amount: l.amount,
                status: v.status,
                type_id: v.type_id,
            });
        }
    }
    postings.sort_by_key(|p| (p.date, p.voucher));
    Books {
        company,
        masters,
        vouchers: vouchers.into_iter().map(|v| (v.id, v)).collect(),
        postings,
        stock_moves: Vec::new(),
    }
}

fn books(vouchers: Vec<Voucher>) -> Books {
    // Bank ledger opening: Dr Rs 1,00,000.00 = 10_000_000 paise.
    books_with(SignedAmount::dr(Paise::new(10_000_000)), true, vouchers)
}

fn raw_line(row: u32, date: Date, debit: i64, credit: i64) -> RawLine {
    RawLine {
        row,
        date,
        value_date: None,
        description: format!("fictional line {row}"),
        reference: None,
        debit: Paise::new(debit),
        credit: Paise::new(credit),
        balance: None,
        raw: vec![format!("cell{row}")],
    }
}

/// A verified statement for [`BANK`]; `tag` makes the file hash distinct.
fn stmt_for(
    ledger: LedgerId,
    tag: &str,
    opening: i64,
    closing: i64,
    lines: Vec<RawLine>,
) -> BankStatement {
    let raw = RawStatement {
        format_id: "fictional-csv".to_owned(),
        account_hint: None,
        opening: Some(Paise::new(opening)),
        closing: Some(Paise::new(closing)),
        lines,
    };
    check_statement(raw, tag.as_bytes(), ledger, &[]).unwrap()
}

fn stmt(tag: &str, opening: i64, closing: i64, lines: Vec<RawLine>) -> BankStatement {
    stmt_for(BANK, tag, opening, closing, lines)
}

fn value(outcome: Result<Outcome<Brs>, BankError>) -> Brs {
    match outcome.unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("unexpected Unsupported: {u:?}"),
    }
}

fn is_unsupported(outcome: Result<Outcome<Brs>, BankError>) -> bool {
    outcome.unwrap().is_unsupported()
}

fn dr(paise: i64) -> SignedAmount {
    SignedAmount::dr(Paise::new(paise))
}

fn cr(paise: i64) -> SignedAmount {
    SignedAmount::cr(Paise::new(paise))
}

// Shared scenario. Bank ledger opens Dr 10_000_000 paise (Rs 1,00,000.00) on 1 Apr 2025.
//   V1  5 Apr  Dr 5_000_000  cheque 000101 deposited, bank date 7 Apr   (cleared)
//   V2 10 Apr  Cr 2_000_000  cheque 000201 issued,    bank date 12 Apr  (cleared)
//   V3 28 Apr  Dr 1_500_000  cheque 000102 deposited, no bank date      (uncleared deposit)
//   V4 29 Apr  Cr   800_000  cheque 000202 issued,    no bank date      (uncleared payment)
//   V5  2 May  Cr   100_000  after the as-of date 30 Apr                (left out)
fn scenario_vouchers() -> Vec<Voucher> {
    vec![
        voucher(
            1,
            apr(5),
            VoucherStatus::Regular,
            bank_line(dr(5_000_000), Some("000101"), Some(apr(7)), true),
        ),
        voucher(
            2,
            apr(10),
            VoucherStatus::Regular,
            bank_line(cr(2_000_000), Some("000201"), Some(apr(12)), true),
        ),
        voucher(
            3,
            apr(28),
            VoucherStatus::Regular,
            bank_line(dr(1_500_000), Some("000102"), None, true),
        ),
        voucher(
            4,
            apr(29),
            VoucherStatus::Regular,
            bank_line(cr(800_000), Some("000202"), None, true),
        ),
        voucher(
            5,
            d(Month::May, 2),
            VoucherStatus::Regular,
            bank_line(cr(100_000), Some("000203"), None, true),
        ),
    ]
}

#[test]
fn books_less_deposits_plus_payments_equals_bank() {
    // Books at 30 Apr: 10_000_000 + 5_000_000 - 2_000_000 + 1_500_000 - 800_000 = 13_700_000.
    // Bank statement: opening 10_000_000; 7 Apr credit 5_000_000; 12 Apr debit 2_000_000;
    // closing 10_000_000 + 5_000_000 - 2_000_000 = 13_000_000.
    // BRS: 13_700_000 - 1_500_000 (uncleared deposit) + 800_000 (uncleared payment)
    //    = 13_000_000 = balance as per bank, difference 0.
    let s = stmt(
        "k3-main",
        10_000_000,
        13_000_000,
        vec![
            raw_line(1, apr(7), 0, 5_000_000),
            raw_line(2, apr(12), 2_000_000, 0),
        ],
    );
    let result = value(brs(&books(scenario_vouchers()), BANK, apr(30), &[s]));
    assert_eq!(result.ledger, BANK);
    assert_eq!(result.as_of, apr(30));
    assert_eq!(result.books_balance, Paise::new(13_700_000));
    assert_eq!(result.uncleared_deposits_total, Paise::new(1_500_000));
    assert_eq!(result.uncleared_payments_total, Paise::new(800_000));
    assert_eq!(result.expected_bank_balance, Paise::new(13_000_000));
    assert_eq!(result.bank_balance, Paise::new(13_000_000));
    assert_eq!(result.difference, Paise::ZERO);
    assert!(result.is_reconciled());
    assert_eq!(result.uncleared_deposits.len(), 1);
    assert_eq!(result.uncleared_deposits[0].voucher, VoucherId(3));
    assert_eq!(result.uncleared_deposits[0].date, apr(28));
    assert_eq!(
        result.uncleared_deposits[0].instrument_number.as_deref(),
        Some("000102")
    );
    assert_eq!(result.uncleared_deposits[0].amount, Paise::new(1_500_000));
    assert_eq!(result.uncleared_payments.len(), 1);
    assert_eq!(result.uncleared_payments[0].voucher, VoucherId(4));
    assert_eq!(result.uncleared_payments[0].amount, Paise::new(800_000));
}

#[test]
fn bank_date_after_the_as_of_date_is_still_uncleared() {
    // Same books, but V2 (payment of 2_000_000) is bank-dated 3 May. As of 30 Apr it is not yet
    // cleared: payments uncleared = 2_000_000 + 800_000 = 2_800_000.
    // Books at 30 Apr 13_700_000. Statement lines up to 30 Apr: only 7 Apr credit 5_000_000, so
    // bank balance at 30 Apr = 10_000_000 + 5_000_000 = 15_000_000 (the 3 May debit is later).
    // BRS: 13_700_000 - 1_500_000 + 2_800_000 = 15_000_000. Difference 0.
    let mut vouchers = scenario_vouchers();
    vouchers[1] = voucher(
        2,
        apr(10),
        VoucherStatus::Regular,
        bank_line(cr(2_000_000), Some("000201"), Some(d(Month::May, 3)), true),
    );
    let s = stmt(
        "k3-late",
        10_000_000,
        13_000_000,
        vec![
            raw_line(1, apr(7), 0, 5_000_000),
            raw_line(2, d(Month::May, 3), 2_000_000, 0),
        ],
    );
    let result = value(brs(&books(vouchers), BANK, apr(30), &[s]));
    assert_eq!(result.uncleared_payments_total, Paise::new(2_800_000));
    assert_eq!(result.uncleared_payments.len(), 2);
    assert_eq!(result.uncleared_payments[0].voucher, VoucherId(2));
    assert_eq!(
        result.uncleared_payments[0].bank_date,
        Some(d(Month::May, 3))
    );
    assert_eq!(result.bank_balance, Paise::new(15_000_000));
    assert_eq!(result.expected_bank_balance, Paise::new(15_000_000));
    assert!(result.is_reconciled());
}

#[test]
fn bank_charge_not_in_books_shows_as_difference() {
    // Books: opening Dr 10_000_000, no vouchers, so books balance 10_000_000 and nothing uncleared;
    // expected bank balance 10_000_000. Statement opens at 10_000_000 and has a 10 Apr debit of
    // 11_800 (Rs 118.00 charge) that is not in the books: bank balance 10_000_000 - 11_800 =
    // 9_988_200. Difference = 9_988_200 - 10_000_000 = -11_800, reported, not absorbed.
    let s = stmt(
        "k3-charge",
        10_000_000,
        9_988_200,
        vec![raw_line(1, apr(10), 11_800, 0)],
    );
    let result = value(brs(&books(Vec::new()), BANK, apr(30), &[s]));
    assert_eq!(result.books_balance, Paise::new(10_000_000));
    assert_eq!(result.bank_balance, Paise::new(9_988_200));
    assert_eq!(result.difference, Paise::new(-11_800));
    assert!(!result.is_reconciled());
}

#[test]
fn only_regular_and_post_dated_vouchers_on_or_before_as_of_count() {
    // Bank ledger opening 10_000_000. Entries, none with a bank date:
    //   V1 Regular     3 Apr  Dr   100_000  counts
    //   V2 Optional    4 Apr  Dr   300_000  left out (kept out of the books)
    //   V3 Cancelled   5 Apr  Dr   700_000  left out (no amounts)
    //   V4 PostDated  20 Apr  Dr   200_000  counts (dated on or before 30 Apr)
    //   V5 PostDated  10 May  Dr 1_000_000  left out (after the as-of date)
    // Books balance = 10_000_000 + 100_000 + 200_000 = 10_300_000; uncleared deposits 300_000;
    // expected bank balance 10_000_000. Statement: opening 10_000_000, one line 8 Apr credit
    // 50_000 so bank = 10_050_000; difference = 10_050_000 - 10_000_000 = 50_000.
    let vouchers = vec![
        voucher(
            1,
            apr(3),
            VoucherStatus::Regular,
            bank_line(dr(100_000), None, None, true),
        ),
        voucher(
            2,
            apr(4),
            VoucherStatus::Optional,
            bank_line(dr(300_000), None, None, true),
        ),
        voucher(
            3,
            apr(5),
            VoucherStatus::Cancelled,
            bank_line(dr(700_000), None, None, true),
        ),
        voucher(
            4,
            apr(20),
            VoucherStatus::PostDated,
            bank_line(dr(200_000), None, None, true),
        ),
        voucher(
            5,
            d(Month::May, 10),
            VoucherStatus::PostDated,
            bank_line(dr(1_000_000), None, None, true),
        ),
    ];
    let s = stmt(
        "k3-status",
        10_000_000,
        10_050_000,
        vec![raw_line(1, apr(8), 0, 50_000)],
    );
    let result = value(brs(&books(vouchers), BANK, apr(30), &[s]));
    assert_eq!(result.books_balance, Paise::new(10_300_000));
    assert_eq!(result.uncleared_deposits_total, Paise::new(300_000));
    assert_eq!(result.expected_bank_balance, Paise::new(10_000_000));
    assert_eq!(result.bank_balance, Paise::new(10_050_000));
    assert_eq!(result.difference, Paise::new(50_000));
}

#[test]
fn bank_line_without_allocation_is_reported_as_uncleared() {
    // One Dr 400_000 bank line with no bank allocation: no bank date, so uncleared.
    // Books 10_000_000 + 400_000 = 10_400_000; expected bank = 10_400_000 - 400_000 = 10_000_000.
    // Statement: opening 10_000_000, 2 Apr debit 100, closing 9_999_900, so bank 9_999_900 and
    // difference = 9_999_900 - 10_000_000 = -100.
    let vouchers = vec![voucher(
        1,
        apr(2),
        VoucherStatus::Regular,
        bank_line(dr(400_000), None, None, false),
    )];
    let s = stmt(
        "k3-noalloc",
        10_000_000,
        9_999_900,
        vec![raw_line(1, apr(2), 100, 0)],
    );
    let result = value(brs(&books(vouchers), BANK, apr(30), &[s]));
    assert_eq!(result.uncleared_deposits.len(), 1);
    assert_eq!(result.uncleared_deposits[0].instrument_number, None);
    assert_eq!(result.books_balance, Paise::new(10_400_000));
    assert_eq!(result.difference, Paise::new(-100));
}

#[test]
fn ledger_without_bank_details_is_not_a_bank_account() {
    let b = books_with(SignedAmount::dr(Paise::ZERO), false, Vec::new());
    let s = stmt("k3-nb", 0, 100, vec![raw_line(1, apr(2), 0, 100)]);
    assert_eq!(
        brs(&b, BANK, apr(30), &[s]),
        Err(BankError::NotABankLedger(BANK))
    );
}

#[test]
fn unknown_ledger_is_an_error() {
    let s = stmt("k3-unk", 0, 100, vec![raw_line(1, apr(2), 0, 100)]);
    let err = brs(&books(Vec::new()), LedgerId(99), apr(30), &[s]).unwrap_err();
    assert!(matches!(err, BankError::Invalid { ref field, .. } if field == "ledger"));
}

#[test]
fn statement_for_another_ledger_is_an_error() {
    let s = stmt_for(
        LedgerId(11),
        "k3-other",
        0,
        100,
        vec![raw_line(1, apr(2), 0, 100)],
    );
    let err = brs(&books(Vec::new()), BANK, apr(30), &[s]).unwrap_err();
    assert!(matches!(err, BankError::Invalid { ref field, .. } if field == "statements"));
}

#[test]
fn the_same_statement_twice_is_an_error() {
    let s = stmt("k3-dup", 0, 100, vec![raw_line(1, apr(2), 0, 100)]);
    let err = brs(&books(Vec::new()), BANK, apr(30), &[s.clone(), s]).unwrap_err();
    assert!(matches!(err, BankError::Invalid { ref field, .. } if field == "statements"));
}

#[test]
fn no_statement_is_unsupported() {
    assert!(is_unsupported(brs(&books(Vec::new()), BANK, apr(30), &[])));
}

#[test]
fn statement_without_lines_is_unsupported() {
    let s = stmt("k3-empty", 500, 500, Vec::new());
    assert!(is_unsupported(brs(&books(Vec::new()), BANK, apr(30), &[s])));
}

#[test]
fn as_of_before_the_first_statement_line_is_unsupported() {
    let s = stmt("k3-early", 0, 100, vec![raw_line(1, apr(10), 0, 100)]);
    assert!(is_unsupported(brs(&books(Vec::new()), BANK, apr(9), &[s])));
}

#[test]
fn chained_statements_give_the_balance_at_the_as_of_date() {
    // S1: opening 10_000_000, 7 Apr credit 5_000_000, closing 15_000_000.
    // S2: opening 15_000_000 (= S1 closing), 20 Apr debit 2_000_000, closing 13_000_000.
    // Books: opening 10_000_000, V1 Dr 5_000_000 bank-dated 7 Apr, V2 Cr 2_000_000 bank-dated
    // 20 Apr: books 13_000_000, nothing uncleared, bank 13_000_000, difference 0.
    // Passed out of order on purpose: the result must not depend on argument order.
    let vouchers = vec![
        voucher(
            1,
            apr(7),
            VoucherStatus::Regular,
            bank_line(dr(5_000_000), None, Some(apr(7)), true),
        ),
        voucher(
            2,
            apr(20),
            VoucherStatus::Regular,
            bank_line(cr(2_000_000), None, Some(apr(20)), true),
        ),
    ];
    let s1 = stmt(
        "k3-c1",
        10_000_000,
        15_000_000,
        vec![raw_line(1, apr(7), 0, 5_000_000)],
    );
    let s2 = stmt(
        "k3-c2",
        15_000_000,
        13_000_000,
        vec![raw_line(1, apr(20), 2_000_000, 0)],
    );
    let result = value(brs(
        &books(vouchers.clone()),
        BANK,
        apr(30),
        &[s2.clone(), s1.clone()],
    ));
    assert_eq!(result.bank_balance, Paise::new(13_000_000));
    assert_eq!(result.books_balance, Paise::new(13_000_000));
    assert!(result.is_reconciled());

    // As of 15 Apr only S1's line counts: bank 15_000_000. Books at 15 Apr: 10_000_000 +
    // 5_000_000 = 15_000_000.
    let mid = value(brs(&books(vouchers), BANK, apr(15), &[s1, s2]));
    assert_eq!(mid.bank_balance, Paise::new(15_000_000));
    assert_eq!(mid.books_balance, Paise::new(15_000_000));
    assert!(mid.is_reconciled());
}

#[test]
fn statements_with_a_gap_are_unsupported() {
    // S2 opens at 14_000_000 but S1 closed at 15_000_000: a statement in between is missing.
    let s1 = stmt(
        "k3-g1",
        10_000_000,
        15_000_000,
        vec![raw_line(1, apr(7), 0, 5_000_000)],
    );
    let s2 = stmt(
        "k3-g2",
        14_000_000,
        13_000_000,
        vec![raw_line(1, apr(20), 1_000_000, 0)],
    );
    assert!(is_unsupported(brs(
        &books(Vec::new()),
        BANK,
        apr(30),
        &[s1, s2]
    )));
}

#[test]
fn overlapping_statements_are_unsupported() {
    // provenance: derived - S1 covers 5 Apr to 20 Apr with opening 10_000_000 and closing
    // 10_000_000 + 100 + 200 = 10_000_300. S2 opens at 10_000_300, so it chains with S1 and only
    // the overlap can fail: it covers 7 Apr, which is inside S1's 5 to 20 Apr range.
    let s1 = stmt(
        "k3-o1",
        10_000_000,
        10_000_300,
        vec![raw_line(1, apr(5), 0, 100), raw_line(2, apr(20), 0, 200)],
    );
    let s2 = stmt(
        "k3-o2",
        10_000_300,
        10_000_400,
        vec![raw_line(1, apr(7), 0, 100)],
    );
    assert!(is_unsupported(brs(
        &books(Vec::new()),
        BANK,
        apr(30),
        &[s1, s2]
    )));
}

#[test]
fn chained_but_overlapping_statements_are_unsupported() {
    // provenance: derived - S1 opens 10_000_000, 5 Apr credit 100, 20 Apr debit 100, closes
    // 10_000_000 (chains: 10_000_000 + 100 - 100). S2 opens 10_000_000 (= S1's closing), 7 Apr
    // credit 100, closes 10_000_100. S2 starts on 7 Apr, before S1 ends on 20 Apr, so the
    // statements overlap and the balance is Unsupported. Before the fix the overlap check compared
    // against S1's start (5 Apr) and accepted this pair as a value.
    let s1 = stmt(
        "k3-co1",
        10_000_000,
        10_000_000,
        vec![raw_line(1, apr(5), 0, 100), raw_line(2, apr(20), 100, 0)],
    );
    let s2 = stmt(
        "k3-co2",
        10_000_000,
        10_000_100,
        vec![raw_line(1, apr(7), 0, 100)],
    );
    assert!(is_unsupported(brs(
        &books(Vec::new()),
        BANK,
        apr(30),
        &[s1, s2]
    )));
}

#[test]
fn postings_that_disagree_with_voucher_lines_are_an_error() {
    // Remove one posting of the bank ledger: the read model no longer matches the vouchers.
    let mut b = books(scenario_vouchers());
    let at = b.postings.iter().position(|p| p.ledger == BANK).unwrap();
    b.postings.remove(at);
    let s = stmt("k3-bad", 0, 100, vec![raw_line(1, apr(2), 0, 100)]);
    let err = brs(&b, BANK, apr(30), &[s]).unwrap_err();
    assert!(matches!(err, BankError::Invalid { ref field, .. } if field == "books"));
}

proptest! {
    // provenance: derived - the BRS identity. Items are (amount, is_debit, cleared). Cleared items
    // are bank-dated 6 Apr and appear on the statement (a debit to the bank ledger is a credit
    // line on the statement, a credit to the bank ledger is a debit line); uncleared items have no
    // bank date and no statement line. So bank = opening + Σ cleared signed amounts, books =
    // opening + Σ all signed amounts, and books - Σ uncleared debits + Σ uncleared credits
    // = opening + Σ cleared signed amounts = bank, difference 0.
    #[test]
    fn identity_holds_for_any_mix_of_cleared_and_uncleared(
        items in proptest::collection::vec((1i64..=1_000_000, any::<bool>(), any::<bool>()), 1..12),
    ) {
        prop_assume!(items.iter().any(|i| i.2));
        let mut vouchers = Vec::new();
        let mut lines = Vec::new();
        let mut all_signed = 0i64;
        let mut cleared_signed = 0i64;
        for (n, (amount, is_debit, cleared)) in items.iter().enumerate() {
            let id = u64::try_from(n).unwrap() + 1;
            let signed = if *is_debit { *amount } else { -*amount };
            all_signed += signed;
            let side = if *is_debit { dr(*amount) } else { cr(*amount) };
            let bank_date = cleared.then(|| apr(6));
            vouchers.push(voucher(
                id,
                apr(5),
                VoucherStatus::Regular,
                bank_line(side, None, bank_date, true),
            ));
            if *cleared {
                cleared_signed += signed;
                let row = u32::try_from(lines.len()).unwrap() + 1;
                lines.push(if *is_debit {
                    raw_line(row, apr(6), 0, *amount)
                } else {
                    raw_line(row, apr(6), *amount, 0)
                });
            }
        }
        let s = stmt("k3-prop", 10_000_000, 10_000_000 + cleared_signed, lines);
        let result = value(brs(&books(vouchers), BANK, apr(30), &[s]));
        prop_assert_eq!(result.books_balance, Paise::new(10_000_000 + all_signed));
        prop_assert_eq!(result.bank_balance, Paise::new(10_000_000 + cleared_signed));
        prop_assert_eq!(result.difference, Paise::ZERO);
    }
}
