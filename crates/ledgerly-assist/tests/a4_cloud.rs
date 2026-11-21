//! Card A4 tests: the cloud BYOK client (spec 6.8, 9). No network and no keychain: the key store
//! and the transport are recorded doubles.
//!
//! Provenance. Every expected string is worked out by hand from the redaction rules in
//! `redact.rs` (placeholders `[KIND_n]` numbered per kind in order of first appearance) and from
//! character counts written next to each case. Identifiers, names and amounts are fictional; the
//! GSTIN is the hand-derived vector of `ledgerly-core`. The provider wire formats (header names,
//! body shape) are derived from the providers' public API documentation as recalled by the card
//! author, not observed against a live endpoint: `pending` until observed against each provider
//! (rule A4); the tests assert only the credential placement and the redacted content, which are
//! this module's own contract.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unnecessary_wraps,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::sync::{Arc, Mutex};

use ledgerly_assist::cloud::{
    CloudClient, CloudConfig, CloudProvider, CloudRequest, CloudResponse, CloudTransport, KeyStore,
    PROMPT_TEMPLATE_VERSION,
};
use ledgerly_assist::{
    AssistError, DraftItem, DraftLine, MappingSuggestion, MatchSuggestion, Proposal, ProposalInput,
    Proposer, SourceRef, Transcribed, VoucherDraft,
};
use ledgerly_core::{
    CancelToken, Decimal, DrCr, LedgerId, Paise, Qty, Rate, SignedAmount, VoucherId,
};
use ledgerly_model::BaseVoucherKind;
use serde_json::{Value, json};
use time::macros::date;

const KEY: &str = "sk-test-0000-fictional-key";

#[derive(Clone, Default)]
struct Spy {
    sent: Arc<Mutex<Vec<CloudRequest>>>,
    asked: Arc<Mutex<Vec<(String, String)>>>,
}

struct FakeKeys {
    key: Result<String, AssistError>,
    spy: Spy,
}

impl KeyStore for FakeKeys {
    fn api_key(&self, service: &str, account: &str) -> Result<String, AssistError> {
        self.spy
            .asked
            .lock()
            .unwrap()
            .push((service.to_owned(), account.to_owned()));
        self.key.clone()
    }
}

struct FakeTransport {
    reply: Result<CloudResponse, AssistError>,
    spy: Spy,
}

impl CloudTransport for FakeTransport {
    fn post(&self, request: &CloudRequest) -> Result<CloudResponse, AssistError> {
        self.spy.sent.lock().unwrap().push(request.clone());
        self.reply.clone()
    }
}

fn config(provider: CloudProvider) -> CloudConfig {
    CloudConfig {
        provider,
        endpoint: "https://llm.example.invalid/v1/answer".to_owned(),
        model: "model-x".to_owned(),
        key_service: "ledgerly".to_owned(),
        key_account: "company-1".to_owned(),
        max_output_tokens: 1024,
    }
}

fn client_with(
    config: CloudConfig,
    key: Result<String, AssistError>,
    reply: Result<CloudResponse, AssistError>,
) -> (CloudClient, Spy) {
    let spy = Spy::default();
    let client = CloudClient::new(
        config,
        Box::new(FakeKeys {
            key,
            spy: spy.clone(),
        }),
        Box::new(FakeTransport {
            reply,
            spy: spy.clone(),
        }),
    );
    (client, spy)
}

/// An OpenAI-style envelope around the model's text.
fn openai_reply(model_text: &str) -> Result<CloudResponse, AssistError> {
    Ok(CloudResponse {
        status: 200,
        body: json!({"choices": [{"message": {"content": model_text}}]}).to_string(),
    })
}

fn anthropic_reply(model_text: &str) -> Result<CloudResponse, AssistError> {
    Ok(CloudResponse {
        status: 200,
        body: json!({"content": [{"type": "text", "text": model_text}]}).to_string(),
    })
}

fn typed<T>(value: T, start: usize, end: usize, confidence_permille: u16) -> Transcribed<T> {
    Transcribed {
        value,
        source: SourceRef::Typed { start, end },
        confidence_permille,
    }
}

