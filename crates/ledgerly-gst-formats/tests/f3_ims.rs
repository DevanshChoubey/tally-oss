//! Card F3 tests: IMS reader and action file writer.
//!
//! Provenance. The fixture is fictional and hand-written (GSTINs are the checksummed vectors of
//! `ledgerly-core`). Both the IMS download shape and the action file shape are **pending
//! observed**: no portal file has been captured (see the module doc of `ims.rs`). These tests
//! pin the documented working shapes, the "user confirms every action" rule of spec section 6.3,
//! and determinism. Every expected string is written by hand from the documented shape and the
//! record values in the test, never copied from the writer's output.
//!
//! Every test in this file depends on the recalled IMS key names and action file shape, so every
//! test is `pending` until a capture exists, whatever its arithmetic provenance says.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Gstin, Paise};
use ledgerly_gst_formats::FormatError;
use ledgerly_gst_formats::gstr2b::InwardDocKind;
use ledgerly_gst_formats::ims::{ImsAction, ImsActionRecord, ims_actions_to_json, parse_ims};
use serde_json::json;
use time::{Date, Month};

const SAMPLE: &str = r#"{
  "data": {
    "gstin": "29ABCDE1234F1ZW",
    "rtnprd": "042024",
    "ims_flag": true,
    "docdata": {
      "b2b": [
        {
          "ctin": "24AAACB1111C1ZT",
          "trdnm": "Fictional Traders",
          "inv": [
            { "inum": "INV-001", "dt": "05-04-2024", "val": 1180.00, "action": "A", "imsid": "X1",
              "items": [ { "rt": 18, "txval": 1000.00, "igst": 180.00, "cgst": 0, "sgst": 0, "cess": 0 } ] },
            { "inum": "INV-002", "dt": "06-04-2024", "val": 236.00, "action": "R", "rmrk": "wrong rate" },
            { "inum": "INV-003", "dt": "07-04-2024", "val": 118.00, "action": "N" },
            { "inum": "INV-004", "dt": "08-04-2024", "val": 118.00 }
          ]
        }
      ],
      "cdnr": [
        {
          "ctin": "07AAACL0000A1ZP",
          "nt": [
            { "ntnum": "CN-9", "typ": "C", "dt": "09-04-2024", "val": 59.00, "action": "P",
              "rmrk": "awaiting credit" }
          ]
        }
      ],
      "ecom": []
    }
  }
}"#;

fn gstin(s: &str) -> Gstin {
    Gstin::parse(s).unwrap()
}

fn date(d: u8) -> Date {
    Date::from_calendar_date(2024, Month::April, d).unwrap()
}

fn record(
    supplier: &str,
    kind: InwardDocKind,
    number: &str,
    day: u8,
    action: ImsAction,
    remarks: Option<&str>,
) -> ImsActionRecord {
    ImsActionRecord {
        supplier: gstin(supplier),
        doc_kind: kind,
        number: number.to_owned(),
        date: date(day),
        action,
        remarks: remarks.map(str::to_owned),
        confirmed: true,
    }
}

const SUP_A: &str = "24AAACB1111C1ZT";
const SUP_B: &str = "07AAACL0000A1ZP";

fn three_records() -> Vec<ImsActionRecord> {
    vec![
        record(
            SUP_A,
            InwardDocKind::Invoice,
            "INV-002",
            6,
            ImsAction::Reject,
            Some("wrong rate"),
        ),
        record(
            SUP_A,
            InwardDocKind::Invoice,
            "INV-001",
            5,
            ImsAction::Accept,
            None,
        ),
        record(
            SUP_B,
            InwardDocKind::CreditNote,
            "CN-9",
            9,
            ImsAction::Pending,
            Some("awaiting credit"),
        ),
    ]
}

fn field_name(err: FormatError) -> String {
    match err {
        FormatError::Field { field, .. } => field,
        other @ (FormatError::Json { .. }
        | FormatError::Spreadsheet(_)
        | FormatError::Unsupported { .. }) => {
            panic!("expected FormatError::Field, got {other:?}")
        }
    }
}

// ---- reader ---------------------------------------------------------------------------------

