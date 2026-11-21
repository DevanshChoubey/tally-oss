//! Card F4 tests: e-invoice INV-01 JSON and local validation (spec 6.3).
//!
//! Provenance: every amount below is hand arithmetic written next to the sample. The structure and
//! limits (`DocDtls.No` of 1 to 16 characters, field names) are `derived` from the NIC INV-01
//! v1.1 schema as recalled in `docs/research/04-gst.md` section 2.13; the field names and limits
//! are not yet quoted from the notified text, so they are listed as such in the module doc.
//! GSTINs are fictional with a valid check character computed by `gstin_check_character`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Decimal, Gstin, Hsn, Paise, StateCode, gstin_check_character};
use ledgerly_gst_formats::FormatError;
use ledgerly_gst_formats::einvoice::{
    DocType, EInvItem, EInvParty, EInvTotals, EInvoice, SupplyType, einvoice_to_json,
    validate_einvoice,
};
use serde_json::Value;
use time::macros::date;

fn gstin(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn party(first14: &str, name: &str, state: u8) -> EInvParty {
    EInvParty {
        gstin: gstin(first14),
        legal_name: name.to_owned(),
        trade_name: None,
        address1: "12 Test Road".to_owned(),
        address2: None,
        location: "Testville".to_owned(),
        pin: 400_001,
        state: StateCode::new(state).unwrap(),
        phone: None,
        email: None,
    }
}

fn p(paise: i64) -> Paise {
    Paise::new(paise)
}

/// Intra-state sample (seller and place of supply both state 27).
///
/// provenance: derived — hand arithmetic.
/// Item 1: 10 x 100.000 = 1000.00; no discount; 18% -> CGST 90.00, SGST 90.00; total 1180.00.
/// Item 2: 2 x 250.50 = 501.00; discount 1.00 -> taxable 500.00; 12% -> CGST 30.00, SGST 30.00;
/// total 560.00.
/// Totals: taxable 1000 + 500 = 1500.00; CGST 90 + 30 = 120.00; SGST 120.00; IGST 0;
/// invoice 1500 + 120 + 120 = 1740.00 (items: 1180 + 560 = 1740).
fn intra_sample() -> EInvoice {
    let item1 = EInvItem {
        description: Some("Test goods A".to_owned()),
        is_service: false,
        hsn: Hsn::parse("8471").unwrap(),
        quantity: Some(Decimal::from(10)),
        free_quantity: None,
        unit: Some("NOS".to_owned()),
        unit_price: Decimal::from(100),
        gross_amount: p(100_000),
        discount: p(0),
        assessable: p(100_000),
        gst_rate: Decimal::from(18),
        igst: p(0),
        cgst: p(9_000),
        sgst: p(9_000),
        cess: p(0),
        state_cess: p(0),
        total: p(118_000),
    };
    let item2 = EInvItem {
        description: None,
        is_service: false,
        hsn: Hsn::parse("8473").unwrap(),
        quantity: Some(Decimal::from(2)),
        free_quantity: None,
        unit: Some("NOS".to_owned()),
        unit_price: Decimal::new(25050, 2),
        gross_amount: p(50_100),
        discount: p(100),
        assessable: p(50_000),
        gst_rate: Decimal::from(12),
        igst: p(0),
        cgst: p(3_000),
        sgst: p(3_000),
        cess: p(0),
        state_cess: p(0),
        total: p(56_000),
    };
    EInvoice {
        supply_type: SupplyType::B2b,
        reverse_charge: false,
        ecom_gstin: None,
        igst_on_intra: false,
        doc_type: DocType::Invoice,
        doc_no: "INV/2026-27/0001".to_owned(),
        doc_date: date!(2026 - 11 - 20),
        seller: party("27AAAAA0000A1Z", "Test Seller Pvt Ltd", 27),
        buyer: party("27BBBBB1111B1Z", "Test Buyer Pvt Ltd", 27),
        place_of_supply: StateCode::new(27).unwrap(),
        items: vec![item1, item2],
        totals: EInvTotals {
            assessable: p(150_000),
            cgst: p(12_000),
            sgst: p(12_000),
            igst: p(0),
            cess: p(0),
            state_cess: p(0),
            discount: p(0),
            other_charges: p(0),
            round_off: p(0),
            total: p(174_000),
        },
    }
}

/// Inter-state sample (seller state 27, place of supply 29) with a round-off.
///
/// provenance: derived — hand arithmetic. One item: 1 x 100.50 = 100.50 taxable; 12% IGST =
/// 100.50 x 0.12 = 12.06 exactly; item total 112.56. Round-off +0.44 makes the invoice 113.00.
fn inter_sample() -> EInvoice {
    let item = EInvItem {
        description: Some("Test service".to_owned()),
        is_service: true,
        hsn: Hsn::parse("998314").unwrap(),
        quantity: Some(Decimal::ONE),
        free_quantity: None,
        unit: None,
        unit_price: Decimal::new(10050, 2),
        gross_amount: p(10_050),
        discount: p(0),
        assessable: p(10_050),
        gst_rate: Decimal::from(12),
        igst: p(1_206),
        cgst: p(0),
        sgst: p(0),
        cess: p(0),
        state_cess: p(0),
        total: p(11_256),
    };
    let mut e = intra_sample();
    e.buyer = party("29BBBBB1111B1Z", "Test Buyer Pvt Ltd", 29);
    e.place_of_supply = StateCode::new(29).unwrap();
    e.items = vec![item];
    e.totals = EInvTotals {
        assessable: p(10_050),
        cgst: p(0),
        sgst: p(0),
        igst: p(1_206),
        cess: p(0),
        state_cess: p(0),
        discount: p(0),
        other_charges: p(0),
        round_off: p(44),
        total: p(11_300),
    };
    e
}

fn field_names(errs: &[FormatError]) -> Vec<String> {
    errs.iter()
        .map(|e| match e {
            FormatError::Field { field, .. } => field.clone(),
            FormatError::Json { path, .. } => path.clone(),
            FormatError::Spreadsheet(s) => s.clone(),
            FormatError::Unsupported { reason } => reason.clone(),
        })
        .collect()
}

fn invalid_fields(e: &EInvoice) -> Vec<String> {
    field_names(&validate_einvoice(e).unwrap_err())
}

#[test]
fn samples_validate() {
    assert!(validate_einvoice(&intra_sample()).is_ok());
    assert!(validate_einvoice(&inter_sample()).is_ok());
}

#[test]
fn required_fields_present() {
    // provenance: derived — INV-01 v1.1 mandatory fields: Version, TranDtls.{TaxSch,SupTyp},
    // DocDtls.{Typ,No,Dt}, SellerDtls.{Gstin,LglNm,Addr1,Loc,Pin,Stcd},
    // BuyerDtls.{Gstin,LglNm,Pos,Addr1,Loc,Pin,Stcd}, ItemList[].{SlNo,IsServc,HsnCd,UnitPrice,
    // TotAmt,AssAmt,GstRt,TotItemVal}, ValDtls.{AssVal,TotInvVal}.
    let json = einvoice_to_json(&intra_sample()).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["Version"], "1.1");
    for k in ["TaxSch", "SupTyp"] {
        assert!(v["TranDtls"].get(k).is_some(), "TranDtls.{k}");
    }
    for k in ["Typ", "No", "Dt"] {
        assert!(v["DocDtls"].get(k).is_some(), "DocDtls.{k}");
    }
    for k in ["Gstin", "LglNm", "Addr1", "Loc", "Pin", "Stcd"] {
        assert!(v["SellerDtls"].get(k).is_some(), "SellerDtls.{k}");
        assert!(v["BuyerDtls"].get(k).is_some(), "BuyerDtls.{k}");
    }
    assert!(v["BuyerDtls"].get("Pos").is_some());
    let items = v["ItemList"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    for item in items {
        for k in [
            "SlNo",
            "IsServc",
            "HsnCd",
            "UnitPrice",
            "TotAmt",
            "AssAmt",
            "GstRt",
            "TotItemVal",
        ] {
            assert!(item.get(k).is_some(), "ItemList[].{k}");
        }
    }
    for k in ["AssVal", "TotInvVal"] {
        assert!(v["ValDtls"].get(k).is_some(), "ValDtls.{k}");
    }
}

#[test]
fn json_values_follow_the_sample() {
    // provenance: derived — the values are the hand arithmetic in `intra_sample`; the document date
    // 2026-11-20 is written DD/MM/YYYY as the schema requires.
    let json = einvoice_to_json(&intra_sample()).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["DocDtls"]["Dt"], "20/11/2026");
    assert_eq!(v["DocDtls"]["Typ"], "INV");
    assert_eq!(v["TranDtls"]["SupTyp"], "B2B");
    assert_eq!(v["TranDtls"]["RegRev"], "N");
    assert_eq!(v["BuyerDtls"]["Pos"], "27");
    assert_eq!(v["ItemList"][0]["SlNo"], "1");
    assert_eq!(v["ItemList"][1]["SlNo"], "2");
    // Amounts are exact two-decimal JSON numbers.
    assert!(json.contains("\"AssVal\":1500.00"));
    assert!(json.contains("\"CgstVal\":120.00"));
    assert!(json.contains("\"TotInvVal\":1740.00"));
    assert!(json.contains("\"TotItemVal\":1180.00"));
    assert!(json.contains("\"Discount\":1.00"));
}

