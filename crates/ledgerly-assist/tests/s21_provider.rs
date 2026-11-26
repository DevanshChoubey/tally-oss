//! Round s21: the provider seam behind the assistant (spec 6.8). No network, no keychain, no
//! model file is read beyond its size, and nothing is downloaded.
//!
//! Provenance (derived). The plain-words case uses the sentence and recorded model JSON of card
//! A2: "25k" is 25 x 1,000 = Rs 25,000.00 = 2,500,000 paise (`nl_entry::parse_amount_text`), and
//! the fixed-answers provider must give the same draft as the recorded model fed straight into
//! `NlEntry`, because it is that pipeline. The file sizes are the byte counts written here.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_assist::cloud::{
    CloudClient, CloudConfig, CloudProvider, CloudRequest, CloudResponse, CloudTransport, KeyStore,
};
use ledgerly_assist::nl_entry::{MasterLedger, NlEntry, RecordedModel};
use ledgerly_assist::ocr::OcrOutput;
use ledgerly_assist::provider::{
    AiProvider, CloudModel, FIXED_ANSWERS_NAME, FixedAnswers, LocalModel, LocalModelFile,
    ProviderInfo, ProviderPlace, ProviderProposer, ProviderStatus,
};
use ledgerly_assist::{AssistError, Proposal, ProposalInput, Proposer, ReportQuery};
use ledgerly_core::{CancelToken, LedgerId, Paise};
use ledgerly_model::Masters;

const RENT: &str = "paid rent 25k to Sharma Properties by HDFC";
const RENT_JSON: &str = "{\"kind\":\"payment\",\"amount_text\":\"25k\",\"date_text\":null,\"party_name\":\"Sharma Properties\",\"account_name\":\"HDFC\",\"narration\":\"rent\",\"confidence_permille\":900}";

fn ledgers() -> Vec<MasterLedger> {
    vec![
        MasterLedger {
            id: LedgerId(5),
            name: "Sharma Properties".to_owned(),
            aliases: vec![],
        },
        MasterLedger {
            id: LedgerId(2),
            name: "HDFC Bank".to_owned(),
            aliases: vec![],
        },
    ]
}

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

#[test]
fn the_local_model_reports_its_file_and_is_never_ready() {
    let none = LocalModel::new(None);
    assert_eq!(none.file(), LocalModelFile::NotChosen);

    let missing_path = scratch("s21-no-such-model.gguf");
    let _removed = std::fs::remove_file(&missing_path);
    let missing = LocalModel::new(Some(missing_path.clone()));
    assert_eq!(missing.file(), LocalModelFile::Missing(missing_path));

    let found_path = scratch("s21-model.gguf");
    std::fs::write(&found_path, [0_u8; 37]).unwrap();
    let found = LocalModel::new(Some(found_path.clone()));
    assert_eq!(
        found.file(),
        LocalModelFile::Found {
            path: found_path,
            bytes: 37
        }
    );

    for model in [&none, &missing, &found] {
        assert!(
            matches!(model.status(), ProviderStatus::NotInstalled { .. }),
            "{:?}",
            model.status()
        );
        assert_eq!(model.info().place, ProviderPlace::ThisComputer);
        let out = model.read_text(RENT, &ledgers(), &CancelToken::new());
        assert!(matches!(out, Err(AssistError::Unsupported(_))), "{out:?}");
        let out = model.ask("cash balance", &CancelToken::new());
        assert!(matches!(out, Err(AssistError::Unsupported(_))), "{out:?}");
    }
}

#[test]
fn fixed_answers_feed_the_real_plain_words_reader() {
    let provider = FixedAnswers::new().with_text(RENT, RENT_JSON);
    assert_eq!(
        provider.info(),
        ProviderInfo {
            name: FIXED_ANSWERS_NAME.to_owned(),
            version: "s21-1".to_owned(),
            place: ProviderPlace::Test,
        }
    );
    assert_eq!(provider.status(), ProviderStatus::Ready);
    let got = provider
        .read_text(RENT, &ledgers(), &CancelToken::new())
        .unwrap();
    let straight = NlEntry::new(
        RecordedModel::new().with_recording(RENT, RENT_JSON),
        ledgers(),
    )
    .propose(&ProposalInput::Text(RENT.to_owned()), &CancelToken::new())
    .unwrap();
    assert_eq!(got, straight);
    let Proposal::VoucherDraft(draft) = got else {
        panic!("expected a voucher draft");
    };
    assert_eq!(
        draft.document_total.map(|t| t.value),
        Some(Paise::new(2_500_000))
    );
}

