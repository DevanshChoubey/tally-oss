//! Card F3 tests: GSTR-2B reader.
//!
//! Provenance. The fixture is fictional and hand-written (rule E7): GSTINs are the checksummed
//! vectors of `ledgerly-core` (`29ABCDE1234F1ZW` recipient, `24AAACB1111C1ZT` and
//! `07AAACL0000A1ZP` suppliers). Its **shape** is recalled from the portal download and is
//! pending observed (no portal file has been captured yet; see the module doc of
//! `gstr2b.rs`), so these tests prove the reader against the documented shape, not against the
//! portal. Every expected value below is read off the fixture text with the conversion written
//! beside it: rupees times 100 is paise; no value comes from running the reader.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Outcome, Paise};
use ledgerly_gst_formats::FormatError;
use ledgerly_gst_formats::gstr2b::{InwardDocKind, ItcAvailability, TaxAmounts, parse_gstr2b};
use serde_json::json;
use time::{Date, Month};

const SAMPLE: &str = r#"{
  "status": 1,
  "data": {
    "gstin": "29ABCDE1234F1ZW",
    "rtnprd": "042024",
    "version": "1.0",
    "gendt": "14-05-2024",
    "chksum": "fictional-checksum",
    "itcsumm": { "note": "summary block the reader does not interpret" },
    "docdata": {
      "b2b": [
        {
          "ctin": "24AAACB1111C1ZT",
          "trdnm": "Fictional Traders",
          "supprd": "042024",
          "supfildt": "11-05-2024",
          "supplier_flag": 7,
          "inv": [
            {
              "inum": "INV-001", "typ": "R", "dt": "05-04-2024", "val": 1180.00,
              "pos": "29", "rev": "N", "itcavl": "Y", "diffprcnt": 1, "irn": "IRN-FICT-1",
              "srctyp": "e-Invoice",
              "items": [
                { "num": 1, "rt": 18, "txval": 1000.00, "igst": 180.00, "cgst": 0, "sgst": 0,
                  "cess": 0, "hsn": "9983" }
              ]
            },
            {
              "inum": "INV/002", "dt": "20-04-2024", "val": 2360.00, "pos": "29", "rev": "Y",
              "itcavl": "N", "rsn": "C", "diffprcnt": 0.65,
              "items": [
                { "num": 1, "rt": 18, "txval": 1000.00, "igst": 0, "cgst": 90.00, "sgst": 90.00, "cess": 0 },
                { "num": 2, "rt": 18, "txval": 1000.00, "igst": 0, "cgst": 90.00, "sgst": 90.00, "cess": 0 }
              ]
            },
            {
              "inum": "INV-003", "dt": "28-04-2024", "val": 118.50, "pos": null,
              "items": [
                { "rt": 18, "txval": 100.42, "igst": 18.08, "cgst": 0, "sgst": 0, "cess": 0 }
              ]
            }
          ]
        }
      ],
      "cdnr": [
        {
          "ctin": "07AAACL0000A1ZP",
          "nt": [
            {
              "ntnum": "CN-1", "typ": "C", "dt": "10-04-2024", "val": 590.00,
              "items": [
                { "rt": 18, "txval": 500.00, "igst": 90.00, "cgst": 0, "sgst": 0, "cess": 0 }
              ]
            },
            {
              "ntnum": "DN-1", "typ": "D", "dt": "11-04-2024", "val": 118.00,
              "items": [
                { "rt": 18, "txval": 100.00, "igst": 18.00, "cgst": 0, "sgst": 0, "cess": 0 }
              ]
            }
          ]
        }
      ],
      "b2ba": [ { "ctin": "24AAACB1111C1ZT", "inv": [] } ]
    }
  }
}"#;

fn paise(p: i64) -> Paise {
    Paise::new(p)
}

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

fn json_path(err: FormatError) -> String {
    match err {
        FormatError::Json { path, .. } => path,
        other @ (FormatError::Field { .. } | FormatError::Spreadsheet(_)) => {
            panic!("expected FormatError::Json, got {other:?}")
        }
    }
}

/// Replaces `from` with `to` once in the sample, failing the test if `from` is not there.
fn mutate(from: &str, to: &str) -> String {
    assert!(SAMPLE.contains(from), "fixture does not contain {from}");
    SAMPLE.replacen(from, to, 1)
}

