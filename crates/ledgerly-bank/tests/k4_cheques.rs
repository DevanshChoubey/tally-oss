//! Card K4: cheque leaves and the bounced-cheque proposal (spec 6.5).
//!
//! Chart (all names fictional):
//!
//! ```text
//! group 1 Current Assets (primary)
//!   group 2 Bank Accounts (reserved BankAccounts)
//!     group 5 Escrow Banks (child of 2)
//!   group 3 Sundry Debtors
//! group 4 Indirect Expenses (primary)
//! ledger 20 Test Bank (group 2)     ledger 21 Other Bank (group 2)
//! ledger 22 Customer One (group 3)  ledger 23 Bank Charges (group 4)
//! ledger 24 Cash (group 1)          ledger 25 Escrow Account (group 5)
//! ```
//!
//! `rs(n)` is `n` rupees. Rule A4: every expected value is derived by hand in a comment.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_bank::BankError;
use ledgerly_bank::cheques::{ChequeBook, bounce_proposal, check_leaf};
use ledgerly_core::{
    CompanyId, Date, DrCr, GroupId, LedgerId, MoneyError, Paise, SignedAmount, StateCode,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BankAllocation, BillAllocation, BillRefKind, Books, Company, EntityKind, Features,
    Group, InstrumentKind, Ledger, LedgerLine, Masters, NegativeStockPolicy, ReservedGroup,
    Voucher, VoucherMode, VoucherStatus,
};
use time::macros::date;

const BANK: u64 = 20;
const OTHER_BANK: u64 = 21;
const CUSTOMER: u64 = 22;
const CHARGES: u64 = 23;
const CASH: u64 = 24;
const ESCROW: u64 = 25;

fn rs(n: i64) -> Paise {
    Paise::new(n * 100)
}

fn group(id: u64, parent: Option<u64>, reserved: Option<ReservedGroup>) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: format!("group {id}"),
        aliases: vec![],
        parent: parent.map(GroupId),
        reserved,
        nature: None,
        affects_gross_profit: false,
        tally: None,
    }
}

fn ledger(id: u64, group: u64) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: format!("ledger {id}"),
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

