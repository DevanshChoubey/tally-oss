//! Round s21, plan round D: a bill photo to a draft purchase through the provider seam (spec 6.8).
//! The same gate as plain words decides whether bill reading may run. The provider returns the OCR
//! lines of the document (recorded here; no OCR engine is linked) and the real bill reader
//! (`ledgerly_assist::ocr::read_bill`) turns them into a draft. A read is always logged: a draft
//! as `DraftStored`, a failure as `ReadFailed`; a refusal runs nothing.
//!
//! Provenance (derived). The bill lines are written here. The reader copies the printed figures:
//! "Taxable Value 68,000.00" is 6,800,000 paise and "Total 68,000.00" the document total
//! (`ocr.rs` module documentation: printed totals are copied, never summed). The input hash is
//! the SHA-256 of the image bytes, the provider's key for its recording.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_api::{
    AiFeature, AiGate, AiOutput, AiTier, Api, ApiError, BillRead, Command, CommandResult,
    FixedAnswers, OcrLine, OcrOutput, Proposal,
};
use ledgerly_core::{
    CancelToken, Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, NegativeStockPolicy,
    ReservedGroup,
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
        confidence_permille: 940,
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

#[test]
fn a_bill_photo_is_refused_until_the_gate_allows_bill_reading() {
    let mut api = books("s21-bill-gate.ledgerly");
    let read = api
        .read_bill_photo(IMAGE, "IMG_2041.jpg", &CancelToken::new())
        .unwrap();
    assert!(
        matches!(read, BillRead::Refused(AiGate::NotChosen)),
        "{read:?}"
    );
    api.set_ai_tier(AiTier::Local).unwrap();
    api.attach_assistant(Box::new(FixedAnswers::new().with_document(bill())));
    api.set_ai_feature(AiFeature::BillReading, false).unwrap();
    let read = api
        .read_bill_photo(IMAGE, "IMG_2041.jpg", &CancelToken::new())
        .unwrap();
    assert!(
        matches!(read, BillRead::Refused(AiGate::FeatureOff)),
        "{read:?}"
    );
    // A refusal runs nothing and logs nothing.
    assert_eq!(api.ai_log_rows().unwrap().len(), 0);
}

#[test]
fn a_read_bill_is_stored_as_a_purchase_draft_with_the_printed_figures() {
    let mut api = books("s21-bill-draft.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    api.attach_assistant(Box::new(FixedAnswers::new().with_document(bill())));
    let read = api
        .read_bill_photo(IMAGE, "IMG_2041.jpg", &CancelToken::new())
        .unwrap();
    let BillRead::Drafted { id, proposal, .. } = read else {
        panic!("expected a draft, got {read:?}");
    };
    let Proposal::VoucherDraft(draft) = proposal else {
        panic!("expected a voucher draft");
    };
    assert_eq!(draft.kind, BaseVoucherKind::Purchase);
    assert_eq!(draft.party, Some(LedgerId(5)));
    assert_eq!(draft.invoice_number.as_ref().unwrap().value, "RP/5521");
    assert_eq!(
        draft.document_total.as_ref().unwrap().value,
        Paise::new(6_800_000)
    );
    let drafts = api.ai_drafts().unwrap();
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].id, id);
    let rows = api.ai_log_rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].feature, AiFeature::BillReading);
    assert_eq!(rows[0].input_label, "IMG_2041.jpg");
    assert_eq!(rows[0].input_hash, image_hash());
}

#[test]
fn a_photo_the_provider_cannot_read_is_logged_as_a_failed_read() {
    let mut api = books("s21-bill-failed.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    api.attach_assistant(Box::new(FixedAnswers::new()));
    let read = api
        .read_bill_photo(IMAGE, "IMG_2041.jpg", &CancelToken::new())
        .unwrap();
    assert!(matches!(read, BillRead::Failed(_)), "{read:?}");
    let rows = api.ai_log_rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert!(
        matches!(rows[0].output, AiOutput::ReadFailed(_)),
        "{:?}",
        rows[0]
    );
    assert_eq!(api.ai_drafts().unwrap().len(), 0);
}
