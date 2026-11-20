//! Card X4 tests: the Tally-importable XML writer.
//!
//! Provenance. The document shape (envelope, `REPORTNAME`, `TALLYMESSAGE`, the `LEDGER`/`VOUCHER`
//! elements) follows the vendor-shape examples in `docs/research/08-data-interop-editions.md`
//! section 2.3 (d) and (e). The debit/credit sign convention (debit = `ISDEEMEDPOSITIVE` Yes and a
//! negative `AMOUNT`, credit = No and a positive `AMOUNT`) is `derived` from R8 2.3 (e). Every
//! expected string below is hand-written from those shapes and the layout rules in the module doc
//! of `writer.rs`; none is output of the writer copied back. Amount expectations are hand
//! arithmetic written beside them.
//!
//! Round trips: the property is `parse(write(x)) == x` using this crate's own parsers (cards X2
//! and X3). Equality is on the structure the generator built, never on text.
//!
//! `pending`: whether TallyPrime itself accepts these documents. No capture from a Tally release
//! backs the shape, so nothing here is `observed`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Date, Decimal, DrCr, Month, Paise, Qty, Rate, TallyRef};
use ledgerly_tally_xml::masters::{MasterRecord, RawElement, TallyMaster, parse_masters_text};
use ledgerly_tally_xml::vouchers::{
    RawNode, TBatchAllocation, TBillAllocation, TInventoryEntry, TLedgerEntry, TQty, TRate,
    TVoucher, parse_vouchers_xml,
};
use ledgerly_tally_xml::writer::{
    WriteError, try_write_masters, try_write_vouchers, write_masters, write_vouchers,
};
use proptest::prelude::*;

const MASTERS_OPEN: &str = "<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER>\
<BODY><IMPORTDATA><REQUESTDESC><REPORTNAME>All Masters</REPORTNAME></REQUESTDESC>\
<REQUESTDATA><TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n";
const VOUCHERS_OPEN: &str = "<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER>\
<BODY><IMPORTDATA><REQUESTDESC><REPORTNAME>Vouchers</REPORTNAME></REQUESTDESC>\
<REQUESTDATA><TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n";
const CLOSE: &str = "</TALLYMESSAGE></REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>\n";

fn s(text: &str) -> String {
    text.to_owned()
}

fn ledger_master(name: &str) -> TallyMaster {
    TallyMaster::Ledger(MasterRecord {
        tally_ref: TallyRef {
            guid: s("g-1"),
            alter_id: Some(7),
        },
        name: s(name),
        parent: Some(s("Sundry Debtors")),
        attributes: vec![(s("ACTION"), s("Create"))],
        extra: Vec::new(),
    })
}

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

fn line(name: &str, side: DrCr, paise: i64, flag: Option<bool>) -> TLedgerEntry {
    TLedgerEntry {
        ledger_name: s(name),
        side,
        amount: Paise::new(paise),
        is_deemed_positive: flag,
        bill_allocations: Vec::new(),
        extra: Vec::new(),
    }
}

