//! Card F1 tests: GSTR-1 and GSTR-1A JSON writer (spec 6.3, rules A3, A4).
//!
//! Provenance. Two kinds of expectation appear below.
//!
//! - Amounts, tax splits, totals and the net-issued count are `derived`: the arithmetic is written
//!   next to each case (tax = taxable x rate; invoice value = taxable + tax; net = total - cancelled).
//!   The two-decimal text is a consequence of "1 rupee = 100 paise".
//! - Key names and code values (`inum`, `itms`, `sply_ty`, `INTRB2B`, ...) are `pending` official
//!   confirmation: the GSTN publishes no schema and nobody on this project has an offline-tool
//!   sample yet (research 04-gst.md, row "GSTR-1 / IFF upload"). They are recalled, so these
//!   tests pin the layout the writer is meant to follow and must be re-checked against a real
//!   offline-tool file before release.
//!
//! The brief asked for an `insta` snapshot. A snapshot written by running our own writer would be
//! engine output copied into an expectation (rule A4), so the expected text is written by hand here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Decimal, Gstin, Hsn, Paise, Period, Qty, Rate, StateCode};
use ledgerly_gst_formats::FormatError;
use ledgerly_gst_formats::gstr1::{
    B2bInvoice, B2bInvoiceType, B2bParty, B2baInvoice, B2baParty, B2clGroup, B2clInvoice, B2csKind,
    B2csRow, CdnrNote, CdnrParty, CdnurNote, CdnurType, DocIssue, DocRange, DocType, ExpGroup,
    ExpInvoice, ExpItem, Gstr1, Gstr1a, HsnRow, ItemTax, LineItem, NilRow, NilSupplyKind, NoteKind,
    gstr1_to_json, gstr1a_to_json,
};
use time::macros::date;

const SUPPLIER: &str = "29ABCDE1234F1ZW";
const RECIPIENT: &str = "07AAACL0000A1ZP";

fn rupees(r: i64) -> Paise {
    Paise::new(r * 100)
}

fn rate(r: i64) -> Rate {
    Rate(Decimal::from(r))
}

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn empty() -> Gstr1 {
    Gstr1 {
        gstin: Gstin::parse(SUPPLIER).unwrap(),
        period: Period::new(date!(2025 - 10 - 01), date!(2025 - 10 - 31)).unwrap(),
        aggregate_turnover: None,
        current_turnover: None,
        b2b: vec![],
        b2cl: vec![],
        b2cs: vec![],
        cdnr: vec![],
        cdnur: vec![],
        exp: vec![],
        nil: vec![],
        hsn: vec![],
        doc_issue: vec![],
    }
}

fn igst_line(r: i64, taxable: i64, igst: i64) -> LineItem {
    LineItem {
        rate: rate(r),
        taxable: rupees(taxable),
        tax: ItemTax::Igst(rupees(igst)),
        cess: Paise::ZERO,
    }
}

fn b2b_invoice(num: &str, items: Vec<LineItem>, value: Paise) -> B2bInvoice {
    B2bInvoice {
        num: num.to_owned(),
        date: date!(2025 - 10 - 05),
        value,
        pos: state(7),
        reverse_charge: false,
        inv_type: B2bInvoiceType::Regular,
        items,
    }
}

fn b2b_party(invoices: Vec<B2bInvoice>) -> B2bParty {
    B2bParty {
        ctin: Gstin::parse(RECIPIENT).unwrap(),
        invoices,
    }
}

