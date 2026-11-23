//! Card W4 tests: the AI tier and the AI activity log in the company file (spec 6.8, rule A10).
//!
//! Provenance. The tier is stored as a `SettingChanged` event (`ai.tier`), so the expected
//! `before` and `after` values below are `derived`: the empty string stands for "no tier yet",
//! and the words are the ones the tier is documented with in `ai::AiTier::stored`. The CSV bytes
//! are `derived` by hand from RFC 4180 quoting: a field with a comma or a quote is wrapped in
//! quotes and each quote inside it is doubled. None is copied from the code under test.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};

use ledgerly_api::{AiLogEntry, AiTier, Api, ApiError};
use ledgerly_core::{Clock, CompanyId, EventSeq, Outcome, StateCode, UserId};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy, ProposalRecord};
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store};
use time::OffsetDateTime;
use time::macros::date;

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

/// A new company file with no tier stored. The file name is unique per test.
fn new_file(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    drop(api);
    path
}

/// The `SettingChanged` events of the company file, as `(key, before, after)`.
fn settings(path: &Path) -> Vec<(String, String, String)> {
    let store = SqliteStore::open(path).unwrap();
    store
        .events(EventSeq(1), 1000)
        .unwrap()
        .into_iter()
        .filter_map(|record| {
            if let Event::SettingChanged { key, before, after } = record.event {
                Some((key, before, after))
            } else {
                None
            }
        })
        .collect()
}

fn meta(at: OffsetDateTime) -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at,
        reason: None,
        app_version: "test".to_owned(),
    }
}

fn proposal(output_json: &str, accepted: bool) -> ProposalRecord {
    ProposalRecord {
        model: "local-small".to_owned(),
        model_version: "1".to_owned(),
        prompt_template_version: "cloud-v1".to_owned(),
        input_hash: "ab12".to_owned(),
        output_json: output_json.to_owned(),
        human_edits_json: None,
        accepted,
    }
}

#[test]
fn a_new_company_has_no_tier_and_an_empty_log() {
    let path = new_file("w4_new_company.ledgerly");
    let api = Api::open(&path).unwrap();
    assert_eq!(api.ai_tier().unwrap(), None);
    assert_eq!(api.ai_log().unwrap(), Vec::<AiLogEntry>::new());
}

#[test]
fn a_chosen_tier_is_kept_in_the_file_and_read_back_after_reopen() {
    let path = new_file("w4_tier_persists.ledgerly");
    {
        let mut api = Api::open(&path).unwrap();
        assert_eq!(api.set_ai_tier(AiTier::Local).unwrap(), Outcome::Value(()));
    }
    // Reopen: the tier comes from the file, not from memory.
    let api = Api::open(&path).unwrap();
    assert_eq!(api.ai_tier().unwrap(), Some(AiTier::Local));
    // One audited setting change: no tier before, "local" after.
    assert_eq!(
        settings(&path),
        vec![("ai.tier".to_owned(), String::new(), "local".to_owned())]
    );
}

#[test]
fn switching_tier_records_the_value_it_replaced() {
    let path = new_file("w4_tier_switch.ledgerly");
    let mut api = Api::open(&path).unwrap();
    api.set_ai_tier(AiTier::Local).unwrap();
    api.set_ai_tier(AiTier::Off).unwrap();
    assert_eq!(api.ai_tier().unwrap(), Some(AiTier::Off));
    assert_eq!(
        settings(&path),
        vec![
            ("ai.tier".to_owned(), String::new(), "local".to_owned()),
            ("ai.tier".to_owned(), "local".to_owned(), "off".to_owned()),
        ]
    );
}

#[test]
fn choosing_the_tier_already_stored_writes_nothing() {
    let path = new_file("w4_tier_same.ledgerly");
    let mut api = Api::open(&path).unwrap();
    api.set_ai_tier(AiTier::Off).unwrap();
    assert_eq!(api.set_ai_tier(AiTier::Off).unwrap(), Outcome::Value(()));
    assert_eq!(settings(&path).len(), 1);
}

