//! Round s21, plan round B: AI settings (spec 6.8, 9). The cloud key lives in a key vault (the OS
//! keychain in the app, an in-memory vault here), never in the company file; the cloud tier can
//! be chosen only once a vault is attached, the provider is described and a key is saved. The
//! local model is a file the user chooses; this build runs no model, so it is never ready.
//!
//! Provenance (derived). Every expected value is written here by hand: the setting words are the
//! ones the API documents, the key is a fictional string, and the file size is the byte count
//! written by the test. Nothing is copied from the code under test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_api::{
    AiFeature, AiTier, Api, CloudSettings, KeyState, LocalModelFile, MemoryVault, ProviderStatus,
};
use ledgerly_core::{Clock, CompanyId, Outcome, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::OffsetDateTime;
use time::macros::date;

const KEY: &str = "fictional-key-0000-s21";

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

fn new_file(name: &str) -> (Api, PathBuf) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    (api, path)
}

fn provider() -> CloudSettings {
    CloudSettings {
        endpoint: Some("https://llm.example.invalid/v1".to_owned()),
        format: Some("chat-completions".to_owned()),
        model: Some("model-x".to_owned()),
        bill_model: None,
        words_model: None,
    }
}

#[test]
fn without_a_vault_the_key_state_says_so_and_cloud_stays_refused() {
    let (mut api, _path) = new_file("s21-settings-no-vault.ledgerly");
    let settings = api.ai_settings().unwrap();
    assert_eq!(settings.key, KeyState::NoVault);
    assert_eq!(settings.cloud, CloudSettings::default());
    assert!(matches!(
        api.set_ai_tier(AiTier::Cloud).unwrap(),
        Outcome::Unsupported(_)
    ));
    assert!(matches!(
        api.save_cloud_key(KEY).unwrap(),
        Outcome::Unsupported(_)
    ));
}

#[test]
fn cloud_needs_the_provider_and_a_saved_key() {
    let (mut api, path) = new_file("s21-settings-cloud.ledgerly");
    api.attach_key_vault(Box::new(MemoryVault::default()));
    assert_eq!(api.ai_settings().unwrap().key, KeyState::NotSet);

    let Outcome::Unsupported(u) = api.set_ai_tier(AiTier::Cloud).unwrap() else {
        panic!("no provider and no key: cloud must be refused");
    };
    assert!(u.reason.contains("endpoint"), "{}", u.reason);

    assert_eq!(
        api.set_cloud_settings(&provider()).unwrap(),
        Outcome::Value(())
    );
    let Outcome::Unsupported(u) = api.set_ai_tier(AiTier::Cloud).unwrap() else {
        panic!("no key: cloud must be refused");
    };
    assert!(u.reason.contains("key"), "{}", u.reason);

    assert_eq!(api.save_cloud_key(KEY).unwrap(), Outcome::Value(()));
    let settings = api.ai_settings().unwrap();
    assert_eq!(settings.key, KeyState::Set);
    assert_eq!(settings.cloud, provider());
    assert_eq!(api.set_ai_tier(AiTier::Cloud).unwrap(), Outcome::Value(()));
    assert_eq!(api.ai_tier().unwrap(), Some(AiTier::Cloud));

    assert_eq!(api.delete_cloud_key().unwrap(), Outcome::Value(()));
    assert_eq!(api.ai_settings().unwrap().key, KeyState::NotSet);

    drop(api);
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        !bytes.windows(KEY.len()).any(|w| w == KEY.as_bytes()),
        "the company file must never hold the key"
    );
}

