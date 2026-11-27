//! Round s21, plan round A: the assistant's draft store (spec 6.8, rule A10).
//!
//! A draft is stored as an audited event; it lists in the review queue with the engine's total;
//! accept posts it through the engine in one commit with the accept event; reject posts nothing.
//!
//! Provenance (derived). The draft is the one the plain-words reader makes of the typed sentence
//! "paid rent 25k to Sharma Properties from Cash": "25k" is 25 x 1,000 = Rs 25,000.00 =
//! 2,500,000 paise (the reader's own documented rule, `nl_entry::parse_amount_text`). A payment
//! debits the payee (Rent) and credits the account (Cash), so the engine's voucher has Dr Rent
//! 25,000.00 and Cr Cash 25,000.00, and its total (the debit sum) is 2,500,000 paise. Nothing here
//! is copied from the code under test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_api::{
    Accepted, AiActivityKind, AiFeature, Api, ApiError, Command, CommandResult, DraftCheck,
    DraftEdits, DraftFigure, DraftLine, NewAiDraft, Proposal, ReviewSource, SourceRef, Transcribed,
    VoucherDraft,
};
use ledgerly_core::{
    Clock, CompanyId, Date, DrCr, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, NegativeStockPolicy,
    ReservedGroup, Voucher,
};
use time::macros::{date, datetime};

const CASH: LedgerId = LedgerId(1);
const POWER: LedgerId = LedgerId(2);
const RENT: LedgerId = LedgerId(3);
const SUPPLIER: LedgerId = LedgerId(4);

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

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

fn ledger(id: LedgerId, name: &str, kind: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
        opening: SignedAmount::dr(rs(1_000_000)),
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

fn committed(out: Result<Outcome<CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn books(name: &str, vouchers: Vec<Voucher>) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(
        &path,
        &company(),
        Clock::Fixed(datetime!(2026-11-21 10:00 UTC)),
    )
    .unwrap();
    for l in [
        ledger(CASH, "Cash", ReservedGroup::CashInHand),
        ledger(POWER, "Electricity", ReservedGroup::IndirectExpenses),
        ledger(RENT, "Rent", ReservedGroup::IndirectExpenses),
        Ledger {
            bill_wise: Some(ledgerly_model::BillWiseConfig {
                default_credit_days: None,
            }),
            opening: SignedAmount::dr(Paise::ZERO),
            ..ledger(SUPPLIER, "Supplier", ReservedGroup::SundryCreditors)
        },
    ] {
        committed(api.execute(Command::CreateLedger(l)));
    }
    for v in vouchers {
        committed(api.execute(Command::PostVoucher(v)));
    }
    api
}

const SENTENCE: &str = "paid rent 25k to Sharma Properties from Cash";

fn typed<T>(value: T, start: usize, end: usize) -> Transcribed<T> {
    Transcribed {
        value,
        source: SourceRef::Typed { start, end },
        confidence_permille: 900,
    }
}

/// The draft the plain-words reader makes of [`SENTENCE`] (no date was typed).
fn rent_draft() -> NewAiDraft {
    let amount = typed(rs(25_000), 10, 13);
    let draft = VoucherDraft {
        kind: BaseVoucherKind::Payment,
        date: None,
        party_name: Some(typed("rent".to_owned(), 5, 9)),
        party: Some(RENT),
        invoice_number: None,
        supplier_gstin: None,
        lines: vec![
            DraftLine {
                ledger_name: typed("rent".to_owned(), 5, 9),
                ledger: Some(RENT),
                side: DrCr::Dr,
                amount: amount.clone(),
            },
            DraftLine {
                ledger_name: typed("Cash".to_owned(), 40, 44),
                ledger: Some(CASH),
                side: DrCr::Cr,
                amount: amount.clone(),
            },
        ],
        items: Vec::new(),
        document_total: Some(amount),
        document_tax: None,
        create_ledgers: Vec::new(),
        narration: "rent Sharma Properties".to_owned(),
    };
    NewAiDraft {
        feature: AiFeature::PlainWords,
        provider: "Fixed answers (test provider)".to_owned(),
        model_version: "s21-1".to_owned(),
        prompt_template_version: "nl-entry/1".to_owned(),
        input_label: SENTENCE.to_owned(),
        input_hash: "ab".repeat(32),
        proposal: Proposal::VoucherDraft(Box::new(draft)),
        confidence_permille: 900,
    }
}

const TODAY: Date = date!(2026 - 11 - 21);

#[test]
fn a_new_file_has_no_drafts() {
    let api = books("s21-drafts-none.ledgerly", Vec::new());
    assert_eq!(api.ai_drafts().unwrap().len(), 0);
    assert_eq!(api.ai_activity().unwrap().len(), 0);
}

#[test]
fn a_recorded_draft_without_a_date_is_listed_blocked_on_the_date() {
    let mut api = books("s21-drafts-listed.ledgerly", Vec::new());
    let id = api.record_ai_draft(rent_draft()).unwrap();
    let drafts = api.ai_drafts().unwrap();
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].id, id);
    assert_eq!(drafts[0].feature, AiFeature::PlainWords);
    let queue = api.review_queue_on(TODAY).unwrap();
    assert_eq!(queue.drafts.len(), 1);
    assert!(
        matches!(&queue.drafts[0].engine, DraftFigure::Blocked(reason) if reason.contains("date")),
        "no date was typed, so the queue shows no figure and says why: {:?}",
        queue.drafts[0].engine
    );
}

