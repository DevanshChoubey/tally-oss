//! Round s21: a bill read by ocrs needs review field by field (coordinator decision 2026-11-27:
//! ocrs gives no confidence, so each value is "needs review", the lowest confidence, and the
//! person confirms each field before the draft can post).
//!
//! Provenance (derived). The bill lines are written here, each at the needs-review confidence an
//! ocrs reading carries. The reader copies the printed figures: "Taxable Value 68,000.00" is
//! 6,800,000 paise and "Total 68,000.00" the document total. With Purchases chosen for the taxable
//! line the voucher is Purchases Dr 68,000.00 against Rupa Polymers Cr 68,000.00 (as in
//! `s21_bill_photo`), but only once every read field is confirmed.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_api::{
    Accepted, AiActivityKind, AiTier, Api, ApiError, BillRead, Command, CommandResult, DraftCheck,
    DraftEdits, FixedAnswers, OcrLine, OcrOutput, Proposal,
};
use ledgerly_assist::review::{DraftField, NEEDS_REVIEW_PERMILLE, needs_review};
use ledgerly_core::{
    CancelToken, Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Ledger, NegativeStockPolicy, ReservedGroup,
};
use sha2::{Digest, Sha256};
use time::macros::{date, datetime};

/// Stand-in image bytes: the reader never sees pixels, only the provider's OCR lines.
const IMAGE: &[u8] = b"fictional bill image bytes";

fn image_hash() -> String {
    use std::fmt::Write as _;
    Sha256::digest(IMAGE)
        .iter()
        .fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

fn line(text: &str, top: u32) -> OcrLine {
    OcrLine {
        page: 1,
        text: text.to_owned(),
        bbox: [40, top, 400, 18],
        confidence_permille: NEEDS_REVIEW_PERMILLE,
    }
}

fn bill() -> OcrOutput {
    OcrOutput {
        doc_hash: image_hash(),
        lines: vec![
            line("Supplier: Rupa Polymers", 20),
            line("Invoice No: RP/5521", 60),
            line("Date: 17/11/2026", 80),
            line("Taxable Value 68,000.00", 300),
            line("Total 68,000.00", 360),
        ],
    }
}

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
        group: Some(group(kind)),
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
        ledger(4, "Purchases", ReservedGroup::PurchaseAccounts),
    ] {
        committed(api.execute(Command::CreateLedger(l)));
    }
    // The supplier is a party ledger: the reader resolves the bill's supplier only among ledgers
    // with party details (`ocr.rs`, `party_ids_by_name`).
    let mut supplier = ledger(5, "Rupa Polymers", ReservedGroup::SundryCreditors);
    supplier.party = Some(ledgerly_model::PartyDetails {
        registration: ledgerly_model::RegistrationType::Unregistered,
        gstin: None,
        pan: None,
        state: Some(StateCode::new(29).unwrap()),
        address: Address::default(),
    });
    committed(api.execute(Command::CreateLedger(supplier)));
    api
}

fn read(api: &mut Api) -> (ledgerly_core::EventSeq, Vec<DraftField>) {
    api.set_ai_tier(AiTier::Local).unwrap();
    api.attach_assistant(Box::new(FixedAnswers::new().with_document(bill())));
    let BillRead::Drafted { id, proposal, .. } = api
        .read_bill_photo(IMAGE, "IMG_2041.png", &CancelToken::new())
        .unwrap()
    else {
        panic!("expected a draft");
    };
    let Proposal::VoucherDraft(draft) = proposal else {
        panic!("expected a voucher draft");
    };
    (id, needs_review(&draft))
}

fn purchases_and(confirmed: Vec<DraftField>) -> DraftEdits {
    DraftEdits {
        line_ledgers: vec![(0, LedgerId(4))],
        confirmed,
        ..DraftEdits::default()
    }
}

#[test]
fn a_draft_with_a_value_still_needing_review_is_blocked_on_that_field() {
    let mut api = books("s21-ocrs-review-blocked.ledgerly");
    let (id, fields) = read(&mut api);
    assert!(fields.contains(&DraftField::Date), "{fields:?}");
    assert!(fields.contains(&DraftField::DocumentTotal), "{fields:?}");
    match api.check_ai_draft(id, &purchases_and(Vec::new())).unwrap() {
        Outcome::Value(DraftCheck::Blocked { field, reason }) => {
            assert_eq!(field, "date");
            assert!(reason.contains("needs review"), "{reason}");
        }
        other @ (Outcome::Value(_) | Outcome::Unsupported(_)) => {
            panic!("expected a block, got {other:?}")
        }
    }
}

#[test]
fn an_unconfirmed_printed_total_blocks_instead_of_being_skipped() {
    // Below the threshold a printed total is otherwise "not compared" (validate.rs); a total read
    // by ocrs and not confirmed must stop the draft, not drop out of the check.
    let mut api = books("s21-ocrs-review-total.ledgerly");
    let (id, fields) = read(&mut api);
    let all_but_total: Vec<DraftField> = fields
        .into_iter()
        .filter(|f| *f != DraftField::DocumentTotal)
        .collect();
    match api
        .check_ai_draft(id, &purchases_and(all_but_total))
        .unwrap()
    {
        Outcome::Value(DraftCheck::Blocked { field, .. }) => assert_eq!(field, "document_total"),
        other @ (Outcome::Value(_) | Outcome::Unsupported(_)) => {
            panic!("expected a block, got {other:?}")
        }
    }
}

#[test]
fn every_field_confirmed_the_engine_balances_the_bill_and_the_confirmations_are_logged() {
    use ledgerly_core::DrCr;
    let mut api = books("s21-ocrs-review-ok.ledgerly");
    let (id, fields) = read(&mut api);
    let edits = purchases_and(fields);
    let Outcome::Value(DraftCheck::Ok(voucher)) = api.check_ai_draft(id, &edits).unwrap() else {
        panic!("expected the engine's voucher");
    };
    let lines: Vec<(LedgerId, DrCr, Paise)> = voucher
        .ledger_lines
        .iter()
        .map(|l| (l.ledger, l.amount.side, l.amount.amount))
        .collect();
    assert_eq!(
        lines,
        vec![
            (LedgerId(4), DrCr::Dr, Paise::new(6_800_000)),
            (LedgerId(5), DrCr::Cr, Paise::new(6_800_000)),
        ]
    );
    assert_eq!(voucher.date, date!(2026 - 11 - 17));
    let accepted = api.accept_ai_draft(id, &edits).unwrap();
    assert!(
        matches!(accepted, Outcome::Value(Accepted::Posted(_))),
        "{accepted:?}"
    );
    let logged = api
        .ai_activity()
        .unwrap()
        .into_iter()
        .find_map(|e| {
            if let AiActivityKind::DraftAccepted {
                human_edits_json, ..
            } = e.activity.kind
            {
                human_edits_json
            } else {
                None
            }
        })
        .unwrap_or_default();
    assert!(logged.contains("\"confirmed\""), "{logged}");
    assert!(logged.contains("document_total"), "{logged}");
}

#[test]
fn confirming_a_field_the_draft_does_not_have_is_unsupported_not_ignored() {
    let mut api = books("s21-ocrs-review-missing.ledgerly");
    let (id, _) = read(&mut api);
    let edits = purchases_and(vec![DraftField::SupplierGstin]);
    let out = api.check_ai_draft(id, &edits).unwrap();
    assert!(matches!(out, Outcome::Unsupported(_)), "{out:?}");
}
