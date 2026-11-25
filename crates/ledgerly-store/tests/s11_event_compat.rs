//! Round s11: adding the GSTR-2B and IMS events must not disturb a company file written before
//! them. A record hash is SHA-256 over the canonical CBOR of `(seq, prev_hash, meta, event)`
//! (`ledgerly_store::commit` module doc), and serde writes an enum variant by its name, so a new
//! variant leaves every old record's bytes alone.
//!
//! Provenance: `OLD_*` are the record hashes of two events with the fixed meta below, taken from
//! the tree at commit 90b7817, before any of these events existed (observed: this test was first
//! run there with the constants empty and printed them). They are not produced by the new code.
#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Clock, CompanyId, Date, Gstin, Month, OffsetDateTime, Pan, StateCode, UserId};
use ledgerly_model::{
    Address, Company, EntityKind, Features, GstRegistration, NegativeStockPolicy,
};
use ledgerly_store::commit::record_hash;
use ledgerly_store::verify::verify_chain;
use ledgerly_store::{Actor, AuditMeta, Event, SqliteStore, Store};

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(7),
        name: "Example Traders Pvt Ltd".to_owned(),
        mailing_name: None,
        address: Address {
            lines: vec!["12 Sample Road".to_owned()],
            pincode: Some("560001".to_owned()),
            state: Some(StateCode::new(29).unwrap()),
            country: String::new(),
        },
        state: StateCode::new(29).unwrap(),
        pan: Some(Pan::parse("ABCDE1234F").unwrap()),
        registrations: vec![GstRegistration {
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: StateCode::new(29).unwrap(),
            effective_from: date(2017, Month::July, 1),
            composition: false,
        }],
        fy_start: date(2026, Month::April, 1),
        books_begin: date(2026, Month::April, 1),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Warn,
    }
}

fn meta() -> AuditMeta {
    AuditMeta {
        actor: Actor::User(UserId(1)),
        at: OffsetDateTime::UNIX_EPOCH,
        reason: Some("test".to_owned()),
        app_version: "0.0.0-test".to_owned(),
    }
}

fn setting() -> Event {
    Event::SettingChanged {
        key: "gst.filing".to_owned(),
        before: String::new(),
        after: "monthly".to_owned(),
    }
}

fn lock() -> Event {
    Event::PeriodLocked {
        until: date(2026, Month::October, 31),
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

const OLD_SETTING_HASH: &str = "be3da3c0caabbf0018c63e9e7f62a5e3a75c4d2d0071ee48df4e3ebbbd14bf24";
const OLD_LOCK_HASH: &str = "779b162af7b3d7cd80a4297edc003db65e40a43b19af00e6fa591409a5e6c7b6";

#[test]
fn old_events_keep_their_record_hashes() {
    let prev = [0u8; 32];
    let a = record_hash(ledgerly_core::EventSeq(1), &prev, &meta(), &setting()).unwrap();
    let b = record_hash(ledgerly_core::EventSeq(2), &a, &meta(), &lock()).unwrap();
    assert_eq!(hex(&a), OLD_SETTING_HASH);
    assert_eq!(hex(&b), OLD_LOCK_HASH);
}

#[test]
fn a_file_of_old_events_opens_and_verifies_after_the_new_events_exist() {
    let dir = tempfile::tempdir_in("/media/devansh/Experiment/playground/claude-tmp").unwrap();
    let path = dir.path().join("old.ledgerly");
    {
        let mut store =
            SqliteStore::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH))
                .unwrap();
        store.commit(&[setting()], &meta()).unwrap();
        store.commit(&[lock()], &meta()).unwrap();
    }
    let store = SqliteStore::open(&path).unwrap();
    let report = verify_chain(&store).unwrap();
    assert!(report.is_ok(), "{}", report.certificate());
    assert_eq!(
        store.events(ledgerly_core::EventSeq(1), 10).unwrap().len(),
        2
    );
}

#[test]
fn the_new_events_commit_read_back_and_keep_the_chain_whole() {
    let dir = tempfile::tempdir_in("/media/devansh/Experiment/playground/claude-tmp").unwrap();
    let path = dir.path().join("new.ledgerly");
    let events = vec![
        Event::Gstr2bImported {
            gstin: "29ABCDE1234F1ZW".to_owned(),
            period_start: date(2026, Month::November, 1),
            file_name: "2b.json".to_owned(),
            file_hash: "ab".repeat(32),
            json: "{}".to_owned(),
        },
        Event::ImsActionRecorded {
            gstin: "29ABCDE1234F1ZW".to_owned(),
            period_start: date(2026, Month::November, 1),
            supplier_gstin: "29ZZZZZ0101Z1ZK".to_owned(),
            doc_number: "KP/2231".to_owned(),
            action: ledgerly_store::StoredImsAction::Accept,
        },
        Event::Gstr3bFiled {
            gstin: "29ABCDE1234F1ZW".to_owned(),
            period_start: date(2026, Month::October, 1),
            filed_on: date(2026, Month::November, 20),
        },
    ];
    {
        let mut store =
            SqliteStore::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH))
                .unwrap();
        store.commit(&[setting()], &meta()).unwrap();
        store.commit(&events, &meta()).unwrap();
    }
    let store = SqliteStore::open(&path).unwrap();
    assert!(verify_chain(&store).unwrap().is_ok());
    let back: Vec<Event> = store
        .events(ledgerly_core::EventSeq(1), 10)
        .unwrap()
        .into_iter()
        .map(|r| r.event)
        .collect();
    assert_eq!(back[1..], events[..]);
    // The books still load with the new events in the log.
    assert!(store.load_books(None).is_ok());
}