#[test]
fn a_dated_draft_is_listed_with_the_engine_total() {
    let mut api = books("s21-drafts-listed-dated.ledgerly", Vec::new());
    let mut draft = rent_draft();
    let Proposal::VoucherDraft(v) = &mut draft.proposal else {
        panic!()
    };
    v.date = Some(typed(TODAY, 0, 0));
    api.record_ai_draft(draft).unwrap();
    let queue = api.review_queue_on(TODAY).unwrap();
    assert_eq!(
        queue.drafts[0].engine,
        DraftFigure::Total(Paise::new(2_500_000))
    );
}

#[test]
fn the_check_is_the_engine_voucher_with_the_offered_date() {
    let mut api = books("s21-drafts-check.ledgerly", Vec::new());
    let id = api.record_ai_draft(rent_draft()).unwrap();
    let blocked = api.check_ai_draft(id, &DraftEdits::default()).unwrap();
    assert!(
        matches!(&blocked, Outcome::Value(DraftCheck::Blocked { field, .. }) if field == "date"),
        "no date was typed, so the draft is blocked on the date: {blocked:?}"
    );
    let edits = DraftEdits {
        date: Some(TODAY),
        place_of_supply: None,
    };
    let Outcome::Value(DraftCheck::Ok(voucher)) = api.check_ai_draft(id, &edits).unwrap() else {
        panic!("the draft with a date should pass the engine's check");
    };
    assert_eq!(voucher.date, TODAY);
    assert_eq!(voucher.ledger_lines.len(), 2);
    assert_eq!(voucher.ledger_lines[0].ledger, RENT);
    assert_eq!(
        voucher.ledger_lines[0].amount,
        SignedAmount::dr(Paise::new(2_500_000))
    );
    assert_eq!(voucher.ledger_lines[1].ledger, CASH);
    assert_eq!(
        voucher.ledger_lines[1].amount,
        SignedAmount::cr(Paise::new(2_500_000))
    );
}

#[test]
fn accept_posts_the_voucher_and_records_the_accept_in_one_commit() {
    let mut api = books("s21-drafts-accept.ledgerly", Vec::new());
    let id = api.record_ai_draft(rent_draft()).unwrap();
    let before = api.books().unwrap();
    let Outcome::Value(before) = before else {
        panic!()
    };
    let edits = DraftEdits {
        date: Some(TODAY),
        place_of_supply: None,
    };
    let Outcome::Value(Accepted::Posted(voucher)) = api.accept_ai_draft(id, &edits).unwrap() else {
        panic!("accept should post");
    };
    let Outcome::Value(after) = api.books().unwrap() else {
        panic!()
    };
    assert_eq!(after.vouchers.len(), before.vouchers.len() + 1);
    assert!(after.vouchers.contains_key(&voucher.id));
    assert!(
        api.ai_drafts().unwrap().is_empty(),
        "an accepted draft is closed"
    );
    let log = api.ai_activity().unwrap();
    assert_eq!(log.len(), 2);
    match &log[1].activity.kind {
        AiActivityKind::DraftAccepted {
            draft,
            voucher: Some(posted),
            human_edits_json: Some(edits),
        } => {
            assert_eq!(*draft, id);
            assert_eq!(*posted, voucher.id);
            assert!(edits.contains("2026-11-21"), "{edits}");
        }
        other @ (AiActivityKind::DraftAccepted { .. }
        | AiActivityKind::DraftStored { .. }
        | AiActivityKind::DraftRejected { .. }
        | AiActivityKind::Answered { .. }
        | AiActivityKind::ReadFailed { .. }
        | AiActivityKind::CloudConsent { .. }) => panic!("expected the accept, got {other:?}"),
    }
    assert_eq!(
        api.ai_origin(voucher.id).unwrap().map(|a| a.feature),
        Some(AiFeature::PlainWords)
    );
    assert!(matches!(
        api.accept_ai_draft(id, &edits),
        Err(ApiError::UnknownAiDraft { .. })
    ));
}

