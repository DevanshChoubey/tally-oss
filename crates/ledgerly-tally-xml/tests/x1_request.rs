//! Card X1 tests: export request builders.
//!
//! Provenance. Every expected document below is written by hand from the documented envelope in
//! `docs/research/08-data-interop-editions.md` sections 2.2 and 2.3 (examples a and b: HEADER with
//! VERSION, TALLYREQUEST, TYPE, ID; BODY/DESC/STATICVARIABLES with SVCURRENTCOMPANY, SVFROMDATE,
//! SVTODATE and SVEXPORTFORMAT; inline TDL COLLECTION with TYPE and FETCH; dates as YYYYMMDD,
//! as in example a: 20260401 to 20270331). All expectations are `derived` from that shape.
//! Not observed: the report name `List of Companies`, the object names in COLLECTION/TYPE and the
//! FETCH field names (research 08 marks every field name unverified until a real export is
//! captured), so a captured export must confirm them before they count as verified.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Date, Month};
use ledgerly_tally_xml::TallyXmlError;
use ledgerly_tally_xml::envelope::parse_envelope;
use ledgerly_tally_xml::masters::RawElement;
use ledgerly_tally_xml::request::{
    MasterCollection, list_companies_request, masters_request, try_masters_request,
    try_vouchers_request, vouchers_request,
};

const LIST_COMPANIES: &str = r"<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>List of Companies</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
    </DESC>
  </BODY>
</ENVELOPE>
";

const LEDGERS_ACME: &str = r#"<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>LedgerlyLedgers</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVCURRENTCOMPANY>Acme Traders</SVCURRENTCOMPANY>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
      <TDL>
        <TDLMESSAGE>
          <COLLECTION NAME="LedgerlyLedgers">
            <TYPE>Ledger</TYPE>
            <FETCH>GUID, AlterID, Name, Parent, OpeningBalance, PartyGSTIN</FETCH>
          </COLLECTION>
        </TDLMESSAGE>
      </TDL>
    </DESC>
  </BODY>
</ENVELOPE>
"#;

const STOCK_ITEMS_ACME: &str = r#"<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>LedgerlyStockItems</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVCURRENTCOMPANY>Acme Traders</SVCURRENTCOMPANY>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
      <TDL>
        <TDLMESSAGE>
          <COLLECTION NAME="LedgerlyStockItems">
            <TYPE>Stock Item</TYPE>
            <FETCH>GUID, AlterID, Name, Parent</FETCH>
          </COLLECTION>
        </TDLMESSAGE>
      </TDL>
    </DESC>
  </BODY>
</ENVELOPE>
"#;

const VOUCHERS_FY: &str = r#"<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>LedgerlyVouchers</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVCURRENTCOMPANY>Acme Traders</SVCURRENTCOMPANY>
        <SVFROMDATE>20260401</SVFROMDATE>
        <SVTODATE>20270331</SVTODATE>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
      <TDL>
        <TDLMESSAGE>
          <COLLECTION NAME="LedgerlyVouchers">
            <TYPE>Voucher</TYPE>
            <FETCH>GUID, AlterID, Date, VoucherTypeName, VoucherNumber, PartyLedgerName, Narration, AllLedgerEntries.*, AllInventoryEntries.*</FETCH>
          </COLLECTION>
        </TDLMESSAGE>
      </TDL>
    </DESC>
  </BODY>
</ENVELOPE>
"#;

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).unwrap()
}

fn child<'a>(element: &'a RawElement, name: &str) -> &'a RawElement {
    element
        .children
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no <{name}> in <{}>", element.name))
}

#[test]
fn list_of_companies_matches_the_documented_envelope() {
    // provenance: derived: research 08 section 2.2 envelope and 2.3 example a (no company is
    // selected because the answer names the companies).
    assert_eq!(list_companies_request(), LIST_COMPANIES);
}

#[test]
fn ledger_masters_request_matches_the_documented_collection_envelope() {
    // provenance: derived: research 08 section 2.3 example b (inline COLLECTION with FETCH);
    // the Ledger field list is the one printed in example b.
    assert_eq!(
        masters_request("Acme Traders", MasterCollection::Ledgers),
        LEDGERS_ACME
    );
}

#[test]
fn stock_item_masters_request_uses_the_spaced_object_name() {
    // provenance: derived: same shape as example b; object name `Stock Item` is unobserved.
    assert_eq!(
        masters_request("Acme Traders", MasterCollection::StockItems),
        STOCK_ITEMS_ACME
    );
}