fn voucher(lines: Vec<TLedgerEntry>) -> TVoucher {
    TVoucher {
        guid: None,
        alter_id: None,
        voucher_type: s("Journal"),
        date: date(2026, 11, 15),
        number: None,
        party: None,
        narration: None,
        attrs: Vec::new(),
        ledger_entries: lines,
        inventory_entries: Vec::new(),
        extra: Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------
// Exact documents
// ---------------------------------------------------------------------------------------------

#[test]
fn master_document_matches_the_documented_shape() {
    // Derived from R8 2.3 (d): a LEDGER in a TALLYMESSAGE inside REQUESTDATA, REPORTNAME All Masters.
    let expected = format!(
        "{MASTERS_OPEN}<LEDGER ACTION=\"Create\"><GUID>g-1</GUID><ALTERID>7</ALTERID>\
         <NAME>Sharma Hardware</NAME><PARENT>Sundry Debtors</PARENT></LEDGER>\n{CLOSE}"
    );
    assert_eq!(write_masters(&[ledger_master("Sharma Hardware")]), expected);
}

#[test]
fn sales_voucher_matches_the_r8_sales_example() {
    // Derived from R8 2.3 (e). Amounts: 11800.00 rupees = 1_180_000 paise (11800 x 100),
    // 10000.00 = 1_000_000 paise, 1800.00 = 180_000 paise; 10000 + 1800 = 11800 so it balances.
    let mut sales = voucher(vec![
        line("Sharma Hardware", DrCr::Dr, 1_180_000, Some(true)),
        line("Sales @18%", DrCr::Cr, 1_000_000, Some(false)),
        line("Output IGST", DrCr::Cr, 180_000, Some(false)),
    ]);
    sales.voucher_type = s("Sales");
    sales.number = Some(s("S-0042"));
    sales.party = Some(s("Sharma Hardware"));
    sales.attrs = vec![(s("VCHTYPE"), s("Sales")), (s("ACTION"), s("Create"))];
    sales.extra = vec![RawNode {
        name: s("PERSISTEDVIEW"),
        attrs: Vec::new(),
        text: s("Accounting Voucher View"),
        children: Vec::new(),
    }];
    let expected = format!(
        "{VOUCHERS_OPEN}<VOUCHER VCHTYPE=\"Sales\" ACTION=\"Create\"><DATE>20261115</DATE>\
         <VOUCHERTYPENAME>Sales</VOUCHERTYPENAME><VOUCHERNUMBER>S-0042</VOUCHERNUMBER>\
         <PARTYLEDGERNAME>Sharma Hardware</PARTYLEDGERNAME>\
         <LEDGERENTRIES.LIST><LEDGERNAME>Sharma Hardware</LEDGERNAME>\
         <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-11800.00</AMOUNT></LEDGERENTRIES.LIST>\
         <LEDGERENTRIES.LIST><LEDGERNAME>Sales @18%</LEDGERNAME>\
         <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>10000.00</AMOUNT></LEDGERENTRIES.LIST>\
         <LEDGERENTRIES.LIST><LEDGERNAME>Output IGST</LEDGERNAME>\
         <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>1800.00</AMOUNT></LEDGERENTRIES.LIST>\
         <PERSISTEDVIEW>Accounting Voucher View</PERSISTEDVIEW></VOUCHER>\n{CLOSE}"
    );
    assert_eq!(write_vouchers(&[sales]), expected);
}

#[test]
fn empty_lists_make_documents_the_readers_accept() {
    // Derived: an envelope with no objects holds none, and raises no warning.
    let masters = parse_masters_text(&write_masters(&[])).unwrap();
    assert_eq!(masters.masters.len(), 0);
    assert_eq!(masters.warnings.len(), 0);
    let (vouchers, warnings) = parse_vouchers_xml(&write_vouchers(&[])).unwrap();
    assert_eq!(vouchers.len(), 0);
    assert_eq!(warnings.len(), 0);
}

// ---------------------------------------------------------------------------------------------
// Escaping and characters
// ---------------------------------------------------------------------------------------------

#[test]
fn text_and_attribute_values_are_escaped() {
    // Derived from the XML 1.0 predefined entities: & < > in text; also " in attributes; a
    // newline or tab in an attribute is a character reference so it survives normalization.
    let master = TallyMaster::Group(MasterRecord {
        tally_ref: TallyRef {
            guid: s("g"),
            alter_id: None,
        },
        name: s("Bolts & \"Nuts\" <A>"),
        parent: None,
        attributes: vec![(s("REMARK"), s("say \"hi\"\n\t&"))],
        extra: Vec::new(),
    });
    let expected = format!(
        "{MASTERS_OPEN}<GROUP REMARK=\"say &quot;hi&quot;&#10;&#9;&amp;\"><GUID>g</GUID>\
         <NAME>Bolts &amp; \"Nuts\" &lt;A&gt;</NAME></GROUP>\n{CLOSE}"
    );
    assert_eq!(write_masters(&[master]), expected);
}

#[test]
fn forbidden_character_is_written_as_a_reference_and_reported_by_try() {
    // Derived: U+0004 is not an XML 1.0 Char; Tally writes it as &#4; and X2 documents that it
    // drops that reference with one warning.
    let master = ledger_master("A\u{4}B");
    let text = write_masters(std::slice::from_ref(&master));
    assert!(text.contains("<NAME>A&#4;B</NAME>"));
    match try_write_masters(&[master]) {
        Err(WriteError::InvalidCharacter { code_point, .. }) => assert_eq!(code_point, 4),
        other => panic!("expected InvalidCharacter, got {other:?}"),
    }
    let parsed = parse_masters_text(&text).unwrap();
    assert_eq!(parsed.warnings.len(), 1);
    match parsed.masters.first() {
        Some(TallyMaster::Ledger(record)) => assert_eq!(record.name, "AB"),
        other => panic!("expected one ledger, got {other:?}"),
    }
}

#[test]
fn carriage_return_in_text_survives_as_a_reference() {
    // Derived: a literal CR is normalised away by XML readers, so it is written as &#13;.
    let mut master = ledger_master("Cash");
    if let TallyMaster::Ledger(record) = &mut master {
        record.extra = vec![RawElement {
            name: s("X_NOTE"),
            attributes: Vec::new(),
            text: s("a\r\nb"),
            children: Vec::new(),
        }];
    }
    let text = write_masters(std::slice::from_ref(&master));
    assert!(text.contains("<X_NOTE>a&#13;\nb</X_NOTE>"));
    assert_eq!(
        parse_masters_text(&text).unwrap().masters,
        vec![master.clone()]
    );
}

#[test]
fn invalid_name_is_reported_and_sanitised() {
    // Derived: a space is not an XML name character.
    let mut sales = voucher(vec![line("Cash", DrCr::Dr, 100, Some(true))]);
    sales.extra = vec![RawNode {
        name: s("bad name"),
        attrs: Vec::new(),
        text: s("x"),
        children: Vec::new(),
    }];
    assert!(matches!(
        try_write_vouchers(std::slice::from_ref(&sales)),
        Err(WriteError::InvalidName { .. })
    ));
    assert!(write_vouchers(&[sales]).contains("<bad_name>x</bad_name>"));
}

// ---------------------------------------------------------------------------------------------
// Amounts and sides
// ---------------------------------------------------------------------------------------------

#[test]
fn amounts_are_written_in_rupees_with_two_decimals() {
    // Derived: 5 paise = 0.05 rupees; -5 paise = -0.05; 100 paise = 1.00; i64::MIN =
    // -9223372036854775808 paise = -92233720368547758 rupees and 08 paise.
    let mut v = voucher(vec![line("Cash", DrCr::Dr, 100, Some(true))]);
    let bill = |paise: i64| TBillAllocation {
        name: s("B"),
        bill_type: None,
        amount: Paise::new(paise),
        extra: Vec::new(),
    };
    v.ledger_entries[0].bill_allocations = vec![bill(5), bill(-5), bill(100)];
    let text = write_vouchers(std::slice::from_ref(&v));
    for expected in [
        "<AMOUNT>0.05</AMOUNT>",
        "<AMOUNT>-0.05</AMOUNT>",
        "<AMOUNT>1.00</AMOUNT>",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    let (parsed, _) = parse_vouchers_xml(&text).unwrap();
    let amounts: Vec<i64> = parsed[0].ledger_entries[0]
        .bill_allocations
        .iter()
        .map(|b| b.amount.get())
        .collect();
    assert_eq!(amounts, vec![5, -5, 100]);
    // The extreme value is checked as text only.
    v.ledger_entries[0].bill_allocations = vec![bill(i64::MIN)];
    assert!(write_vouchers(&[v]).contains("<AMOUNT>-92233720368547758.08</AMOUNT>"));
}

#[test]
fn zero_amount_keeps_its_side_through_the_flag() {
    // Derived from X3: a zero amount is read as Dr with Yes and Cr with No, so it needs the flag.
    let v = voucher(vec![
        line("A", DrCr::Dr, 0, Some(true)),
        line("B", DrCr::Cr, 0, Some(false)),
    ]);
    let text = try_write_vouchers(std::slice::from_ref(&v)).unwrap();
    assert!(text.contains("<ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>0.00</AMOUNT>"));
    assert!(text.contains("<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>0.00</AMOUNT>"));
    let (parsed, _) = parse_vouchers_xml(&text).unwrap();
    assert_eq!(parsed, vec![v]);
}

#[test]
fn flag_that_contradicts_the_side_is_reported_and_rendered_from_the_side() {
    // Derived from R8 2.3 (e): a debit is Yes with a negative amount.
    let v = voucher(vec![line("Cash", DrCr::Dr, 250, Some(false))]);
    assert!(matches!(
        try_write_vouchers(std::slice::from_ref(&v)),
        Err(WriteError::Inconsistent { .. })
    ));
    let text = write_vouchers(&[v]);
    assert!(text.contains("<ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-2.50</AMOUNT>"));
}

#[test]
fn zero_amount_without_flag_is_reported() {
    // Derived from X3: such a line cannot be read back (the side cannot be told).
    let v = voucher(vec![line("Cash", DrCr::Cr, 0, None)]);
    assert!(matches!(
        try_write_vouchers(std::slice::from_ref(&v)),
        Err(WriteError::Inconsistent { .. })
    ));
    assert!(
        write_vouchers(&[v])
            .contains("<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>0.00</AMOUNT>")
    );
}

#[test]
fn negative_magnitude_is_reported_and_written_as_its_absolute_value() {
    let v = voucher(vec![line("Cash", DrCr::Cr, -300, Some(false))]);
    assert!(matches!(
        try_write_vouchers(std::slice::from_ref(&v)),
        Err(WriteError::Inconsistent { .. })
    ));
    assert!(write_vouchers(&[v]).contains("<AMOUNT>3.00</AMOUNT>"));
}

// ---------------------------------------------------------------------------------------------
// Other refusals
// ---------------------------------------------------------------------------------------------

#[test]
fn try_write_reports_empty_and_whitespace_values() {
    let mut empty_name = voucher(vec![line("", DrCr::Dr, 100, Some(true))]);
    assert!(matches!(
        try_write_vouchers(std::slice::from_ref(&empty_name)),
        Err(WriteError::Empty { .. })
    ));
    empty_name.ledger_entries[0].ledger_name = s("Cash");
    empty_name.narration = Some(s(" padded "));
    assert!(matches!(
        try_write_vouchers(&[empty_name]),
        Err(WriteError::SurroundingWhitespace { .. })
    ));
}

#[test]
fn units_the_reader_cannot_split_are_reported() {
    // Derived from X3's parsers: a quantity unit holding '=' is a compound quantity (refused), and
    // a rate is split at its last '/'.
    let mut v = voucher(Vec::new());
    v.inventory_entries = vec![TInventoryEntry {
        stock_item: s("Bolt"),
        is_deemed_positive: None,
        rate: None,
        billed_qty: Some(TQty {
            qty: Qty(Decimal::new(10, 0)),
            unit: s("Box=12 Nos"),
        }),
        actual_qty: None,
        amount: None,
        batches: Vec::new(),
        extra: Vec::new(),
    }];
    assert!(matches!(
        try_write_vouchers(std::slice::from_ref(&v)),
        Err(WriteError::Inconsistent { .. })
    ));
    v.inventory_entries[0].billed_qty = None;
    v.inventory_entries[0].rate = Some(TRate {
        rate: Rate(Decimal::new(100, 0)),
        unit: s("a/b"),
    });
    assert!(matches!(
        try_write_vouchers(&[v]),
        Err(WriteError::Inconsistent { .. })
    ));
}

#[test]
fn unmodelled_child_that_collides_with_a_modelled_tag_is_reported() {
    let mut master = ledger_master("Cash");
    if let TallyMaster::Ledger(record) = &mut master {
        record.extra = vec![RawElement {
            name: s("NAME"),
            attributes: Vec::new(),
            text: s("again"),
            children: Vec::new(),
        }];
    }
    assert!(matches!(
        try_write_masters(&[master]),
        Err(WriteError::Inconsistent { .. })
    ));
}

#[test]
fn name_attribute_that_disagrees_with_the_name_is_reported_and_corrected() {
    let mut master = ledger_master("Cash");
    if let TallyMaster::Ledger(record) = &mut master {
        record.attributes = vec![(s("NAME"), s("Other"))];
    }
    assert!(matches!(
        try_write_masters(std::slice::from_ref(&master)),
        Err(WriteError::Inconsistent { .. })
    ));
    let text = write_masters(&[master]);
    assert!(text.contains("<LEDGER NAME=\"Cash\">"));
    assert_eq!(parse_masters_text(&text).unwrap().masters.len(), 1);
}

#[test]
fn deeply_nested_extras_do_not_overflow_the_stack() {
    // Rule B2 / D4: hostile depth is bounded. 150 > the 100-level writer limit.
    let mut node = RawNode {
        name: s("X_LEAF"),
        attrs: Vec::new(),
        text: String::new(),
        children: Vec::new(),
    };
    for _ in 0..150 {
        node = RawNode {
            name: s("X_NEST"),
            attrs: Vec::new(),
            text: String::new(),
            children: vec![node],
        };
    }
    let mut v = voucher(Vec::new());
    v.extra = vec![node];
    assert!(matches!(
        try_write_vouchers(std::slice::from_ref(&v)),
        Err(WriteError::Inconsistent { .. })
    ));
    assert!(parse_vouchers_xml(&write_vouchers(&[v])).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Writing what the parser read
// ---------------------------------------------------------------------------------------------

const FIXTURE: &str = r#"<ENVELOPE><BODY><DATA><TALLYMESSAGE>
  <VOUCHER VCHTYPE="Sales" ACTION="Create">
    <GUID>guid-0001</GUID><ALTERID>42</ALTERID><DATE>20261115</DATE>
    <VOUCHERTYPENAME>Sales</VOUCHERTYPENAME><VOUCHERNUMBER>S-0042</VOUCHERNUMBER>
    <PARTYLEDGERNAME>Sharma Hardware</PARTYLEDGERNAME><NARRATION>Bolts &amp; nuts</NARRATION>
    <PERSISTEDVIEW>Invoice Voucher View</PERSISTEDVIEW>
    <ALLLEDGERENTRIES.LIST>
      <LEDGERNAME>Sharma Hardware</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
      <AMOUNT>-11800.00</AMOUNT>
      <BILLALLOCATIONS.LIST><NAME>S-0042</NAME><BILLTYPE>New Ref</BILLTYPE>
        <AMOUNT>-11800.00</AMOUNT></BILLALLOCATIONS.LIST>
    </ALLLEDGERENTRIES.LIST>
    <ALLLEDGERENTRIES.LIST>
      <LEDGERNAME>Sales @18%</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
      <AMOUNT>10000.00</AMOUNT>
    </ALLLEDGERENTRIES.LIST>
    <ALLINVENTORYENTRIES.LIST>
      <STOCKITEMNAME>Bolt M8</STOCKITEMNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
      <RATE>100.00/Nos</RATE><BILLEDQTY>100 Nos</BILLEDQTY><ACTUALQTY>100 Nos</ACTUALQTY>
      <AMOUNT>10000.00</AMOUNT>
      <BATCHALLOCATIONS.LIST><GODOWNNAME>Main</GODOWNNAME><BATCHNAME>B1</BATCHNAME>
        <AMOUNT>10000.00</AMOUNT><BILLEDQTY>100 Nos</BILLEDQTY></BATCHALLOCATIONS.LIST>
    </ALLINVENTORYENTRIES.LIST>
  </VOUCHER>
</TALLYMESSAGE></DATA></BODY></ENVELOPE>"#;

#[test]
fn a_parsed_voucher_is_written_and_read_back_equal() {
    let (first, _) = parse_vouchers_xml(FIXTURE).unwrap();
    // Sanity on the fixture itself: one voucher, two ledger lines, one inventory line.
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].ledger_entries.len(), 2);
    assert_eq!(first[0].inventory_entries.len(), 1);
    let text = try_write_vouchers(&first).unwrap();
    let (again, _) = parse_vouchers_xml(&text).unwrap();
    assert_eq!(again, first);
}

#[test]
fn a_parsed_master_is_written_and_read_back_equal() {
    let source = "<ENVELOPE><TALLYMESSAGE>\
        <LEDGER NAME=\"Sharma &amp; Co\" ACTION=\"Create\"><GUID>g-9</GUID><ALTERID>12</ALTERID>\
        <NAME>Sharma &amp; Co</NAME><PARENT>Sundry Debtors</PARENT>\
        <OPENINGBALANCE>-100.00</OPENINGBALANCE>\
        <LEDGERMAILINGDETAILS.LIST><ADDRESS>1 Main Rd</ADDRESS></LEDGERMAILINGDETAILS.LIST>\
        </LEDGER></TALLYMESSAGE></ENVELOPE>";
    let first = parse_masters_text(source).unwrap();
    assert_eq!(first.masters.len(), 1);
    let text = try_write_masters(&first.masters).unwrap();
    let again = parse_masters_text(&text).unwrap();
    assert_eq!(again.masters, first.masters);
    assert_eq!(again.warnings.len(), 0);
}

// ---------------------------------------------------------------------------------------------
// Property tests: parse(write(x)) == x
// ---------------------------------------------------------------------------------------------

/// Text that the voucher reader returns unchanged: no surrounding whitespace, not empty.
const TRIMMED: &str = "[A-Za-z0-9&<>é₹\"']([A-Za-z0-9 &<>é₹\"']{0,8}[A-Za-z0-9])?";

fn attrs_strategy(value: &'static str) -> impl Strategy<Value = Vec<(String, String)>> {
    proptest::collection::btree_map("A_[A-Z]{1,4}", value, 0..3)
        .prop_map(|map| map.into_iter().collect())
}

fn element_leaf() -> impl Strategy<Value = RawElement> {
    (
        "X_[A-Z]{1,4}",
        attrs_strategy("[ -~é₹\\t\\n\\r]{0,8}"),
        "[ -~é₹\\t\\n\\r]{0,10}",
    )
        .prop_map(|(name, attributes, text)| RawElement {
            name,
            attributes,
            text,
            children: Vec::new(),
        })
}

fn element() -> impl Strategy<Value = RawElement> {
    prop_oneof![
        element_leaf(),
        (
            "X_[A-Z]{1,4}",
            attrs_strategy("[ -~é₹]{0,8}"),
            prop_oneof![Just(String::new()), "[a-z]{1,4}"],
            proptest::collection::vec(element_leaf(), 1..3),
        )
            .prop_map(|(name, attributes, text, children)| RawElement {
                name,
                attributes,
                text,
                children,
            }),
    ]
}

fn master_strategy() -> impl Strategy<Value = TallyMaster> {
    (
        0usize..9,
        "[A-Za-z0-9-]{1,12}",
        proptest::option::of(any::<u64>()),
        "[ -~é₹]{1,12}",
        proptest::option::of("[ -~é₹]{1,12}"),
        attrs_strategy("[ -~é₹\\t\\n\\r]{0,8}"),
        proptest::collection::vec(element(), 0..3),
    )
        .prop_map(|(kind, guid, alter_id, name, parent, attributes, extra)| {
            let record = MasterRecord {
                tally_ref: TallyRef { guid, alter_id },
                name,
                parent,
                attributes,
                extra,
            };
            match kind {
                0 => TallyMaster::Group(record),
                1 => TallyMaster::Ledger(record),
                2 => TallyMaster::StockItem(record),
                3 => TallyMaster::Unit(record),
                4 => TallyMaster::Godown(record),
                5 => TallyMaster::VoucherType(record),
                6 => TallyMaster::CostCentre(record),
                7 => TallyMaster::CostCategory(record),
                _ => TallyMaster::StockGroup(record),
            }
        })
}

fn node_leaf() -> impl Strategy<Value = RawNode> {
    (
        "X_[A-Z]{1,4}",
        attrs_strategy("[A-Za-z0-9 &<>é₹\"']{0,8}"),
        prop_oneof![Just(String::new()), TRIMMED],
    )
        .prop_map(|(name, attrs, text)| RawNode {
            name,
            attrs,
            text,
            children: Vec::new(),
        })
}

fn node() -> impl Strategy<Value = RawNode> {
    prop_oneof![
        node_leaf(),
        (
            "X_[A-Z]{1,4}",
            attrs_strategy("[A-Za-z0-9 ]{0,8}"),
            prop_oneof![Just(String::new()), TRIMMED],
            proptest::collection::vec(node_leaf(), 1..3),
        )
            .prop_map(|(name, attrs, text, children)| RawNode {
                name,
                attrs,
                text,
                children,
            }),
    ]
}

fn nodes() -> impl Strategy<Value = Vec<RawNode>> {
    proptest::collection::vec(node(), 0..2)
}

fn paise() -> impl Strategy<Value = Paise> {
    (-1_000_000_000_000i64..1_000_000_000_000).prop_map(Paise::new)
}

fn decimal() -> impl Strategy<Value = Decimal> {
    (-10_000_000i64..10_000_000, 0u32..5).prop_map(|(n, scale)| Decimal::new(n, scale))
}

fn unit() -> impl Strategy<Value = String> {
    prop_oneof![Just(String::new()), "[A-Za-z]{1,5}"]
}

fn qty() -> impl Strategy<Value = Option<TQty>> {
    proptest::option::of((decimal(), unit()).prop_map(|(d, unit)| TQty { qty: Qty(d), unit }))
}

fn bill() -> impl Strategy<Value = TBillAllocation> {
    (TRIMMED, proptest::option::of(TRIMMED), paise(), nodes()).prop_map(
        |(name, bill_type, amount, extra)| TBillAllocation {
            name,
            bill_type,
            amount,
            extra,
        },
    )
}

fn ledger_line() -> impl Strategy<Value = TLedgerEntry> {
    (
        TRIMMED,
        any::<bool>(),
        0i64..1_000_000_000_000,
        any::<bool>(),
        proptest::collection::vec(bill(), 0..2),
        nodes(),
    )
        .prop_map(
            |(ledger_name, debit, magnitude, flagged, bill_allocations, extra)| TLedgerEntry {
                ledger_name,
                side: if debit { DrCr::Dr } else { DrCr::Cr },
                amount: Paise::new(magnitude),
                // A zero amount needs the flag to carry its side.
                is_deemed_positive: (flagged || magnitude == 0).then_some(debit),
                bill_allocations,
                extra,
            },
        )
}

fn batch() -> impl Strategy<Value = TBatchAllocation> {
    (
        proptest::option::of(TRIMMED),
        proptest::option::of(TRIMMED),
        proptest::option::of(paise()),
        qty(),
        qty(),
        nodes(),
    )
        .prop_map(
            |(godown, batch, amount, billed_qty, actual_qty, extra)| TBatchAllocation {
                godown,
                batch,
                amount,
                billed_qty,
                actual_qty,
                extra,
            },
        )
}

fn inventory_line() -> impl Strategy<Value = TInventoryEntry> {
    (
        TRIMMED,
        proptest::option::of(any::<bool>()),
        proptest::option::of((decimal(), unit()).prop_map(|(d, unit)| TRate {
            rate: Rate(d),
            unit,
        })),
        qty(),
        qty(),
        proptest::option::of(paise()),
        proptest::collection::vec(batch(), 0..2),
        nodes(),
    )
        .prop_map(
            |(
                stock_item,
                is_deemed_positive,
                rate,
                billed_qty,
                actual_qty,
                amount,
                batches,
                extra,
            )| TInventoryEntry {
                stock_item,
                is_deemed_positive,
                rate,
                billed_qty,
                actual_qty,
                amount,
                batches,
                extra,
            },
        )
}

fn voucher_date() -> impl Strategy<Value = Date> {
    (2000i32..2031, 1u8..=12, 1u8..=28).prop_map(|(y, m, d)| date(y, m, d))
}

fn voucher_strategy() -> impl Strategy<Value = TVoucher> {
    (
        (
            proptest::option::of("[A-Za-z0-9-]{1,12}"),
            proptest::option::of(any::<u64>()),
            TRIMMED,
            any::<bool>(),
            voucher_date(),
            proptest::option::of(TRIMMED),
            proptest::option::of(TRIMMED),
            proptest::option::of(TRIMMED),
        ),
        (
            attrs_strategy("[A-Za-z0-9 &<>é₹\"']{0,8}"),
            proptest::collection::vec(ledger_line(), 0..3),
            proptest::collection::vec(inventory_line(), 0..2),
            nodes(),
        ),
    )
        .prop_map(
            |(
                (guid, alter_id, voucher_type, with_vchtype, date, number, party, narration),
                (mut attrs, ledger_entries, inventory_entries, extra),
            )| {
                if with_vchtype {
                    attrs.insert(0, ("VCHTYPE".to_owned(), voucher_type.clone()));
                }
                TVoucher {
                    guid,
                    alter_id,
                    voucher_type,
                    date,
                    number,
                    party,
                    narration,
                    attrs,
                    ledger_entries,
                    inventory_entries,
                    extra,
                }
            },
        )
}

proptest! {
    #[test]
    fn masters_round_trip(masters in proptest::collection::vec(master_strategy(), 0..4)) {
        let text = write_masters(&masters);
        let parsed = parse_masters_text(&text).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(parsed.masters, masters.clone());
        prop_assert_eq!(parsed.warnings.len(), 0);
        prop_assert_eq!(try_write_masters(&masters), Ok(text));
    }

    #[test]
    fn vouchers_round_trip(vouchers in proptest::collection::vec(voucher_strategy(), 0..3)) {
        let text = write_vouchers(&vouchers);
        let (parsed, warnings) =
            parse_vouchers_xml(&text).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(parsed, vouchers.clone());
        prop_assert_eq!(warnings.len(), 0);
        prop_assert_eq!(try_write_vouchers(&vouchers), Ok(text));
    }
}