#[test]
fn sample_4a_b2cs_hsn_matches_hand_written_json() {
    let mut r = empty();
    // provenance: derived. Invoice INV/001, inter-state to Delhi (07).
    //   line 1: 10,000.00 at 18%  -> IGST 10,000.00 x 0.18 = 1,800.00
    //   line 2:  2,000.00 at  5%  -> IGST  2,000.00 x 0.05 =   100.00
    //   value = (10,000.00 + 1,800.00) + (2,000.00 + 100.00) = 11,800.00 + 2,100.00 = 13,900.00
    r.b2b = vec![b2b_party(vec![b2b_invoice(
        "INV/001",
        vec![igst_line(18, 10_000, 1_800), igst_line(5, 2_000, 100)],
        rupees(13_900),
    )])];
    // provenance: derived. Intra-state to Karnataka (29), 5,000.00 at 12%: total tax
    //   5,000.00 x 0.12 = 600.00, split CGST 6% = 300.00 and SGST 6% = 300.00.
    r.b2cs = vec![B2csRow {
        intra_state: true,
        kind: B2csKind::Other,
        pos: state(29),
        line: LineItem {
            rate: rate(12),
            taxable: rupees(5_000),
            tax: ItemTax::CgstSgst {
                cgst: rupees(300),
                sgst: rupees(300),
            },
            cess: Paise::ZERO,
        },
    }];
    // provenance: derived. HSN 6109, 50.5 pieces: taxable 10,000.00, IGST 1,800.00, value
    //   10,000.00 + 1,800.00 = 11,800.00. The rate is given as 18.00 and must print as 18.
    r.hsn = vec![HsnRow {
        hsn: Hsn::parse("6109").unwrap(),
        description: "T-shirts".to_owned(),
        uqc: "NOS".to_owned(),
        qty: Qty(Decimal::new(505, 1)),
        value: rupees(11_800),
        taxable: rupees(10_000),
        rate: Rate(Decimal::new(1800, 2)),
        igst: rupees(1_800),
        cgst: Paise::ZERO,
        sgst: Paise::ZERO,
        cess: Paise::ZERO,
    }];
    let expected = concat!(
        r#"{"gstin":"29ABCDE1234F1ZW","fp":"102025","#,
        r#""b2b":[{"ctin":"07AAACL0000A1ZP","inv":[{"inum":"INV/001","idt":"05-10-2025","#,
        r#""val":13900.00,"pos":"07","rchrg":"N","inv_typ":"R","itms":["#,
        r#"{"num":1,"itm_det":{"rt":18,"txval":10000.00,"iamt":1800.00}},"#,
        r#"{"num":2,"itm_det":{"rt":5,"txval":2000.00,"iamt":100.00}}]}]}],"#,
        r#""b2cs":[{"sply_ty":"INTRA","rt":12,"typ":"OE","pos":"29","txval":5000.00,"#,
        r#""camt":300.00,"samt":300.00}],"#,
        r#""hsn":{"data":[{"num":1,"hsn_sc":"6109","desc":"T-shirts","uqc":"NOS","qty":50.5,"#,
        r#""val":11800.00,"txval":10000.00,"iamt":1800.00,"camt":0.00,"samt":0.00,"#,
        r#""csamt":0.00,"rt":18}]}}"#,
    );
    let json = gstr1_to_json(&r).unwrap();
    assert_eq!(json, expected);
    // The output is well-formed JSON.
    serde_json::from_str::<serde_json::Value>(&json).unwrap();
}

#[test]
fn empty_sections_are_omitted() {
    // provenance: derived from the writer's documented rule "empty sections are left out"; fp is
    // the month of the period end (October 2025 -> 102025).
    assert_eq!(
        gstr1_to_json(&empty()).unwrap(),
        r#"{"gstin":"29ABCDE1234F1ZW","fp":"102025"}"#
    );
}

