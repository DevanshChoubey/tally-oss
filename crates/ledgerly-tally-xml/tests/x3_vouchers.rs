//! Card X3 tests: voucher parsing.
//!
//! Provenance. The sign convention (debit line = `ISDEEMEDPOSITIVE` Yes and a negative `AMOUNT`,
//! credit line = No and a positive `AMOUNT`) is `derived` from R8 section 2.3(e)
//! (`docs/research/08-data-interop-editions.md`, vendor sample shape). The inner tag names of
//! inventory, batch and bill allocations (`STOCKITEMNAME`, `BILLEDQTY`, `GODOWNNAME`, `BATCHNAME`,
//! `BILLTYPE`, `NAME`) are NOT in R8: the fixtures below are hand-built shapes, so assertions on
//! them are `derived` from the fixture text only, and the tag names stay `pending` an `observed`
//! capture. Expected values are hand arithmetic written next to each assertion.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]

use ledgerly_core::{Date, DrCr, Month, Paise};
use ledgerly_tally_xml::TallyXmlError;
use ledgerly_tally_xml::vouchers::parse_vouchers_xml;

/// R8 2.3(e) sales voucher, wrapped in the response envelope shape.
const SALES: &str = r#"<ENVELOPE><HEADER><VERSION>1</VERSION></HEADER><BODY><DATA><TALLYMESSAGE>
  <VOUCHER VCHTYPE="Sales" ACTION="Create">
    <GUID>guid-0001</GUID>
    <ALTERID>42</ALTERID>
    <DATE>20261115</DATE>
    <VOUCHERTYPENAME>Sales</VOUCHERTYPENAME>
    <VOUCHERNUMBER>S-0042</VOUCHERNUMBER>
    <PARTYLEDGERNAME>Sharma Hardware</PARTYLEDGERNAME>
    <NARRATION>Bolts &amp; nuts</NARRATION>
    <PERSISTEDVIEW>Accounting Voucher View</PERSISTEDVIEW>
    <ALLLEDGERENTRIES.LIST>
      <LEDGERNAME>Sharma Hardware</LEDGERNAME>
      <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
      <AMOUNT>-11800.00</AMOUNT>
      <BILLALLOCATIONS.LIST>
        <NAME>S-0042</NAME>
        <BILLTYPE>New Ref</BILLTYPE>
        <AMOUNT>-11800.00</AMOUNT>
      </BILLALLOCATIONS.LIST>
    </ALLLEDGERENTRIES.LIST>
    <ALLLEDGERENTRIES.LIST>
      <LEDGERNAME>Sales @18%</LEDGERNAME>
      <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
      <AMOUNT>10000.00</AMOUNT>
    </ALLLEDGERENTRIES.LIST>
    <ALLLEDGERENTRIES.LIST>
      <LEDGERNAME>Output IGST</LEDGERNAME>
      <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
      <AMOUNT>1800.00</AMOUNT>
    </ALLLEDGERENTRIES.LIST>
  </VOUCHER>
</TALLYMESSAGE></DATA></BODY></ENVELOPE>"#;

fn voucher_with_line(line: &str) -> String {
    format!(
        "<ENVELOPE><TALLYMESSAGE><VOUCHER><DATE>20261115</DATE>\
         <VOUCHERTYPENAME>Journal</VOUCHERTYPENAME>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash</LEDGERNAME>{line}</ALLLEDGERENTRIES.LIST>\
         </VOUCHER></TALLYMESSAGE></ENVELOPE>"
    )
}