#[test]
fn cloud_is_refused_because_no_key_store_is_wired_and_nothing_is_written() {
    let path = new_file("w4_tier_cloud.ledgerly");
    let mut api = Api::open(&path).unwrap();
    let out = api.set_ai_tier(AiTier::Cloud).unwrap();
    let Outcome::Unsupported(u) = out else {
        panic!("cloud must be unsupported until a key store exists, got {out:?}");
    };
    assert_eq!(u.what, "AI cloud tier");
    assert!(u.reason.contains("keychain"), "reason: {}", u.reason);
    assert_eq!(api.ai_tier().unwrap(), None);
    assert_eq!(settings(&path).len(), 0);
}

#[test]
fn an_unknown_stored_tier_is_an_error_not_a_default() {
    let path = new_file("w4_tier_unknown.ledgerly");
    {
        let mut store = SqliteStore::open(&path).unwrap();
        store
            .commit(
                &[Event::SettingChanged {
                    key: "ai.tier".to_owned(),
                    before: String::new(),
                    after: "cloud-v9".to_owned(),
                }],
                &meta(OffsetDateTime::UNIX_EPOCH),
            )
            .unwrap();
    }
    let api = Api::open(&path).unwrap();
    match api.ai_tier() {
        Err(ApiError::UnknownAiTier { value }) => assert_eq!(value, "cloud-v9"),
        other => panic!("expected UnknownAiTier, got {other:?}"),
    }
}

#[test]
fn the_log_lists_recorded_proposals_in_sequence_order() {
    let path = new_file("w4_log.ledgerly");
    {
        let mut store = SqliteStore::open(&path).unwrap();
        store
            .commit(
                &[Event::AiProposalRecorded(proposal("{\"a\":1}", true))],
                &meta(OffsetDateTime::UNIX_EPOCH),
            )
            .unwrap();
        store
            .commit(
                &[Event::AiProposalRecorded(proposal("{\"b\":2}", false))],
                &meta(OffsetDateTime::UNIX_EPOCH),
            )
            .unwrap();
    }
    let api = Api::open(&path).unwrap();
    let log = api.ai_log().unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].output_json, "{\"a\":1}");
    assert!(log[0].accepted);
    assert_eq!(log[1].output_json, "{\"b\":2}");
    assert!(!log[1].accepted);
    assert_eq!(log[0].model, "local-small");
    assert_eq!(log[0].input_hash, "ab12");
}

#[test]
fn export_refuses_an_empty_log_and_writes_no_file() {
    let path = new_file("w4_export_empty.ledgerly");
    let api = Api::open(&path).unwrap();
    let out_path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("w4_export_empty.csv");
    let _removed = std::fs::remove_file(&out_path);
    let out = api.export_ai_log(&out_path).unwrap();
    assert!(matches!(out, Outcome::Unsupported(_)), "got {out:?}");
    assert!(!out_path.exists());
}

#[test]
fn export_writes_the_log_as_csv_with_quoted_fields() {
    let path = new_file("w4_export.ledgerly");
    {
        let mut store = SqliteStore::open(&path).unwrap();
        store
            .commit(
                &[Event::AiProposalRecorded(proposal("{\"a\":\"x\"}", true))],
                &meta(OffsetDateTime::UNIX_EPOCH),
            )
            .unwrap();
    }
    let api = Api::open(&path).unwrap();
    let out_path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("w4_export.csv");
    let _removed = std::fs::remove_file(&out_path);
    assert_eq!(api.export_ai_log(&out_path).unwrap(), Outcome::Value(()));
    let text = std::fs::read_to_string(&out_path).unwrap();
    // Header, then the one row. The output JSON has a quote, so it is wrapped and doubled.
    assert_eq!(
        text,
        "When,Model,Input hash,Output,Your decision\n\
         1970-01-01 00:00,local-small,ab12,\"{\"\"a\"\":\"\"x\"\"}\",Accepted\n"
    );
}

#[test]
fn the_key_never_reaches_the_company_file() {
    // The app never has a key to store (cloud is refused above). The file must hold no secret
    // string after every tier operation, including a refused cloud choice.
    let path = new_file("w4_no_key.ledgerly");
    let mut api = Api::open(&path).unwrap();
    api.set_ai_tier(AiTier::Local).unwrap();
    api.set_ai_tier(AiTier::Cloud).unwrap();
    api.set_ai_tier(AiTier::Off).unwrap();
    drop(api);
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.windows(3).any(|w| w == b"sk-"));
}