#[test]
fn vouchers_request_for_a_date_range_matches_the_documented_envelope() {
    // provenance: derived: research 08 example a uses SVFROMDATE 20260401 and SVTODATE 20270331
    // for the year 1 April 2026 to 31 March 2027; dates are YYYYMMDD.
    let text = vouchers_request(
        "Acme Traders",
        date(2026, Month::April, 1),
        date(2027, Month::March, 31),
    );
    assert_eq!(text, VOUCHERS_FY);
}

#[test]
fn dates_are_zero_padded_yyyymmdd() {
    // provenance: derived: 5 January 2026 is 2026, 01, 05 written without separators.
    let text = vouchers_request(
        "Acme Traders",
        date(2026, Month::January, 5),
        date(2026, Month::January, 9),
    );
    assert!(text.contains("<SVFROMDATE>20260105</SVFROMDATE>"));
    assert!(text.contains("<SVTODATE>20260109</SVTODATE>"));
}

#[test]
fn every_master_kind_has_its_own_collection_and_object_name() {
    // provenance: derived: the nine kinds of the frozen TallyMaster enum (card X2), each with
    // the upper-case element name Tally exports it under.
    let expected = [
        (MasterCollection::Groups, "LedgerlyGroups", "Group"),
        (MasterCollection::Ledgers, "LedgerlyLedgers", "Ledger"),
        (
            MasterCollection::StockItems,
            "LedgerlyStockItems",
            "Stock Item",
        ),
        (MasterCollection::Units, "LedgerlyUnits", "Unit"),
        (MasterCollection::Godowns, "LedgerlyGodowns", "Godown"),
        (
            MasterCollection::VoucherTypes,
            "LedgerlyVoucherTypes",
            "Voucher Type",
        ),
        (
            MasterCollection::CostCentres,
            "LedgerlyCostCentres",
            "Cost Centre",
        ),
        (
            MasterCollection::CostCategories,
            "LedgerlyCostCategories",
            "Cost Category",
        ),
        (
            MasterCollection::StockGroups,
            "LedgerlyStockGroups",
            "Stock Group",
        ),
    ];
    for (kind, name, object) in expected {
        assert_eq!(kind.collection_name(), name);
        assert_eq!(kind.object_type(), object);
        let text = masters_request("Acme Traders", kind);
        assert!(text.contains(&format!("<ID>{name}</ID>")));
        assert!(text.contains(&format!("<COLLECTION NAME=\"{name}\">")));
        assert!(text.contains(&format!("<TYPE>{object}</TYPE>")));
    }
    assert_eq!(MasterCollection::ALL.len(), expected.len());
    for (kind, _, _) in expected {
        assert!(MasterCollection::ALL.contains(&kind));
    }
}

#[test]
fn requests_parse_back_through_the_envelope_parser() {
    // provenance: derived: a request is itself an envelope, so the HEADER fields written are the
    // ones read back (VERSION 1, Export, Collection, the collection name as ID).
    for kind in MasterCollection::ALL {
        let text = masters_request("Acme Traders", kind);
        let (envelope, warnings) = parse_envelope(&text).unwrap();
        assert_eq!(warnings.len(), 0);
        assert_eq!(envelope.header.version.as_deref(), Some("1"));
        assert_eq!(envelope.header.request.as_deref(), Some("Export"));
        assert_eq!(envelope.header.kind.as_deref(), Some("Collection"));
        assert_eq!(envelope.header.id.as_deref(), Some(kind.collection_name()));
        assert_eq!(envelope.messages.len(), 0);
        let [desc] = envelope.extras.as_slice() else {
            panic!("expected one DESC, got {:?}", envelope.extras);
        };
        assert_eq!(desc.name, "DESC");
        let vars = child(desc, "STATICVARIABLES");
        assert_eq!(child(vars, "SVCURRENTCOMPANY").text, "Acme Traders");
        let collection = child(child(child(desc, "TDL"), "TDLMESSAGE"), "COLLECTION");
        assert_eq!(
            collection.attributes,
            [("NAME".to_owned(), kind.collection_name().to_owned())]
        );
        assert_eq!(child(collection, "TYPE").text, kind.object_type());
        assert_eq!(child(collection, "FETCH").text, kind.fetch_fields());
    }
}