/// Recorded (hand-written, fictional) model output for the input
/// `"PAN ABCDE1234F paid 25000 to Sharma"` as the model sees it:
/// `"PAN [PAN_1] paid 25000 to Sharma"`. Redacted-text character ranges: `25000` is 17..22 and
/// `Sharma` is 26..32. It also carries two ids the model cannot know.
fn recorded_draft() -> Proposal {
    Proposal::VoucherDraft(Box::new(VoucherDraft {
        kind: BaseVoucherKind::Payment,
        date: None,
        party_name: Some(typed("Sharma".to_owned(), 26, 32, 900)),
        party: Some(LedgerId(99)),
        invoice_number: None,
        supplier_gstin: None,
        lines: vec![DraftLine {
            ledger_name: typed("Sharma".to_owned(), 26, 32, 900),
            ledger: Some(LedgerId(5)),
            side: DrCr::Dr,
            amount: typed(Paise::new(2_500_000), 17, 22, 950),
        }],
        items: vec![],
        document_total: None,
        document_tax: None,
        create_ledgers: vec![],
        narration: "Payment for [PAN_1]".to_owned(),
    }))
}

const TEXT_INPUT: &str = "PAN ABCDE1234F paid 25000 to Sharma";

fn text_input() -> ProposalInput {
    ProposalInput::Text(TEXT_INPUT.to_owned())
}

fn body_of(request: &CloudRequest) -> Value {
    serde_json::from_str(&request.body).unwrap()
}

#[test]
fn identifiers_are_redacted_before_anything_is_sent() {
    // Fictional private identifiers of every kind. Derived by hand: PAN ABCDE1234F; GSTIN
    // 29ABCDE1234F1ZW (ledgerly-core vector); account 123456789012345 (15 digits, inside 9..=18);
    // IFSC HDFC0000123 (HDFC, 0, 000123); mobile 9876543210 (10 digits starting 9).
    let originals = [
        "ABCDE1234F",
        "29ABCDE1234F1ZW",
        "123456789012345",
        "HDFC0000123",
        "9876543210",
    ];
    let text = format!(
        "rent to {} gstin {} a/c {} ifsc {} mob {}",
        originals[0], originals[1], originals[2], originals[3], originals[4]
    );
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply("not a proposal"),
    );
    // The answer is rejected, but the request has already been made: that is what is inspected.
    let _ = client.propose(&ProposalInput::Text(text), &CancelToken::new());
    let sent = spy.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    let body = &sent[0].body;
    for original in originals {
        assert!(!body.contains(original), "{original} leaked in the body");
        assert!(!sent[0].url.contains(original));
    }
    let message = body_of(&sent[0])["messages"][1]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    // Placeholders are numbered per kind: PAN_1, GSTIN_1, ACCOUNT_1, IFSC_1, PHONE_1.
    assert!(message.ends_with(
        "INPUT:\nrent to [PAN_1] gstin [GSTIN_1] a/c [ACCOUNT_1] ifsc [IFSC_1] mob [PHONE_1]"
    ));
}

#[test]
fn key_comes_from_the_key_store_and_only_travels_in_a_header() {
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply("{}"),
    );
    let _ = client.propose(&text_input(), &CancelToken::new());
    assert_eq!(
        *spy.asked.lock().unwrap(),
        vec![("ledgerly".to_owned(), "company-1".to_owned())]
    );
    let sent = spy.sent.lock().unwrap();
    let request = &sent[0];
    assert!(!request.body.contains(KEY));
    assert!(!request.url.contains(KEY));
    assert!(
        request
            .headers
            .iter()
            .any(|(n, v)| n == "Authorization" && v == &format!("Bearer {KEY}"))
    );
    // Neither the request's nor the client's Debug output shows the key.
    assert!(!format!("{request:?}").contains(KEY));
    assert!(!format!("{client:?}").contains(KEY));
}

#[test]
fn anthropic_style_request_uses_its_own_header_and_envelope() {
    let (client, spy) = client_with(
        config(CloudProvider::Anthropic),
        Ok(KEY.to_owned()),
        anthropic_reply("{}"),
    );
    let _ = client.propose(&text_input(), &CancelToken::new());
    let sent = spy.sent.lock().unwrap();
    let request = &sent[0];
    assert!(
        request
            .headers
            .iter()
            .any(|(n, v)| n == "x-api-key" && v == KEY)
    );
    assert!(!request.body.contains(KEY));
    let body = body_of(request);
    assert_eq!(body["model"], "model-x");
    assert_eq!(body["max_tokens"], 1024);
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .ends_with("PAN [PAN_1] paid 25000 to Sharma")
    );
}

