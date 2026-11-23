//! Card C3 tests: the bank screen in worker mode against a real `Api` on a company file.
//!
//! Provenance `derived`: every figure is worked out by hand from the company below.
//
// Company: bank ledger (debit opening Rs 1,000.00 = 100_000 paise), one customer, one receipt
// of Rs 500.00 (50_000) on 2026-04-05 into the bank, no bank date. MT940 statement: opening
// 1000,00, one credit of 500,00 on 2026-04-05, closing 1500,00 dated 2026-04-30.
//
// BRS as of 2026-04-30, before matching: books 100_000 + 50_000 = 150_000; the receipt is not
// reflected in the bank, so deposits 50_000, payments 0; balance per bank 150_000 - 50_000 +
// 0 = 100_000; the statement balance on 30 Apr (its stated end date) is 100_000 + 50_000 =
// 150_000; difference 150_000 - 100_000 = 50_000.
//
// After the receipt is matched (bank date 2026-04-05): deposits 0, balance per bank 150_000,
// difference 0.
//
//! Tally (accessed 2026-11-21): the Bank Reconciliation page
//! <https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm>
//! lists "Balance as per company books", the amounts not reflected in bank, "Balance as per bank",
//! "Balance as per Imported Bank Statement" and a Difference; the screen's rows use those labels.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{Api, ApiError, Command};
use ledgerly_app::screens::bank::{BankScreen, BrsRowKind, LineStatus};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{
    Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankAllocation, BankDetails, BaseVoucherKind, Company, EntityKind, Features,
    InstrumentKind, Ledger, LedgerLine, NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode,
    VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const BANK: LedgerId = LedgerId(10);
const CUSTOMER: LedgerId = LedgerId(12);

const STATEMENT: &str = ":20:S1\r\n:25:0123456789\r\n:60F:C260401INR1000,00\r\n:61:260405C500,00NTRFNONREF//BK1\r\n:62F:C260430INR1500,00\r\n";

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
    opening: i64,
) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
        opening: SignedAmount::dr(Paise::new(opening)),
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