#[test]
fn inter_state_round_off_is_written() {
    // provenance: derived — see `inter_sample`: IGST 12.06, round-off +0.44, invoice 113.00.
    let json = einvoice_to_json(&inter_sample()).unwrap();
    assert!(json.contains("\"IgstVal\":12.06"));
    assert!(json.contains("\"RndOffAmt\":0.44"));
    assert!(json.contains("\"TotInvVal\":113.00"));
    assert!(json.contains("\"IsServc\":\"Y\""));
}

#[test]
fn doc_no_sixteen_chars_ok_seventeen_rejected() {
    // provenance: derived — INV-01 DocDtls.No: 1 to 16 characters.
    let mut e = intra_sample();
    e.doc_no = "A234567890123456".to_owned(); // 16 characters
    assert_eq!(e.doc_no.chars().count(), 16);
    assert!(validate_einvoice(&e).is_ok());
    e.doc_no = "A2345678901234567".to_owned(); // 17 characters
    assert_eq!(e.doc_no.chars().count(), 17);
    assert_eq!(invalid_fields(&e), vec!["DocDtls.No".to_owned()]);
    match einvoice_to_json(&e) {
        Err(FormatError::Field { field, reason }) => {
            assert_eq!(field, "DocDtls.No");
            assert!(reason.contains("17"), "{reason}");
        }
        other => panic!("expected a DocDtls.No error, got {other:?}"),
    }
}

