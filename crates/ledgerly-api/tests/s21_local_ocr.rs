//! Round s21: the local tier reads a bill photo with ocrs on this computer (owner decision
//! 2026-11-27: OCR is `ocrs`; coordinator decision: its values need review).
//!
//! The reader is attached to the API the way the key vault is. These tests attach a recorded
//! reader (its output is written here, at the needs-review confidence ocrs gives); the real
//! `OcrsReader` is attached by the app's `main` only. Nothing here loads a model or opens a
//! connection.
//!
//! Provenance (derived). The bill lines and their figures are those of `s21_ocrs_review`: "Total
//! 68,000.00" is 6,800,000 paise. The provider name and version are ocrs 0.14.0, the version in
//! Cargo.lock.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ledgerly_api::{
    AiFeature, AiGate, AiTier, Api, ApiError, BillOcr, BillRead, CloudSettings, Command,
    CommandResult, FixedAnswers, KEY_SERVICE, MemoryVault, OCRS_VERSION, OcrLine, OcrOutput,
    Proposal, ProviderInfo, ProviderPlace, ProviderStatus,
};
use ledgerly_assist::AssistError;
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

/// A reader that answers with the bill above and counts its reads.
#[derive(Debug)]
struct Recorded {
    status: ProviderStatus,
    reads: AtomicUsize,
}

impl BillOcr for Recorded {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "ocrs".to_owned(),
            version: OCRS_VERSION.to_owned(),
            place: ProviderPlace::ThisComputer,
        }
    }
    fn status(&self) -> ProviderStatus {
        self.status.clone()
    }
    fn read_png(
        &self,
        _png: &[u8],
        sha256: &str,
        _cancel: &CancelToken,
    ) -> Result<OcrOutput, AssistError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        assert_eq!(sha256, image_hash());
        Ok(bill())
    }
}

fn reader(status: ProviderStatus) -> Arc<Recorded> {
    Arc::new(Recorded {
        status,
        reads: AtomicUsize::new(0),
    })
}

#[test]
fn the_ocrs_version_is_the_locked_one() {
    assert_eq!(OCRS_VERSION, "0.14.0");
}

#[test]
fn with_the_local_tier_and_a_ready_reader_bill_reading_is_ready_as_ocrs_and_nothing_is_read() {
    let mut api = books("s21-local-ocr-gate.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    let r = reader(ProviderStatus::Ready);
    api.attach_bill_ocr(r.clone());
    match api.ai_gate(AiFeature::BillReading).unwrap() {
        AiGate::Ready(info) => {
            assert_eq!(info.name, "ocrs");
            assert_eq!(info.version, "0.14.0");
            assert_eq!(info.place, ProviderPlace::ThisComputer);
        }
        other @ (AiGate::NotChosen
        | AiGate::Off
        | AiGate::FeatureOff
        | AiGate::NotInstalled(_)
        | AiGate::NotConfigured(_)) => panic!("expected Ready, got {other:?}"),
    }
    // The reader reads bills only: plain words still have no local model.
    assert!(matches!(
        api.ai_gate(AiFeature::PlainWords).unwrap(),
        AiGate::NotInstalled(_)
    ));
    assert_eq!(r.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn a_reader_without_its_models_refuses_with_its_reason_and_reads_nothing() {
    let mut api = books("s21-local-ocr-missing.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    let r = reader(ProviderStatus::NotInstalled {
        reason: "the OCR model files are not there: text-detection.onnx".to_owned(),
    });
    api.attach_bill_ocr(r.clone());
    let out = api
        .read_bill_photo(IMAGE, "IMG_2041.png", &CancelToken::new())
        .unwrap();
    assert_eq!(
        out,
        BillRead::Refused(AiGate::NotInstalled(
            "the OCR model files are not there: text-detection.onnx".to_owned()
        ))
    );
    assert_eq!(r.reads.load(Ordering::SeqCst), 0);
    assert!(api.ai_activity().unwrap().is_empty());
}

#[test]
fn the_local_reader_drafts_the_bill_logged_as_ocrs_with_every_value_needing_review() {
    let mut api = books("s21-local-ocr-read.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    let r = reader(ProviderStatus::Ready);
    api.attach_bill_ocr(r.clone());
    let BillRead::Drafted { id, proposal, .. } = api
        .read_bill_photo(IMAGE, "IMG_2041.png", &CancelToken::new())
        .unwrap()
    else {
        panic!("expected a draft");
    };
    assert_eq!(r.reads.load(Ordering::SeqCst), 1);
    let Proposal::VoucherDraft(draft) = proposal else {
        panic!("expected a voucher draft");
    };
    assert_eq!(
        draft.document_total.as_ref().unwrap().value,
        Paise::new(6_800_000)
    );
    let fields = needs_review(&draft);
    assert!(fields.contains(&DraftField::DocumentTotal), "{fields:?}");
    assert!(fields.contains(&DraftField::Date), "{fields:?}");
    let log = api.ai_activity().unwrap();
    let entry = log.iter().find(|e| e.seq == id).unwrap();
    assert_eq!(entry.activity.provider, "ocrs");
    assert_eq!(entry.activity.model_version, "0.14.0");
}

#[test]
fn an_attached_provider_still_answers_before_the_reader() {
    let mut api = books("s21-local-ocr-provider.ledgerly");
    api.set_ai_tier(AiTier::Local).unwrap();
    let r = reader(ProviderStatus::Ready);
    api.attach_bill_ocr(r.clone());
    api.attach_assistant(Box::new(FixedAnswers::new().with_document(bill())));
    let BillRead::Drafted { .. } = api
        .read_bill_photo(IMAGE, "IMG_2041.png", &CancelToken::new())
        .unwrap()
    else {
        panic!("expected a draft");
    };
    assert_eq!(r.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn the_reader_does_not_serve_the_cloud_tier() {
    let mut api = books("s21-local-ocr-cloud.ledgerly");
    api.attach_key_vault(Box::new(MemoryVault::with_key(
        KEY_SERVICE,
        "company-1-Fictional Traders",
        "fictional-key-local-ocr-s21",
    )));
    let settings = CloudSettings {
        format: Some("messages".to_owned()),
        ..CloudSettings::default()
    };
    assert_eq!(
        api.set_cloud_settings(&settings).unwrap(),
        Outcome::Value(())
    );
    assert_eq!(api.set_ai_tier(AiTier::Cloud).unwrap(), Outcome::Value(()));
    let r = reader(ProviderStatus::Ready);
    api.attach_bill_ocr(r.clone());
    let gate = api.ai_gate(AiFeature::BillReading).unwrap();
    assert!(matches!(gate, AiGate::NotConfigured(_)), "{gate:?}");
}