#[test]
fn quarterly_period_uses_the_last_month() {
    let mut r = empty();
    r.period = Period::new(date!(2025 - 10 - 01), date!(2025 - 12 - 31)).unwrap();
    // provenance: derived; a QRMP quarter October to December ends in December 2025.
    assert!(gstr1_to_json(&r).unwrap().contains(r#""fp":"122025""#));
}

#[test]
fn amounts_are_two_decimal_numbers_from_paise() {
    let mut r = empty();
    // provenance: derived; 1 rupee = 100 paise, so 5 paise = 0.05, 100 paise = 1.00,
    // -12,345 paise = -123.45 and i64::MAX = 9,223,372,036,854,775,807 paise = 92233720368547758.07.
    r.aggregate_turnover = Some(Paise::new(5));
    r.current_turnover = Some(Paise::new(100));
    assert!(
        gstr1_to_json(&r)
            .unwrap()
            .contains(r#""gt":0.05,"cur_gt":1.00"#)
    );
    r.aggregate_turnover = Some(Paise::new(-12_345));
    r.current_turnover = Some(Paise::new(i64::MAX));
    assert!(
        gstr1_to_json(&r)
            .unwrap()
            .contains(r#""gt":-123.45,"cur_gt":92233720368547758.07"#)
    );
}

#[test]
fn invoice_number_length_is_checked() {
    // provenance: derived; CGST Rules 2017 rule 46(b): invoice number of not more than 16 characters.
    let sixteen = "ABCDEFGHIJKLMNOP";
    let seventeen = "ABCDEFGHIJKLMNOPQ";
    let line = vec![igst_line(18, 100, 18)];
    let mut r = empty();
    r.b2b = vec![b2b_party(vec![b2b_invoice(
        sixteen,
        line.clone(),
        rupees(118),
    )])];
    gstr1_to_json(&r).unwrap();
    r.b2b = vec![b2b_party(vec![b2b_invoice(
        seventeen,
        line.clone(),
        rupees(118),
    )])];
    match gstr1_to_json(&r) {
        Err(FormatError::Field { field, .. }) => assert_eq!(field, "b2b[0].inv[0].inum"),
        other => panic!("expected a field error, got {other:?}"),
    }
    r.b2b = vec![b2b_party(vec![b2b_invoice("", line, rupees(118))])];
    assert!(matches!(gstr1_to_json(&r), Err(FormatError::Field { .. })));
}

#[test]
fn invoice_without_items_is_an_error_naming_the_path() {
    let mut r = empty();
    r.b2b = vec![b2b_party(vec![b2b_invoice("A1", vec![], Paise::ZERO)])];
    match gstr1_to_json(&r) {
        Err(FormatError::Field { field, .. }) => assert_eq!(field, "b2b[0].inv[0].itms"),
        other => panic!("expected a field error, got {other:?}"),
    }
    r.b2b = vec![b2b_party(vec![])];
    assert!(matches!(gstr1_to_json(&r), Err(FormatError::Field { .. })));
}

#[test]
fn b2cl_cdnr_cdnur_exp_nil_layout() {
    // provenance: pending. The exact text uses key names recalled from the offline-tool layout
    // (see the module doc above); it must be re-checked against an official sample.
    let mut r = empty();
    // provenance: derived. B2CL: 3,00,000.00 at 18% inter-state to Delhi: IGST = 54,000.00;
    //   value = 3,54,000.00.
    r.b2cl = vec![B2clGroup {
        pos: state(7),
        invoices: vec![B2clInvoice {
            num: "B2CL-1".to_owned(),
            date: date!(2025 - 10 - 09),
            value: rupees(354_000),
            items: vec![igst_line(18, 300_000, 54_000)],
        }],
    }];
    // provenance: derived. Credit note on a 1,000.00 line at 18%: IGST 180.00, value 1,180.00.
    r.cdnr = vec![CdnrParty {
        ctin: Gstin::parse(RECIPIENT).unwrap(),
        notes: vec![CdnrNote {
            kind: NoteKind::Credit,
            num: "CN-1".to_owned(),
            date: date!(2025 - 10 - 12),
            value: rupees(1_180),
            pos: state(7),
            reverse_charge: true,
            inv_type: B2bInvoiceType::Regular,
            items: vec![igst_line(18, 1_000, 180)],
        }],
    }];
    // provenance: derived. Debit note against an export without payment: no tax, value = taxable.
    r.cdnur = vec![CdnurNote {
        ur_type: CdnurType::ExportWithoutPayment,
        kind: NoteKind::Debit,
        num: "DN-1".to_owned(),
        date: date!(2025 - 10 - 13),
        value: rupees(500),
        items: vec![igst_line(0, 500, 0)],
    }];
    // provenance: derived. Export with payment: 2,000.00 at 18%: IGST 360.00; value 2,360.00.
    r.exp = vec![ExpGroup {
        with_payment: true,
        invoices: vec![ExpInvoice {
            num: "EX-1".to_owned(),
            date: date!(2025 - 10 - 20),
            value: rupees(2_360),
            port_code: Some("INMAA1".to_owned()),
            shipping_bill: Some(("1234567".to_owned(), date!(2025 - 10 - 21))),
            items: vec![ExpItem {
                rate: rate(18),
                taxable: rupees(2_000),
                igst: rupees(360),
                cess: Paise::ZERO,
            }],
        }],
    }];
    r.nil = vec![NilRow {
        kind: NilSupplyKind::InterB2b,
        nil_rated: rupees(10),
        exempt: rupees(20),
        non_gst: rupees(30),
    }];
    let expected = concat!(
        r#"{"gstin":"29ABCDE1234F1ZW","fp":"102025","#,
        r#""b2cl":[{"pos":"07","inv":[{"inum":"B2CL-1","idt":"09-10-2025","val":354000.00,"#,
        r#""itms":[{"num":1,"itm_det":{"rt":18,"txval":300000.00,"iamt":54000.00}}]}]}],"#,
        r#""cdnr":[{"ctin":"07AAACL0000A1ZP","nt":[{"ntty":"C","nt_num":"CN-1","#,
        r#""nt_dt":"12-10-2025","val":1180.00,"pos":"07","rchrg":"Y","inv_typ":"R","#,
        r#""itms":[{"num":1,"itm_det":{"rt":18,"txval":1000.00,"iamt":180.00}}]}]}],"#,
        r#""cdnur":[{"typ":"EXPWOP","ntty":"D","nt_num":"DN-1","nt_dt":"13-10-2025","#,
        r#""val":500.00,"itms":[{"num":1,"itm_det":{"rt":0,"txval":500.00,"iamt":0.00}}]}],"#,
        r#""exp":[{"exp_typ":"WPAY","inv":[{"inum":"EX-1","idt":"20-10-2025","val":2360.00,"#,
        r#""sbpcode":"INMAA1","sbnum":"1234567","sbdt":"21-10-2025","#,
        r#""itms":[{"txval":2000.00,"rt":18,"iamt":360.00}]}]}],"#,
        r#""nil":{"inv":[{"sply_ty":"INTRB2B","expt_amt":20.00,"nil_amt":10.00,"#,
        r#""ngsup_amt":30.00}]}}"#,
    );
    assert_eq!(gstr1_to_json(&r).unwrap(), expected);
}

#[test]
fn cess_is_written_only_when_not_zero() {
    // provenance: pending. The exact text uses key names recalled from the offline-tool layout
    // (see the module doc above); it must be re-checked against an official sample.
    let mut r = empty();
    // provenance: derived. 1,000.00 at 28% inter-state: IGST 280.00; cess 12% of 1,000.00 = 120.00.
    r.b2cs = vec![B2csRow {
        intra_state: false,
        kind: B2csKind::Ecommerce {
            etin: Gstin::parse("24AAACB1111C1ZT").unwrap(),
        },
        pos: state(7),
        line: LineItem {
            rate: rate(28),
            taxable: rupees(1_000),
            tax: ItemTax::Igst(rupees(280)),
            cess: rupees(120),
        },
    }];
    let expected = concat!(
        r#"{"gstin":"29ABCDE1234F1ZW","fp":"102025","#,
        r#""b2cs":[{"sply_ty":"INTER","rt":28,"typ":"E","etin":"24AAACB1111C1ZT","pos":"07","#,
        r#""txval":1000.00,"iamt":280.00,"csamt":120.00}]}"#,
    );
    assert_eq!(gstr1_to_json(&r).unwrap(), expected);
}

#[test]
fn doc_issue_net_is_total_minus_cancelled() {
    // provenance: pending. The exact text uses key names recalled from the offline-tool layout
    // (see the module doc above); it must be re-checked against an official sample.
    let mut r = empty();
    // provenance: derived. Invoices 1 to 10 issued, 2 cancelled: net = 10 - 2 = 8.
    r.doc_issue = vec![DocIssue {
        doc_type: DocType::OutwardInvoice,
        ranges: vec![DocRange {
            from: "1".to_owned(),
            to: "10".to_owned(),
            total: 10,
            cancelled: 2,
        }],
    }];
    let expected = concat!(
        r#"{"gstin":"29ABCDE1234F1ZW","fp":"102025","#,
        r#""doc_issue":{"doc_det":[{"doc_num":1,"docs":[{"num":1,"from":"1","to":"10","#,
        r#""totnum":10,"cancel":2,"net_issue":8}]}]}}"#,
    );
    assert_eq!(gstr1_to_json(&r).unwrap(), expected);
    // More cancelled than issued is an error, not a negative or wrapped count.
    r.doc_issue = vec![DocIssue {
        doc_type: DocType::CreditNote,
        ranges: vec![DocRange {
            from: "1".to_owned(),
            to: "3".to_owned(),
            total: 3,
            cancelled: 4,
        }],
    }];
    match gstr1_to_json(&r) {
        Err(FormatError::Field { field, .. }) => {
            assert_eq!(field, "doc_issue.doc_det[0].docs[0].cancel");
        }
        other => panic!("expected a field error, got {other:?}"),
    }
}

#[test]
fn hsn_without_unit_code_is_an_error() {
    let mut r = empty();
    r.hsn = vec![HsnRow {
        hsn: Hsn::parse("998314").unwrap(),
        description: String::new(),
        uqc: " ".to_owned(),
        qty: Qty::ZERO,
        value: Paise::ZERO,
        taxable: Paise::ZERO,
        rate: rate(18),
        igst: Paise::ZERO,
        cgst: Paise::ZERO,
        sgst: Paise::ZERO,
        cess: Paise::ZERO,
    }];
    match gstr1_to_json(&r) {
        Err(FormatError::Field { field, .. }) => assert_eq!(field, "hsn.data[0].uqc"),
        other => panic!("expected a field error, got {other:?}"),
    }
}

#[test]
fn gstr1a_b2ba_carries_original_and_amended_invoice() {
    // provenance: pending. The exact text uses key names recalled from the offline-tool layout
    // (see the module doc above); it must be re-checked against an official sample.
    // provenance: derived. Original INV/001 of 05-10-2025 amended to 1,000.00 at 18%: IGST 180.00,
    // value 1,180.00.
    let r = Gstr1a {
        gstin: Gstin::parse(SUPPLIER).unwrap(),
        period: Period::new(date!(2025 - 10 - 01), date!(2025 - 10 - 31)).unwrap(),
        b2ba: vec![B2baParty {
            ctin: Gstin::parse(RECIPIENT).unwrap(),
            invoices: vec![B2baInvoice {
                original_num: "INV/001".to_owned(),
                original_date: date!(2025 - 10 - 05),
                amended: B2bInvoice {
                    num: "INV/001A".to_owned(),
                    date: date!(2025 - 10 - 06),
                    ..b2b_invoice("x", vec![igst_line(18, 1_000, 180)], rupees(1_180))
                },
            }],
        }],
    };
    let expected = concat!(
        r#"{"gstin":"29ABCDE1234F1ZW","fp":"102025","#,
        r#""b2ba":[{"ctin":"07AAACL0000A1ZP","inv":[{"oinum":"INV/001","oidt":"05-10-2025","#,
        r#""inum":"INV/001A","idt":"06-10-2025","val":1180.00,"pos":"07","rchrg":"N","#,
        r#""inv_typ":"R","itms":[{"num":1,"itm_det":{"rt":18,"txval":1000.00,"iamt":180.00}}]}]}]}"#,
    );
    assert_eq!(gstr1a_to_json(&r).unwrap(), expected);
}