#[test]
fn fixed_answers_refuse_what_they_have_no_recording_for() {
    let provider = FixedAnswers::new();
    let out = provider.read_text("bought tea", &ledgers(), &CancelToken::new());
    assert!(out.is_err(), "{out:?}");
    let out = provider.ocr(b"bytes", &"cd".repeat(32), &CancelToken::new());
    assert!(matches!(out, Err(AssistError::Output { .. })), "{out:?}");
    let off = FixedAnswers::new()
        .with_text(RENT, RENT_JSON)
        .not_installed("no model chosen");
    assert!(matches!(off.status(), ProviderStatus::NotInstalled { .. }));
    let out = off.read_text(RENT, &ledgers(), &CancelToken::new());
    assert!(matches!(out, Err(AssistError::Unsupported(_))), "{out:?}");
}

/// A provider that reads some other document than the one it was handed.
#[derive(Debug)]
struct WrongDocument;

impl AiProvider for WrongDocument {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "wrong document".to_owned(),
            version: "0".to_owned(),
            place: ProviderPlace::Test,
        }
    }
    fn status(&self) -> ProviderStatus {
        ProviderStatus::Ready
    }
    fn read_text(
        &self,
        _text: &str,
        _ledgers: &[MasterLedger],
        _cancel: &CancelToken,
    ) -> Result<Proposal, AssistError> {
        Err(AssistError::Output {
            reason: "not used".to_owned(),
        })
    }
    fn ocr(
        &self,
        _bytes: &[u8],
        _sha256: &str,
        _cancel: &CancelToken,
    ) -> Result<OcrOutput, AssistError> {
        Ok(OcrOutput {
            doc_hash: "ee".repeat(32),
            lines: Vec::new(),
        })
    }
    fn ask(&self, _question: &str, _cancel: &CancelToken) -> Result<ReportQuery, AssistError> {
        Err(AssistError::Output {
            reason: "not used".to_owned(),
        })
    }
}

#[test]
fn a_reading_of_another_document_is_refused() {
    let masters = Masters::default();
    let seam = ProviderProposer::new(&WrongDocument, &masters);
    let out = seam.propose(
        &ProposalInput::Document {
            bytes: b"bill".to_vec(),
            mime: "image/jpeg".to_owned(),
            sha256: "aa".repeat(32),
        },
        &CancelToken::new(),
    );
    assert!(
        matches!(&out, Err(AssistError::Output { reason }) if reason.contains("different document")),
        "{out:?}"
    );
}

struct NoKey;

impl KeyStore for NoKey {
    fn api_key(&self, _service: &str, _account: &str) -> Result<String, AssistError> {
        panic!("reading a document in the cloud must not ask for the key");
    }
}

struct NoNetwork;

impl CloudTransport for NoNetwork {
    fn post(&self, _request: &CloudRequest) -> Result<CloudResponse, AssistError> {
        panic!("reading a document in the cloud must not send anything");
    }
}

#[test]
fn the_cloud_model_never_sends_a_document_image() {
    let client = CloudClient::new(
        CloudConfig {
            provider: CloudProvider::OpenAiCompatible,
            endpoint: "https://llm.example.invalid/v1/answer".to_owned(),
            model: "model-x".to_owned(),
            key_service: "ledgerly".to_owned(),
            key_account: "company-1".to_owned(),
            max_output_tokens: 1024,
        },
        Box::new(NoKey),
        Box::new(NoNetwork),
    );
    let model = CloudModel::new(client);
    assert_eq!(model.info().place, ProviderPlace::Cloud);
    let out = model.ocr(b"bill", &"aa".repeat(32), &CancelToken::new());
    assert!(matches!(out, Err(AssistError::Unsupported(_))), "{out:?}");
}
