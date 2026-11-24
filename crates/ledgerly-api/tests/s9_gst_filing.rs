//! Round S9 tests: the GST filing frequency is stored in the company file.
//!
//! Provenance. The frequency is a `SettingChanged` event under `gst.filing`, like the AI tier.
//! Expected values are `derived`: the empty string means "none chosen yet" (the same convention
//! the AI tier uses), the words are the ones `GstFiling::stored` documents, and the two
//! frequencies are the two a regular taxpayer can have under the GST law: a return every month,
//! or every quarter under the quarterly scheme for small taxpayers (CBIC, QRMP).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_api::{Api, GstFiling};
use ledgerly_core::{Clock, CompanyId, EventSeq, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use ledgerly_store::{Event, SqliteStore, Store};
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

fn new_file(name: &str) -> (PathBuf, Api) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    (path, api)
}

#[test]
fn a_new_company_has_no_filing_frequency_chosen() {
    let (_path, api) = new_file("s9_filing_none.ledgerly");
    // provenance: derived; nothing has been stored, so there is no answer, not a default.
    assert_eq!(api.gst_filing().unwrap(), None);
}

#[test]
fn the_frequency_chosen_is_the_one_read_back_after_reopening() {
    let (path, mut api) = new_file("s9_filing_roundtrip.ledgerly");
    api.set_gst_filing(GstFiling::Quarterly).unwrap();
    drop(api);
    let api = Api::open(&path).unwrap();
    // provenance: derived; quarterly was stored, so quarterly is read.
    assert_eq!(api.gst_filing().unwrap(), Some(GstFiling::Quarterly));
}

#[test]
fn a_change_is_one_audited_event_and_the_same_choice_writes_nothing() {
    let (path, mut api) = new_file("s9_filing_events.ledgerly");
    api.set_gst_filing(GstFiling::Monthly).unwrap();
    api.set_gst_filing(GstFiling::Monthly).unwrap();
    api.set_gst_filing(GstFiling::Quarterly).unwrap();
    drop(api);
    let store = SqliteStore::open(&path).unwrap();
    let changes: Vec<(String, String, String)> = store
        .events(EventSeq(1), 1000)
        .unwrap()
        .into_iter()
        .filter_map(|record| {
            if let Event::SettingChanged { key, before, after } = record.event {
                (key == "gst.filing").then_some((key, before, after))
            } else {
                None
            }
        })
        .collect();
    // provenance: derived; two real changes (none to monthly, monthly to quarterly), the repeat
    // of monthly is not a change.
    assert_eq!(
        changes,
        vec![
            ("gst.filing".to_owned(), String::new(), "monthly".to_owned()),
            (
                "gst.filing".to_owned(),
                "monthly".to_owned(),
                "quarterly".to_owned()
            ),
        ]
    );
}