fn books_with(vouchers: Vec<Voucher>) -> Books {
    let mut masters = Masters::default();
    for g in [
        group(1, None, None),
        group(2, Some(1), Some(ReservedGroup::BankAccounts)),
        group(3, Some(1), None),
        group(4, None, None),
        group(5, Some(2), None),
    ] {
        masters.groups.insert(g.id, g);
    }
    for l in [
        ledger(BANK, 2),
        ledger(OTHER_BANK, 2),
        ledger(CUSTOMER, 3),
        ledger(CHARGES, 4),
        ledger(CASH, 1),
        ledger(ESCROW, 5),
    ] {
        masters.ledgers.insert(l.id, l);
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
        vouchers: vouchers
            .into_iter()
            .map(|v| (v.id, v))
            .collect::<BTreeMap<_, _>>(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn cheque_alloc(number: Option<&str>) -> BankAllocation {
    BankAllocation {
        instrument: InstrumentKind::Cheque,
        instrument_number: number.map(str::to_string),
        instrument_date: Some(date!(2026 - 05 - 02)),
        bank_date: None,
        favouring: None,
    }
}

fn line(ledger: u64, amount: SignedAmount, bank: Option<BankAllocation>) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger: LedgerId(ledger),
        amount,
        bills: vec![],
        cost: vec![],
        bank,
    }
}

fn voucher(id: u64, number: &str, on: Date, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(3),
        number: Some(number.to_string()),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: Some(LedgerId(CUSTOMER)),
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

/// Payment of Rs 5,000 from `bank` by cheque `number` to the customer ledger.
fn payment(id: u64, bank: u64, number: &str) -> Voucher {
    voucher(
        id,
        &format!("P-{id}"),
        date!(2026 - 05 - 02),
        vec![
            line(CUSTOMER, SignedAmount::dr(rs(5000)), None),
            line(
                bank,
                SignedAmount::cr(rs(5000)),
                Some(cheque_alloc(Some(number))),
            ),
        ],
    )
}

fn book() -> ChequeBook {
    ChequeBook::new(LedgerId(BANK), 123_450, 123_459, date!(2026 - 04 - 01)).unwrap()
}

fn leaf_err(number: &str, reason_part: &str, err: BankError) {
    match err {
        BankError::ChequeLeaf { number: n, reason } => {
            assert_eq!(n, number);
            assert!(reason.contains(reason_part), "reason was: {reason}");
        }
        other
        @ (BankError::Money(_) | BankError::NotABankLedger(_) | BankError::Invalid { .. }) => {
            panic!("expected ChequeLeaf, got {other:?}")
        }
    }
}

// ---- leaves -------------------------------------------------------------------------------

#[test]
fn free_leaf_in_range_is_accepted() {
    // provenance: derived - book is 123450..=123459; 123455 is inside and no voucher uses it.
    let books = books_with(vec![payment(1, BANK, "123451")]);
    assert_eq!(check_leaf(&book(), "123455", &books), Ok(()));
}

#[test]
fn first_and_last_leaf_are_inside_the_range() {
    // provenance: derived - the range is inclusive: 123450 and 123459 are the end leaves.
    let books = books_with(vec![]);
    assert_eq!(check_leaf(&book(), "123450", &books), Ok(()));
    assert_eq!(check_leaf(&book(), "123459", &books), Ok(()));
}

#[test]
fn reusing_a_leaf_errors() {
    // provenance: derived - voucher P-1 is a credit on the book's bank ledger with cheque 123451,
    // so leaf 123451 is used.
    let books = books_with(vec![payment(1, BANK, "123451")]);
    let err = check_leaf(&book(), "123451", &books).unwrap_err();
    leaf_err("123451", "already used on voucher P-1", err);
}

#[test]
fn leaf_compared_as_integer_so_leading_zeros_match() {
    // provenance: derived - "0123451" and "123451" are both the integer 123451.
    let books = books_with(vec![payment(1, BANK, "0123451")]);
    let err = check_leaf(&book(), "123451", &books).unwrap_err();
    leaf_err("123451", "already used", err);
}

#[test]
fn cancelled_voucher_still_uses_the_leaf() {
    // provenance: derived - module rule: the paper leaf is gone even if the voucher is cancelled.
    let mut v = payment(1, BANK, "123452");
    v.status = VoucherStatus::Cancelled;
    let books = books_with(vec![v]);
    let err = check_leaf(&book(), "123452", &books).unwrap_err();
    leaf_err("123452", "already used", err);
}

#[test]
fn post_dated_voucher_uses_the_leaf() {
    // provenance: derived - a post-dated cheque is already handed over, so its leaf is used.
    let mut v = payment(1, BANK, "123453");
    v.status = VoucherStatus::PostDated;
    let books = books_with(vec![v]);
    let err = check_leaf(&book(), "123453", &books).unwrap_err();
    leaf_err("123453", "already used", err);
}

#[test]
fn same_number_on_another_bank_ledger_does_not_count() {
    // provenance: derived - leaf numbers are per bank account; the voucher is on ledger 21.
    let books = books_with(vec![payment(1, OTHER_BANK, "123451")]);
    assert_eq!(check_leaf(&book(), "123451", &books), Ok(()));
}

#[test]
fn received_cheque_with_same_number_does_not_count() {
    // provenance: derived - a debit line on our bank ledger is a cheque received from a customer;
    // its number comes from the customer's book, not ours.
    let received = voucher(
        1,
        "R-1",
        date!(2026 - 05 - 02),
        vec![
            line(
                BANK,
                SignedAmount::dr(rs(900)),
                Some(cheque_alloc(Some("123451"))),
            ),
            line(CUSTOMER, SignedAmount::cr(rs(900)), None),
        ],
    );
    let books = books_with(vec![received]);
    assert_eq!(check_leaf(&book(), "123451", &books), Ok(()));
}

#[test]
fn non_cheque_instrument_with_same_number_does_not_count() {
    // provenance: derived - a NEFT reference is not a cheque leaf.
    let mut v = payment(1, BANK, "123451");
    v.ledger_lines[1].bank = Some(BankAllocation {
        instrument: InstrumentKind::Neft,
        instrument_number: Some("123451".to_string()),
        instrument_date: None,
        bank_date: None,
        favouring: None,
    });
    let books = books_with(vec![v]);
    assert_eq!(check_leaf(&book(), "123451", &books), Ok(()));
}

#[test]
fn leaf_outside_the_book_errors() {
    // provenance: derived - 123449 < first (123450) and 123460 > last (123459).
    let books = books_with(vec![]);
    leaf_err(
        "123449",
        "outside the cheque book 123450 to 123459",
        check_leaf(&book(), "123449", &books).unwrap_err(),
    );
    leaf_err(
        "123460",
        "outside the cheque book",
        check_leaf(&book(), "123460", &books).unwrap_err(),
    );
}

#[test]
fn non_numeric_or_empty_number_errors() {
    // provenance: derived - a cheque number is digits only (module docs).
    let books = books_with(vec![]);
    for bad in ["", "  ", "12345A", "-123451", "123 451", "1.5"] {
        leaf_err(
            bad,
            "digits only",
            check_leaf(&book(), bad, &books).unwrap_err(),
        );
    }
}

#[test]
fn number_too_large_for_u64_errors_not_panics() {
    // provenance: derived - u64::MAX is 18_446_744_073_709_551_615. 99_999_999_999_999_999_999 is
    // all digits (20 of them) and larger than u64::MAX, so it is not a non-digit error: the reason
    // must say the number has too many digits, not that it is not digits.
    let books = books_with(vec![]);
    leaf_err(
        "99999999999999999999",
        "too many digits",
        check_leaf(&book(), "99999999999999999999", &books).unwrap_err(),
    );
}

#[test]
fn used_number_with_a_suffix_blocks_the_leaf_check_instead_of_being_skipped() {
    // provenance: derived - voucher P-1 is a payment by cheque "123451/25" on the book's bank
    // ledger. "123451/25" is not digits only, so it cannot be read as leaf 123451. The old code
    // skipped it and reported 123451 free. Now the check fails and names P-1, because the leaf
    // cannot be confirmed free. Leaf 123455 is blocked too: it is inside the book, and the
    // unreadable number could be a leaf that is in use.
    let books = books_with(vec![payment(1, BANK, "123451/25")]);
    leaf_err(
        "123451",
        "cannot be read",
        check_leaf(&book(), "123451", &books).unwrap_err(),
    );
    leaf_err(
        "123455",
        "P-1",
        check_leaf(&book(), "123455", &books).unwrap_err(),
    );
}

#[test]
fn book_on_a_non_bank_ledger_errors() {
    // provenance: derived - ledger 24 (Cash) sits under Current Assets, not Bank Accounts/OD.
    let b = ChequeBook::new(LedgerId(CASH), 1, 10, date!(2026 - 04 - 01)).unwrap();
    let books = books_with(vec![]);
    assert_eq!(
        check_leaf(&b, "5", &books),
        Err(BankError::NotABankLedger(LedgerId(CASH)))
    );
}

#[test]
fn book_on_a_ledger_in_a_sub_group_of_bank_accounts_is_a_bank() {
    // provenance: derived - ledger 25 is in group 5, whose parent is group 2 (Bank Accounts).
    let b = ChequeBook::new(LedgerId(ESCROW), 1, 10, date!(2026 - 04 - 01)).unwrap();
    let books = books_with(vec![]);
    assert_eq!(check_leaf(&b, "5", &books), Ok(()));
}

#[test]
fn book_on_a_missing_ledger_errors() {
    // provenance: derived - ledger 99 is not in the masters.
    let b = ChequeBook::new(LedgerId(99), 1, 10, date!(2026 - 04 - 01)).unwrap();
    let books = books_with(vec![]);
    assert!(matches!(
        check_leaf(&b, "5", &books),
        Err(BankError::Invalid { .. })
    ));
}

#[test]
fn book_with_first_after_last_is_rejected() {
    // provenance: derived - 10 > 5 is not a range.
    assert!(matches!(
        ChequeBook::new(LedgerId(BANK), 10, 5, date!(2026 - 04 - 01)),
        Err(BankError::Invalid { .. })
    ));
}

// ---- bounce -------------------------------------------------------------------------------

/// Receipt R-7 of 1 May 2026: Dr Test Bank Rs 10,000 by cheque 654321, Cr Customer One Rs 10,000
/// against bill INV-1 (due 1 June 2026).
fn receipt() -> Voucher {
    let mut cust = line(CUSTOMER, SignedAmount::cr(rs(10_000)), None);
    cust.bills = vec![BillAllocation {
        kind: BillRefKind::Against,
        name: "INV-1".to_string(),
        due_date: Some(date!(2026 - 06 - 01)),
        credit_days: Some(30),
        amount: SignedAmount::cr(rs(10_000)),
    }];
    voucher(
        7,
        "R-7",
        date!(2026 - 05 - 01),
        vec![
            line(
                BANK,
                SignedAmount::dr(rs(10_000)),
                Some(cheque_alloc(Some("654321"))),
            ),
            cust,
        ],
    )
}

#[test]
fn bounce_reverses_every_line_and_adds_charges() {
    // provenance: derived - reversing a voucher flips each line's side.
    //   original:  Dr Bank 1_000_000 paise, Cr Customer 1_000_000 paise (Rs 10,000.00)
    //   reversal:  Cr Bank 1_000_000,       Dr Customer 1_000_000
    //   charges Rs 354.00 = 35_400 paise:  Dr Bank Charges 35_400, Cr Bank 35_400
    //   Dr total = 1_000_000 + 35_400 = 1_035_400; Cr total = 1_000_000 + 35_400 = 1_035_400.
    let v = receipt();
    let p = bounce_proposal(
        &v,
        Paise::new(35_400),
        LedgerId(CHARGES),
        date!(2026 - 05 - 09),
    )
    .unwrap();

    assert_eq!(p.ledger_lines.len(), 4);
    assert_eq!(p.ledger_lines[0].ledger, LedgerId(BANK));
    assert_eq!(
        p.ledger_lines[0].amount,
        SignedAmount::cr(Paise::new(1_000_000))
    );
    assert_eq!(p.ledger_lines[1].ledger, LedgerId(CUSTOMER));
    assert_eq!(
        p.ledger_lines[1].amount,
        SignedAmount::dr(Paise::new(1_000_000))
    );
    assert_eq!(p.ledger_lines[2].ledger, LedgerId(CHARGES));
    assert_eq!(
        p.ledger_lines[2].amount,
        SignedAmount::dr(Paise::new(35_400))
    );
    assert_eq!(p.ledger_lines[3].ledger, LedgerId(BANK));
    assert_eq!(
        p.ledger_lines[3].amount,
        SignedAmount::cr(Paise::new(35_400))
    );

    let mut dr = 0_i64;
    let mut cr = 0_i64;
    for l in &p.ledger_lines {
        match l.amount.side {
            DrCr::Dr => dr += l.amount.amount.get(),
            DrCr::Cr => cr += l.amount.amount.get(),
        }
    }
    assert_eq!(dr, 1_035_400);
    assert_eq!(cr, 1_035_400);
}

#[test]
fn bounce_is_a_draft_not_a_posting() {
    // provenance: derived - spec 6.5 and rule A10: a proposal never enters the books. Status
    // Optional is "kept out of the books" (model docs); id 0 and no number mean unassigned.
    let v = receipt();
    let p = bounce_proposal(&v, rs(100), LedgerId(CHARGES), date!(2026 - 05 - 09)).unwrap();
    assert_eq!(p.status, VoucherStatus::Optional);
    assert_eq!(p.id, VoucherId(0));
    assert_eq!(p.number, None);
    assert_eq!(p.date, date!(2026 - 05 - 09));
    assert_eq!(p.reference.as_deref(), Some("R-7"));
    assert_eq!(p.reference_date, Some(date!(2026 - 05 - 01)));
    assert_eq!(p.party, Some(LedgerId(CUSTOMER)));
    // The draft's type is not asserted: it still carries the original voucher's type, which is
    // wrong for a journal. The caller must set a journal type (interface request, K4 in
    // docs/interface-requests.md).
    assert!(p.narration.contains("654321"), "narration: {}", p.narration);
    assert!(p.narration.contains("R-7"), "narration: {}", p.narration);
    // The original is untouched.
    assert_eq!(v, receipt());
}

#[test]
fn bounce_does_not_copy_the_cheque_instrument() {
    // provenance: derived - module docs: the instrument is dropped from the reversing lines so
    // the leaf is not counted twice. Checked through check_leaf on a book that has the proposal
    // added as a regular voucher (as if accepted): cheque 654321 would be "used" only through the
    // reversing credit on the bank ledger if the instrument were copied.
    let v = receipt();
    let mut p = bounce_proposal(&v, rs(100), LedgerId(CHARGES), date!(2026 - 05 - 09)).unwrap();
    p.id = VoucherId(8);
    p.status = VoucherStatus::Regular;
    assert!(p.ledger_lines.iter().all(|l| l.bank.is_none()));
    let b = ChequeBook::new(LedgerId(BANK), 654_300, 654_399, date!(2026 - 04 - 01)).unwrap();
    let books = books_with(vec![v, p]);
    assert_eq!(check_leaf(&b, "654321", &books), Ok(()));
}

#[test]
fn bounce_reverses_bill_allocations() {
    // provenance: pending - the reversal rule (a New or Advance bill allocation becomes an Against
    // of the opposite side) is a netting design choice. No Act, rule, notification, accounting
    // standard or format spec is cited for it, and the owner has not confirmed it. The arithmetic
    // below is the rule as written: the original settles INV-1 with Cr 1_000_000 paise and the
    // reversal carries Dr 1_000_000 against INV-1.
    let v = receipt();
    let p = bounce_proposal(&v, Paise::ZERO, LedgerId(CHARGES), date!(2026 - 05 - 09)).unwrap();
    let bills = &p.ledger_lines[1].bills;
    assert_eq!(bills.len(), 1);
    assert_eq!(bills[0].kind, BillRefKind::Against);
    assert_eq!(bills[0].name, "INV-1");
    assert_eq!(bills[0].amount, SignedAmount::dr(Paise::new(1_000_000)));
    assert_eq!(bills[0].due_date, None);
    assert_eq!(bills[0].credit_days, None);
}

#[test]
fn bounce_new_and_advance_references_reverse_as_against() {
    // provenance: pending - the reversal of New and Advance references as Against of the opposite
    // side is a design choice, not a cited rule; the owner must confirm it before this is derived.
    // OnAccount stays OnAccount (the same design choice).
    let mut v = receipt();
    v.ledger_lines[1].bills = vec![
        BillAllocation {
            kind: BillRefKind::New,
            name: "A".to_string(),
            due_date: None,
            credit_days: None,
            amount: SignedAmount::cr(rs(1)),
        },
        BillAllocation {
            kind: BillRefKind::Advance,
            name: "B".to_string(),
            due_date: None,
            credit_days: None,
            amount: SignedAmount::cr(rs(2)),
        },
        BillAllocation {
            kind: BillRefKind::OnAccount,
            name: "C".to_string(),
            due_date: None,
            credit_days: None,
            amount: SignedAmount::cr(rs(3)),
        },
    ];
    let p = bounce_proposal(&v, Paise::ZERO, LedgerId(CHARGES), date!(2026 - 05 - 09)).unwrap();
    let kinds: Vec<BillRefKind> = p.ledger_lines[1].bills.iter().map(|b| b.kind).collect();
    assert_eq!(
        kinds,
        vec![
            BillRefKind::Against,
            BillRefKind::Against,
            BillRefKind::OnAccount
        ]
    );
    assert!(
        p.ledger_lines[1]
            .bills
            .iter()
            .all(|b| b.amount.side == DrCr::Dr)
    );
}

#[test]
fn bounce_with_zero_charges_adds_no_charges_lines() {
    // provenance: derived - zero charges means nothing to post: 2 reversed lines only.
    let p = bounce_proposal(
        &receipt(),
        Paise::ZERO,
        LedgerId(CHARGES),
        date!(2026 - 05 - 09),
    )
    .unwrap();
    assert_eq!(p.ledger_lines.len(), 2);
    assert!(p.ledger_lines.iter().all(|l| l.ledger != LedgerId(CHARGES)));
}

#[test]
fn bounce_of_an_issued_cheque_credits_back_the_bank() {
    // provenance: derived - payment P-1 is Dr Customer 500_000, Cr Bank 500_000 (Rs 5,000). Its
    // reversal is Cr Customer 500_000, Dr Bank 500_000; charges Rs 100 = 10_000 paise add
    // Dr Charges 10_000, Cr Bank 10_000, so the bank nets Dr 500_000 - 10_000 = 490_000.
    let p = bounce_proposal(
        &payment(1, BANK, "123451"),
        Paise::new(10_000),
        LedgerId(CHARGES),
        date!(2026 - 05 - 09),
    )
    .unwrap();
    assert_eq!(
        p.ledger_lines[0].amount,
        SignedAmount::cr(Paise::new(500_000))
    );
    assert_eq!(p.ledger_lines[0].ledger, LedgerId(CUSTOMER));
    assert_eq!(
        p.ledger_lines[1].amount,
        SignedAmount::dr(Paise::new(500_000))
    );
    assert_eq!(p.ledger_lines[1].ledger, LedgerId(BANK));
    assert_eq!(
        p.ledger_lines[3].amount,
        SignedAmount::cr(Paise::new(10_000))
    );
    assert_eq!(p.ledger_lines[3].ledger, LedgerId(BANK));
}

fn invalid_field(r: Result<Voucher, BankError>, field: &str) {
    match r {
        Err(BankError::Invalid { field: f, .. }) => assert_eq!(f, field),
        other => panic!("expected Invalid({field}), got {other:?}"),
    }
}

#[test]
fn bounce_rejects_a_voucher_not_in_the_books() {
    // provenance: derived - Optional, PostDated and Cancelled vouchers have no postings to reverse.
    for status in [
        VoucherStatus::Optional,
        VoucherStatus::PostDated,
        VoucherStatus::Cancelled,
    ] {
        let mut v = receipt();
        v.status = status;
        invalid_field(
            bounce_proposal(&v, rs(1), LedgerId(CHARGES), date!(2026 - 05 - 09)),
            "voucher",
        );
    }
}

#[test]
fn bounce_rejects_negative_charges() {
    // provenance: derived - charges are an expense; a negative would flip the entry's meaning.
    invalid_field(
        bounce_proposal(
            &receipt(),
            Paise::new(-1),
            LedgerId(CHARGES),
            date!(2026 - 05 - 09),
        ),
        "charges",
    );
}

#[test]
fn bounce_rejects_a_date_before_the_voucher() {
    // provenance: derived - the voucher is dated 1 May 2026; 30 April 2026 is earlier.
    invalid_field(
        bounce_proposal(&receipt(), rs(1), LedgerId(CHARGES), date!(2026 - 04 - 30)),
        "date",
    );
    // Same day is allowed.
    assert!(bounce_proposal(&receipt(), rs(1), LedgerId(CHARGES), date!(2026 - 05 - 01)).is_ok());
}

#[test]
fn bounce_rejects_no_cheque_line_and_two_cheque_lines() {
    // provenance: derived - the reversal is anchored on exactly one cheque line.
    let mut none = receipt();
    none.ledger_lines[0].bank = None;
    invalid_field(
        bounce_proposal(&none, rs(1), LedgerId(CHARGES), date!(2026 - 05 - 09)),
        "voucher",
    );

    let mut neft = receipt();
    neft.ledger_lines[0].bank = Some(BankAllocation {
        instrument: InstrumentKind::Neft,
        instrument_number: None,
        instrument_date: None,
        bank_date: None,
        favouring: None,
    });
    invalid_field(
        bounce_proposal(&neft, rs(1), LedgerId(CHARGES), date!(2026 - 05 - 09)),
        "voucher",
    );

    let mut two = receipt();
    two.ledger_lines[1].bank = Some(cheque_alloc(Some("1")));
    invalid_field(
        bounce_proposal(&two, rs(1), LedgerId(CHARGES), date!(2026 - 05 - 09)),
        "voucher",
    );
}

#[test]
fn bounce_rejects_charges_ledger_equal_to_the_bank() {
    // provenance: derived - Dr bank / Cr bank for the charges would be a no-op entry.
    invalid_field(
        bounce_proposal(&receipt(), rs(1), LedgerId(BANK), date!(2026 - 05 - 09)),
        "charges ledger",
    );
}

#[test]
fn bounce_rejects_unbalanced_original() {
    // provenance: derived - Dr 1_000_000 against Cr 900_000 leaves 100_000 paise unmatched.
    let mut v = receipt();
    v.ledger_lines[1].amount = SignedAmount::cr(Paise::new(900_000));
    invalid_field(
        bounce_proposal(&v, rs(1), LedgerId(CHARGES), date!(2026 - 05 - 09)),
        "voucher",
    );
}

#[test]
fn bounce_overflow_in_original_is_a_money_error() {
    // provenance: derived - i64::MAX + 1 does not fit in i64.
    let mut v = receipt();
    v.ledger_lines[0].amount = SignedAmount::dr(Paise::new(i64::MAX));
    v.ledger_lines[1].amount = SignedAmount::dr(Paise::new(1));
    assert_eq!(
        bounce_proposal(&v, rs(1), LedgerId(CHARGES), date!(2026 - 05 - 09)),
        Err(BankError::Money(MoneyError::Overflow))
    );
}

#[test]
fn bounce_rejects_gst_and_stock_vouchers() {
    // provenance: derived - reversing the ledger lines alone would leave GST and stock behind.
    let mut g = receipt();
    g.gst = Some(ledgerly_model::StoredGst {
        pack_version: "x".to_string(),
        lines: vec![],
        round_off: Paise::ZERO,
    });
    invalid_field(
        bounce_proposal(&g, rs(1), LedgerId(CHARGES), date!(2026 - 05 - 09)),
        "voucher",
    );
}