#[test]
fn doc_no_shape() {
    // provenance: derived — INV-01 DocDtls.No pattern: first character a-zA-Z1-9, then
    // a-zA-Z0-9/- (no leading zero, slash or hyphen; no spaces; not empty).
    for bad in ["", "0001", "/A1", "-A1", "A 1", "A_1"] {
        let mut e = intra_sample();
        e.doc_no = bad.to_owned();
        assert_eq!(invalid_fields(&e), vec!["DocDtls.No".to_owned()], "{bad:?}");
    }
    for good in ["1", "A", "a/b-c9", "INV-2026/27/1"] {
        let mut e = intra_sample();
        e.doc_no = good.to_owned();
        assert!(validate_einvoice(&e).is_ok(), "{good:?}");
    }
}

#[test]
fn totals_equal_sum_of_items() {
    // provenance: derived — ValDtls totals are the sums over ItemList (hand sums in `intra_sample`).
    // Taxable 150_001 differs from the item sum 150_000, and the invoice total 174_000 then differs
    // from 150_001 + 12_000 + 12_000 = 174_001.
    let mut e = intra_sample();
    e.totals.assessable = p(150_001);
    assert_eq!(
        invalid_fields(&e),
        vec!["ValDtls.AssVal".to_owned(), "ValDtls.TotInvVal".to_owned()]
    );

    // CGST 119.99 + SGST 120.01 still add up to 240.00, so only the two tax totals are wrong.
    let mut e = intra_sample();
    e.totals.cgst = p(11_999);
    e.totals.sgst = p(12_001);
    assert_eq!(
        invalid_fields(&e),
        vec!["ValDtls.CgstVal".to_owned(), "ValDtls.SgstVal".to_owned()]
    );

    let mut e = intra_sample();
    e.totals.total = p(174_100);
    assert_eq!(invalid_fields(&e), vec!["ValDtls.TotInvVal".to_owned()]);
}