#[test]
fn all_ledger_entries_list_parsed_with_header_fields() {
    let (vs, warnings) = parse_vouchers_xml(SALES).unwrap();
    assert!(matches!(warnings.as_slice(), []));
    assert_eq!(vs.len(), 1);
    let v = &vs[0];
    assert_eq!(v.guid.as_deref(), Some("guid-0001"));
    assert_eq!(v.alter_id, Some(42));
    assert_eq!(v.voucher_type, "Sales");
    // derived: <DATE>20261115</DATE> is YYYYMMDD = 15 November 2026.
    assert_eq!(
        v.date,
        Date::from_calendar_date(2026, Month::November, 15).unwrap()
    );
    assert_eq!(v.number.as_deref(), Some("S-0042"));
    assert_eq!(v.party.as_deref(), Some("Sharma Hardware"));
    // derived: `&amp;` in the file is the single character `&`.
    assert_eq!(v.narration.as_deref(), Some("Bolts & nuts"));
    assert_eq!(v.ledger_entries.len(), 3);
    let names: Vec<&str> = v
        .ledger_entries
        .iter()
        .map(|e| e.ledger_name.as_str())
        .collect();
    assert_eq!(names, ["Sharma Hardware", "Sales @18%", "Output IGST"]);
    // PERSISTEDVIEW is not modelled: it must be kept, not dropped.
    assert!(v.extra.iter().any(|n| n.name == "PERSISTEDVIEW"));
}

#[test]
fn negative_amount_is_debit_and_positive_is_credit() {
    let (vs, _) = parse_vouchers_xml(SALES).unwrap();
    let e = &vs[0].ledger_entries;
    // derived (R8 2.3(e)): -11800.00 with ISDEEMEDPOSITIVE=Yes is a debit.
    // 11800.00 rupees x 100 = 1,180,000 paise.
    assert_eq!(e[0].side, DrCr::Dr);
    assert_eq!(e[0].amount, Paise::new(1_180_000));
    // 10000.00 x 100 = 1,000,000 paise credit.
    assert_eq!(e[1].side, DrCr::Cr);
    assert_eq!(e[1].amount, Paise::new(1_000_000));
    // 1800.00 x 100 = 180,000 paise credit.
    assert_eq!(e[2].side, DrCr::Cr);
    assert_eq!(e[2].amount, Paise::new(180_000));
    // derived: the voucher balances: 1,180,000 Dr = 1,000,000 + 180,000 Cr.
    assert_eq!(1_180_000, 1_000_000 + 180_000);
}

#[test]
fn amounts_parse_to_paise_exactly() {
    for (text, paise) in [
        ("11800.5", 1_180_050), // 11800.50 x 100
        ("0.07", 7),            // 0.07 x 100
        ("1234567.89", 123_456_789),
        ("10.000", 1_000), // trailing zero beyond 2 dp is exact: 10 x 100
        ("5", 500),
    ] {
        let xml = voucher_with_line(&format!(
            "<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>{text}</AMOUNT>"
        ));
        let (vs, _) = parse_vouchers_xml(&xml).unwrap();
        assert_eq!(vs[0].ledger_entries[0].amount, Paise::new(paise), "{text}");
        assert_eq!(vs[0].ledger_entries[0].side, DrCr::Cr, "{text}");
    }
}

#[test]
fn amount_needing_rounding_is_an_error_not_a_rounded_value() {
    // 10.005 rupees is 1000.5 paise: not representable, so never silently rounded.
    let xml = voucher_with_line("<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>10.005</AMOUNT>");
    assert!(matches!(
        parse_vouchers_xml(&xml),
        Err(TallyXmlError::Value { tag, .. }) if tag == "AMOUNT"
    ));
}

#[test]
fn garbage_and_foreign_currency_amounts_are_errors() {
    for text in ["abc", "1,180.00", "$ 10 = Rs 830", ""] {
        let xml = voucher_with_line(&format!(
            "<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>{text}</AMOUNT>"
        ));
        assert!(parse_vouchers_xml(&xml).is_err(), "{text:?}");
    }
}

#[test]
fn flag_contradicting_sign_is_an_error() {
    // R8: Yes goes with a negative amount, No with a positive one. A contradiction is refused.
    for (flag, amount) in [("Yes", "100.00"), ("No", "-100.00")] {
        let xml = voucher_with_line(&format!(
            "<ISDEEMEDPOSITIVE>{flag}</ISDEEMEDPOSITIVE><AMOUNT>{amount}</AMOUNT>"
        ));
        assert!(
            matches!(parse_vouchers_xml(&xml), Err(TallyXmlError::Value { .. })),
            "{flag} {amount}"
        );
    }
}