fn ok(out: Result<Outcome<ledgerly_api::CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A company file with a bank ledger, a customer, the receipt and the imported statement. The
/// file name is unique per test.
fn setup(name: &str) -> Api {
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
    ok(api.execute(Command::PostVoucher(receipt())));
    match api
        .import_bank_statement(STATEMENT.as_bytes(), "generic-mt940", BANK)
        .unwrap()
    {
        Outcome::Value(_) => {}
        other @ Outcome::Unsupported(_) => panic!("statement not imported: {other:?}"),
    }
    api
}

/// Polls the screen until it has nothing pending. Bounded, with a short sleep between polls.
fn settle(screen: &mut BankScreen, worker: &ApiWorker) {
    for _ in 0..2000 {
        screen.tick(Some(worker));
        if !screen.is_loading() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the bank screen did not settle within the bound");
}

fn amount_of(screen: &BankScreen, label: &str) -> Option<Paise> {
    match screen.brs() {
        Outcome::Value(view) => view
            .rows
            .iter()
            .find(|r| r.label == label)
            .map(|r| r.amount),
        Outcome::Unsupported(_) => None,
    }
}

#[test]
fn the_screen_shows_a_brs_and_the_statement_lines_from_the_worker() {
    let worker = ApiWorker::spawn(setup("c3_screen_brs.ledgerly")).unwrap();
    let mut screen = BankScreen::worker("Bank Current Account", BANK, date!(2026 - 04 - 30));
    // Nothing is known before the first tick: the screen says it is waiting.
    assert!(screen.is_loading());
    assert!(screen.brs().is_unsupported());
    settle(&mut screen, &worker);
    assert_eq!(screen.notice(), None);
    assert_eq!(
        amount_of(&screen, "Balance as per company books"),
        Some(Paise::new(150_000))
    );
    assert_eq!(
        amount_of(&screen, "Amounts not reflected in bank: deposits"),
        Some(Paise::new(50_000))
    );
    assert_eq!(
        amount_of(&screen, "Balance as per bank"),
        Some(Paise::new(100_000))
    );
    assert_eq!(
        amount_of(&screen, "Balance as per imported bank statement"),
        Some(Paise::new(150_000))
    );
    assert_eq!(amount_of(&screen, "Difference"), Some(Paise::new(50_000)));
    // derived: the statement's only line is on row 4 of the MT940 text; the matcher pairs it
    // with voucher 1 (same amount, same date).
    assert_eq!(screen.status(4), Some(LineStatus::Pending));
    let s = screen.suggestion_for(4).unwrap();
    assert_eq!(s.vouchers, vec![VoucherId(1)]);
    assert_eq!(s.bank_date, date!(2026 - 04 - 05));
}

#[test]
fn each_statement_row_says_which_line_of_the_mock_it_is() {
    // provenance: design/mock-v2/index.html:1741-1745: the books balance is a `tr.sub`, the
    // amounts not yet in the bank are indented lines, the expected bank balance is the
    // `tr.total`, the statement's closing balance a plain line and the difference bold. Our rows
    // in order (the labels asserted above): books, deposits, payments, bank, imported statement,
    // difference.
    let worker = ApiWorker::spawn(setup("c3_screen_kinds.ledgerly")).unwrap();
    let mut screen = BankScreen::worker("Bank Current Account", BANK, date!(2026 - 04 - 30));
    settle(&mut screen, &worker);
    let Outcome::Value(view) = screen.brs() else {
        panic!("expected a BRS");
    };
    let kinds: Vec<BrsRowKind> = view.rows.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        [
            BrsRowKind::Books,
            BrsRowKind::Uncleared,
            BrsRowKind::Uncleared,
            BrsRowKind::Expected,
            BrsRowKind::Statement,
            BrsRowKind::Difference,
        ]
    );
}

#[test]
fn accepting_a_suggestion_confirms_it_through_the_api_and_the_brs_reconciles() {
    let worker = ApiWorker::spawn(setup("c3_screen_confirm.ledgerly")).unwrap();
    let mut screen = BankScreen::worker("Bank Current Account", BANK, date!(2026 - 04 - 30));
    settle(&mut screen, &worker);
    screen.accept_focused().unwrap();
    settle(&mut screen, &worker);
    assert_eq!(screen.notice(), None);
    assert_eq!(screen.status(4), Some(LineStatus::Matched));
    // derived: bank date set, so deposits not reflected 0, balance per bank 150_000 = statement.
    assert_eq!(
        amount_of(&screen, "Amounts not reflected in bank: deposits"),
        Some(Paise::new(0))
    );
    assert_eq!(
        amount_of(&screen, "Balance as per bank"),
        Some(Paise::new(150_000))
    );
    assert_eq!(amount_of(&screen, "Difference"), Some(Paise::new(0)));
}

#[test]
fn an_answer_for_an_older_as_of_date_is_dropped() {
    let worker = ApiWorker::spawn(setup("c3_screen_stale.ledgerly")).unwrap();
    let mut screen = BankScreen::worker("Bank Current Account", BANK, date!(2026 - 04 - 10));
    screen.tick(Some(&worker));
    // The first request is on its way for 10 Apr; the user moves the date to 30 Apr.
    screen.set_as_of(date!(2026 - 04 - 30));
    settle(&mut screen, &worker);
    match screen.brs() {
        Outcome::Value(view) => assert_eq!(view.as_of, date!(2026 - 04 - 30)),
        Outcome::Unsupported(u) => panic!("expected a BRS, got {u:?}"),
    }
}

#[test]
fn a_request_without_a_company_says_so_and_runs_nothing() {
    let mut screen = BankScreen::worker("Bank", BANK, date!(2026 - 04 - 30));
    screen.tick(None);
    assert!(!screen.is_loading());
    assert!(screen.notice().unwrap().contains("No company is open"));
    assert!(screen.brs().is_unsupported());
}