#[test]
fn invoice_total_includes_document_level_charges() {
    // provenance: derived — TotInvVal = AssVal + taxes + OthChrg - Discount + RndOffAmt:
    // 1740.00 + 10.00 - 5.00 - 0.50 = 1744.50 = 174_450 paise.
    let mut e = intra_sample();
    e.totals.other_charges = p(1_000);
    e.totals.discount = p(500);
    e.totals.round_off = p(-50);
    e.totals.total = p(174_450);
    assert!(validate_einvoice(&e).is_ok());
    e.totals.total = p(174_000);
    assert_eq!(invalid_fields(&e), vec!["ValDtls.TotInvVal".to_owned()]);
}

#[test]
fn item_arithmetic_is_checked() {
    // provenance: derived — AssAmt = TotAmt - Discount; TotItemVal = AssAmt + tax amounts.
    let mut e = intra_sample();
    if let Some(it) = e.items.get_mut(1) {
        it.assessable = p(50_100);
    }
    let fields = invalid_fields(&e);
    assert!(
        fields.contains(&"ItemList[1].AssAmt".to_owned()),
        "{fields:?}"
    );

    let mut e = intra_sample();
    if let Some(it) = e.items.get_mut(0) {
        it.total = p(118_001);
    }
    let fields = invalid_fields(&e);
    assert!(
        fields.contains(&"ItemList[0].TotItemVal".to_owned()),
        "{fields:?}"
    );
}

#[test]
fn schema_invalid_names_the_field() {
    let mut e = intra_sample();
    e.seller.legal_name = "AB".to_owned();
    assert_eq!(invalid_fields(&e), vec!["SellerDtls.LglNm".to_owned()]);

    let mut e = intra_sample();
    e.buyer.pin = 12_345;
    assert_eq!(invalid_fields(&e), vec!["BuyerDtls.Pin".to_owned()]);

    let mut e = intra_sample();
    e.buyer.location = "X".to_owned();
    assert_eq!(invalid_fields(&e), vec!["BuyerDtls.Loc".to_owned()]);

    let mut e = intra_sample();
    e.seller.address1 = String::new();
    assert_eq!(invalid_fields(&e), vec!["SellerDtls.Addr1".to_owned()]);

    // The seller's state code must match its GSTIN (27...).
    let mut e = intra_sample();
    e.seller.state = StateCode::new(29).unwrap();
    assert!(invalid_fields(&e).contains(&"SellerDtls.Stcd".to_owned()));

    let mut e = intra_sample();
    e.items.clear();
    assert!(invalid_fields(&e).contains(&"ItemList".to_owned()));
}

#[test]
fn schema_invalid_error_message_is_user_readable() {
    let mut e = intra_sample();
    e.seller.legal_name = "AB".to_owned();
    let err = einvoice_to_json(&e).unwrap_err();
    // provenance: derived — LglNm is 3 to 100 characters.
    assert_eq!(
        err.to_string(),
        "SellerDtls.LglNm: must be 3 to 100 characters, found 2"
    );
}

