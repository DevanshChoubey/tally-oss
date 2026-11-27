//! Round s22: the predefined Profit & Loss A/c ledger has no group (it sits under Primary), so a
//! ledger's group became optional. A company file written before that change must still open:
//! its ledger records keep their bytes and record hashes, and decode with the group they had.
//!
//! Provenance: `OLD_LEDGER_CBOR` and `OLD_LEDGER_HASH` are the canonical CBOR of the event below
//! and its record hash with the fixed meta, taken from the tree before the change (observed: this
//! test was first run there, on branch s22/import-fixes after commit f4367374, with the constants
//! empty, and printed them). They are not produced by the new code.
#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{EventSeq, GroupId, LedgerId, OffsetDateTime, Paise, SignedAmount, UserId};
use ledgerly_model::{Ledger, MasterSnapshot};
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

const OLD_LEDGER_CBOR: &str = "a16d4d617374657243726561746564a1664c6564676572ad6269641828646e616d656452656e7467616c6961736573806567726f757008676f70656e696e67a266616d6f756e741a000249f064736964656244726962696c6c5f77697365f677636f73745f63656e747265735f6170706c696361626c65f4657061727479f66462616e6bf663746178f663677374f6687265736572766564f66574616c6c79f6";
const OLD_LEDGER_HASH: &str = "3ecf3dd6e3b12590eabb4d1d9d943615296b0bf350c0afc6af51d56075e9385c";

fn rent() -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(40),
        name: "Rent".to_owned(),
        aliases: vec![],
        group: Some(GroupId(8)),
        opening: SignedAmount::dr(Paise::new(150_000)),
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

#[test]
fn an_old_ledger_record_decodes_with_its_group() {
    let event: Event = ciborium::from_reader(unhex(OLD_LEDGER_CBOR).as_slice()).unwrap();
    assert_eq!(event, Event::MasterCreated(MasterSnapshot::Ledger(rent())));
}

#[test]
fn an_old_ledger_record_keeps_its_bytes_and_record_hash() {
    let event = Event::MasterCreated(MasterSnapshot::Ledger(rent()));
    let mut bytes = Vec::new();
    ciborium::into_writer(&event, &mut bytes).unwrap();
    assert_eq!(hex(&bytes), OLD_LEDGER_CBOR);
    let h = record_hash(EventSeq(1), &[0u8; 32], &meta(), &event).unwrap();
    assert_eq!(hex(&h), OLD_LEDGER_HASH);
}
