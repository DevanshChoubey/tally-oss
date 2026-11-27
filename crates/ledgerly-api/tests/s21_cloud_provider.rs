//! Round s21: the cloud provider the owner chose (2026-11-27). Two request formats: the Anthropic
//! Messages API with the user's own key, and an OpenAI-compatible chat endpoint given as a base
//! URL plus key. With the Messages format and no model named, bill reading uses
//! `claude-sonnet-5-5` and plain words `claude-haiku-5-5`; the user's own model replaces either.
//! No test here opens a connection: the transport is a recording double, and the key is a
//! fictional string in an in-memory vault.
//!
//! Provenance (derived). The two default model names are the owner's decision of 2026-11-27,
//! relayed by the coordinator. The Messages URL `https://api.anthropic.com/v1/messages` and the
//! `x-api-key` header are Anthropic's documented Messages API; the `/chat/completions` path under
//! a base URL and the `Authorization: Bearer` header are the OpenAI-compatible chat format. The
//! request shapes themselves are pinned by `ledgerly-assist`'s `a4_cloud` tests.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ledgerly_api::{
    AiFeature, AiProvider, Api, CloudRequest, CloudResponse, CloudSettings, CloudTransport,
    KEY_SERVICE, MemoryVault, cloud_model_for, cloud_request_url,
};
use ledgerly_assist::AssistError;
use ledgerly_core::{CancelToken, Clock, CompanyId, Outcome, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::OffsetDateTime;
use time::macros::date;

const KEY: &str = "fictional-key-cloud-s21";
const ACCOUNT: &str = "company-1-Fictional Traders";

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

fn new_file(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

fn messages() -> CloudSettings {
    CloudSettings {
        format: Some("messages".to_owned()),
        ..CloudSettings::default()
    }
}

/// Records each request and answers 401, so nothing past the request is needed.
#[derive(Clone, Default)]
struct Recorder {
    sent: Arc<Mutex<Vec<CloudRequest>>>,
}

impl CloudTransport for Recorder {
    fn post(&self, request: &CloudRequest) -> Result<CloudResponse, AssistError> {
        self.sent.lock().unwrap().push(request.clone());
        Ok(CloudResponse {
            status: 401,
            body: String::new(),
        })
    }
}

#[test]
fn the_messages_format_defaults_to_sonnet_for_bills_and_haiku_for_plain_words() {
    let s = messages();
    assert_eq!(
        cloud_model_for(&s, AiFeature::BillReading).as_deref(),
        Some("claude-sonnet-5-5")
    );
    assert_eq!(
        cloud_model_for(&s, AiFeature::PlainWords).as_deref(),
        Some("claude-haiku-5-5")
    );
    // No default was chosen for the other features: they need a model named.
    assert_eq!(cloud_model_for(&s, AiFeature::Ask), None);
    assert_eq!(
        cloud_request_url(&s).as_deref(),
        Some("https://api.anthropic.com/v1/messages")
    );
}

#[test]
fn a_model_the_user_names_replaces_the_default() {
    let s = CloudSettings {
        bill_model: Some("model-bills".to_owned()),
        words_model: Some("model-words".to_owned()),
        model: Some("model-other".to_owned()),
        ..messages()
    };
    assert_eq!(
        cloud_model_for(&s, AiFeature::BillReading).as_deref(),
        Some("model-bills")
    );
    assert_eq!(
        cloud_model_for(&s, AiFeature::PlainWords).as_deref(),
        Some("model-words")
    );
    assert_eq!(
        cloud_model_for(&s, AiFeature::Ask).as_deref(),
        Some("model-other")
    );
}

#[test]
fn the_openai_compatible_format_takes_a_base_url_and_has_no_default_model() {
    let s = CloudSettings {
        endpoint: Some("https://llm.example.invalid/v1/".to_owned()),
        format: Some("chat-completions".to_owned()),
        ..CloudSettings::default()
    };
    assert_eq!(
        cloud_request_url(&s).as_deref(),
        Some("https://llm.example.invalid/v1/chat/completions")
    );
    assert_eq!(cloud_model_for(&s, AiFeature::PlainWords), None);
    let named = CloudSettings {
        model: Some("model-x".to_owned()),
        ..s
    };
    assert_eq!(
        cloud_model_for(&named, AiFeature::PlainWords).as_deref(),
        Some("model-x")
    );
    // A chat endpoint with no base URL has nowhere to send to.
    assert_eq!(
        cloud_request_url(&CloudSettings {
            format: Some("chat-completions".to_owned()),
            ..CloudSettings::default()
        }),
        None
    );
}

#[test]
fn plain_words_go_to_the_messages_api_with_haiku_and_the_key_from_the_vault() {
    let mut api = new_file("s21-cloud-words.ledgerly");
    api.attach_key_vault(Box::new(MemoryVault::with_key(KEY_SERVICE, ACCOUNT, KEY)));
    assert_eq!(
        api.set_cloud_settings(&messages()).unwrap(),
        Outcome::Value(())
    );
    let recorder = Recorder::default();
    let Outcome::Value(cloud) = api.cloud_assistant(Box::new(recorder.clone())).unwrap() else {
        panic!("a described provider and a saved key make a cloud assistant");
    };
    let refused = cloud.read_text("paid 500 to Ramesh", &[], &CancelToken::new());
    assert!(refused.is_err(), "the recorder answers 401");
    let sent = recorder.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    let request = &sent[0];
    assert_eq!(request.url, "https://api.anthropic.com/v1/messages");
    assert!(
        request
            .headers
            .iter()
            .any(|(n, v)| n == "x-api-key" && v == KEY),
        "the key travels only in its header"
    );
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["model"], "claude-haiku-5-5");
    assert!(!request.body.contains(KEY), "the key is never in the body");
}

#[test]
fn without_a_saved_key_no_cloud_assistant_is_made() {
    let mut api = new_file("s21-cloud-nokey.ledgerly");
    api.attach_key_vault(Box::new(MemoryVault::default()));
    assert_eq!(
        api.set_cloud_settings(&messages()).unwrap(),
        Outcome::Value(())
    );
    let Outcome::Unsupported(u) = api.cloud_assistant(Box::new(Recorder::default())).unwrap()
    else {
        panic!("no key: no cloud assistant");
    };
    assert!(u.reason.contains("key"), "{}", u.reason);
}

#[test]
fn the_cloud_assistant_reads_no_bill_image() {
    let mut api = new_file("s21-cloud-ocr.ledgerly");
    api.attach_key_vault(Box::new(MemoryVault::with_key(KEY_SERVICE, ACCOUNT, KEY)));
    assert_eq!(
        api.set_cloud_settings(&messages()).unwrap(),
        Outcome::Value(())
    );
    let recorder = Recorder::default();
    let Outcome::Value(cloud) = api.cloud_assistant(Box::new(recorder.clone())).unwrap() else {
        panic!("a cloud assistant");
    };
    let Err(AssistError::Unsupported(u)) = cloud.ocr(b"\x89PNG", "00", &CancelToken::new()) else {
        panic!("a bill image is not sent: it cannot be redacted, and no consent screen is built");
    };
    assert!(u.reason.contains("redacted"), "{}", u.reason);
    assert!(recorder.sent.lock().unwrap().is_empty(), "nothing was sent");
}