#[test]
fn answer_is_restored_checked_and_stripped_of_invented_ids() {
    let model_text = format!(
        "```json\n{}\n```",
        serde_json::to_string(&recorded_draft()).unwrap()
    );
    let (client, _) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply(&model_text),
    );
    let outcome = client
        .propose_detailed(&text_input(), &CancelToken::new())
        .unwrap();
    let Proposal::VoucherDraft(draft) = outcome.proposal else {
        panic!("expected a voucher draft");
    };
    // Placeholder restored locally.
    assert_eq!(draft.narration, "Payment for ABCDE1234F");
    // Ids the model could not know are dropped (never invented ids, spec 6.8).
    assert_eq!(draft.party, None);
    assert_eq!(draft.lines[0].ledger, None);
    // Offsets mapped back to the user's text. Hand count in the original text
    // "PAN ABCDE1234F paid 25000 to Sharma": PAN is 0..3, the PAN number 4..14, "paid" 15..19,
    // "25000" 20..25, "to" 26..28, "Sharma" 29..35. In the redacted text the same words are
    // 17..22 and 26..32 (the 10-character PAN became the 7-character [PAN_1]: shift 3).
    assert_eq!(
        draft.lines[0].amount.source,
        SourceRef::Typed { start: 20, end: 25 }
    );
    assert_eq!(
        draft.party_name.as_ref().unwrap().source,
        SourceRef::Typed { start: 29, end: 35 }
    );
    // The figure is the model's transcription; the engine will still recompute (rule A10).
    assert_eq!(draft.lines[0].amount.value, Paise::new(2_500_000));
    // Audit facts.
    assert_eq!(outcome.model, "model-x");
    assert_eq!(outcome.prompt_template_version, PROMPT_TEMPLATE_VERSION);
    assert_eq!(outcome.redactions.len(), 1);
    assert!(
        outcome
            .sent
            .ends_with("INPUT:\nPAN [PAN_1] paid 25000 to Sharma")
    );
    assert!(!outcome.sent.contains("ABCDE1234F"));
}

#[test]
fn preview_shows_what_would_be_sent_without_the_key_or_a_call() {
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Err(AssistError::Key {
            reason: "must not be asked".to_owned(),
        }),
        openai_reply("{}"),
    );
    let preview = client.preview(&text_input()).unwrap();
    assert!(
        preview
            .message
            .ends_with("INPUT:\nPAN [PAN_1] paid 25000 to Sharma")
    );
    assert_eq!(preview.redactions.len(), 1);
    assert!(spy.sent.lock().unwrap().is_empty());
    assert!(spy.asked.lock().unwrap().is_empty());
}

#[test]
fn bank_line_is_redacted_and_match_suggestion_loses_invented_voucher_ids() {
    let input = ProposalInput::BankLine {
        row: 3,
        date: date!(2026 - 03 - 31),
        description: "UPI/ABCDE1234F/rent".to_owned(),
        reference: Some("HDFC0000123".to_owned()),
        amount: SignedAmount {
            amount: Paise::new(2_500_000),
            side: DrCr::Dr,
        },
    };
    let suggestion = Proposal::MatchSuggestion(MatchSuggestion {
        statement_row: 3,
        vouchers: vec![VoucherId(1)],
        ledger_name: Some("Rent".to_owned()),
        reason: "rent".to_owned(),
        confidence_permille: 800,
    });
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply(&serde_json::to_string(&suggestion).unwrap()),
    );
    let proposal = client.propose(&input, &CancelToken::new()).unwrap();
    // 2_500_000 paise is 25000.00 rupees (derived: 2_500_000 / 100).
    let sent = spy.sent.lock().unwrap();
    let message = body_of(&sent[0])["messages"][1]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.ends_with(
        "INPUT:\nrow: 3\ndate: 2026-03-31\ndescription: UPI/[PAN_1]/rent\nreference: [IFSC_1]\n\
         amount: 25000.00 debit (bank's view)"
    ));
    let Proposal::MatchSuggestion(m) = proposal else {
        panic!("expected a match suggestion");
    };
    assert_eq!(m.vouchers, [] as [ledgerly_core::VoucherId; 0]);
    assert_eq!(m.ledger_name.as_deref(), Some("Rent"));
}

