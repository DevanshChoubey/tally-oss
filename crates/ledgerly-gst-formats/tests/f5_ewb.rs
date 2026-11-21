//! Card F5: e-way bill JSON writer and local validation.
//!
//! Provenance. Money and arithmetic in the expected values are `derived` by hand, shown next to
//! each case. The portal field names, their order, the numeric codes and the limits (4000 km,
//! 16-character document number, vehicle-number shapes) are recalled from the public NIC e-way
//! bill documentation and are `pending` an official sample file: the exact-output test is the
//! first thing to re-check against it (rule A4, A5).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and index"
)]

use ledgerly_core::{Date, Decimal, Gstin, Hsn, Month, Paise, StateCode};
use ledgerly_gst_formats::FormatError;
use ledgerly_gst_formats::ewb::{
    DocType, EwayBill, EwayItem, Party, SubSupplyType, SupplyType, TransMode, TransactionType,
    Transport, VehicleType, ewb_to_json, is_valid_vehicle_no, validate_ewb,
};

// GSTINs below are the fictional ones with valid check characters already used by the core tests.
const SUPPLIER: &str = "29ABCDE1234F1ZW";
const BUYER: &str = "07AAACL0000A1ZP";

fn party(gstin: Option<&str>, place: &str, pincode: u32, state: u8) -> Party {
    let state = StateCode::new(state).unwrap();
    Party {
        gstin: gstin.map(|g| Gstin::parse(g).unwrap()),
        trade_name: format!("{place} Trader"),
        addr1: "1 Test Road".to_owned(),
        addr2: None,
        place: place.to_owned(),
        pincode,
        state_code: state,
        actual_state_code: state,
    }
}

/// Bengaluru to New Delhi (inter-state), 10 x Steel Bolts at 10,000.00 taxable, 18% IGST.
///
/// provenance: derived. IGST = 18% x 10,000.00 = 1,800.00 (that is 18 x 1,000,000 / 100 =
/// 180,000 paise); invoice total = 10,000.00 + 1,800.00 = 11,800.00 (1,180,000 paise).
fn sample() -> EwayBill {
    EwayBill {
        user_gstin: Gstin::parse(SUPPLIER).unwrap(),
        supply_type: SupplyType::Outward,
        sub_supply_type: SubSupplyType::Supply,
        sub_supply_desc: None,
        doc_type: DocType::Invoice,
        doc_no: "INV/26-27/0001".to_owned(),
        doc_date: Date::from_calendar_date(2026, Month::November, 20).unwrap(),
        from: party(Some(SUPPLIER), "Bengaluru", 560_001, 29),
        to: party(Some(BUYER), "New Delhi", 110_001, 7),
        transaction_type: TransactionType::Regular,
        other_value: Paise::ZERO,
        total_value: Paise::new(1_000_000),
        cgst_value: Paise::ZERO,
        sgst_value: Paise::ZERO,
        igst_value: Paise::new(180_000),
        cess_value: Paise::ZERO,
        cess_non_advol_value: Paise::ZERO,
        tot_inv_value: Paise::new(1_180_000),
        transport: Transport {
            transporter_id: None,
            transporter_name: None,
            mode: Some(TransMode::Road),
            distance_km: 2150,
            doc_no: None,
            doc_date: None,
            vehicle_no: Some("KA01AB1234".to_owned()),
            vehicle_type: Some(VehicleType::Regular),
        },
        items: vec![item("Steel Bolts", "7318", 1_000_000)],
    }
}

fn item(name: &str, hsn: &str, taxable: i64) -> EwayItem {
    EwayItem {
        product_name: name.to_owned(),
        product_desc: None,
        hsn: Hsn::parse(hsn).unwrap(),
        quantity: Decimal::from(10),
        qty_unit: "NOS".to_owned(),
        taxable_amount: Paise::new(taxable),
        cgst_rate: Decimal::ZERO,
        sgst_rate: Decimal::ZERO,
        igst_rate: Decimal::from(18),
        cess_rate: Decimal::ZERO,
        cess_non_advol: Paise::ZERO,
    }
}

fn fields(r: Result<(), Vec<FormatError>>) -> Vec<String> {
    r.err()
        .unwrap_or_default()
        .into_iter()
        .map(|e| match e {
            FormatError::Field { field, .. } => field,
            FormatError::Json { path, .. } => path,
            FormatError::Spreadsheet(s) => s,
            FormatError::Unsupported { reason } => reason,
        })
        .collect()
}

fn with_vehicle(v: &str) -> EwayBill {
    let mut e = sample();
    e.transport.vehicle_no = Some(v.to_owned());
    e
}

