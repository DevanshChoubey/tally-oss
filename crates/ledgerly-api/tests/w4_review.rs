//! Card W4 tests: the Review queue's list from the engine (mock `review`, `SCREENS.review`).
//!
//! Provenance. Expected values are `derived` by hand from the company and statement written in
//! this file. The queue lists the matcher's suggestions for the newest stored statement of each
//! bank ledger, with no match rules, as the bank screen does (`bank_suggestions(id, &[])`).
//! The sources the engine cannot list yet are named with their reason; no row is made for them.
//!
//! Tally citation, read 2026-11-23:
//! <https://help.tallysolutions.com/docs/te9rel66/Reports/MIS_Reports/Display_Optional_Vouchers.htm>
//! says the "Display Optional Vouchers" report lists vouchers marked Optional, which are not
//! posted and stay in the Optional Register, and sits under Exception Reports. It does not
//! describe a queue of proposals awaiting a decision, so it is the nearest equivalent only in
//! its place in the menu, and the queue is not built from it.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ledgerly_api::{Api, ApiError, Candidate, Command, CommandResult, ReviewSource};

use ledgerly_core::{
    Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankAllocation, BankDetails, BaseVoucherKind, Company, EntityKind, Features,
    InstrumentKind, Ledger, LedgerLine, MatchKind, NegativeStockPolicy, ReservedGroup, Voucher,
    VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const BANK: LedgerId = LedgerId(10);
const CUSTOMER: LedgerId = LedgerId(12);

/// Row 4 is a credit of 500,00 on 5 Apr (money in); row 5 is a debit of 300,00 on 12 Apr with no
/// voucher of that amount, so it has no suggestion. Derived closing: 1000 + 500 - 300 = 1200.
const STATEMENT: &str = ":20:S1\r\n:25:0123456789\r\n:60F:C260401INR1000,00\r\n:61:260405C500,00NTRFNONREF//BK1\r\n:61:260412D300,00NTRFCHQ000123\r\n:62F:C260430INR1200,00\r\n";

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn ledger(
    id: LedgerId,
    name: &str,
    kind: ReservedGroup,
    bank: Option<BankDetails>,
    open: i64,
) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
        opening: SignedAmount::dr(Paise::new(open)),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

/// A receipt of Rs 500.00 into the bank from the customer, on 5 Apr 2026.
fn receipt() -> Voucher {
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Receipt)
        .unwrap()
        .id;
    let bank_line = LedgerLine {
        ineligible_itc: None,
        ledger: BANK,
        amount: SignedAmount::dr(Paise::new(50_000)),
        bills: Vec::new(),
        cost: Vec::new(),
        bank: Some(BankAllocation {
            instrument: InstrumentKind::Neft,
            instrument_number: None,
            instrument_date: None,
            bank_date: None,
            favouring: None,
        }),
    };
    let party_line = LedgerLine {
        ineligible_itc: None,
        ledger: CUSTOMER,
        amount: SignedAmount::cr(Paise::new(50_000)),
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    };
    Voucher {
        id: VoucherId(1),
        type_id,
        number: None,
        series: None,
        date: date!(2026 - 04 - 05),
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
        ledger_lines: vec![bank_line, party_line],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn ok(out: Result<Outcome<CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A company file with a bank account and a customer. With `statement`, the receipt is posted
/// and the statement imported; without it the company has no proposals at all.
fn setup(name: &str, statement: bool) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let mut api = Api::create(&path, &company(), clock).unwrap();
    let details = BankDetails {
        account_number: "0123456789".to_owned(),
        ifsc: "TEST0000001".to_owned(),
        bank_name: "Test Bank".to_owned(),
        branch: "Main".to_owned(),
    };
    ok(api.execute(Command::CreateLedger(ledger(
        BANK,
        "Bank Current Account",
        ReservedGroup::BankAccounts,
        Some(details),
        100_000,
    ))));
    ok(api.execute(Command::CreateLedger(ledger(
        CUSTOMER,
        "Customer A",
        ReservedGroup::SundryDebtors,
        None,
        0,
    ))));
    if statement {
        ok(api.execute(Command::PostVoucher(receipt())));
        match api
            .import_bank_statement(STATEMENT.as_bytes(), "generic-mt940", BANK)
            .unwrap()
        {
            Outcome::Value(_) => {}
            other @ Outcome::Unsupported(_) => panic!("statement not imported: {other:?}"),
        }
    }
    api
}

#[test]
fn the_queue_lists_the_bank_match_of_the_newest_statement() {
    // provenance: derived. Row 4 (5 Apr, credit 500,00 = 50_000 paise) pairs with voucher 1
    // (5 Apr, 50_000). Row 5 has no voucher of 300,00 and no rule, so it gives no candidate.
    let api = setup("w4_review_match.ledgerly", true);
    let queue = api.review_queue().unwrap();
    assert_eq!(queue.items.len(), 1);
    let item = &queue.items[0];
    assert_eq!(item.ledger, BANK);
    assert_eq!(
        item.candidate,
        Candidate {
            statement_lines: vec![4],
            vouchers: vec![VoucherId(1)],
            kind: MatchKind::Exact,
            suggested_ledger: None,
            proposed_bank_date: date!(2026 - 04 - 05),
            ambiguous: false,
        }
    );
    assert_eq!(item.amount, Some(Paise::new(50_000)));
    assert!(item.money_in);
}

#[test]
fn the_sources_the_engine_cannot_list_are_named_with_reasons() {
    // provenance: derived. The mock's kinds are bill drafts, plain-words drafts, bank matches,
    // ledger suggestions, duplicate checks and 2B explanations. Five have no engine list; the
    // bank match list is connected, so it is not named as unlisted.
    let api = setup("w4_review_named.ledgerly", true);
    let queue = api.review_queue().unwrap();
    let sources: Vec<ReviewSource> = queue.unlisted.iter().map(|u| u.source).collect();
    assert_eq!(
        sources,
        vec![
            ReviewSource::BillDraft,
            ReviewSource::PlainWordsDraft,
            ReviewSource::NewLedgerSuggestion,
            ReviewSource::DuplicateCheck,
            ReviewSource::Gstr2bExplanation,
        ]
    );
    assert!(queue.unlisted.iter().all(|u| !u.reason.trim().is_empty()));
}

#[test]
fn a_company_with_no_statement_lists_nothing_and_still_names_the_sources() {
    // provenance: derived. No statement and no voucher: no bank suggestion, so no item.
    let api = setup("w4_review_empty.ledgerly", false);
    let queue = api.review_queue().unwrap();
    assert_eq!(queue.items, Vec::new());
    assert_eq!(queue.unlisted.len(), 5);
}

#[test]
fn the_count_is_withheld_while_any_source_is_unlisted() {
    // provenance: derived. Five sources are not connected, so the queue cannot say how many
    // proposals are open; the badge rule gives no count (`None`), never the listed rows alone.
    let api = setup("w4_review_count.ledgerly", true);
    let queue = api.review_queue().unwrap();
    assert_eq!(queue.count(), None);
}