#[test]
fn sign_decides_when_flag_absent() {
    let xml = voucher_with_line("<AMOUNT>-250.00</AMOUNT>");
    let (vs, _) = parse_vouchers_xml(&xml).unwrap();
    // derived (R8): negative = Dr; 250.00 x 100 = 25,000 paise.
    assert_eq!(vs[0].ledger_entries[0].side, DrCr::Dr);
    assert_eq!(vs[0].ledger_entries[0].amount, Paise::new(25_000));
}

#[test]
fn zero_amount_with_flag_takes_side_from_flag_and_without_flag_is_an_error() {
    let yes = voucher_with_line("<ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>0.00</AMOUNT>");
    let (vs, _) = parse_vouchers_xml(&yes).unwrap();
    assert_eq!(vs[0].ledger_entries[0].side, DrCr::Dr);
    assert_eq!(vs[0].ledger_entries[0].amount, Paise::ZERO);
    assert!(parse_vouchers_xml(&voucher_with_line("<AMOUNT>0.00</AMOUNT>")).is_err());
}

#[test]
fn bill_allocations_list_parsed() {
    let (vs, _) = parse_vouchers_xml(SALES).unwrap();
    let bills = &vs[0].ledger_entries[0].bill_allocations;
    assert_eq!(bills.len(), 1);
    assert_eq!(bills[0].name, "S-0042");
    assert_eq!(bills[0].bill_type.as_deref(), Some("New Ref"));
    // As written in the file, sign kept: -11800.00 x 100 = -1,180,000 paise. The sign convention
    // for bill lines is not documented in R8, so it is not interpreted (pending).
    assert_eq!(bills[0].amount, Paise::new(-1_180_000));
    assert!(matches!(
        vs[0].ledger_entries[1].bill_allocations.as_slice(),
        []
    ));
}

const INVOICE: &str = r#"<ENVELOPE><TALLYMESSAGE><VOUCHER VCHTYPE="Sales">
    <DATE>20261116</DATE>
    <VOUCHERTYPENAME>Sales</VOUCHERTYPENAME>
    <ISINVOICE>Yes</ISINVOICE>
    <ALLINVENTORYENTRIES.LIST>
      <STOCKITEMNAME>Bolt M8</STOCKITEMNAME>
      <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
      <RATE>1000.00/Nos</RATE>
      <BILLEDQTY>10 Nos</BILLEDQTY>
      <ACTUALQTY>10 Nos</ACTUALQTY>
      <AMOUNT>10000.00</AMOUNT>
      <BATCHALLOCATIONS.LIST>
        <GODOWNNAME>Main Location</GODOWNNAME>
        <BATCHNAME>B-1</BATCHNAME>
        <AMOUNT>6000.00</AMOUNT>
        <BILLEDQTY>6 Nos</BILLEDQTY>
        <ACTUALQTY>6 Nos</ACTUALQTY>
      </BATCHALLOCATIONS.LIST>
      <BATCHALLOCATIONS.LIST>
        <GODOWNNAME>Main Location</GODOWNNAME>
        <BATCHNAME>B-2</BATCHNAME>
        <AMOUNT>4000.00</AMOUNT>
        <BILLEDQTY>4 Nos</BILLEDQTY>
        <ACTUALQTY>4 Nos</ACTUALQTY>
      </BATCHALLOCATIONS.LIST>
      <ACCOUNTINGALLOCATIONS.LIST><LEDGERNAME>Sales @18%</LEDGERNAME></ACCOUNTINGALLOCATIONS.LIST>
    </ALLINVENTORYENTRIES.LIST>
    <LEDGERENTRIES.LIST>
      <LEDGERNAME>Sharma Hardware</LEDGERNAME>
      <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
      <AMOUNT>-10000.00</AMOUNT>
    </LEDGERENTRIES.LIST>
  </VOUCHER></TALLYMESSAGE></ENVELOPE>"#;