#[test]
fn snapshot_regular_road_bill() {
    let mut e = sample();
    e.items[0].product_desc = Some("M10 steel bolts".to_owned());
    e.from.addr2 = Some("Industrial Area".to_owned());
    let got = ewb_to_json(&e).unwrap();
    // provenance: derived (values) / pending (key names and order, recalled from the NIC bulk
    // upload sample). Amounts: 10,000.00 taxable, 1,800.00 IGST, 11,800.00 total as above;
    // docDate 2026-11-20 is written dd/mm/yyyy; hsnCode is the number 7318; distance is a
    // string; state codes 29 (Karnataka) and 7 (Delhi) come from the GST state-code list.
    let want = concat!(
        r#"{"version":"1.0.0621","billLists":[{"userGstin":"29ABCDE1234F1ZW","#,
        r#""supplyType":"O","subSupplyType":1,"docType":"INV","docNo":"INV/26-27/0001","#,
        r#""docDate":"20/11/2026","fromGstin":"29ABCDE1234F1ZW","fromTrdName":"Bengaluru Trader","#,
        r#""fromAddr1":"1 Test Road","fromAddr2":"Industrial Area","fromPlace":"Bengaluru","#,
        r#""fromPincode":560001,"actFromStateCode":29,"fromStateCode":29,"#,
        r#""toGstin":"07AAACL0000A1ZP","toTrdName":"New Delhi Trader","toAddr1":"1 Test Road","#,
        r#""toPlace":"New Delhi","toPincode":110001,"actToStateCode":7,"toStateCode":7,"#,
        r#""transactionType":1,"otherValue":0.00,"totalValue":10000.00,"cgstValue":0.00,"#,
        r#""sgstValue":0.00,"igstValue":1800.00,"cessValue":0.00,"cessNonAdvolValue":0.00,"#,
        r#""totInvValue":11800.00,"transMode":1,"transDistance":"2150","vehicleNo":"KA01AB1234","#,
        r#""vehicleType":"R","itemList":[{"productName":"Steel Bolts","productDesc":"M10 steel bolts","#,
        r#""hsnCode":7318,"quantity":10,"qtyUnit":"NOS","taxableAmount":10000.00,"sgstRate":0,"#,
        r#""cgstRate":0,"igstRate":18,"cessRate":0,"cessNonAdvol":0.00}]}]}"#,
    );
    assert_eq!(got, want);
}

#[test]
fn output_is_valid_json_with_one_bill() {
    let json = ewb_to_json(&sample()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["billLists"].as_array().unwrap().len(), 1);
    assert_eq!(v["billLists"][0]["itemList"].as_array().unwrap().len(), 1);
}

#[test]
fn strings_are_escaped_and_urp_is_written() {
    let mut e = sample();
    e.to = party(None, "Mysuru", 570_001, 29);
    e.to.trade_name = "A \"Quoted\" \\ Name".to_owned();
    e.igst_value = Paise::ZERO;
    e.cgst_value = Paise::new(90_000);
    e.sgst_value = Paise::new(90_000);
    e.items[0].igst_rate = Decimal::ZERO;
    e.items[0].cgst_rate = Decimal::new(9, 0);
    e.items[0].sgst_rate = Decimal::new(9, 0);
    let json = ewb_to_json(&e).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let bill = &v["billLists"][0];
    // provenance: derived. JSON string escaping (RFC 8259): quote and backslash are escaped;
    // an unregistered recipient is written as the literal URP. 9% + 9% of 10,000.00 is 900.00
    // each (90,000 paise), total 10,000.00 + 900.00 + 900.00 = 11,800.00, so totInvValue holds.
    assert_eq!(bill["toTrdName"], "A \"Quoted\" \\ Name");
    assert_eq!(bill["toGstin"], "URP");
}