#[test]
fn columns_are_redacted() {
    let input = ProposalInput::Columns {
        headers: vec!["Date".to_owned(), "PAN".to_owned()],
        sample_rows: vec![vec!["31-03-2026".to_owned(), "ABCDE1234F".to_owned()]],
    };
    let mapping = Proposal::MappingSuggestion(MappingSuggestion {
        columns: vec![("Date".to_owned(), "date".to_owned())],
        confidence_permille: 700,
    });
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply(&serde_json::to_string(&mapping).unwrap()),
    );
    assert!(client.propose(&input, &CancelToken::new()).is_ok());
    let sent = spy.sent.lock().unwrap();
    assert!(!sent[0].body.contains("ABCDE1234F"));
    let message = body_of(&sent[0])["messages"][1]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.ends_with("INPUT:\nheaders: Date | PAN\nrow: 31-03-2026 | [PAN_1]"));
}

#[test]
fn documents_are_unsupported_for_the_cloud_tier() {
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply("{}"),
    );
    let input = ProposalInput::Document {
        bytes: vec![1, 2, 3],
        mime: "image/png".to_owned(),
        sha256: "00".repeat(32),
    };
    assert!(matches!(
        client.propose(&input, &CancelToken::new()),
        Err(AssistError::Unsupported(_))
    ));
    assert!(spy.sent.lock().unwrap().is_empty());
}

#[test]
fn a_cancelled_token_stops_before_the_key_is_read_or_anything_is_sent() {
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply("{}"),
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        client.propose(&text_input(), &cancel),
        Err(AssistError::Cancelled)
    );
    assert!(spy.sent.lock().unwrap().is_empty());
    assert!(spy.asked.lock().unwrap().is_empty());
}

#[test]
fn a_plain_http_endpoint_is_refused_before_any_send() {
    let mut cfg = config(CloudProvider::OpenAiCompatible);
    cfg.endpoint = "http://llm.example.invalid/v1".to_owned();
    let (client, spy) = client_with(cfg, Ok(KEY.to_owned()), openai_reply("{}"));
    assert!(matches!(
        client.propose(&text_input(), &CancelToken::new()),
        Err(AssistError::Unsupported(_))
    ));
    assert!(spy.sent.lock().unwrap().is_empty());
    assert!(spy.asked.lock().unwrap().is_empty());
}

#[test]
fn missing_or_empty_key_is_a_key_error_and_nothing_is_sent() {
    let missing = AssistError::Key {
        reason: "no key".to_owned(),
    };
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Err(missing.clone()),
        openai_reply("{}"),
    );
    assert_eq!(
        client.propose(&text_input(), &CancelToken::new()),
        Err(missing)
    );
    assert!(spy.sent.lock().unwrap().is_empty());

    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok("   ".to_owned()),
        openai_reply("{}"),
    );
    assert!(matches!(
        client.propose(&text_input(), &CancelToken::new()),
        Err(AssistError::Key { .. })
    ));
    assert!(spy.sent.lock().unwrap().is_empty());
}

#[test]
fn http_statuses_map_to_key_and_network_errors_without_echoing_the_key() {
    for (status, is_key) in [(401_u16, true), (403, true), (429, false), (500, false)] {
        let (client, _) = client_with(
            config(CloudProvider::OpenAiCompatible),
            Ok(KEY.to_owned()),
            Ok(CloudResponse {
                status,
                body: format!("denied for {KEY}"),
            }),
        );
        let err = client
            .propose(&text_input(), &CancelToken::new())
            .unwrap_err();
        assert_eq!(matches!(err, AssistError::Key { .. }), is_key, "{status}");
        assert_eq!(
            matches!(err, AssistError::Network { .. }),
            !is_key,
            "{status}"
        );
        assert!(!err.to_string().contains(KEY));
    }
}

