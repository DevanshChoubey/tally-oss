//! Card K2: suggesting matches between statement lines and vouchers (spec 6.5).
//!
//! Every expected value below is derived by hand from the plan's card K2 text ("many-to-many (one
//! deposit, three receipts); amount + date +-3 days; rule `NEFT-*-RENT` -> Rent ledger
//! suggestion; matches are suggestions until confirmed") and spec 6.5 ("only user-confirmed
//! matches set the bank date"). The date window, the deposit/withdrawal direction and the `*`
//! pattern semantics are the definitions written in `ledgerly_bank::matcher`'s module docs; the
//! arithmetic is written next to each case. No expected value is an output of the matcher.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_bank::matcher::{
    Candidate, DATE_TOLERANCE_DAYS, LineDirection, MatchRule, pattern_matches, suggest_matches,
};
use ledgerly_bank::statement::{BankStatement, check_statement};
use ledgerly_bankfmt::{RawLine, RawStatement};
use ledgerly_core::{
    CompanyId, Date, LedgerId, Month, Paise, SignedAmount, StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BankAllocation, Books, Company, EntityKind, Features, InstrumentKind, LedgerLine,
    Masters, MatchKind, NegativeStockPolicy, Voucher, VoucherMode, VoucherStatus,
};

const BANK: LedgerId = LedgerId(10);
const PARTY: LedgerId = LedgerId(20);
const RENT: LedgerId = LedgerId(30);
const OTHER: LedgerId = LedgerId(31);

fn d(day: u8) -> Date {
    Date::from_calendar_date(2025, Month::April, day).unwrap()
}

fn line(row: u32, day: u8, debit: i64, credit: i64, description: &str) -> RawLine {
    RawLine {
        row,
        date: d(day),
        value_date: None,
        description: description.to_string(),
        reference: None,
        debit: Paise::new(debit),
        credit: Paise::new(credit),
        balance: None,
        raw: vec![format!("cell{row}")],
    }
}

/// Opening `1_000_000` paise; closing is opening + credits - debits (scaffolding so the K1 balance
/// check accepts the file; it is not an expected value of any test).
fn statement(lines: Vec<RawLine>) -> BankStatement {
    let net: i64 = lines.iter().map(|l| l.credit.get() - l.debit.get()).sum();
    let raw = RawStatement {
        format_id: "fictional-csv".to_string(),
        account_hint: None,
        opening: Some(Paise::new(1_000_000)),
        closing: Some(Paise::new(1_000_000 + net)),
        lines,
    };
    check_statement(raw, b"k2", BANK, &[]).unwrap()
}

fn bank_line(
    amount: SignedAmount,
    instrument_number: Option<&str>,
    cleared: Option<Date>,
) -> LedgerLine {
    LedgerLine {
        ledger: BANK,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: Some(BankAllocation {
            instrument: InstrumentKind::Neft,
            instrument_number: instrument_number.map(str::to_string),
            instrument_date: None,
            bank_date: cleared,
            favouring: None,
        }),
    }
}

fn plain_line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn voucher_with(id: u64, day: u8, bank: LedgerLine, counter: LedgerLine) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(1),
        number: None,
        series: None,
        date: d(day),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![bank, counter],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

/// A receipt: debit the bank, credit the party.
fn receipt(id: u64, day: u8, paise: i64) -> Voucher {
    voucher_with(
        id,
        day,
        bank_line(SignedAmount::dr(Paise::new(paise)), None, None),
        plain_line(PARTY, SignedAmount::cr(Paise::new(paise))),
    )
}

/// A payment: credit the bank, debit the party.
fn payment(id: u64, day: u8, paise: i64) -> Voucher {
    voucher_with(
        id,
        day,
        bank_line(SignedAmount::cr(Paise::new(paise)), None, None),
        plain_line(PARTY, SignedAmount::dr(Paise::new(paise))),
    )
}