#[test]
fn negative_other_value_and_leading_zero_hsn() {
    let mut e = sample();
    e.other_value = Paise::new(-5_000);
    e.tot_inv_value = Paise::new(1_175_000);
    e.items[0].hsn = Hsn::parse("0101").unwrap();
    let json = ewb_to_json(&e).unwrap();
    // provenance: derived. Round-off of -50.00: 10,000.00 + 1,800.00 - 50.00 = 11,750.00.
    // The portal's hsnCode is numeric, so 0101 is written as 101 (a JSON number cannot start
    // with 0).
    assert!(json.contains(r#""otherValue":-50.00"#), "{json}");
    assert!(json.contains(r#""totInvValue":11750.00"#), "{json}");
    assert!(json.contains(r#""hsnCode":101,"#), "{json}");
}

#[test]
fn sample_bill_is_valid() {
    assert!(validate_ewb(&sample()).is_ok());
}

#[test]
fn distance_limit() {
    // provenance: derived (limit recalled from the NIC documentation, pending official check):
    // 0 asks the portal to compute, 4000 km is the largest accepted, 4001 is not.
    for km in [0, 1, 4000] {
        let mut e = sample();
        e.transport.distance_km = km;
        assert!(validate_ewb(&e).is_ok(), "{km} km should pass");
    }
    let mut e = sample();
    e.transport.distance_km = 4001;
    assert_eq!(fields(validate_ewb(&e)), ["transDistance"]);
    assert!(matches!(
        ewb_to_json(&e),
        Err(FormatError::Field { field, .. }) if field == "transDistance"
    ));
}

#[test]
fn vehicle_number_shapes() {
    // provenance: derived from the shapes stated in `is_valid_vehicle_no`: state series is
    // 2 letters + 2 digits + 0..3 letters + 4 digits; Bharat series is 2 digits + BH + 4 digits
    // + 1..2 letters. Capital letters only, no separators.
    for ok in [
        "KA01AB1234",
        "MH12A1234",
        "DL01C1234",
        "KA011234",
        "TN09ABC1234",
        "22BH1234AA",
        "22BH1234A",
    ] {
        assert!(is_valid_vehicle_no(ok), "{ok} should be valid");
        assert!(validate_ewb(&with_vehicle(ok)).is_ok(), "{ok} in a bill");
    }
    for bad in [
        "",
        "ka01ab1234",
        "KA 01 AB 1234",
        "KA-01-AB-1234",
        "KA01AB123",
        "KA01AB12345",
        "KA01ABCD1234",
        "KA1AB1234",
        "1234KA01AB",
        "22BH1234",
        "22BH1234AAA",
        "22bh1234AA",
    ] {
        assert!(!is_valid_vehicle_no(bad), "{bad:?} should be invalid");
        assert_eq!(
            fields(validate_ewb(&with_vehicle(bad))),
            ["vehicleNo"],
            "{bad:?} in a bill"
        );
    }
}

#[test]
fn part_b_requirements_by_mode() {
    // provenance: derived from the card's Part B rules: road needs a vehicle; rail/air/ship need
    // a transport document number and date; no mode needs a transporter id (Part A only).
    let mut road = sample();
    road.transport.vehicle_no = None;
    road.transport.vehicle_type = None;
    assert_eq!(fields(validate_ewb(&road)), ["vehicleNo"]);

    let mut rail = sample();
    rail.transport.mode = Some(TransMode::Rail);
    rail.transport.vehicle_no = None;
    rail.transport.vehicle_type = None;
    assert_eq!(fields(validate_ewb(&rail)), ["transDocNo", "transDocDate"]);
    rail.transport.doc_no = Some("RR123".to_owned());
    rail.transport.doc_date = Some(Date::from_calendar_date(2026, Month::November, 21).unwrap());
    assert!(validate_ewb(&rail).is_ok());

    let mut part_a = sample();
    part_a.transport.mode = None;
    part_a.transport.vehicle_no = None;
    part_a.transport.vehicle_type = None;
    assert_eq!(fields(validate_ewb(&part_a)), ["transporterId"]);
    part_a.transport.transporter_id = Some("29AAACT1111A1Z5".to_owned());
    assert!(validate_ewb(&part_a).is_ok());
    let json = ewb_to_json(&part_a).unwrap();
    assert!(!json.contains("transMode"), "{json}");
    assert!(
        json.contains(r#""transporterId":"29AAACT1111A1Z5""#),
        "{json}"
    );

    let mut air = sample();
    air.transport.mode = Some(TransMode::Air);
    assert_eq!(
        fields(validate_ewb(&air)),
        ["vehicleNo", "transDocNo", "transDocDate"]
    );
}

#[test]
fn vehicle_type_required_with_vehicle() {
    let mut e = sample();
    e.transport.vehicle_type = None;
    assert_eq!(fields(validate_ewb(&e)), ["vehicleType"]);
}

#[test]
fn totals_must_agree() {
    // provenance: derived. Items add to 10,000.00; header says 10,000.01 (1,000,001 paise).
    let mut e = sample();
    e.total_value = Paise::new(1_000_001);
    e.tot_inv_value = Paise::new(1_180_001);
    assert_eq!(fields(validate_ewb(&e)), ["totalValue"]);

    // Total invoice value must be 10,000.00 + 1,800.00 = 11,800.00, not 11,900.00.
    let mut e = sample();
    e.tot_inv_value = Paise::new(1_190_000);
    assert_eq!(fields(validate_ewb(&e)), ["totInvValue"]);
}

#[test]
fn igst_with_cgst_is_rejected() {
    let mut e = sample();
    // provenance: derived. 10,000.00 + 900.00 CGST + 1,800.00 IGST = 12,700.00 (1,270,000 paise)
    // adds up, but IGST and CGST are never charged on the same supply, so it is still rejected.
    e.cgst_value = Paise::new(90_000);
    e.tot_inv_value = Paise::new(1_270_000);
    assert_eq!(fields(validate_ewb(&e)), ["igstValue"]);

    let mut e = sample();
    e.items[0].cgst_rate = Decimal::from(9);
    assert_eq!(fields(validate_ewb(&e)), ["itemList[0].igstRate"]);
}

#[test]
fn item_rules() {
    let mut e = sample();
    e.items.clear();
    e.total_value = Paise::ZERO;
    e.tot_inv_value = Paise::new(180_000);
    assert!(fields(validate_ewb(&e)).contains(&"itemList".to_owned()));

    let mut e = sample();
    e.items[0].quantity = Decimal::ZERO;
    e.items[0].qty_unit = "nos".to_owned();
    e.items[0].product_name = " ".to_owned();
    e.items[0].igst_rate = Decimal::from(101);
    assert_eq!(
        fields(validate_ewb(&e)),
        [
            "itemList[0].productName",
            "itemList[0].quantity",
            "itemList[0].qtyUnit",
            "itemList[0].igstRate"
        ]
    );
}

#[test]
fn header_rules() {
    let mut e = sample();
    e.doc_no = "INV/2026-27/000000001".to_owned(); // 21 characters
    assert_eq!(fields(validate_ewb(&e)), ["docNo"]);
    e.doc_no = "INV 0001".to_owned();
    assert_eq!(fields(validate_ewb(&e)), ["docNo"]);

    let mut e = sample();
    e.sub_supply_type = SubSupplyType::Others;
    assert_eq!(fields(validate_ewb(&e)), ["subSupplyDesc"]);
    e.sub_supply_desc = Some("Samples for testing".to_owned());
    assert!(validate_ewb(&e).is_ok());

    let mut e = sample();
    e.sub_supply_type = SubSupplyType::Export;
    e.supply_type = SupplyType::Inward;
    assert_eq!(fields(validate_ewb(&e)), ["subSupplyType"]);
    let mut e = sample();
    e.sub_supply_type = SubSupplyType::Import;
    assert_eq!(fields(validate_ewb(&e)), ["subSupplyType"]);
}

#[test]
fn party_rules_and_regular_transaction_states() {
    let mut e = sample();
    e.from.pincode = 56_001;
    e.to.pincode = 1_000_000;
    e.to.place = String::new();
    assert_eq!(
        fields(validate_ewb(&e)),
        ["fromPincode", "toPlace", "toPincode"]
    );

    let mut e = sample();
    e.to.actual_state_code = StateCode::new(29).unwrap();
    assert_eq!(fields(validate_ewb(&e)), ["actToStateCode"]);
    e.transaction_type = TransactionType::BillToShipTo;
    assert!(validate_ewb(&e).is_ok());
}

#[test]
fn every_problem_is_reported_not_just_the_first() {
    let mut e = sample();
    e.transport.distance_km = 5000;
    e.transport.vehicle_no = Some("bad".to_owned());
    e.doc_no = String::new();
    let f = fields(validate_ewb(&e));
    assert_eq!(f, ["docNo", "transDistance", "vehicleNo"]);
    // The writer refuses and reports the first of them.
    assert!(matches!(
        ewb_to_json(&e),
        Err(FormatError::Field { field, .. }) if field == "docNo"
    ));
}

#[test]
fn sub_supply_and_transaction_codes() {
    // provenance: pending official check; recalled NIC code list 1..12 in this order.
    let all = [
        SubSupplyType::Supply,
        SubSupplyType::Import,
        SubSupplyType::Export,
        SubSupplyType::JobWork,
        SubSupplyType::ForOwnUse,
        SubSupplyType::JobWorkReturns,
        SubSupplyType::SalesReturn,
        SubSupplyType::Others,
        SubSupplyType::SkdCkd,
        SubSupplyType::LineSales,
        SubSupplyType::RecipientNotKnown,
        SubSupplyType::ExhibitionOrFairs,
    ];
    let codes: Vec<u8> = all.iter().map(|s| s.code()).collect();
    assert_eq!(codes, (1..=12).collect::<Vec<u8>>());
    assert_eq!(TransactionType::Combination.code(), 4);
    assert_eq!(DocType::DeliveryChallan.code(), "CHL");
}