#[test]
fn transport_failure_is_a_network_error() {
    let (client, _) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        Err(AssistError::Network {
            reason: "connection refused".to_owned(),
        }),
    );
    assert!(matches!(
        client.propose(&text_input(), &CancelToken::new()),
        Err(AssistError::Network { .. })
    ));
}

#[test]
fn answers_that_do_not_fit_are_rejected_not_repaired() {
    let good = serde_json::to_string(&recorded_draft()).unwrap();
    let report = serde_json::to_string(&Proposal::MappingSuggestion(MappingSuggestion {
        columns: vec![],
        confidence_permille: 1,
    }))
    .unwrap();
    // A source past the end of the text that was sent (redacted text has 32 characters).
    let past_end = good.replace("\"end\":32", "\"end\":33");
    // A source that claims a document region for typed text.
    let document_source = good.replace(
        "{\"Typed\":{\"start\":17,\"end\":22}}",
        "{\"Document\":{\"doc_hash\":\"ab\",\"page\":1,\"bbox\":[0,0,1,1]}}",
    );
    // A confidence above 1000.
    let too_sure = good.replace(
        "\"confidence_permille\":950",
        "\"confidence_permille\":1001",
    );
    for bad in [
        "no json here".to_owned(),
        "{\"Nonsense\": {}}".to_owned(),
        report,
        past_end,
        document_source,
        too_sure,
    ] {
        assert_ne!(bad, good);
        let (client, _) = client_with(
            config(CloudProvider::OpenAiCompatible),
            Ok(KEY.to_owned()),
            openai_reply(&bad),
        );
        assert!(
            matches!(
                client.propose(&text_input(), &CancelToken::new()),
                Err(AssistError::Output { .. })
            ),
            "{bad}"
        );
    }
}

#[test]
fn provider_envelope_without_text_is_rejected() {
    let (client, _) = client_with(
        config(CloudProvider::Anthropic),
        Ok(KEY.to_owned()),
        Ok(CloudResponse {
            status: 200,
            body: "{\"content\": []}".to_owned(),
        }),
    );
    assert!(matches!(
        client.propose(&text_input(), &CancelToken::new()),
        Err(AssistError::Output { .. })
    ));
}

fn propose_with_reply(model_text: &str, input: &ProposalInput) -> Result<Proposal, AssistError> {
    let (client, _) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply(model_text),
    );
    client.propose(input, &CancelToken::new())
}

#[test]
fn a_figure_that_the_cited_text_does_not_state_is_rejected() {
    // Provenance: derived. In "PAN [PAN_1] paid 25000 to Sharma" the characters 17..22 are
    // "25000", i.e. 25000 rupees = 2_500_000 paise (25000 * 100).
    let good = serde_json::to_string(&recorded_draft()).unwrap();
    // The right amount is accepted.
    assert!(propose_with_reply(&good, &text_input()).is_ok());
    // 2_600_000 paise (26000 rupees) cited at "25000": the text does not say 26000.
    let wrong_amount = good.replace("2500000", "2600000");
    assert_ne!(wrong_amount, good);
    assert!(matches!(
        propose_with_reply(&wrong_amount, &text_input()),
        Err(AssistError::Output { .. })
    ));
    // 2_500_000 paise cited at 26..32, which is the word "Sharma", not an amount.
    let amount_on_a_name = good.replace(
        "\"amount\":{\"value\":2500000,\"source\":{\"Typed\":{\"start\":17,\"end\":22}}",
        "\"amount\":{\"value\":2500000,\"source\":{\"Typed\":{\"start\":26,\"end\":32}}",
    );
    assert_ne!(amount_on_a_name, good);
    assert!(matches!(
        propose_with_reply(&amount_on_a_name, &text_input()),
        Err(AssistError::Output { .. })
    ));
}

#[test]
fn a_name_that_the_cited_text_does_not_state_is_rejected() {
    // The text at 26..32 is "Sharma"; the model claims the party is "Verma".
    let good = serde_json::to_string(&recorded_draft()).unwrap();
    let wrong_name = good.replace("\"value\":\"Sharma\"", "\"value\":\"Verma\"");
    assert_ne!(wrong_name, good);
    assert!(matches!(
        propose_with_reply(&wrong_name, &text_input()),
        Err(AssistError::Output { .. })
    ));
}