#[test]
fn the_local_model_file_is_stored_and_reported_never_ready() {
    let (mut api, _path) = new_file("s21-settings-local.ledgerly");
    let settings = api.ai_settings().unwrap();
    assert_eq!(settings.model_file, LocalModelFile::NotChosen);
    assert!(matches!(
        settings.local_status,
        ProviderStatus::NotInstalled { .. }
    ));

    let file = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s21-settings-model.gguf");
    std::fs::write(&file, [7_u8; 12]).unwrap();
    assert_eq!(
        api.set_local_model_file(Some(&file)).unwrap(),
        Outcome::Value(())
    );
    let settings = api.ai_settings().unwrap();
    assert_eq!(
        settings.model_file,
        LocalModelFile::Found {
            path: file.clone(),
            bytes: 12
        }
    );
    assert!(matches!(
        settings.local_status,
        ProviderStatus::NotInstalled { .. }
    ));

    std::fs::remove_file(&file).unwrap();
    assert_eq!(
        api.ai_settings().unwrap().model_file,
        LocalModelFile::Missing(file)
    );
    assert_eq!(api.set_local_model_file(None).unwrap(), Outcome::Value(()));
    assert_eq!(
        api.ai_settings().unwrap().model_file,
        LocalModelFile::NotChosen
    );
}

#[test]
fn each_feature_is_on_until_switched_off_and_the_switch_is_audited() {
    let (mut api, _path) = new_file("s21-settings-features.ledgerly");
    for feature in AiFeature::ALL {
        assert!(api.ai_feature_on(feature).unwrap(), "{feature:?} starts on");
    }
    assert_eq!(
        api.set_ai_feature(AiFeature::BankSuggestion, false)
            .unwrap(),
        Outcome::Value(())
    );
    assert!(!api.ai_feature_on(AiFeature::BankSuggestion).unwrap());
    assert!(api.ai_feature_on(AiFeature::BillReading).unwrap());
    assert_eq!(
        api.ai_settings().unwrap().features_off,
        vec![AiFeature::BankSuggestion]
    );
    api.set_ai_feature(AiFeature::BankSuggestion, true).unwrap();
    assert!(api.ai_feature_on(AiFeature::BankSuggestion).unwrap());
    assert_eq!(api.ai_settings().unwrap().features_off, Vec::new());
}

/// The cloud client sends only to `https://` addresses (`ledgerly_assist::cloud`, "it must be
/// `https`"), so a provider described with a plain `http://` endpoint is refused when it is
/// stored, not later when a document is about to be sent. Nothing is stored.
#[test]
fn an_endpoint_that_is_not_https_is_refused_and_nothing_is_stored() {
    let (mut api, _path) = new_file("s21-settings-http.ledgerly");
    let plain = CloudSettings {
        endpoint: Some("http://llm.example.invalid/v1/chat/completions".to_owned()),
        ..provider()
    };
    let Outcome::Unsupported(u) = api.set_cloud_settings(&plain).unwrap() else {
        panic!("an http endpoint must be refused");
    };
    assert!(u.reason.contains("https://"), "{}", u.reason);
    assert_eq!(api.ai_settings().unwrap().cloud, CloudSettings::default());
}

/// The request formats the cloud client speaks are the two `CloudProvider` wire formats, written
/// `chat-completions` and `messages` in the settings (`CloudSettings::format`). Any other word is
/// refused, so a format the client cannot speak is never stored as if it could.
#[test]
fn a_request_format_the_client_does_not_speak_is_refused() {
    let (mut api, _path) = new_file("s21-settings-format.ledgerly");
    let odd = CloudSettings {
        format: Some("completions-v0".to_owned()),
        ..provider()
    };
    let Outcome::Unsupported(u) = api.set_cloud_settings(&odd).unwrap() else {
        panic!("an unknown format must be refused");
    };
    assert!(u.reason.contains("chat-completions"), "{}", u.reason);
    assert!(u.reason.contains("messages"), "{}", u.reason);
    assert_eq!(api.ai_settings().unwrap().cloud, CloudSettings::default());
}

/// An empty key is not a key: saving one is refused and the vault is left as it was.
#[test]
fn an_empty_key_is_not_saved() {
    let (mut api, _path) = new_file("s21-settings-empty-key.ledgerly");
    api.attach_key_vault(Box::new(MemoryVault::default()));
    let Outcome::Unsupported(u) = api.save_cloud_key("   ").unwrap() else {
        panic!("an empty key must be refused");
    };
    assert!(u.reason.contains("empty"), "{}", u.reason);
    assert_eq!(api.ai_settings().unwrap().key, KeyState::NotSet);
}
