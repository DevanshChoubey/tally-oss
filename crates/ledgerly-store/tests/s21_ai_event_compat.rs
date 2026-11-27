//! Round s21: an ask the engine cannot run gets its own AI event kind (`NotRun`); before, it was
//! logged as `ReadFailed` with a reason starting "Unsupported". A company file written before the
//! change must still open: the old entry keeps its bytes and record hash and decodes as the
//! `ReadFailed` it was.
//!
//! Provenance: `OLD_ASK_CBOR` and `OLD_ASK_HASH` are the canonical CBOR of the event below and its
//! record hash with the fixed meta, taken from the tree before the change (observed: this test was
//! first run on branch s21/ai at commit f72dc790, before `NotRun` existed, with the constants
//! empty, and printed them). They are not produced by the new code.
#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{EventSeq, OffsetDateTime, UserId};
use ledgerly_model::{AiActivity, AiActivityKind, AiFeature};
use ledgerly_store::commit::record_hash;
use ledgerly_store::{Actor, AuditMeta, Event};

fn meta() -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at: OffsetDateTime::UNIX_EPOCH,
        reason: Some("test".to_owned()),
        app_version: "0.0.0-test".to_owned(),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

const OLD_ASK_CBOR: &str = "a172416941637469766974795265636f72646564a767666561747572656341736b6870726f76696465726d546573742070726f76696465726d6d6f64656c5f76657273696f6e657332312d317770726f6d70745f74656d706c6174655f76657273696f6e6561736b2f316b696e7075745f6c6162656c7824776861742077696c6c206d792070726f666974206265206e65787420717561727465723f6a696e7075745f68617368784061626162616261626162616261626162616261626162616261626162616261626162616261626162616261626162616261626162616261626162616261626162646b696e64a16a526561644661696c6564a166726561736f6e782f556e737570706f727465643a206e6f207265706f727420657374696d61746573206675747572652066696775726573";
const OLD_ASK_HASH: &str = "05542a00e1609e062e6e6eb7fb920d5adf2b2f1b4d3de3c8d3b0b861bc1ded7d";

/// An unsupported ask as round E logged it.
fn old_ask() -> Event {
    Event::AiActivityRecorded(AiActivity {
        feature: AiFeature::Ask,
        provider: "Test provider".to_owned(),
        model_version: "s21-1".to_owned(),
        prompt_template_version: "ask/1".to_owned(),
        input_label: "what will my profit be next quarter?".to_owned(),
        input_hash: "ab".repeat(32),
        kind: AiActivityKind::ReadFailed {
            reason: "Unsupported: no report estimates future figures".to_owned(),
        },
    })
}

#[test]
fn an_old_unsupported_ask_record_decodes_as_the_read_failure_it_was() {
    let event: Event = ciborium::from_reader(unhex(OLD_ASK_CBOR).as_slice()).unwrap();
    assert_eq!(event, old_ask());
}

#[test]
fn an_old_unsupported_ask_record_keeps_its_bytes_and_record_hash() {
    let mut bytes = Vec::new();
    ciborium::into_writer(&old_ask(), &mut bytes).unwrap();
    let h = record_hash(EventSeq(1), &[0u8; 32], &meta(), &old_ask()).unwrap();
    assert_eq!(hex(&bytes), OLD_ASK_CBOR);
    assert_eq!(hex(&h), OLD_ASK_HASH);
}