#[test]
fn inventory_entries_list_parsed() {
    let (vs, _) = parse_vouchers_xml(INVOICE).unwrap();
    let v = &vs[0];
    assert_eq!(v.inventory_entries.len(), 1);
    let i = &v.inventory_entries[0];
    assert_eq!(i.stock_item, "Bolt M8");
    // derived from the fixture: 10 Nos at 1000.00/Nos = 10,000.00 = 1,000,000 paise.
    assert_eq!(i.billed_qty.as_ref().unwrap().unit, "Nos");
    assert_eq!(i.billed_qty.as_ref().unwrap().qty.0.to_string(), "10");
    assert_eq!(i.rate.as_ref().unwrap().unit, "Nos");
    assert_eq!(i.rate.as_ref().unwrap().rate.0.to_string(), "1000.00");
    assert_eq!(i.amount, Some(Paise::new(1_000_000)));
    // The accounting-view tag LEDGERENTRIES.LIST also parses, in document order.
    assert_eq!(v.ledger_entries.len(), 1);
    assert_eq!(v.ledger_entries[0].side, DrCr::Dr);
    // ACCOUNTINGALLOCATIONS.LIST is not modelled: kept.
    assert!(
        i.extra
            .iter()
            .any(|n| n.name == "ACCOUNTINGALLOCATIONS.LIST")
    );
}

#[test]
fn batch_allocations_list_parsed_and_sum_to_line() {
    let (vs, _) = parse_vouchers_xml(INVOICE).unwrap();
    let b = &vs[0].inventory_entries[0].batches;
    assert_eq!(b.len(), 2);
    assert_eq!(b[0].godown.as_deref(), Some("Main Location"));
    assert_eq!(b[0].batch.as_deref(), Some("B-1"));
    assert_eq!(b[0].amount, Some(Paise::new(600_000))); // 6000.00 x 100
    assert_eq!(b[1].batch.as_deref(), Some("B-2"));
    assert_eq!(b[1].amount, Some(Paise::new(400_000))); // 4000.00 x 100
    assert_eq!(b[1].billed_qty.as_ref().unwrap().qty.0.to_string(), "4");
    // derived: 6,000.00 + 4,000.00 = 10,000.00 = the line amount.
    assert_eq!(600_000 + 400_000, 1_000_000);
}

#[test]
fn unknown_tags_are_preserved_with_nesting_and_attributes() {
    let xml = r#"<ENVELOPE><TALLYMESSAGE><VOUCHER>
        <DATE>20261115</DATE><VOUCHERTYPENAME>Journal</VOUCHERTYPENAME>
        <UDF:VEHICLE TYPE="String">MH12AB1234</UDF:VEHICLE>
        <EWAYDETAILS.LIST><NO>1</NO><NO>2</NO></EWAYDETAILS.LIST>
      </VOUCHER></TALLYMESSAGE></ENVELOPE>"#;
    let (vs, _) = parse_vouchers_xml(xml).unwrap();
    let x = &vs[0].extra;
    assert_eq!(x.len(), 2);
    assert_eq!(x[0].name, "UDF:VEHICLE");
    assert_eq!(x[0].text, "MH12AB1234");
    assert_eq!(x[0].attrs, [("TYPE".to_owned(), "String".to_owned())]);
    assert_eq!(x[1].children.len(), 2);
}

#[test]
fn several_vouchers_keep_document_order() {
    let xml = format!(
        "<ENVELOPE>{}{}</ENVELOPE>",
        "<TALLYMESSAGE><VOUCHER><DATE>20261101</DATE><VOUCHERTYPENAME>A</VOUCHERTYPENAME></VOUCHER></TALLYMESSAGE>",
        "<TALLYMESSAGE><VOUCHER><DATE>20261102</DATE><VOUCHERTYPENAME>B</VOUCHERTYPENAME></VOUCHER></TALLYMESSAGE>"
    );
    let (vs, _) = parse_vouchers_xml(&xml).unwrap();
    let types: Vec<&str> = vs.iter().map(|v| v.voucher_type.as_str()).collect();
    assert_eq!(types, ["A", "B"]);
}

