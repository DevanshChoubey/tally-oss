//! Round s21, plan round F: the assistant's bank match suggestions (mock state
//! `bank.ai-suggestions`, `design/mock-v2/index.html`, SCREENS.bank).
//!
//! The engine's exact matcher runs first; a statement line it leaves unclaimed is offered to the
//! provider with the vouchers the matcher leaves open. The provider names one voucher; the engine
//! checks the row, the voucher, the direction and the amount, stores the suggestion as a draft
//! (`DraftStored`, feature `BankSuggestion`), and the person accepts or rejects it one line at a
//! time. Accepting records the match as `MatchKind::AiSuggested` with the line's date as the bank
//! date, and the accept in the same commit.
//!
//! Provenance (derived). The statement below is written here:
//!
//! - row 6: 2026-04-12 debit 300,00
//! - row 7: 2026-04-26 credit 500,00
//! - row 8: 2026-04-28 debit 10,00, "BANK CHARGES" (row 9 is its `:86:`)
//!
//! Opening 1000,00 + 500 - 300 - 10 = 1190,00, the stated `:62F:`. Voucher 1 is a receipt of
//! Rs 500.00 (50,000 paise) on 2026-04-12, voucher 2 a payment of Rs 300.00 on 2026-04-12. The
//! exact matcher pairs row 6 with voucher 2 (same amount, same day, within its 3-day tolerance).
//! Row 7 is 14 days after voucher 1, past the tolerance, so the matcher leaves both open: that is
//! the line the assistant is asked about. Row 8 has no voucher.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_api::{
    AiActivityKind, AiFeature, AiTier, Api, ApiError, BankAiAnswer, Command, CommandResult,
    FixedAnswers, MatchSuggestion,
};
use ledgerly_core::{
    BankStatementId, CancelToken, Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise,
    SignedAmount, StateCode, VoucherId, VoucherTypeId,
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
const RENT: LedgerId = LedgerId(11);
const CUSTOMER: LedgerId = LedgerId(12);
const MT940: &str = "generic-mt940";

const STATEMENT: &str = "{1:F01TESTINBBAXXX0000000000}{2:O9400000000000TESTINBBXXXX00000000000000000000N}{4:\r\n\
:20:STMT0001\r\n\
:25:0123456789\r\n\
:28C:00001/001\r\n\
:60F:C260401INR1000,00\r\n\
:61:260412D300,00NTRFCHQ000123\r\n\
:61:260426C500,00NTRFNONREF//BK1\r\n\
:61:260428D10,00NTRFNONREF//BANKCHG\r\n\
:86:BANK CHARGES\r\n\
:62F:C260428INR1190,00\r\n\
-}\r\n";

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn reserved(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
        .unwrap()
        .id
}

fn voucher_type(kind: BaseVoucherKind) -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == kind)
        .unwrap()
        .id
}

fn master(id: LedgerId, name: &str, group: GroupId, bank: Option<BankDetails>) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group),
        opening: SignedAmount::dr(if bank.is_some() {
            Paise::new(100_000)
        } else {
            Paise::ZERO
        }),
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

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: (ledger == BANK).then_some(BankAllocation {
            instrument: InstrumentKind::Neft,
            instrument_number: None,
            instrument_date: None,
            bank_date: None,
            favouring: None,
        }),
    }
}