/// Hand-written input with no identifier in it. Character positions (0-based, end exclusive):
/// `Sharma` 0..6, `sold` 7..11, `2.5` 12..15, `rice` 16..20, `at` 21..23, `40` 24..26,
/// `on` 27..29, `31-03-2026` 30..40, `for` 41..44, `100` 45..48; 48 characters in all.
const ITEM_TEXT: &str = "Sharma sold 2.5 rice at 40 on 31-03-2026 for 100";

fn item_draft() -> Proposal {
    Proposal::VoucherDraft(Box::new(VoucherDraft {
        kind: BaseVoucherKind::Payment,
        date: Some(typed(date!(2026 - 03 - 31), 30, 40, 900)),
        party_name: Some(typed("Sharma".to_owned(), 0, 6, 900)),
        party: None,
        invoice_number: None,
        supplier_gstin: None,
        lines: vec![],
        items: vec![DraftItem {
            name: typed("rice".to_owned(), 16, 20, 900),
            qty: Some(typed(Qty(Decimal::new(25, 1)), 12, 15, 900)),
            rate: Some(typed(Rate(Decimal::from(40)), 24, 26, 900)),
            // 100 rupees = 10_000 paise (100 * 100).
            amount: Some(typed(Paise::new(10_000), 45, 48, 900)),
        }],
        document_total: None,
        document_tax: None,
        create_ledgers: vec![],
        narration: String::new(),
    }))
}

#[test]
fn a_draft_with_a_date_and_decimal_items_round_trips_in_both_date_forms() {
    let input = ProposalInput::Text(ITEM_TEXT.to_owned());
    let proposal = item_draft();

    // Form 1: exactly what serde writes for a `Proposal` (the date is a `[year, ordinal]` array).
    let as_serde = serde_json::to_string(&proposal).unwrap();
    // Form 2: what the system prompt asks a model for (the date is an ISO string).
    let mut value = serde_json::to_value(&proposal).unwrap();
    *value.pointer_mut("/VoucherDraft/date/value").unwrap() = json!("2026-03-31");
    let as_prompted = value.to_string();
    assert_ne!(as_serde, as_prompted);

    for text in [as_serde, as_prompted] {
        let answer = propose_with_reply(&text, &input).unwrap();
        // No identifier in the text, so offsets are unchanged and the draft comes back equal.
        assert_eq!(answer, proposal, "{text}");
    }

    // The date the text does not state is rejected (31 March, not 30 March).
    let mut wrong = serde_json::to_value(&proposal).unwrap();
    *wrong.pointer_mut("/VoucherDraft/date/value").unwrap() = json!("2026-03-30");
    assert!(matches!(
        propose_with_reply(&wrong.to_string(), &input),
        Err(AssistError::Output { .. })
    ));
}

#[test]
fn the_system_prompt_states_the_wire_formats() {
    let (client, spy) = client_with(
        config(CloudProvider::OpenAiCompatible),
        Ok(KEY.to_owned()),
        openai_reply("no json here"),
    );
    let _ = client.propose(&text_input(), &CancelToken::new());
    let sent = spy.sent.lock().unwrap();
    let system = body_of(&sent[0])["messages"][0]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    for needle in [
        "YYYY-MM-DD",
        "paise",
        "JSON string",
        "\"Dr\"",
        "VoucherDraft",
    ] {
        assert!(system.contains(needle), "{needle}");
    }
}

#[test]
fn a_match_suggestion_for_another_statement_row_is_rejected() {
    let input = ProposalInput::BankLine {
        row: 3,
        date: date!(2026 - 03 - 31),
        description: "rent".to_owned(),
        reference: None,
        amount: SignedAmount {
            amount: Paise::new(2_500_000),
            side: DrCr::Dr,
        },
    };
    let suggestion = |row: u32| {
        serde_json::to_string(&Proposal::MatchSuggestion(MatchSuggestion {
            statement_row: row,
            vouchers: vec![],
            ledger_name: None,
            reason: "rent".to_owned(),
            confidence_permille: 800,
        }))
        .unwrap()
    };
    assert!(propose_with_reply(&suggestion(3), &input).is_ok());
    assert!(matches!(
        propose_with_reply(&suggestion(4), &input),
        Err(AssistError::Output { .. })
    ));
}
