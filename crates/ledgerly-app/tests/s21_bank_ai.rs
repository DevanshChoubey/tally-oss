//! Round s21, plan round F: the bank screen shows the assistant's match suggestions (mock state
//! `bank.ai-suggestions`, `design/mock-v2/index.html`, SCREENS.bank) and the person accepts or
//! rejects each one.
//!
//! Provenance (derived). Company: a bank ledger, one customer, one receipt of Rs 500.00 (50,000
//! paise) on 2026-04-05 into the bank. Statement: one credit of 500,00 on 2026-04-25, on row 4 of
//! the MT940 text. The line is 20 days after the receipt, past the exact matcher's 3-day
//! tolerance, so the matcher leaves it and the assistant is asked; its recorded answer is
//! voucher 1. The screen's reason is the engine's (same amount, days apart), never the provider's words;
//! 800 per-mille shows as High, the label the screen already shows for the matcher.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{AiActivityKind, AiTier, Api, ApiError, Command, FixedAnswers, MatchSuggestion};
use ledgerly_app::screens::bank::{BankScreen, Confidence, LineStatus};
use ledgerly_app::worker::{ApiWorker, Poll};
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

const STATEMENT: &str = ":20:S1\r\n:25:0123456789\r\n:60F:C260401INR1000,00\r\n:61:260425C500,00NTRFNONREF//BK1\r\n:62F:C260430INR1500,00\r\n";

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
        group: Some(group(kind)),
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
fn setup(name: &str, tier: AiTier) -> Api {
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
    api.set_ai_tier(tier).unwrap();
    api.attach_assistant(Box::new(FixedAnswers::new().with_bank_line(
        4,
        MatchSuggestion {
            statement_row: 4,
            vouchers: vec![VoucherId(1)],
            ledger_name: None,
            reason: "A reason the screen does not show".to_owned(),
            confidence_permille: 800,
        },
    )));
    api
}

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

fn on_worker<T: Send + 'static>(
    worker: &ApiWorker,
    f: impl FnOnce(&mut Api) -> T + Send + 'static,
) -> T {
    let mut ticket = worker.run(f);
    for _ in 0..2000 {
        if let Poll::Ready(value) = ticket.poll() {
            return value;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("the worker never answered");
}

fn activity(worker: &ApiWorker) -> Vec<AiActivityKind> {
    on_worker(worker, |api| {
        api.ai_activity()
            .unwrap()
            .into_iter()
            .map(|e| e.activity.kind)
            .collect()
    })
}

fn screen(name: &str, tier: AiTier) -> (ApiWorker, BankScreen) {
    let worker = ApiWorker::spawn(setup(name, tier)).unwrap();
    let mut screen = BankScreen::worker("Bank Current Account", BANK, date!(2026 - 04 - 30));
    settle(&mut screen, &worker);
    (worker, screen)
}

#[test]
fn a_line_the_matcher_leaves_shows_the_assistants_suggestion_with_the_engines_reason() {
    ledgerly_app::release::set_ai_enabled(true);
    let (_worker, screen) = screen("s21_bank_ai_shown.ledgerly", AiTier::Local);
    assert_eq!(screen.notice(), None);
    assert_eq!(screen.status(4), Some(LineStatus::Pending));
    let s = screen
        .suggestion_for(4)
        .expect("the assistant's suggestion");
    assert_eq!(s.kind, MatchKind::AiSuggested);
    assert_eq!(s.vouchers, vec![VoucherId(1)]);
    assert_eq!(s.bank_date, date!(2026 - 04 - 25));
    assert_eq!(s.confidence, Confidence::High);
    assert!(s.reason.contains("Same amount"), "{}", s.reason);
    assert!(s.reason.contains("20 days apart"), "{}", s.reason);
    assert!(!s.reason.contains("does not show"), "{}", s.reason);
}

#[test]
fn accepting_the_suggestion_confirms_an_ai_suggested_match() {
    ledgerly_app::release::set_ai_enabled(true);
    let (worker, mut screen) = screen("s21_bank_ai_accept.ledgerly", AiTier::Local);
    screen.accept_focused().unwrap();
    settle(&mut screen, &worker);
    assert_eq!(screen.notice(), None);
    assert_eq!(screen.status(4), Some(LineStatus::Matched));
    let kinds: Vec<MatchKind> = on_worker(&worker, |api| {
        let id = api.stored_bank_statements(BANK).unwrap().last().unwrap().0;
        api.bank_matches(id)
            .unwrap()
            .into_iter()
            .map(|m| m.kind)
            .collect()
    });
    assert_eq!(kinds, vec![MatchKind::AiSuggested]);
    assert!(
        activity(&worker)
            .iter()
            .any(|k| matches!(k, AiActivityKind::DraftAccepted { voucher: None, .. }))
    );
}

#[test]
fn skipping_an_ai_suggested_line_rejects_the_suggestion() {
    ledgerly_app::release::set_ai_enabled(true);
    let (worker, mut screen) = screen("s21_bank_ai_reject.ledgerly", AiTier::Local);
    screen.skip_focused().unwrap();
    settle(&mut screen, &worker);
    assert_eq!(screen.status(4), Some(LineStatus::Skipped));
    assert!(
        activity(&worker)
            .iter()
            .any(|k| matches!(k, AiActivityKind::DraftRejected { .. }))
    );
}

#[test]
fn with_ai_off_in_the_release_nothing_is_asked() {
    ledgerly_app::release::set_ai_enabled(false);
    let (worker, screen) = screen("s21_bank_ai_off.ledgerly", AiTier::Local);
    assert_eq!(screen.suggestion_for(4), None);
    assert_eq!(activity(&worker).len(), 0);
}
