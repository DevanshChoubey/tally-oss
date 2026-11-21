//! Card G7: the `ineligible_itc` setting on ledgers and stock items persists through the CBOR
//! blobs. A set value survives a round trip, and a blob encoded before the field existed (the key
//! absent) decodes as `None`, so old data keeps its meaning: nothing set.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ciborium::Value;
use ledgerly_core::{GroupId, LedgerId, Paise, SignedAmount, StockItemId, UnitId};
use ledgerly_model::{Ledger, StockItem, ValuationMethod};
use serde::Serialize;
use serde::de::DeserializeOwned;

const KEY: &str = "ineligible_itc";

fn ledger(ineligible: Option<bool>) -> Ledger {
    Ledger {
        id: LedgerId(7),
        name: "Motor car expense".to_owned(),
        aliases: vec![],
        group: GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        ineligible_itc: ineligible,
        tally: None,
    }
}

fn stock_item(ineligible: Option<bool>) -> StockItem {
    StockItem {
        id: StockItemId(3),
        name: "Diesel".to_owned(),
        aliases: vec![],
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: None,
        gst_history: vec![],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: vec![],
        standard_costs: vec![],
        ineligible_itc: ineligible,
        tally: None,
    }
}

fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(value, &mut out).expect("CBOR encoding of a model value");
    out
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> T {
    ciborium::from_reader(bytes).expect("CBOR decoding of a model value")
}

/// The same encoding with the `ineligible_itc` key removed: what a blob written before the field
/// existed looks like.
fn without_key(bytes: &[u8]) -> Vec<u8> {
    let mut value: Value = decode(bytes);
    let Value::Map(entries) = &mut value else {
        panic!("a struct encodes as a CBOR map");
    };
    let before = entries.len();
    entries.retain(|(k, _)| k.as_text() != Some(KEY));
    assert_eq!(
        entries.len() + 1,
        before,
        "the field was encoded under `{KEY}`"
    );
    encode(&value)
}

#[test]
fn a_set_ledger_setting_survives_the_round_trip() {
    for set in [true, false] {
        let bytes = encode(&ledger(Some(set)));
        let back: Ledger = decode(&bytes);
        assert_eq!(back.ineligible_itc, Some(set));
    }
}

#[test]
fn a_ledger_blob_without_the_field_decodes_as_none() {
    let old = without_key(&encode(&ledger(Some(true))));
    let back: Ledger = decode(&old);
    assert_eq!(back.ineligible_itc, None);
}

#[test]
fn a_set_stock_item_setting_survives_the_round_trip() {
    for set in [true, false] {
        let bytes = encode(&stock_item(Some(set)));
        let back: StockItem = decode(&bytes);
        assert_eq!(back.ineligible_itc, Some(set));
    }
}

#[test]
fn a_stock_item_blob_without_the_field_decodes_as_none() {
    let old = without_key(&encode(&stock_item(Some(true))));
    let back: StockItem = decode(&old);
    assert_eq!(back.ineligible_itc, None);
}