#[test]
fn parses_documents_with_their_actions() {
    let ims = parse_ims(SAMPLE).unwrap();
    // provenance: pending (key names recalled, not observed) - read off the fixture: A accept, R reject, N and absent mean no
    // action, P pending; val 1180.00 rupees = 118000 paise; 236.00 = 23600; 59.00 = 5900.
    assert_eq!(ims.gstin.as_deref(), Some("29ABCDE1234F1ZW"));
    let rp = ims.return_period.unwrap();
    assert_eq!((rp.year, rp.month), (2024, Month::April));
    let docs = &ims.b2b[0].docs;
    assert_eq!(docs.len(), 4);
    assert_eq!(docs[0].doc.number, "INV-001");
    assert_eq!(docs[0].doc.value, Paise::new(118_000));
    assert_eq!(docs[0].action, Some(ImsAction::Accept));
    assert_eq!(docs[0].remarks, None);
    assert_eq!(docs[0].doc.items.len(), 1);
    assert_eq!(docs[1].action, Some(ImsAction::Reject));
    assert_eq!(docs[1].remarks.as_deref(), Some("wrong rate"));
    assert_eq!(docs[1].doc.value, Paise::new(23_600));
    assert_eq!(docs[2].action, None);
    assert_eq!(docs[3].action, None);
    let note = &ims.cdnr[0].docs[0];
    assert_eq!(note.doc.kind, InwardDocKind::CreditNote);
    assert_eq!(note.doc.value, Paise::new(5_900));
    assert_eq!(note.action, Some(ImsAction::Pending));
    assert_eq!(note.remarks.as_deref(), Some("awaiting credit"));
}

#[test]
fn unknown_fields_are_preserved_in_extra() {
    let ims = parse_ims(SAMPLE).unwrap();
    // provenance: pending (key names recalled, not observed) - the fixture's keys the reader has no field for.
    assert_eq!(
        serde_json::Value::Object(ims.extra.clone()),
        json!({ "ims_flag": true })
    );
    assert_eq!(
        serde_json::Value::Object(ims.docdata_extra.clone()),
        json!({ "ecom": [] })
    );
    assert_eq!(
        serde_json::Value::Object(ims.b2b[0].docs[0].doc.extra.clone()),
        json!({ "imsid": "X1" })
    );
    // `action` and `rmrk` are fields of the reader, not leftovers.
    assert!(ims.b2b[0].docs[1].doc.extra.is_empty());
    assert!(ims.root_extra.is_empty());
}