#[test]
fn parses_header_and_supplier_rows() {
    let r = parse_gstr2b(SAMPLE).unwrap();
    // provenance: derived - read off the fixture header: gstin, rtnprd "042024" = April 2024,
    // gendt "14-05-2024" = 14 May 2024.
    assert_eq!(r.gstin, "29ABCDE1234F1ZW");
    assert_eq!(r.return_period.year, 2024);
    assert_eq!(r.return_period.month, Month::April);
    assert_eq!(r.version.as_deref(), Some("1.0"));
    assert_eq!(r.generated_on, Some(date(2024, Month::May, 14)));
    assert_eq!(r.checksum.as_deref(), Some("fictional-checksum"));
    assert_eq!(r.b2b.len(), 1);
    assert_eq!(r.cdnr.len(), 1);
    let s = &r.b2b[0];
    assert_eq!(s.ctin, "24AAACB1111C1ZT");
    assert_eq!(s.trade_name.as_deref(), Some("Fictional Traders"));
    assert_eq!(s.supplier_period.as_deref(), Some("042024"));
    assert_eq!(s.supplier_filing_date.as_deref(), Some("11-05-2024"));
    assert_eq!(s.docs.len(), 3);
}

#[test]
fn parses_invoice_fields_and_amounts_as_paise() {
    let r = parse_gstr2b(SAMPLE).unwrap();
    let inv = &r.b2b[0].docs[0];
    // provenance: derived - fixture INV-001: val 1180.00 rupees = 118000 paise; item txval
    // 1000.00 = 100000, igst 180.00 = 18000, zero heads = 0.
    assert_eq!(inv.kind, InwardDocKind::Invoice);
    assert_eq!(inv.number, "INV-001");
    assert_eq!(inv.date, date(2024, Month::April, 5));
    assert_eq!(inv.value, paise(118_000));
    assert_eq!(inv.invoice_type.as_deref(), Some("R"));
    assert_eq!(inv.place_of_supply.as_deref(), Some("29"));
    assert_eq!(inv.reverse_charge, Some(false));
    assert_eq!(inv.itc_available, Some(ItcAvailability::Available));
    assert_eq!(inv.irn.as_deref(), Some("IRN-FICT-1"));
    assert_eq!(
        inv.diff_percent.map(|d| d.to_string()).as_deref(),
        Some("1")
    );
    assert_eq!(inv.items.len(), 1);
    let item = &inv.items[0];
    assert_eq!(item.num, Some(1));
    assert_eq!(item.rate.map(|d| d.to_string()).as_deref(), Some("18"));
    assert_eq!(
        item.amounts,
        TaxAmounts {
            taxable: paise(100_000),
            igst: paise(18_000),
            cgst: Paise::ZERO,
            sgst: Paise::ZERO,
            cess: Paise::ZERO,
        }
    );
}

#[test]
fn second_invoice_flags_rcm_itc_not_available_and_sums_two_items() {
    let r = parse_gstr2b(SAMPLE).unwrap();
    let inv = &r.b2b[0].docs[1];
    assert_eq!(inv.number, "INV/002");
    assert_eq!(inv.reverse_charge, Some(true));
    assert_eq!(inv.itc_available, Some(ItcAvailability::Unavailable));
    assert_eq!(inv.reason.as_deref(), Some("C"));
    assert_eq!(
        inv.diff_percent.map(|d| d.to_string()).as_deref(),
        Some("0.65")
    );
    // provenance: derived - two items, each txval 1000.00 (100000 paise), cgst 90.00 (9000),
    // sgst 90.00 (9000): taxable 200000, cgst 18000, sgst 18000, igst 0, cess 0;
    // total tax 18000 + 18000 = 36000; the document value 2360.00 = 200000 + 36000 = 236000.
    let Outcome::Value(total) = inv.tax_totals().unwrap() else {
        panic!("expected a value");
    };
    assert_eq!(total.taxable, paise(200_000));
    assert_eq!(total.cgst, paise(18_000));
    assert_eq!(total.sgst, paise(18_000));
    assert_eq!(total.igst, Paise::ZERO);
    assert_eq!(total.checked_total_tax().unwrap(), paise(36_000));
    assert_eq!(inv.value, paise(236_000));
}