fn voucher(id: u64, kind: BaseVoucherKind, on: Date, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: voucher_type(kind),
        number: None,
        series: None,
        date: on,
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
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn committed(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out {
        Ok(Outcome::Value(result)) => result,
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// The assistant's answer for row 7: voucher 1.
fn row7(vouchers: Vec<u64>) -> MatchSuggestion {
    MatchSuggestion {
        statement_row: 7,
        vouchers: vouchers.into_iter().map(VoucherId).collect(),
        ledger_name: None,
        reason: "Same amount; the customer's receipt was entered two weeks before it cleared"
            .to_owned(),
        confidence_permille: 800,
    }
}

fn setup(
    tier: AiTier,
    provider: Option<FixedAnswers>,
) -> (tempfile::TempDir, Api, BankStatementId) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let details = BankDetails {
        account_number: "0123456789".to_owned(),
        ifsc: "TEST0000001".to_owned(),
        bank_name: "Test Bank".to_owned(),
        branch: "Main".to_owned(),
    };
    for l in [
        master(
            BANK,
            "Bank Current Account",
            reserved(ReservedGroup::BankAccounts),
            Some(details),
        ),
        master(
            RENT,
            "Rent",
            reserved(ReservedGroup::IndirectExpenses),
            None,
        ),
        master(
            CUSTOMER,
            "Customer A",
            reserved(ReservedGroup::SundryDebtors),
            None,
        ),
    ] {
        committed(api.execute(Command::CreateLedger(l)));
    }
    let on = date!(2026 - 04 - 12);
    committed(api.execute(Command::PostVoucher(voucher(
        1,
        BaseVoucherKind::Receipt,
        on,
        vec![
            line(BANK, SignedAmount::dr(Paise::new(50_000))),
            line(CUSTOMER, SignedAmount::cr(Paise::new(50_000))),
        ],
    ))));
    committed(api.execute(Command::PostVoucher(voucher(
        2,
        BaseVoucherKind::Payment,
        on,
        vec![
            line(RENT, SignedAmount::dr(Paise::new(30_000))),
            line(BANK, SignedAmount::cr(Paise::new(30_000))),
        ],
    ))));
    let Outcome::Value(statement) = api
        .import_bank_statement(STATEMENT.as_bytes(), MT940, BANK)
        .unwrap()
    else {
        panic!("the statement imports")
    };
    api.set_ai_tier(tier).unwrap();
    if let Some(provider) = provider {
        api.attach_assistant(Box::new(provider));
    }
    (dir, api, statement)
}

fn suggested(api: &mut Api, statement: BankStatementId) -> BankAiAnswer {
    match api
        .bank_ai_suggestions(statement, &CancelToken::new())
        .unwrap()
    {
        Outcome::Value(answer) => answer,
        Outcome::Unsupported(u) => panic!("expected suggestions, got {u:?}"),
    }
}

fn kinds(api: &Api) -> Vec<AiActivityKind> {
    api.ai_activity()
        .unwrap()
        .into_iter()
        .map(|e| e.activity.kind)
        .collect()
}

#[test]
fn a_line_the_exact_matcher_leaves_is_suggested_with_the_engines_checks() {
    let provider = FixedAnswers::new().with_bank_line(7, row7(vec![1]));
    let (_dir, mut api, statement) = setup(AiTier::Local, Some(provider));
    let BankAiAnswer::Suggested { matches, not_read } = suggested(&mut api, statement) else {
        panic!("the gate is ready")
    };
    assert_eq!(matches.len(), 1, "{matches:?}");
    let m = &matches[0];
    assert_eq!(m.line, 7);
    assert_eq!(m.voucher, VoucherId(1));
    assert_eq!(m.amount, Paise::new(50_000));
    assert_eq!(m.days_apart, 14);
    assert_eq!(m.bank_date, date!(2026 - 04 - 26));
    // Row 6 is the exact matcher's and is not asked about; row 8 has no recorded answer.
    assert_eq!(not_read.len(), 1, "{not_read:?}");
    assert_eq!(not_read[0].0, 8);
    let log = api.ai_activity().unwrap();
    assert_eq!(log.len(), 2, "{log:?}");
    assert_eq!(log[0].activity.feature, AiFeature::BankSuggestion);
    assert!(matches!(
        log[0].activity.kind,
        AiActivityKind::DraftStored {
            confidence_permille: 800,
            ..
        }
    ));
    assert_eq!(log[0].seq, m.draft);
    assert!(matches!(
        log[1].activity.kind,
        AiActivityKind::ReadFailed { .. }
    ));
}

#[test]
fn asking_again_reuses_the_open_suggestion() {
    let provider = FixedAnswers::new().with_bank_line(7, row7(vec![1]));
    let (_dir, mut api, statement) = setup(AiTier::Local, Some(provider));
    let BankAiAnswer::Suggested { matches: first, .. } = suggested(&mut api, statement) else {
        panic!()
    };
    let BankAiAnswer::Suggested { matches: again, .. } = suggested(&mut api, statement) else {
        panic!()
    };
    assert_eq!(first, again);
    let stored = kinds(&api)
        .into_iter()
        .filter(|k| matches!(k, AiActivityKind::DraftStored { .. }))
        .count();
    assert_eq!(stored, 1);
}

#[test]
fn a_voucher_the_engine_did_not_offer_is_refused() {
    // Voucher 2 is the exact matcher's (row 6), so it is not among the vouchers offered.
    let provider = FixedAnswers::new().with_bank_line(7, row7(vec![2]));
    let (_dir, mut api, statement) = setup(AiTier::Local, Some(provider));
    let BankAiAnswer::Suggested { matches, not_read } = suggested(&mut api, statement) else {
        panic!()
    };
    assert!(matches.is_empty(), "{matches:?}");
    let (row, reason) = not_read.iter().find(|(r, _)| *r == 7).unwrap();
    assert_eq!(*row, 7);
    assert!(reason.contains("not one the engine offered"), "{reason}");
    assert!(
        !kinds(&api)
            .iter()
            .any(|k| matches!(k, AiActivityKind::DraftStored { .. }))
    );
}

#[test]
fn a_suggestion_for_another_row_or_with_two_vouchers_is_refused() {
    let mut other_row = row7(vec![1]);
    other_row.statement_row = 8;
    for (answer, words) in [
        (other_row, "another statement line"),
        (row7(vec![1, 2]), "exactly one voucher"),
        (row7(Vec::new()), "exactly one voucher"),
    ] {
        let provider = FixedAnswers::new().with_bank_line(7, answer);
        let (_dir, mut api, statement) = setup(AiTier::Local, Some(provider));
        let BankAiAnswer::Suggested { matches, not_read } = suggested(&mut api, statement) else {
            panic!()
        };
        assert!(matches.is_empty(), "{matches:?}");
        let (_, reason) = not_read.iter().find(|(r, _)| *r == 7).unwrap();
        assert!(reason.contains(words), "{words}: {reason}");
    }
}

#[test]
fn a_voucher_from_the_line_is_not_built_and_is_refused() {
    let mut draft = row7(Vec::new());
    draft.ledger_name = Some("Customer A".to_owned());
    let provider = FixedAnswers::new().with_bank_line(7, draft);
    let (_dir, mut api, statement) = setup(AiTier::Local, Some(provider));
    let BankAiAnswer::Suggested { matches, not_read } = suggested(&mut api, statement) else {
        panic!()
    };
    assert_eq!(matches.len(), 0);
    let (_, reason) = not_read.iter().find(|(r, _)| *r == 7).unwrap();
    assert!(
        reason.contains("a voucher from a bank line is not built"),
        "{reason}"
    );
}

#[test]
fn accepting_records_an_ai_suggested_match_and_the_accept() {
    let provider = FixedAnswers::new().with_bank_line(7, row7(vec![1]));
    let (_dir, mut api, statement) = setup(AiTier::Local, Some(provider));
    let BankAiAnswer::Suggested { matches, .. } = suggested(&mut api, statement) else {
        panic!()
    };
    let result = committed(api.accept_bank_ai_match(statement, matches[0].draft));
    assert!(result.posted.is_none());
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!()
    };
    let cleared = books.vouchers[&VoucherId(1)]
        .ledger_lines
        .iter()
        .find(|l| l.ledger == BANK)
        .and_then(|l| l.bank.as_ref())
        .and_then(|b| b.bank_date);
    assert_eq!(cleared, Some(date!(2026 - 04 - 26)));
    let confirmed = api.bank_matches(statement).unwrap();
    assert_eq!(confirmed.len(), 1, "{confirmed:?}");
    assert_eq!(confirmed[0].kind, MatchKind::AiSuggested);
    assert_eq!(confirmed[0].statement_lines, vec![7]);
    assert_eq!(confirmed[0].vouchers, vec![VoucherId(1)]);
    assert!(kinds(&api).iter().any(|k| matches!(
        k,
        AiActivityKind::DraftAccepted {
            draft,
            voucher: None,
            ..
        } if *draft == matches[0].draft
    )));
    // The match is closed: the line is not suggested again, and the review queue has no draft.
    let BankAiAnswer::Suggested { matches: after, .. } = suggested(&mut api, statement) else {
        panic!()
    };
    assert!(after.is_empty(), "{after:?}");
    assert_eq!(api.ai_drafts().unwrap().len(), 0);
}

#[test]
fn a_rejected_suggestion_is_not_asked_again() {
    let provider = FixedAnswers::new().with_bank_line(7, row7(vec![1]));
    let (_dir, mut api, statement) = setup(AiTier::Local, Some(provider));
    let BankAiAnswer::Suggested { matches, .. } = suggested(&mut api, statement) else {
        panic!()
    };
    api.reject_ai_draft(matches[0].draft, None).unwrap();
    let BankAiAnswer::Suggested { matches: after, .. } = suggested(&mut api, statement) else {
        panic!()
    };
    assert!(after.is_empty(), "{after:?}");
    let stored = kinds(&api)
        .into_iter()
        .filter(|k| matches!(k, AiActivityKind::DraftStored { .. }))
        .count();
    assert_eq!(stored, 1);
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!()
    };
    let cleared = books.vouchers[&VoucherId(1)]
        .ledger_lines
        .iter()
        .find(|l| l.ledger == BANK)
        .and_then(|l| l.bank.as_ref())
        .and_then(|b| b.bank_date);
    assert_eq!(cleared, None, "a rejected suggestion clears nothing");
}