#[test]
fn company_names_are_escaped_and_read_back_unchanged() {
    // provenance: derived: XML 1.0 requires & and < to be escaped in text; `&amp;` is `&`.
    let company = "Sharma & Sons <Pvt> \"Ltd\" 'A'";
    let text = masters_request(company, MasterCollection::Groups);
    assert!(text.contains(
        "<SVCURRENTCOMPANY>Sharma &amp; Sons &lt;Pvt&gt; \"Ltd\" 'A'</SVCURRENTCOMPANY>"
    ));
    let (envelope, warnings) = parse_envelope(&text).unwrap();
    assert_eq!(warnings.len(), 0);
    let desc = envelope.extras.first().unwrap();
    assert_eq!(
        child(child(desc, "STATICVARIABLES"), "SVCURRENTCOMPANY").text,
        company
    );
}

#[test]
fn non_ascii_company_names_pass_through() {
    // provenance: derived: Unicode is legal XML text and is not escaped.
    let company = "शर्मा हार्डवेयर ₹";
    let text = masters_request(company, MasterCollection::Ledgers);
    assert!(text.contains("<SVCURRENTCOMPANY>शर्मा हार्डवेयर ₹</SVCURRENTCOMPANY>"));
}

#[test]
fn a_character_xml_cannot_carry_is_never_written_raw() {
    // provenance: derived: XML 1.0 production Char excludes U+0004; the builder writes the
    // numeric reference `&#4;`, which a receiver rejects as malformed.
    let text = masters_request("Ac\u{4}me", MasterCollection::Groups);
    assert!(text.contains("<SVCURRENTCOMPANY>Ac&#4;me</SVCURRENTCOMPANY>"));
    assert!(!text.contains('\u{4}'));
}

#[test]
fn try_masters_request_rejects_bad_company_names() {
    // provenance: derived: an empty name selects no company; U+0004 is not an XML character.
    let empty = try_masters_request("  ", MasterCollection::Groups).unwrap_err();
    assert!(matches!(
        empty,
        TallyXmlError::Value { ref tag, .. } if tag == "SVCURRENTCOMPANY"
    ));
    let control = try_masters_request("Ac\u{4}me", MasterCollection::Groups).unwrap_err();
    let TallyXmlError::Value { tag, reason, .. } = control else {
        panic!("expected a Value error");
    };
    assert_eq!(tag, "SVCURRENTCOMPANY");
    assert!(reason.contains("U+0004"));
    assert_eq!(
        try_masters_request("Acme Traders", MasterCollection::Ledgers).unwrap(),
        LEDGERS_ACME
    );
}

#[test]
fn try_vouchers_request_checks_the_date_range() {
    // provenance: derived: a range ends on or after its start; one day (from == to) is valid.
    let april = date(2026, Month::April, 1);
    let march = date(2027, Month::March, 31);
    assert_eq!(
        try_vouchers_request("Acme Traders", april, march).unwrap(),
        VOUCHERS_FY
    );
    assert!(try_vouchers_request("Acme Traders", april, april).is_ok());
    let TallyXmlError::Value { tag, reason, .. } =
        try_vouchers_request("Acme Traders", march, april).unwrap_err()
    else {
        panic!("expected a Value error");
    };
    assert_eq!(tag, "SVTODATE");
    assert!(reason.contains("20270331"));
}

#[test]
fn try_vouchers_request_rejects_years_that_do_not_fit_yyyymmdd() {
    // provenance: derived: YYYYMMDD has a four-digit year, so 1..=9999 fits and year 0 does not.
    let zero = date(0, Month::January, 1);
    let ok = date(2026, Month::April, 1);
    let TallyXmlError::Value { tag, .. } =
        try_vouchers_request("Acme Traders", zero, ok).unwrap_err()
    else {
        panic!("expected a Value error");
    };
    assert_eq!(tag, "SVFROMDATE");
    let TallyXmlError::Value { tag, .. } =
        try_vouchers_request("Acme Traders", ok, zero).unwrap_err()
    else {
        panic!("expected a Value error");
    };
    assert_eq!(tag, "SVTODATE");
    let last = date(9999, Month::December, 31);
    assert!(try_vouchers_request("Acme Traders", ok, last).is_ok());
}

#[test]
fn try_vouchers_request_rejects_an_empty_company() {
    // provenance: derived: as for masters.
    let april = date(2026, Month::April, 1);
    assert!(try_vouchers_request("", april, april).is_err());
}