#[test]
fn paise_amounts_are_exact_and_null_means_absent() {
    let r = parse_gstr2b(SAMPLE).unwrap();
    let inv = &r.b2b[0].docs[2];
    // provenance: derived - fixture INV-003: val 118.50 = 11850 paise; txval 100.42 = 10042;
    // igst 18.08 = 1808; 10042 + 1808 = 11850. "pos": null reads as absent.
    assert_eq!(inv.value, paise(11_850));
    assert_eq!(inv.items[0].amounts.taxable, paise(10_042));
    assert_eq!(inv.items[0].amounts.igst, paise(1_808));
    assert_eq!(inv.place_of_supply, None);
    assert_eq!(inv.items[0].num, None);
}

#[test]
fn credit_and_debit_notes_are_told_apart() {
    let r = parse_gstr2b(SAMPLE).unwrap();
    let notes = &r.cdnr[0].docs;
    // provenance: derived - fixture: CN-1 typ C val 590.00 = 59000 paise (500.00 + 90.00 igst);
    // DN-1 typ D val 118.00 = 11800 paise.
    assert_eq!(notes[0].kind, InwardDocKind::CreditNote);
    assert_eq!(notes[0].number, "CN-1");
    assert_eq!(notes[0].value, paise(59_000));
    assert_eq!(notes[0].date, date(2024, Month::April, 10));
    assert_eq!(notes[1].kind, InwardDocKind::DebitNote);
    assert_eq!(notes[1].value, paise(11_800));
    assert_eq!(r.cdnr[0].trade_name, None);
}

#[test]
fn unknown_fields_are_preserved_in_extra_at_every_level() {
    let r = parse_gstr2b(SAMPLE).unwrap();
    // provenance: derived - the fixture's keys the reader has no field for, as written there.
    assert_eq!(
        serde_json::Value::Object(r.root_extra.clone()),
        json!({ "status": 1 })
    );
    assert_eq!(
        serde_json::Value::Object(r.extra.clone()),
        json!({ "itcsumm": { "note": "summary block the reader does not interpret" } })
    );
    assert_eq!(
        serde_json::Value::Object(r.docdata_extra.clone()),
        json!({ "b2ba": [ { "ctin": "24AAACB1111C1ZT", "inv": [] } ] })
    );
    assert_eq!(
        serde_json::Value::Object(r.b2b[0].extra.clone()),
        json!({ "supplier_flag": 7 })
    );
    assert_eq!(
        serde_json::Value::Object(r.b2b[0].docs[0].extra.clone()),
        json!({ "srctyp": "e-Invoice" })
    );
    assert_eq!(
        serde_json::Value::Object(r.b2b[0].docs[0].items[0].extra.clone()),
        json!({ "hsn": "9983" })
    );
    // Known fields do not leak into extra.
    assert!(r.b2b[0].docs[1].extra.is_empty());
    assert!(r.cdnr[0].docs[0].extra.is_empty());
}

#[test]
fn unknown_itc_code_is_kept_as_written() {
    let text = mutate(r#""itcavl": "Y""#, r#""itcavl": "T""#);
    let r = parse_gstr2b(&text).unwrap();
    assert_eq!(
        r.b2b[0].docs[0].itc_available,
        Some(ItcAvailability::Other("T".to_owned()))
    );
}

#[test]
fn document_without_items_has_no_tax_split_and_is_unsupported() {
    let text = r#"{"data":{"gstin":"29ABCDE1234F1ZW","rtnprd":"042024","docdata":{"b2b":[
        {"ctin":"24AAACB1111C1ZT","inv":[{"inum":"X1","dt":"01-04-2024","val":10.00}]}]}}}"#;
    let r = parse_gstr2b(text).unwrap();
    let doc = &r.b2b[0].docs[0];
    assert!(doc.items.is_empty());
    // A zero here would be a guess: the file carries no tax heads.
    assert!(doc.tax_totals().unwrap().is_unsupported());
    // provenance: derived - "val": 10.00 rupees = 1000 paise.
    assert_eq!(doc.value, paise(1_000));
}

#[test]
fn absent_sections_are_empty_not_errors() {
    let text = r#"{"data":{"gstin":"29ABCDE1234F1ZW","rtnprd":"042024","docdata":{}}}"#;
    let r = parse_gstr2b(text).unwrap();
    assert!(r.b2b.is_empty());
    assert!(r.cdnr.is_empty());
    assert_eq!(r.version, None);
    assert_eq!(r.generated_on, None);
}

#[test]
fn parsing_is_deterministic() {
    assert_eq!(parse_gstr2b(SAMPLE).unwrap(), parse_gstr2b(SAMPLE).unwrap());
}

// ---- malformed input: an error naming the JSON path ---------------------------------------