#[test]
fn missing_date_or_type_and_bad_date_are_errors() {
    let no_date = "<ENVELOPE><VOUCHER><VOUCHERTYPENAME>A</VOUCHERTYPENAME></VOUCHER></ENVELOPE>";
    assert!(matches!(
        parse_vouchers_xml(no_date),
        Err(TallyXmlError::Missing { tag, .. }) if tag == "DATE"
    ));
    let no_type = "<ENVELOPE><VOUCHER><DATE>20261101</DATE></VOUCHER></ENVELOPE>";
    assert!(matches!(
        parse_vouchers_xml(no_type),
        Err(TallyXmlError::Missing { .. })
    ));
    // 31 November does not exist.
    let bad = "<ENVELOPE><VOUCHER><DATE>20261131</DATE><VOUCHERTYPENAME>A</VOUCHERTYPENAME></VOUCHER></ENVELOPE>";
    assert!(matches!(
        parse_vouchers_xml(bad),
        Err(TallyXmlError::Value { tag, .. }) if tag == "DATE"
    ));
}

#[test]
fn voucher_type_falls_back_to_vchtype_attribute() {
    let xml = r#"<ENVELOPE><VOUCHER VCHTYPE="Receipt"><DATE>20261101</DATE></VOUCHER></ENVELOPE>"#;
    let (vs, _) = parse_vouchers_xml(xml).unwrap();
    assert_eq!(vs[0].voucher_type, "Receipt");
}

#[test]
fn truncated_document_is_an_error_with_offset() {
    let cut = &SALES[..SALES.len() - 40];
    assert!(matches!(
        parse_vouchers_xml(cut),
        Err(TallyXmlError::Truncated { offset }) if offset > 0
    ));
}

#[test]
fn duplicate_scalar_tag_is_an_error() {
    let xml = "<ENVELOPE><VOUCHER><DATE>20261101</DATE><DATE>20261102</DATE>\
               <VOUCHERTYPENAME>A</VOUCHERTYPENAME></VOUCHER></ENVELOPE>";
    assert!(matches!(
        parse_vouchers_xml(xml),
        Err(TallyXmlError::Value { .. })
    ));
}

#[test]
fn no_vouchers_is_an_empty_list() {
    let (vs, _) = parse_vouchers_xml("<ENVELOPE><BODY/></ENVELOPE>").unwrap();
    assert!(matches!(vs.as_slice(), []));
}

#[test]
fn deep_nesting_is_refused_not_a_stack_overflow() {
    let xml = format!(
        "<ENVELOPE><VOUCHER><DATE>20261101</DATE><VOUCHERTYPENAME>A</VOUCHERTYPENAME>{}{}</VOUCHER></ENVELOPE>",
        "<X>".repeat(5000),
        "</X>".repeat(5000)
    );
    assert!(parse_vouchers_xml(&xml).is_err());
}

mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // derived: an amount written as whole rupees r and paise p (two digits) is r*100+p paise.
        #[test]
        fn rupees_and_paise_text_round_trips(r in 0i64..1_000_000_000, p in 0i64..100) {
            let text = format!("{r}.{p:02}");
            let xml = voucher_with_line(&format!(
                "<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>{text}</AMOUNT>"
            ));
            let (vs, _) = parse_vouchers_xml(&xml).unwrap();
            prop_assert_eq!(vs[0].ledger_entries[0].amount, Paise::new(r * 100 + p));
        }

        #[test]
        fn arbitrary_text_never_panics(s in "\\PC{0,200}") {
            let _ = parse_vouchers_xml(&s);
        }
    }
}
