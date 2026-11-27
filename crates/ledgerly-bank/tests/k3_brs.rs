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
        group: Some(GroupId(1)),
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

/// The statement-side balance, which the test expects to be known.
fn statement_value(b: &Brs) -> Paise {
    match &b.statement_balance {
        Outcome::Value(p) => *p,
        Outcome::Unsupported(u) => panic!("statement side unexpectedly unsupported: {u:?}"),
    }
}

/// The difference, which the test expects to be known.
fn difference_value(b: &Brs) -> Paise {
    match &b.difference {
        Outcome::Value(p) => *p,
        Outcome::Unsupported(u) => panic!("difference unexpectedly unsupported: {u:?}"),
    }
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

// Provenance for every test below: derived. The rule is TallyPrime's documented bank
// reconciliation model (https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm,
// accessed 2026-11-21): balance as per books on the as-of date; vouchers with a blank or later
// bank date are not reflected in the bank; balance as per bank is the book balance offset by
// those amounts. The page does not give the sign per adjustment (derived here from the identity:
// a deposit is debited in the books and not yet credited by the bank, so it is subtracted; a
// payment is credited in the books and not yet debited by the bank, so it is added back, for a
// debit-balance bank ledger) and does not say which way the difference is subtracted, so the
// difference is our convention: statement balance minus balance as per bank. The statement-side
// rule (a statement covers a date from its first line up to, not including, its last line) is
// ours, derived from the BankStatement type; see the brs module docs.

#[test]
fn books_less_deposits_plus_payments_equals_bank() {
    // Books at 30 Apr: 10_000_000 + 5_000_000 - 2_000_000 + 1_500_000 - 800_000 = 13_700_000.
    // Uncleared (no bank date, or bank date after 30 Apr): deposit 1_500_000 (V3), payment 800_000
    // (V4). V1 (bank date 7 Apr) and V2 (bank date 12 Apr) are cleared. V5 (2 May) is after as-of.
    // Balance as per bank = 13_700_000 - 1_500_000 + 800_000 = 13_000_000.
    // Statement: opening 10_000_000; 7 Apr credit 5_000_000; 12 Apr debit 2_000_000; 3 May credit
    // 1_000_000; closing 14_000_000. It covers 30 Apr (7 Apr <= 30 Apr < 3 May), and its balance
    // at 30 Apr is 10_000_000 + 5_000_000 - 2_000_000 = 13_000_000. Difference 13_000_000 -
    // 13_000_000 = 0.
    // Replaces the round-2 policy in which the statement lines ended the BRS (no statement end
    // date, so the round-2 rule was the whole BRS Unsupported); the books figures are unchanged.
    let s = stmt(
        "k3-main",
        10_000_000,
        14_000_000,
        vec![
            raw_line(1, apr(7), 0, 5_000_000),
            raw_line(2, apr(12), 2_000_000, 0),
            raw_line(3, d(Month::May, 3), 0, 1_000_000),
        ],
    );
    let result = value(brs(&books(scenario_vouchers()), BANK, apr(30), &[s]));
    assert_eq!(result.ledger, BANK);
    assert_eq!(result.as_of, apr(30));
    assert_eq!(result.books_balance, Paise::new(13_700_000));
    assert_eq!(result.uncleared_deposits_total, Paise::new(1_500_000));
    assert_eq!(result.uncleared_payments_total, Paise::new(800_000));
    assert_eq!(result.balance_as_per_bank, Paise::new(13_000_000));
    assert_eq!(statement_value(&result), Paise::new(13_000_000));
    assert_eq!(difference_value(&result), Paise::ZERO);
    assert_eq!(result.is_reconciled(), Outcome::Value(true));
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
fn as_of_inside_the_statement_range_reconciles_at_that_date() {
    // Same statement as above, as of 10 Apr. Books at 10 Apr: 10_000_000 + 5_000_000 - 2_000_000
    // = 13_000_000 (V1 dated 5 Apr, V2 dated 10 Apr; V3 and V4 are later). Uncleared: V2 is a
    // payment of 2_000_000 whose bank date 12 Apr is after 10 Apr; V1 (bank date 7 Apr) is cleared.
    // Balance as per bank = 13_000_000 + 2_000_000 = 15_000_000 (a payment is added back).
    // Statement at 10 Apr: 10_000_000 + 5_000_000 = 15_000_000 (the 12 Apr debit is later).
    // Difference 0. Provenance: derived as above.
    let s = stmt(
        "k3-inside",
        10_000_000,
        14_000_000,
        vec![
            raw_line(1, apr(7), 0, 5_000_000),
            raw_line(2, apr(12), 2_000_000, 0),
            raw_line(3, d(Month::May, 3), 0, 1_000_000),
        ],
    );
    let result = value(brs(&books(scenario_vouchers()), BANK, apr(10), &[s]));
    assert_eq!(result.books_balance, Paise::new(13_000_000));
    assert_eq!(result.uncleared_payments_total, Paise::new(2_000_000));
    assert_eq!(result.uncleared_deposits_total, Paise::ZERO);
    assert_eq!(result.balance_as_per_bank, Paise::new(15_000_000));
    assert_eq!(statement_value(&result), Paise::new(15_000_000));
    assert_eq!(difference_value(&result), Paise::ZERO);
}

#[test]
fn bank_date_after_the_as_of_date_is_still_uncleared() {
    // Books as in the scenario, but V2 (payment 2_000_000) is bank-dated 3 May. As of 30 Apr it is
    // not yet cleared: payments uncleared = 2_000_000 + 800_000 = 2_800_000.
    // Books at 30 Apr 13_700_000. Balance as per bank = 13_700_000 - 1_500_000 + 2_800_000
    // = 15_000_000.
    // Statement: opening 10_000_000; 7 Apr credit 5_000_000; 3 May debit 2_000_000; closing
    // 13_000_000. It covers 30 Apr (7 Apr <= 30 Apr < 3 May); its balance at 30 Apr is
    // 10_000_000 + 5_000_000 = 15_000_000. Difference 0.
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
    assert_eq!(statement_value(&result), Paise::new(15_000_000));
    assert_eq!(result.balance_as_per_bank, Paise::new(15_000_000));
    assert_eq!(result.is_reconciled(), Outcome::Value(true));
}

#[test]
fn bank_charge_not_in_books_shows_as_difference() {
    // Books: opening Dr 10_000_000, no vouchers: books balance 10_000_000, nothing uncleared,
    // balance as per bank 10_000_000. Statement: opening 10_000_000; 10 Apr debit 11_800 (a Rs
    // 118.00 charge not in the books); 3 May credit 100_000 (later than 30 Apr); closing
    // 10_088_200. Statement at 30 Apr = 10_000_000 - 11_800 = 9_988_200. Difference
    // = 9_988_200 - 10_000_000 = -11_800, reported, not absorbed. Provenance: derived as above.
    let s = stmt(
        "k3-charge",
        10_000_000,
        10_088_200,
        vec![
            raw_line(1, apr(10), 11_800, 0),
            raw_line(2, d(Month::May, 3), 0, 100_000),
        ],
    );
    let result = value(brs(&books(Vec::new()), BANK, apr(30), &[s]));
    assert_eq!(result.books_balance, Paise::new(10_000_000));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert_eq!(statement_value(&result), Paise::new(9_988_200));
    assert_eq!(difference_value(&result), Paise::new(-11_800));
    assert_eq!(result.is_reconciled(), Outcome::Value(false));
}

#[test]
fn as_of_after_the_last_statement_line_leaves_only_the_statement_side_unsupported() {
    // Books: opening 10_000_000, no vouchers: books balance and balance as per bank are 10_000_000
    // as of 30 Apr. Statement: one line on 10 Apr (last line before 30 Apr), so no statement
    // covers 30 Apr and the statement side is Unsupported. Replaces the round-2 policy, under
    // which the whole BRS was Unsupported; the books figures are still a value, per the Tally
    // page (balance as per books and balance as per bank do not depend on the statement).
    let s = stmt(
        "k3-after",
        10_000_000,
        9_988_200,
        vec![raw_line(1, apr(10), 11_800, 0)],
    );
    let result = value(brs(&books(Vec::new()), BANK, apr(30), &[s]));
    assert_eq!(result.books_balance, Paise::new(10_000_000));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert!(result.statement_balance.is_unsupported());
    assert!(result.difference.is_unsupported());
    assert!(result.is_reconciled().is_unsupported());
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
    // balance as per bank = 10_000_000. Statement: opening 10_000_000; 8 Apr credit 50_000;
    // 3 May credit 1_000_000; closing 11_050_000. It covers 30 Apr, and its balance there is
    // 10_050_000. Difference = 10_050_000 - 10_000_000 = 50_000.
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
        11_050_000,
        vec![
            raw_line(1, apr(8), 0, 50_000),
            raw_line(2, d(Month::May, 3), 0, 1_000_000),
        ],
    );
    let result = value(brs(&books(vouchers), BANK, apr(30), &[s]));
    assert_eq!(result.books_balance, Paise::new(10_300_000));
    assert_eq!(result.uncleared_deposits_total, Paise::new(300_000));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert_eq!(statement_value(&result), Paise::new(10_050_000));
    assert_eq!(difference_value(&result), Paise::new(50_000));
}

#[test]
fn bank_line_without_allocation_is_reported_as_uncleared() {
    // One Dr 400_000 bank line with no bank allocation: no bank date, so uncleared.
    // Books 10_000_000 + 400_000 = 10_400_000; balance as per bank = 10_400_000 - 400_000 =
    // 10_000_000. Statement: opening 10_000_000; 2 Apr debit 100; 3 May credit 1_000; closing
    // 10_000_900. At 30 Apr it is 10_000_000 - 100 = 9_999_900, so difference = -100.
    let vouchers = vec![voucher(
        1,
        apr(2),
        VoucherStatus::Regular,
        bank_line(dr(400_000), None, None, false),
    )];
    let s = stmt(
        "k3-noalloc",
        10_000_000,
        10_000_900,
        vec![
            raw_line(1, apr(2), 100, 0),
            raw_line(2, d(Month::May, 3), 0, 1_000),
        ],
    );
    let result = value(brs(&books(vouchers), BANK, apr(30), &[s]));
    assert_eq!(result.uncleared_deposits.len(), 1);
    assert_eq!(result.uncleared_deposits[0].instrument_number, None);
    assert_eq!(result.books_balance, Paise::new(10_400_000));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert_eq!(difference_value(&result), Paise::new(-100));
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
fn no_statement_leaves_the_books_side_as_a_value() {
    // Replaces the round-2 policy (no statement made the whole BRS Unsupported). Books: opening
    // 10_000_000, no vouchers: books balance and balance as per bank are 10_000_000. The
    // statement side is Unsupported because no statement was imported.
    let result = value(brs(&books(Vec::new()), BANK, apr(30), &[]));
    assert_eq!(result.books_balance, Paise::new(10_000_000));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert!(result.statement_balance.is_unsupported());
    assert!(result.difference.is_unsupported());
}

#[test]
fn statement_without_lines_leaves_the_statement_side_unsupported() {
    // A statement with no lines covers no date (its position in time is unknown), so the
    // statement side is Unsupported. Books: 10_000_000 as of 30 Apr. Replaces the round-2 policy.
    let s = stmt("k3-empty", 500, 500, Vec::new());
    let result = value(brs(&books(Vec::new()), BANK, apr(30), &[s]));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert!(result.statement_balance.is_unsupported());
}

#[test]
fn as_of_before_the_first_statement_line_leaves_the_statement_side_unsupported() {
    // Statement covers from 10 Apr. As of 9 Apr no statement covers the date, so the statement
    // side is Unsupported and the books side is 10_000_000 (no vouchers). Replaces the round-2 policy.
    let s = stmt("k3-early", 0, 100, vec![raw_line(1, apr(10), 0, 100)]);
    let result = value(brs(&books(Vec::new()), BANK, apr(9), &[s]));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert!(result.statement_balance.is_unsupported());
}

#[test]
fn chained_statements_give_the_balance_at_the_as_of_date() {
    // S1: opening 10_000_000; 7 Apr credit 5_000_000; 14 Apr credit 1_000_000; closing 16_000_000.
    // S2: opening 16_000_000 (= S1 closing); 20 Apr debit 2_000_000; 26 Apr debit 1_000_000;
    //     closing 13_000_000.
    // Books: V1 Dr 5_000_000 bank 7 Apr; V2 Dr 1_000_000 bank 14 Apr; V3 Cr 2_000_000 bank 20 Apr;
    //     V4 Cr 1_000_000 bank 26 Apr. Books at 30 Apr: 10M + 5M + 1M - 2M - 1M = 13_000_000.
    // As of 10 Apr: books 15_000_000 (V1 only, cleared). Only S1 covers 10 Apr (7 Apr <= 10 Apr <
    // 14 Apr): balance 10_000_000 + 5_000_000 = 15_000_000. Difference 0.
    // As of 22 Apr: books 14_000_000 (V1, V2, V3; V4 is later). Only S2 covers 22 Apr (20 Apr <=
    // 22 Apr < 26 Apr): balance 16_000_000 - 2_000_000 = 14_000_000. Difference 0.
    // As of 30 Apr: no statement covers the date (S2 ends 26 Apr), so the statement side is
    // Unsupported; books 13_000_000. Passed out of order on purpose.
    let vouchers = vec![
        voucher(
            1,
            apr(7),
            VoucherStatus::Regular,
            bank_line(dr(5_000_000), None, Some(apr(7)), true),
        ),
        voucher(
            2,
            apr(14),
            VoucherStatus::Regular,
            bank_line(dr(1_000_000), None, Some(apr(14)), true),
        ),
        voucher(
            3,
            apr(20),
            VoucherStatus::Regular,
            bank_line(cr(2_000_000), None, Some(apr(20)), true),
        ),
        voucher(
            4,
            apr(26),
            VoucherStatus::Regular,
            bank_line(cr(1_000_000), None, Some(apr(26)), true),
        ),
    ];
    let s1 = stmt(
        "k3-c1",
        10_000_000,
        16_000_000,
        vec![
            raw_line(1, apr(7), 0, 5_000_000),
            raw_line(2, apr(14), 0, 1_000_000),
        ],
    );
    let s2 = stmt(
        "k3-c2",
        16_000_000,
        13_000_000,
        vec![
            raw_line(1, apr(20), 2_000_000, 0),
            raw_line(2, apr(26), 1_000_000, 0),
        ],
    );
    let books = books(vouchers);

    let at_22 = value(brs(&books, BANK, apr(22), &[s2.clone(), s1.clone()]));
    assert_eq!(at_22.books_balance, Paise::new(14_000_000));
    assert_eq!(statement_value(&at_22), Paise::new(14_000_000));
    assert_eq!(difference_value(&at_22), Paise::ZERO);

    let at_10 = value(brs(&books, BANK, apr(10), &[s1.clone(), s2.clone()]));
    assert_eq!(at_10.books_balance, Paise::new(15_000_000));
    assert_eq!(statement_value(&at_10), Paise::new(15_000_000));
    assert_eq!(difference_value(&at_10), Paise::ZERO);

    let at_30 = value(brs(&books, BANK, apr(30), &[s1, s2]));
    assert_eq!(at_30.books_balance, Paise::new(13_000_000));
    assert!(at_30.statement_balance.is_unsupported());
}

#[test]
fn gap_between_statements_leaves_dates_in_the_gap_unsupported() {
    // S1: opening 10_000_000; 7 Apr credit 5_000_000; 12 Apr credit 1_000_000; closing 16_000_000.
    // S2: opening 14_000_000 (a gap of 2_000_000: S1 closed at 16_000_000); 20 Apr debit
    //     1_000_000; 25 Apr debit 1_000_000; closing 12_000_000.
    // Books: no vouchers, so 10_000_000 on every date.
    // As of 15 Apr: no statement covers it (S1 ends at 12 Apr, S2 starts at 20 Apr): statement
    //     side Unsupported.
    // As of 10 Apr: only S1 covers it: 15_000_000; difference 5_000_000.
    // As of 22 Apr: only S2 covers it (20 Apr <= 22 Apr < 25 Apr): 14_000_000 - 1_000_000 =
    //     13_000_000; difference 3_000_000. The gap does not affect a date that one statement covers
    //     exactly. Replaces the round-2 policy (a gap made the whole BRS Unsupported).
    let s1 = stmt(
        "k3-g1",
        10_000_000,
        16_000_000,
        vec![
            raw_line(1, apr(7), 0, 5_000_000),
            raw_line(2, apr(12), 0, 1_000_000),
        ],
    );
    let s2 = stmt(
        "k3-g2",
        14_000_000,
        12_000_000,
        vec![
            raw_line(1, apr(20), 1_000_000, 0),
            raw_line(2, apr(25), 1_000_000, 0),
        ],
    );
    let in_gap = value(brs(
        &books(Vec::new()),
        BANK,
        apr(15),
        &[s1.clone(), s2.clone()],
    ));
    assert!(in_gap.statement_balance.is_unsupported());
    assert_eq!(in_gap.balance_as_per_bank, Paise::new(10_000_000));

    let at_10 = value(brs(
        &books(Vec::new()),
        BANK,
        apr(10),
        &[s1.clone(), s2.clone()],
    ));
    assert_eq!(statement_value(&at_10), Paise::new(15_000_000));
    assert_eq!(difference_value(&at_10), Paise::new(5_000_000));

    let at_22 = value(brs(&books(Vec::new()), BANK, apr(22), &[s1, s2]));
    assert_eq!(statement_value(&at_22), Paise::new(13_000_000));
    assert_eq!(difference_value(&at_22), Paise::new(3_000_000));
}

#[test]
fn overlapping_statements_do_not_block_the_books_side_when_none_covers_the_date() {
    // provenance: derived - S1 covers 5 Apr to 20 Apr with opening 10_000_000 and closing
    // 10_000_300. S2 opens at 10_000_300 and has one line on 7 Apr, so it covers no date (its
    // first and last line are both 7 Apr). As of 30 Apr no statement covers the date: statement
    // side Unsupported; books 10_000_000. Replaces the round-2 policy (overlap made the whole BRS
    // Unsupported). The overlap itself is decided only when two statements cover the same date
    // (see the agreement and disagreement tests).
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
    let result = value(brs(&books(Vec::new()), BANK, apr(30), &[s1, s2]));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert!(result.statement_balance.is_unsupported());
}

#[test]
fn overlapping_statements_that_agree_at_the_as_of_date_give_the_balance() {
    // S1: opening 10_000_000; 5 Apr credit 100; 20 Apr credit 200; closing 10_000_300.
    // S2: opening 10_000_000; 7 Apr credit 100; 9 Apr credit 1; closing 10_000_101.
    // As of 8 Apr both cover the date (S1: 5 Apr <= 8 Apr < 20 Apr; S2: 7 Apr <= 8 Apr < 9 Apr).
    // S1 balance: 10_000_000 + 100 = 10_000_100. S2 balance: 10_000_000 + 100 = 10_000_100. They
    // agree, so the statement side is 10_000_100. Books 10_000_000; difference 100.
    // Provenance: derived (ours, from the BankStatement type; see the brs module docs).
    let s1 = stmt(
        "k3-a1",
        10_000_000,
        10_000_300,
        vec![raw_line(1, apr(5), 0, 100), raw_line(2, apr(20), 0, 200)],
    );
    let s2 = stmt(
        "k3-a2",
        10_000_000,
        10_000_101,
        vec![raw_line(1, apr(7), 0, 100), raw_line(2, apr(9), 0, 1)],
    );
    let result = value(brs(&books(Vec::new()), BANK, apr(8), &[s1, s2]));
    assert_eq!(statement_value(&result), Paise::new(10_000_100));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert_eq!(difference_value(&result), Paise::new(100));
}

#[test]
fn overlapping_statements_that_disagree_at_the_as_of_date_are_unsupported() {
    // S1 as above: at 8 Apr its balance is 10_000_100. S2: opening 10_000_000; 7 Apr credit 200;
    // 9 Apr credit 1; closing 10_000_201. At 8 Apr S2 gives 10_000_000 + 200 = 10_000_200.
    // The two statements give different balances for the same date, so one of them is wrong or
    // incomplete: statement side Unsupported, books side still 10_000_000.
    // Provenance: derived (ours; see the brs module docs).
    let s1 = stmt(
        "k3-d1",
        10_000_000,
        10_000_300,
        vec![raw_line(1, apr(5), 0, 100), raw_line(2, apr(20), 0, 200)],
    );
    let s2 = stmt(
        "k3-d2",
        10_000_000,
        10_000_201,
        vec![raw_line(1, apr(7), 0, 200), raw_line(2, apr(9), 0, 1)],
    );
    let result = value(brs(&books(Vec::new()), BANK, apr(8), &[s1, s2]));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert!(result.statement_balance.is_unsupported());
    assert!(result.difference.is_unsupported());
}

#[test]
fn chained_but_overlapping_statements_are_unsupported_by_coverage() {
    // provenance: derived - S1 opens 10_000_000, 5 Apr credit 100, 20 Apr debit 100, closes
    // 10_000_000 (chains: 10_000_000 + 100 - 100). S2 opens 10_000_000 (= S1's closing), 7 Apr
    // credit 100, closes 10_000_100. S2 starts on 7 Apr, before S1 ends on 20 Apr, so the two
    // statements overlap. Under the round-2 policy the whole BRS was Unsupported. Replaces the
    // round-2 policy: overlap now decides only the statement-side figure, and only when two
    // statements cover the same as-of date. At 30 Apr neither statement covers the date (S1 ends
    // 20 Apr, S2 has one line on 7 Apr), so the statement side is Unsupported and the books side
    // is a value: balance as per books 10_000_000 (no vouchers), balance as per bank 10_000_000.
    // Provenance: derived (ours; see the brs module docs).
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
    let result = value(brs(&books(Vec::new()), BANK, apr(30), &[s1, s2]));
    assert_eq!(result.books_balance, Paise::new(10_000_000));
    assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000));
    assert!(result.statement_balance.is_unsupported());
    assert!(result.difference.is_unsupported());
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
    // The as-of date is 6 Apr (replaces the earlier 30 Apr): a statement covers a date only before
    // its last line, so a zero-amount marker line on 7 Apr ends the statement. The marker does not
    // change any balance.
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
        let marker = u32::try_from(lines.len()).unwrap() + 1;
        lines.push(raw_line(marker, apr(7), 0, 0));
        let s = stmt("k3-prop", 10_000_000, 10_000_000 + cleared_signed, lines);
        let result = value(brs(&books(vouchers), BANK, apr(6), &[s]));
        prop_assert_eq!(result.books_balance, Paise::new(10_000_000 + all_signed));
        prop_assert_eq!(result.balance_as_per_bank, Paise::new(10_000_000 + cleared_signed));
        prop_assert_eq!(statement_value(&result), Paise::new(10_000_000 + cleared_signed));
        prop_assert_eq!(difference_value(&result), Paise::ZERO);
    }
}