#[test]
fn accepting_after_the_voucher_changed_is_refused_by_the_engine() {
    let provider = FixedAnswers::new().with_bank_line(7, row7(vec![1]));
    let (_dir, mut api, statement) = setup(AiTier::Local, Some(provider));
    let BankAiAnswer::Suggested { matches, .. } = suggested(&mut api, statement) else {
        panic!()
    };
    // Voucher 1 now carries Rs 499.00: the amounts differ, so the engine refuses the accept.
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!()
    };
    let mut altered = books.vouchers[&VoucherId(1)].clone();
    altered.ledger_lines = vec![
        line(BANK, SignedAmount::dr(Paise::new(49_900))),
        line(CUSTOMER, SignedAmount::cr(Paise::new(49_900))),
    ];
    committed(api.execute(Command::AlterVoucher(altered)));
    match api
        .accept_bank_ai_match(statement, matches[0].draft)
        .unwrap()
    {
        Outcome::Unsupported(u) => assert!(u.reason.contains("amounts differ"), "{u:?}"),
        Outcome::Value(v) => panic!("expected a refusal, got {v:?}"),
    }
    assert_eq!(api.bank_matches(statement).unwrap().len(), 0);
}

#[test]
fn with_ai_off_nothing_is_asked_or_logged() {
    let provider = FixedAnswers::new().with_bank_line(7, row7(vec![1]));
    let (_dir, mut api, statement) = setup(AiTier::Off, Some(provider));
    assert!(matches!(
        suggested(&mut api, statement),
        BankAiAnswer::Refused(_)
    ));
    assert_eq!(api.ai_activity().unwrap().len(), 0);
}
