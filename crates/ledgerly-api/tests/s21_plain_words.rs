//! Round s21, plan round C: plain words to a draft voucher through the provider seam
//! (spec 6.8). One gate decides whether a feature may run: the tier, the feature's switch and the
//! provider's status. A read is always logged: a draft as `DraftStored`, a failure as
//! `ReadFailed`.
//!
//! Provenance (derived). The sentence and the recorded model JSON are written here. "25k" is
//! 25 x 1,000 = Rs 25,000.00 = 2,500,000 paise (`nl_entry::parse_amount_text`, its documented
//! rule). A payment debits the payee (Rent) and credits the account (Cash). The words of each
//! refusal are not checked, only which refusal it is.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_api::{
    AiDecision, AiFeature, AiGate, AiOutput, AiTier, Api, ApiError, Command, CommandResult,
    DraftCheck, DraftEdits, FixedAnswers, Proposal, WordsRead,
};
use ledgerly_core::{
    CancelToken, Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, NegativeStockPolicy,
    ReservedGroup,
};
use time::macros::{date, datetime};

const SENTENCE: &str = "paid rent 25k to Sharma Properties from Cash";
const JSON: &str = "{\"kind\":\"payment\",\"amount_text\":\"25k\",\"date_text\":null,\"party_name\":\"rent\",\"account_name\":\"Cash\",\"narration\":\"rent Sharma Properties\",\"confidence_permille\":900}";

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn ledger(id: u64, name: &str, kind: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(kind),
        opening: SignedAmount::dr(Paise::new(100_000_000)),
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

fn books(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let company = Company {
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
    };
    let mut api = Api::create(
        &path,
        &company,
        Clock::Fixed(datetime!(2026-11-21 10:00 UTC)),
    )
    .unwrap();
    for l in [
        ledger(1, "Cash", ReservedGroup::CashInHand),
        ledger(3, "Rent", ReservedGroup::IndirectExpenses),
    ] {
        committed(api.execute(Command::CreateLedger(l)));
    }
    api
}

fn gate(api: &Api) -> AiGate {
    api.ai_gate(AiFeature::PlainWords).unwrap()
}

#[test]
fn the_gate_refuses_until_a_tier_a_switch_and_a_ready_provider_allow_it() {
    let mut api = books("s21-words-gate.ledgerly");
    assert_eq!(gate(&api), AiGate::NotChosen);
    api.set_ai_tier(AiTier::Off).unwrap();
    assert_eq!(gate(&api), AiGate::Off);
    api.set_ai_tier(AiTier::Local).unwrap();
    // No provider attached and no runtime linked: the local model is not installed.
    assert!(
        matches!(gate(&api), AiGate::NotInstalled(_)),
        "{:?}",
        gate(&api)
    );
    api.attach_assistant(Box::new(FixedAnswers::new().with_text(SENTENCE, JSON)));
    assert!(matches!(gate(&api), AiGate::Ready(_)), "{:?}", gate(&api));
    api.set_ai_feature(AiFeature::PlainWords, false).unwrap();
    assert_eq!(gate(&api), AiGate::FeatureOff);
    // A refused read runs nothing and logs nothing.
    let read = api.read_words(SENTENCE, &CancelToken::new()).unwrap();
    assert_eq!(read, WordsRead::Refused(AiGate::FeatureOff));
    assert_eq!(api.ai_log_rows().unwrap().len(), 0);
}

#[test]
fn a_sentence_becomes_a_stored_draft_the_engine_balances_with_the_offered_date() {
    let mut api = books("s21-words-draft.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    api.attach_assistant(Box::new(FixedAnswers::new().with_text(SENTENCE, JSON)));
    let WordsRead::Drafted { id, proposal } =
        api.read_words(SENTENCE, &CancelToken::new()).unwrap()
    else {
        panic!("expected a draft");
    };
    let Proposal::VoucherDraft(draft) = &proposal else {
        panic!("expected a voucher draft");
    };
    assert_eq!(draft.kind, BaseVoucherKind::Payment);
    assert_eq!(draft.date, None, "no date was typed");
    let stored = api.ai_draft(id).unwrap();
    assert_eq!(stored.feature, AiFeature::PlainWords);
    assert_eq!(stored.input_label, SENTENCE);
    assert_eq!(stored.proposal, proposal);

    let edits = DraftEdits {
        date: Some(date!(2026 - 11 - 21)),
        place_of_supply: None,
    };
    let Outcome::Value(DraftCheck::Ok(voucher)) = api.check_ai_draft(id, &edits).unwrap() else {
        panic!("the engine should balance the draft with the offered date");
    };
    let lines: Vec<_> = voucher
        .ledger_lines
        .iter()
        .map(|l| (l.ledger, l.amount))
        .collect();
    assert!(
        lines.contains(&(LedgerId(3), SignedAmount::dr(Paise::new(2_500_000)))),
        "{lines:?}"
    );
    let rows = api.ai_log_rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].output,
        AiOutput::DraftVoucher(BaseVoucherKind::Payment)
    );
    assert_eq!(rows[0].decision, AiDecision::Waiting);
}

#[test]
fn a_sentence_the_provider_cannot_read_is_logged_as_a_failed_read() {
    let mut api = books("s21-words-failed.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    api.attach_assistant(Box::new(FixedAnswers::new()));
    let read = api.read_words("bought tea", &CancelToken::new()).unwrap();
    assert!(matches!(read, WordsRead::Failed(_)), "{read:?}");
    let rows = api.ai_log_rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert!(
        matches!(rows[0].output, AiOutput::ReadFailed(_)),
        "{:?}",
        rows[0]
    );
    assert_eq!(rows[0].input_label, "bought tea");
    assert_eq!(api.ai_drafts().unwrap().len(), 0);
}