#[test]
fn an_unknown_action_code_is_an_error_with_its_path() {
    let text = SAMPLE.replacen(r#""action": "R""#, r#""action": "Z""#, 1);
    match parse_ims(&text).unwrap_err() {
        FormatError::Json { path, reason } => {
            assert_eq!(path, "$.data.docdata.b2b[0].inv[1].action");
            assert!(reason.contains('Z'), "reason was: {reason}");
        }
        other @ (FormatError::Field { .. }
        | FormatError::Spreadsheet(_)
        | FormatError::Unsupported { .. }) => {
            panic!("expected FormatError::Json, got {other:?}")
        }
    }
}

#[test]
fn malformed_json_and_mistyped_fields_name_the_path() {
    match parse_ims("{\"data\": ").unwrap_err() {
        FormatError::Json { path, .. } => assert_eq!(path, "$"),
        other @ (FormatError::Field { .. }
        | FormatError::Spreadsheet(_)
        | FormatError::Unsupported { .. }) => {
            panic!("expected FormatError::Json, got {other:?}")
        }
    }
    let text = SAMPLE.replacen(r#""val": 236.00"#, r#""val": [236]"#, 1);
    match parse_ims(&text).unwrap_err() {
        FormatError::Json { path, .. } => {
            assert_eq!(path, "$.data.docdata.b2b[0].inv[1].val");
        }
        other @ (FormatError::Field { .. }
        | FormatError::Spreadsheet(_)
        | FormatError::Unsupported { .. }) => {
            panic!("expected FormatError::Json, got {other:?}")
        }
    }
}

// ---- writer ---------------------------------------------------------------------------------

#[test]
fn writes_confirmed_actions_grouped_and_sorted() {
    // provenance: pending (key names recalled, not observed) - the documented action file shape (module doc of `ims.rs`), written by
    // hand: suppliers sorted by GSTIN text ("07..." before "24..." in each section; here each
    // section has one), documents sorted by number (INV-001 before INV-002), dates dd-mm-yyyy,
    // key order inum/ntnum, dt, typ (notes), action, rmrk. Pending observed against the portal.
    let expected = concat!(
        r#"{"b2b":[{"ctin":"24AAACB1111C1ZT","inv":["#,
        r#"{"inum":"INV-001","dt":"05-04-2024","action":"A"},"#,
        r#"{"inum":"INV-002","dt":"06-04-2024","action":"R","rmrk":"wrong rate"}]}],"#,
        r#""cdnr":[{"ctin":"07AAACL0000A1ZP","nt":["#,
        r#"{"ntnum":"CN-9","dt":"09-04-2024","typ":"C","action":"P","rmrk":"awaiting credit"}]}]}"#
    );
    assert_eq!(ims_actions_to_json(&three_records()).unwrap(), expected);
}

#[test]
fn output_does_not_depend_on_the_order_the_actions_were_made_in() {
    let forward = three_records();
    let mut reversed = three_records();
    reversed.reverse();
    assert_eq!(
        ims_actions_to_json(&forward).unwrap(),
        ims_actions_to_json(&reversed).unwrap()
    );
}

#[test]
fn credit_and_debit_notes_with_one_number_are_two_documents() {
    let records = vec![
        record(
            SUP_B,
            InwardDocKind::CreditNote,
            "N-1",
            9,
            ImsAction::Accept,
            None,
        ),
        record(
            SUP_B,
            InwardDocKind::DebitNote,
            "N-1",
            9,
            ImsAction::Reject,
            None,
        ),
    ];
    // provenance: pending (key names recalled, not observed) - C sorts before D in the (number, type) key; typ codes C and D.
    let expected = concat!(
        r#"{"cdnr":[{"ctin":"07AAACL0000A1ZP","nt":["#,
        r#"{"ntnum":"N-1","dt":"09-04-2024","typ":"C","action":"A"},"#,
        r#"{"ntnum":"N-1","dt":"09-04-2024","typ":"D","action":"R"}]}]}"#
    );
    assert_eq!(ims_actions_to_json(&records).unwrap(), expected);
}

#[test]
fn an_unconfirmed_action_is_never_written() {
    let mut records = three_records();
    records[1].confirmed = false;
    // The error names the record, so the UI can point at it.
    assert_eq!(
        field_name(ims_actions_to_json(&records).unwrap_err()),
        "actions[1].confirmed"
    );
}

#[test]
fn an_empty_list_is_an_error_not_an_empty_file() {
    assert_eq!(field_name(ims_actions_to_json(&[]).unwrap_err()), "actions");
}

#[test]
fn remarks_only_with_reject_or_pending_and_never_empty() {
    let accept_with_remarks = vec![record(
        SUP_A,
        InwardDocKind::Invoice,
        "INV-1",
        1,
        ImsAction::Accept,
        Some("fine"),
    )];
    assert_eq!(
        field_name(ims_actions_to_json(&accept_with_remarks).unwrap_err()),
        "actions[0].remarks"
    );
    let blank = vec![record(
        SUP_A,
        InwardDocKind::Invoice,
        "INV-1",
        1,
        ImsAction::Reject,
        Some("  "),
    )];
    assert_eq!(
        field_name(ims_actions_to_json(&blank).unwrap_err()),
        "actions[0].remarks"
    );
}

#[test]
fn duplicate_documents_and_empty_numbers_are_refused() {
    let dup = vec![
        record(
            SUP_A,
            InwardDocKind::Invoice,
            "INV-1",
            1,
            ImsAction::Accept,
            None,
        ),
        record(
            SUP_A,
            InwardDocKind::Invoice,
            "INV-1",
            1,
            ImsAction::Accept,
            None,
        ),
    ];
    assert_eq!(
        field_name(ims_actions_to_json(&dup).unwrap_err()),
        "actions[1].number"
    );
    let empty = vec![record(
        SUP_A,
        InwardDocKind::Invoice,
        " ",
        1,
        ImsAction::Accept,
        None,
    )];
    assert_eq!(
        field_name(ims_actions_to_json(&empty).unwrap_err()),
        "actions[0].number"
    );
}

#[test]
fn the_same_number_from_two_suppliers_is_not_a_duplicate() {
    let records = vec![
        record(
            SUP_A,
            InwardDocKind::Invoice,
            "1",
            1,
            ImsAction::Accept,
            None,
        ),
        record(
            SUP_B,
            InwardDocKind::Invoice,
            "1",
            1,
            ImsAction::Accept,
            None,
        ),
    ];
    // provenance: pending (key names recalled, not observed) - suppliers sorted by GSTIN text: "07..." then "24...".
    let expected = concat!(
        r#"{"b2b":[{"ctin":"07AAACL0000A1ZP","inv":[{"inum":"1","dt":"01-04-2024","action":"A"}]},"#,
        r#"{"ctin":"24AAACB1111C1ZT","inv":[{"inum":"1","dt":"01-04-2024","action":"A"}]}]}"#
    );
    assert_eq!(ims_actions_to_json(&records).unwrap(), expected);
}

#[test]
fn action_codes_are_the_single_letters_of_the_download() {
    assert_eq!(ImsAction::Accept.code(), "A");
    assert_eq!(ImsAction::Reject.code(), "R");
    assert_eq!(ImsAction::Pending.code(), "P");
}