#[test]
fn a_blocked_draft_posts_nothing_and_stays_open() {
    let mut api = books("s21-drafts-blocked.ledgerly", Vec::new());
    let id = api.record_ai_draft(rent_draft()).unwrap();
    let out = api.accept_ai_draft(id, &DraftEdits::default()).unwrap();
    assert!(
        matches!(out, Outcome::Value(Accepted::Blocked { .. })),
        "{out:?}"
    );
    assert_eq!(api.ai_drafts().unwrap().len(), 1);
    assert_eq!(api.ai_activity().unwrap().len(), 1);
}

#[test]
fn reject_records_the_reason_and_posts_nothing() {
    let mut api = books("s21-drafts-reject.ledgerly", Vec::new());
    let id = api.record_ai_draft(rent_draft()).unwrap();
    let Outcome::Value(before) = api.books().unwrap() else {
        panic!()
    };
    api.reject_ai_draft(id, Some("Wrong ledger".to_owned()))
        .unwrap();
    let Outcome::Value(after) = api.books().unwrap() else {
        panic!()
    };
    assert_eq!(after.vouchers.len(), before.vouchers.len());
    assert_eq!(api.ai_drafts().unwrap().len(), 0);
    let log = api.ai_activity().unwrap();
    assert!(matches!(
        &log[1].activity.kind,
        AiActivityKind::DraftRejected { draft, reason: Some(r) } if *draft == id && r == "Wrong ledger"
    ));
    assert_eq!(api.review_queue_on(TODAY).unwrap().drafts.len(), 0);
}

#[test]
fn without_an_assistant_the_draft_sources_stay_unlisted() {
    let api = books("s21-drafts-unlisted.ledgerly", Vec::new());
    let queue = api.review_queue_on(TODAY).unwrap();
    assert!(
        queue
            .unlisted
            .iter()
            .any(|u| u.source == ReviewSource::BillDraft)
    );
    assert!(
        queue
            .unlisted
            .iter()
            .any(|u| u.source == ReviewSource::PlainWordsDraft)
    );
}

/// Provenance (derived): the header is the AI settings log's columns (mock `AI_LOG`,
/// `design/mock-v2/index.html`) plus the provider and the full input hash; the row is the draft
/// this test records, still waiting.
#[test]
fn the_activity_log_exports_each_assistant_event_with_its_decision() {
    let mut api = books("s21-drafts-export.ledgerly", Vec::new());
    let id = api.record_ai_draft(rent_draft()).unwrap();
    let out = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s21-activity.csv");
    let _removed = std::fs::remove_file(&out);
    assert_eq!(
        api.export_ai_activity(&out).unwrap(),
        ledgerly_core::Outcome::Value(())
    );
    let text = std::fs::read_to_string(&out).unwrap();
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("When,Feature,Provider,Input,Input hash,Output,Your decision")
    );
    let row = lines.next().unwrap();
    assert!(row.contains(",Plain words,"), "{row}");
    assert!(row.contains(SENTENCE), "{row}");
    assert!(row.contains(",Draft payment,"), "{row}");
    assert!(row.ends_with(",Waiting in review"), "{row}");
    assert_eq!(lines.next(), None);
    api.reject_ai_draft(id, Some("wrong supplier".to_owned()))
        .unwrap();
    api.export_ai_activity(&out).unwrap();
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(
        text.lines()
            .nth(1)
            .unwrap()
            .ends_with(",Rejected: wrong supplier"),
        "{text}"
    );
}