fn books(vouchers: Vec<Voucher>) -> Books {
    Books {
        company: Company {
            id: CompanyId(1),
            name: "Fictional Traders".to_string(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: d(1),
            books_begin: d(1),
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters: Masters::default(),
        vouchers: vouchers.into_iter().map(|v| (v.id, v)).collect(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn run(lines: Vec<RawLine>, vouchers: Vec<Voucher>, rules: &[MatchRule]) -> Vec<Candidate> {
    suggest_matches(&statement(lines), &books(vouchers), rules)
}

#[test]
fn one_deposit_matches_three_receipts() {
    // provenance: derived - card K2 "many-to-many (one deposit, three receipts)".
    // Receipts 150_000 + 250_000 + 100_000 = 500_000 paise (Rs 5,000.00) = the deposit.
    // Dates 2, 3, 4 April are 3, 2, 1 days from the deposit on 5 April: all inside +-3.
    // Receipt 104 (77_777 paise) is a distractor: no pair or triple including it sums to 500_000
    // (pairs: 150_000+77_777=227_777, 250_000+77_777=327_777, 100_000+77_777=177_777; triples with
    // it: 150_000+250_000+77_777=477_777, 150_000+100_000+77_777=327_777, 250_000+100_000+77_777
    // =427_777), so the only solution is {101, 102, 103}.
    let out = run(
        vec![line(1, 5, 0, 500_000, "BY CLG DEPOSIT")],
        vec![
            receipt(101, 2, 150_000),
            receipt(102, 3, 250_000),
            receipt(103, 4, 100_000),
            receipt(104, 5, 77_777),
        ],
        &[],
    );
    assert_eq!(out.len(), 1);
    let c = &out[0];
    assert_eq!(c.statement_lines, vec![1]);
    assert_eq!(
        c.vouchers,
        vec![VoucherId(101), VoucherId(102), VoucherId(103)]
    );
    assert_eq!(c.kind, MatchKind::Exact);
    assert_eq!(c.suggested_ledger, None);
    assert_eq!(c.proposed_bank_date, d(5));
    assert!(!c.ambiguous);
}

#[test]
fn deposit_that_the_receipts_do_not_add_up_to_is_not_matched() {
    // provenance: derived - 150_000 + 250_000 + 100_000 = 500_000, so a deposit of 499_999
    // (one paisa less) has no exact combination: no suggestion, never a near-miss.
    let out = run(
        vec![line(1, 5, 0, 499_999, "BY CLG DEPOSIT")],
        vec![
            receipt(101, 2, 150_000),
            receipt(102, 3, 250_000),
            receipt(103, 4, 100_000),
        ],
        &[],
    );
    assert_eq!(out, [] as [ledgerly_bank::matcher::Candidate; 0]);
}

#[test]
fn one_voucher_matches_several_lines() {
    // provenance: derived - 60_000 + 40_000 = 100_000 paise; the voucher is dated 5 April and the
    // lines 4 and 6 April, each 1 day away. The proposed bank date is the later line, 6 April.
    let out = run(
        vec![
            line(1, 4, 0, 60_000, "CHQ DEP"),
            line(2, 6, 0, 40_000, "CHQ DEP"),
        ],
        vec![receipt(7, 5, 100_000)],
        &[],
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].statement_lines, vec![1, 2]);
    assert_eq!(out[0].vouchers, vec![VoucherId(7)]);
    assert_eq!(out[0].kind, MatchKind::Exact);
    assert_eq!(out[0].proposed_bank_date, d(6));
}

#[test]
fn date_window_is_three_days_each_way_inclusive() {
    // provenance: derived - card K2 "amount + date +-3 days". The line is on 10 April. A voucher
    // on 7 April is 3 days before and on 13 April 3 days after: both match. 6 April and 14 April
    // are 4 days away: no match.
    assert_eq!(DATE_TOLERANCE_DAYS, 3);
    for (day, expect) in [
        (7_u8, true),
        (13, true),
        (6, false),
        (14, false),
        (10, true),
    ] {
        let out = run(
            vec![line(1, 10, 0, 100_000, "NEFT IN")],
            vec![receipt(1, day, 100_000)],
            &[],
        );
        assert_eq!(out.len(), usize::from(expect), "voucher on {day} April");
    }
}

#[test]
fn amount_must_be_equal_to_the_paisa() {
    // provenance: derived - 100_000 != 100_001 and != 99_999, so neither voucher matches.
    for paise in [100_001, 99_999] {
        let out = run(
            vec![line(1, 10, 0, 100_000, "NEFT IN")],
            vec![receipt(1, 10, paise)],
            &[],
        );
        assert!(out.is_empty(), "voucher of {paise} paise");
    }
}

#[test]
fn deposits_match_receipts_and_withdrawals_match_payments_only() {
    // provenance: derived - a statement credit is money into the account, which is a debit to the
    // bank ledger (a receipt); a statement debit is a credit to the bank ledger (a payment).
    let deposit = line(1, 10, 0, 100_000, "IN");
    let withdrawal = line(1, 10, 100_000, 0, "OUT");
    assert_eq!(
        run(vec![deposit.clone()], vec![receipt(1, 10, 100_000)], &[]).len(),
        1
    );
    assert_eq!(
        run(vec![deposit], vec![payment(1, 10, 100_000)], &[]),
        [] as [ledgerly_bank::matcher::Candidate; 0]
    );
    assert_eq!(
        run(vec![withdrawal.clone()], vec![payment(1, 10, 100_000)], &[]).len(),
        1
    );
    assert_eq!(
        run(vec![withdrawal], vec![receipt(1, 10, 100_000)], &[]),
        [] as [ledgerly_bank::matcher::Candidate; 0]
    );
}

#[test]
fn rule_neft_rent_suggests_the_rent_ledger() {
    // provenance: derived - card K2 rule `"NEFT-*-RENT"` -> Rent ledger suggestion. No voucher
    // exists for the 2_500_000 paise (Rs 25,000.00) withdrawal, so the suggestion carries the rule's
    // ledger and no voucher.
    let rules = [MatchRule::new("NEFT-*-RENT", RENT)];
    let out = run(
        vec![line(3, 12, 2_500_000, 0, "NEFT-HDFC0001234-RENT")],
        Vec::new(),
        &rules,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, MatchKind::Rule);
    assert_eq!(out[0].suggested_ledger, Some(RENT));
    assert_eq!(out[0].statement_lines, vec![3]);
    assert_eq!(out[0].vouchers, [] as [ledgerly_core::VoucherId; 0]);
    assert_eq!(out[0].proposed_bank_date, d(12));

    let none = run(
        vec![line(3, 12, 2_500_000, 0, "NEFT-HDFC0001234-SALARY")],
        Vec::new(),
        &rules,
    );
    assert_eq!(none, [] as [ledgerly_bank::matcher::Candidate; 0]);
}

#[test]
fn first_matching_rule_wins_and_direction_restricts_a_rule() {
    // provenance: derived - rules are tried in the order given (module docs); a rule restricted
    // to deposits does not apply to a withdrawal.
    let rules = [
        MatchRule::new("NEFT-*", OTHER),
        MatchRule::new("NEFT-*-RENT", RENT),
    ];
    let out = run(vec![line(1, 12, 100, 0, "NEFT-X-RENT")], Vec::new(), &rules);
    assert_eq!(out[0].suggested_ledger, Some(OTHER));

    let deposit_only = [MatchRule {
        pattern: "NEFT-*".to_string(),
        ledger: OTHER,
        direction: Some(LineDirection::Deposit),
    }];
    assert_eq!(
        run(
            vec![line(1, 12, 100, 0, "NEFT-X")],
            Vec::new(),
            &deposit_only
        ),
        [] as [ledgerly_bank::matcher::Candidate; 0]
    );
    assert_eq!(
        run(
            vec![line(1, 12, 0, 100, "NEFT-X")],
            Vec::new(),
            &deposit_only
        )
        .len(),
        1
    );
}

#[test]
fn a_voucher_match_takes_priority_over_a_rule() {
    // provenance: derived - module docs: rules are only for lines no voucher claimed. The
    // 2_500_000 paise withdrawal has a payment of the same amount a day earlier.
    let rules = [MatchRule::new("NEFT-*-RENT", RENT)];
    let out = run(
        vec![line(1, 12, 2_500_000, 0, "NEFT-HDFC0001234-RENT")],
        vec![payment(5, 11, 2_500_000)],
        &rules,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, MatchKind::Exact);
    assert_eq!(out[0].vouchers, vec![VoucherId(5)]);
}

#[test]
fn suggestions_never_change_the_books_or_confirm_anything() {
    // provenance: derived - spec 6.5 "only user-confirmed matches set the bank date". The books
    // are compared before and after; every bank line still has no bank date; every candidate is
    // Exact or Rule (never Manual, and never a kind that implies confirmation).
    let stmt = statement(vec![
        line(1, 5, 0, 500_000, "DEP"),
        line(2, 6, 2_500_000, 0, "NEFT-A-RENT"),
    ]);
    let all = books(vec![receipt(1, 4, 200_000), receipt(2, 5, 300_000)]);
    let before = all.clone();
    let out = suggest_matches(&stmt, &all, &[MatchRule::new("NEFT-*-RENT", RENT)]);
    assert_eq!(all, before);
    assert_eq!(out.len(), 2);
    for c in &out {
        assert!(matches!(c.kind, MatchKind::Exact | MatchKind::Rule));
    }
    for v in all.vouchers.values() {
        for l in &v.ledger_lines {
            assert!(l.bank.as_ref().is_none_or(|b| b.bank_date.is_none()));
        }
    }
}

#[test]
fn cancelled_optional_and_already_cleared_vouchers_are_not_offered() {
    // provenance: derived - a cancelled voucher has no amounts and an optional one is outside the
    // books (spec 6.4/6.5 voucher statuses); a bank line that already carries a bank date was
    // confirmed earlier.
    let mut cancelled = receipt(1, 10, 100_000);
    cancelled.status = VoucherStatus::Cancelled;
    let mut optional = receipt(2, 10, 100_000);
    optional.status = VoucherStatus::Optional;
    let cleared = voucher_with(
        3,
        10,
        bank_line(SignedAmount::dr(Paise::new(100_000)), None, Some(d(10))),
        plain_line(PARTY, SignedAmount::cr(Paise::new(100_000))),
    );
    let out = run(
        vec![line(1, 10, 0, 100_000, "IN")],
        vec![cancelled, optional, cleared],
        &[],
    );
    assert_eq!(out, [] as [ledgerly_bank::matcher::Candidate; 0]);
}

#[test]
fn vouchers_not_on_the_statements_bank_ledger_are_ignored() {
    // provenance: derived - a receipt into another bank ledger (id 11) is not this account's.
    let mut other = receipt(1, 10, 100_000);
    if let Some(first) = other.ledger_lines.first_mut() {
        first.ledger = LedgerId(11);
    }
    let out = run(vec![line(1, 10, 0, 100_000, "IN")], vec![other], &[]);
    assert_eq!(out, [] as [ledgerly_bank::matcher::Candidate; 0]);
}

#[test]
fn two_equal_vouchers_are_both_offered_as_ambiguous() {
    // provenance: derived - two receipts of 100_000 paise on 4 and 6 April, both 1 day from the
    // line on 5 April; nothing distinguishes them, so both are offered and flagged.
    let out = run(
        vec![line(1, 5, 0, 100_000, "IN")],
        vec![receipt(1, 4, 100_000), receipt(2, 6, 100_000)],
        &[],
    );
    assert_eq!(out.len(), 2);
    assert!(
        out.iter()
            .all(|c| c.ambiguous && c.statement_lines == vec![1])
    );
    assert_eq!(out[0].vouchers, vec![VoucherId(1)]);
    assert_eq!(out[1].vouchers, vec![VoucherId(2)]);
}

#[test]
fn a_matching_reference_number_breaks_the_tie() {
    // provenance: derived - as above, but the statement line's reference "000123" equals the
    // instrument number on voucher 2's bank line (compared ignoring case and surrounding space),
    // so only voucher 2 is offered, unambiguously.
    let mut with_ref = line(1, 5, 0, 100_000, "IN");
    with_ref.reference = Some(" 000123 ".to_string());
    let v2 = voucher_with(
        2,
        6,
        bank_line(SignedAmount::dr(Paise::new(100_000)), Some("000123"), None),
        plain_line(PARTY, SignedAmount::cr(Paise::new(100_000))),
    );
    let out = run(vec![with_ref], vec![receipt(1, 4, 100_000), v2], &[]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].vouchers, vec![VoucherId(2)]);
    assert!(!out[0].ambiguous);
}

#[test]
fn pattern_semantics() {
    // provenance: derived - the definition in `MatchRule`'s docs: whole string, ignoring case,
    // `*` = any run of characters including none, nothing else is special.
    let cases = [
        ("NEFT-*-RENT", "NEFT-A-RENT", true),
        ("neft-*-rent", "NEFT-A-RENT", true),
        ("NEFT-*-RENT", "NEFT-A-RENT-APR", false),
        ("NEFT-*-RENT", "NEFT--RENT", true),
        ("*", "", true),
        ("*", "anything", true),
        ("a*b*c", "aXXbYYc", true),
        ("a*b*c", "aXXbYY", false),
        ("a**b", "ab", true),
        ("*RENT*", "pay rent apr", true),
        ("", "", true),
        ("", "x", false),
        ("ab", "abc", false),
        ("a?c", "abc", false),
    ];
    for (pattern, text, expect) in cases {
        assert_eq!(
            pattern_matches(pattern, text),
            expect,
            "{pattern:?} vs {text:?}"
        );
    }
}

#[test]
fn two_lines_competing_for_one_voucher_are_both_ambiguous() {
    // provenance: derived - row 1 is a credit of 100_000 paise on 5 April and row 2 a credit of
    // the same amount on 6 April; the only voucher is one receipt of 100_000 on 6 April. Both rows
    // fit it (1 day and 0 days apart), so both candidates name it and both are ambiguous. Neither
    // row may take the receipt just because it was looked at first.
    let out = run(
        vec![
            line(1, 5, 0, 100_000, "IN A"),
            line(2, 6, 0, 100_000, "IN B"),
        ],
        vec![receipt(1, 6, 100_000)],
        &[],
    );
    assert_eq!(out.len(), 2);
    assert!(
        out.iter()
            .all(|c| c.ambiguous && c.vouchers == vec![VoucherId(1)])
    );
    assert_eq!(out[0].statement_lines, vec![1]);
    assert_eq!(out[1].statement_lines, vec![2]);
}

#[test]
fn a_pass_two_claim_cannot_take_a_voucher_that_an_ambiguous_line_offers() {
    // provenance: derived - row 1 is 100_000 on 5 April and fits receipts 1 (5 April) and 2
    // (4 April), both 100_000, so row 1 is ambiguous and both receipts are offered. Rows 2 and 3
    // are 50_000 each on 5 April and add up to receipt 1's amount, which pass 2 would claim for
    // receipt 1 if it were free. No candidate may claim receipt 1 or 2 unambiguously.
    let out = run(
        vec![
            line(1, 5, 0, 100_000, "IN"),
            line(2, 5, 0, 50_000, "IN"),
            line(3, 5, 0, 50_000, "IN"),
        ],
        vec![receipt(1, 5, 100_000), receipt(2, 4, 100_000)],
        &[],
    );
    assert!(out.iter().any(|c| c.statement_lines == vec![1]));
    for c in out.iter().filter(|c| {
        c.vouchers.contains(&VoucherId(1)) || c.vouchers.contains(&VoucherId(2))
    }) {
        assert!(c.ambiguous, "unambiguous claim on a contested voucher: {c:?}");
    }
}

#[test]
fn a_pair_found_while_the_search_pool_was_cut_short_is_ambiguous() {
    // provenance: derived - the deposit is 100_000 paise on 5 April. Twenty-four receipts are dated
    // 5 April, which fill the search pool (POOL_CAP = 24): 40_000 and 60_000 among them, and 22
    // others from 70_000 up, so no other pair or triple reaches 100_000. Six more receipts are
    // dated 8 April, 3 days away (inside the window, outside the pool): 30_000 and 70_000 form a
    // second pair that sums to 100_000 and is never searched. The pair in the pool is offered, but
    // as ambiguous, because the search was cut short.
    let mut vouchers = vec![receipt(1, 5, 40_000), receipt(2, 5, 60_000)];
    let mut id = 3_u64;
    for k in 0..22 {
        vouchers.push(receipt(id, 5, 70_000 + k));
        id += 1;
    }
    vouchers.push(receipt(id, 8, 30_000));
    vouchers.push(receipt(id + 1, 8, 70_000));
    let mut later = id + 2;
    for k in 0..4_i64 {
        vouchers.push(receipt(later, 8, 80_000 + k));
        later += 1;
    }
    let out = run(vec![line(1, 5, 0, 100_000, "IN")], vouchers, &[]);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].vouchers, vec![VoucherId(1), VoucherId(2)]);
    assert!(out[0].ambiguous);
}

#[test]
fn a_pair_and_a_triple_with_the_same_sum_are_both_offered() {
    // provenance: derived - the deposit is 30_000 on 5 April. Receipts X 10_000 and Y 20_000 make
    // a pair of 30_000; receipts P 12_000, Q 9_000 and R 9_000 make a triple of 30_000. Checked by
    // hand: the other pairs sum to 22_000 (X+P), 19_000 (X+Q, X+R), 32_000 (Y+P), 29_000 (Y+Q,
    // Y+R), 21_000 (P+Q, P+R) and 18_000 (Q+R); the other triples to 31_000 (X+P+Q, X+P+R),
    // 28_000 (X+Q+R), 38_000 (Y+Q+R) and 39_000 (X+Y+Q). Both solutions are offered, and both are
    // ambiguous because they compete for the same line.
    let out = run(
        vec![line(1, 5, 0, 30_000, "IN")],
        vec![
            receipt(1, 5, 10_000),
            receipt(2, 5, 20_000),
            receipt(3, 5, 12_000),
            receipt(4, 5, 9_000),
            receipt(5, 5, 9_000),
        ],
        &[],
    );
    assert_eq!(out.len(), 2, "{out:?}");
    assert!(out.iter().all(|c| c.ambiguous));
    assert_eq!(out[0].vouchers, vec![VoucherId(1), VoucherId(2)]);
    assert_eq!(
        out[1].vouchers,
        vec![VoucherId(3), VoucherId(4), VoucherId(5)]
    );
}

#[test]
fn two_vouchers_that_both_sum_the_same_two_lines_are_both_ambiguous() {
    // provenance: derived - rows 1 and 2 are 50_000 each on 5 April; receipts 1 and 2 are each
    // 100_000 on 5 April. No single line matches a voucher, and each voucher is the sum of the
    // same two lines, so both are offered and both are flagged.
    let out = run(
        vec![line(1, 5, 0, 50_000, "IN"), line(2, 5, 0, 50_000, "IN")],
        vec![receipt(1, 5, 100_000), receipt(2, 5, 100_000)],
        &[],
    );
    assert_eq!(out.len(), 2, "{out:?}");
    assert!(
        out.iter()
            .all(|c| c.ambiguous && c.statement_lines == vec![1, 2])
    );
    assert_eq!(out[0].vouchers, vec![VoucherId(1)]);
    assert_eq!(out[1].vouchers, vec![VoucherId(2)]);
}