#[test]
fn text_that_is_not_json_is_an_error_at_the_root_with_line_and_column() {
    let text = &SAMPLE[..SAMPLE.len() - 10];
    let err = parse_gstr2b(text).unwrap_err();
    match err {
        FormatError::Json { path, reason } => {
            assert_eq!(path, "$");
            assert!(reason.contains("line"), "reason was: {reason}");
            assert!(reason.contains("column"), "reason was: {reason}");
        }
        other @ (FormatError::Field { .. } | FormatError::Spreadsheet(_)) => {
            panic!("expected FormatError::Json, got {other:?}")
        }
    }
    assert_eq!(json_path(parse_gstr2b("").unwrap_err()), "$");
}

#[test]
fn a_string_where_an_amount_belongs_names_the_path() {
    let text = mutate(r#""val": 2360.00"#, r#""val": "2360.00""#);
    let err = parse_gstr2b(&text).unwrap_err();
    assert_eq!(json_path(err), "$.data.docdata.b2b[0].inv[1].val");
}

#[test]
fn a_missing_required_field_names_the_path() {
    let text = mutate(r#""inum": "INV-001", "#, "");
    assert_eq!(
        json_path(parse_gstr2b(&text).unwrap_err()),
        "$.data.docdata.b2b[0].inv[0].inum"
    );
    let no_data = r#"{"status":1}"#;
    assert_eq!(json_path(parse_gstr2b(no_data).unwrap_err()), "$.data");
    let no_tax = mutate(r#""igst": 180.00, "#, "");
    assert_eq!(
        json_path(parse_gstr2b(&no_tax).unwrap_err()),
        "$.data.docdata.b2b[0].inv[0].items[0].igst"
    );
}

#[test]
fn bad_dates_and_periods_name_the_path() {
    let bad_date = mutate(r#""dt": "20-04-2024""#, r#""dt": "31-02-2024""#);
    assert_eq!(
        json_path(parse_gstr2b(&bad_date).unwrap_err()),
        "$.data.docdata.b2b[0].inv[1].dt"
    );
    let bad_period = mutate(r#""rtnprd": "042024""#, r#""rtnprd": "132024""#);
    assert_eq!(
        json_path(parse_gstr2b(&bad_period).unwrap_err()),
        "$.data.rtnprd"
    );
    let iso_date = mutate(r#""gendt": "14-05-2024""#, r#""gendt": "2024-05-14""#);
    assert_eq!(
        json_path(parse_gstr2b(&iso_date).unwrap_err()),
        "$.data.gendt"
    );
}

#[test]
fn an_amount_with_more_than_two_decimals_is_refused_not_rounded() {
    let text = mutate(r#""val": 118.50"#, r#""val": 118.505"#);
    assert_eq!(
        json_path(parse_gstr2b(&text).unwrap_err()),
        "$.data.docdata.b2b[0].inv[2].val"
    );
}

#[test]
fn an_unknown_note_type_and_a_bad_flag_are_refused() {
    let bad_type = mutate(r#""typ": "D""#, r#""typ": "X""#);
    assert_eq!(
        json_path(parse_gstr2b(&bad_type).unwrap_err()),
        "$.data.docdata.cdnr[0].nt[1].typ"
    );
    let bad_flag = mutate(r#""rev": "N""#, r#""rev": "maybe""#);
    assert_eq!(
        json_path(parse_gstr2b(&bad_flag).unwrap_err()),
        "$.data.docdata.b2b[0].inv[0].rev"
    );
}

#[test]
fn wrong_container_types_name_the_path() {
    let not_array = mutate(r#""b2b": ["#, r#""b2b": {"x": 1}, "unused": ["#);
    assert_eq!(
        json_path(parse_gstr2b(&not_array).unwrap_err()),
        "$.data.docdata.b2b"
    );
    assert_eq!(json_path(parse_gstr2b("[]").unwrap_err()), "$");
    let item_not_object = mutate(
        r#"{ "num": 1, "rt": 18, "txval": 1000.00, "igst": 180.00,"#,
        r#"5, { "num": 1, "rt": 18, "txval": 1000.00, "igst": 180.00,"#,
    );
    assert_eq!(
        json_path(parse_gstr2b(&item_not_object).unwrap_err()),
        "$.data.docdata.b2b[0].inv[0].items[0]"
    );
}

#[test]
fn deeply_nested_input_is_an_error_not_a_crash() {
    let text = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
    assert_eq!(json_path(parse_gstr2b(&text).unwrap_err()), "$");
}