#[test]
fn item_field_rules() {
    // provenance: derived — IsServc is Y only for a SAC (99...); quantities and prices allow 3
    // decimal places and are not negative.
    let mut e = intra_sample();
    if let Some(it) = e.items.get_mut(0) {
        it.is_service = true; // HSN 8471 is goods
        it.quantity = Some(Decimal::new(1_0001, 4)); // 4 decimal places
        it.unit_price = Decimal::new(-1, 0);
    }
    let fields = invalid_fields(&e);
    for want in [
        "ItemList[0].IsServc",
        "ItemList[0].Qty",
        "ItemList[0].UnitPrice",
    ] {
        assert!(fields.contains(&want.to_owned()), "{want} in {fields:?}");
    }
}

#[test]
fn tax_kind_must_match_place_of_supply() {
    // provenance: derived — IGST Act s.7/s.8 and CGST Act s.9: an intra-state supply carries
    // CGST + SGST, an inter-state supply carries IGST.
    let mut e = intra_sample();
    if let Some(it) = e.items.get_mut(0) {
        it.igst = p(18_000);
        it.cgst = p(0);
        it.sgst = p(0);
    }
    assert!(invalid_fields(&e).contains(&"ItemList[0].IgstAmt".to_owned()));

    let mut e = inter_sample();
    if let Some(it) = e.items.get_mut(0) {
        it.cgst = p(603);
        it.sgst = p(603);
        it.igst = p(0);
    }
    assert!(invalid_fields(&e).contains(&"ItemList[0].CgstAmt".to_owned()));

    // IGST on an intra-state supply is allowed only when IgstOnIntra is Y.
    let mut e = intra_sample();
    e.igst_on_intra = true;
    for it in &mut e.items {
        it.igst = p(it.cgst.get() + it.sgst.get());
        it.cgst = p(0);
        it.sgst = p(0);
    }
    e.totals.igst = p(24_000);
    e.totals.cgst = p(0);
    e.totals.sgst = p(0);
    assert!(validate_einvoice(&e).is_ok());

    // IgstOnIntra = Y with a place of supply outside the supplier's state is itself wrong.
    let mut e = inter_sample();
    e.igst_on_intra = true;
    assert!(invalid_fields(&e).contains(&"TranDtls.IgstOnIntra".to_owned()));
}

#[test]
fn overflowing_sums_are_errors_not_panics() {
    // provenance: derived — two items of i64::MAX paise cannot be summed; rule A3 requires an
    // error naming the total, never a wrapped value.
    let mut e = intra_sample();
    for it in &mut e.items {
        it.gross_amount = p(i64::MAX);
        it.assessable = p(i64::MAX);
    }
    let fields = invalid_fields(&e);
    assert!(fields.contains(&"ValDtls.AssVal".to_owned()), "{fields:?}");
}

#[test]
fn text_is_escaped_in_json() {
    // provenance: derived — JSON (RFC 8259) requires `"`, `\` and control characters to be escaped.
    let mut e = intra_sample();
    e.seller.legal_name = "Test \"Quoted\" \\ Seller\n".to_owned();
    let json = einvoice_to_json(&e).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["SellerDtls"]["LglNm"], "Test \"Quoted\" \\ Seller\n");
}

#[test]
fn optional_fields_appear_only_when_set() {
    // provenance: derived — optional schema fields are omitted, never written as empty.
    let json = einvoice_to_json(&intra_sample()).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert!(v["SellerDtls"].get("TrdNm").is_none());
    assert!(v["TranDtls"].get("EcmGstin").is_none());
    assert!(v["ValDtls"].get("CesVal").is_none());

    let mut e = intra_sample();
    e.seller.trade_name = Some("Seller Trading".to_owned());
    e.ecom_gstin = Some(gstin("29CCCCC2222C1Z"));
    let v: Value = serde_json::from_str(&einvoice_to_json(&e).unwrap()).unwrap();
    assert_eq!(v["SellerDtls"]["TrdNm"], "Seller Trading");
    assert_eq!(
        v["TranDtls"]["EcmGstin"],
        e.ecom_gstin.as_ref().unwrap().as_str()
    );
}

#[test]
fn output_is_deterministic() {
    // provenance: derived — rule A7: same input, same output.
    let e = intra_sample();
    assert_eq!(einvoice_to_json(&e).unwrap(), einvoice_to_json(&e).unwrap());
}
