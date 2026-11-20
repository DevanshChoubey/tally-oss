//! Card X2 tests: master parser.
//!
//! Provenance. Every fixture below is a fictional, hand-written document whose shape follows the
//! documented import/export envelope in `docs/research/08-data-interop-editions.md` section 2.2
//! and 2.3 (vendor shape). Every expected value is `derived`: it is the literal written in the
//! fixture itself (a name, GUID or ALTERID typed by hand into the XML), never engine output.
//! No fixture is `observed`; none was captured from a TallyPrime release.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants,
    reason = "test code: helpers unwrap fixtures and panic with a message on a wrong variant"
)]

use ledgerly_core::TallyRef;
use ledgerly_tally_xml::TallyXmlError;
use ledgerly_tally_xml::masters::{
    MastersParsed, RawElement, TallyMaster, parse_masters_bytes, parse_masters_text,
};

fn wrap(body: &str) -> String {
    format!(
        "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>1</STATUS></HEADER><BODY><DATA>\
         <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">{body}</TALLYMESSAGE></DATA></BODY></ENVELOPE>"
    )
}

fn tref(guid: &str, alter: Option<u64>) -> TallyRef {
    TallyRef {
        guid: guid.to_owned(),
        alter_id: alter,
    }
}

fn ok(text: &str) -> MastersParsed {
    parse_masters_text(text).expect("fixture parses")
}

const ALL_KINDS: &str = "\
<GROUP NAME=\"Sundry Debtors\"><GUID>guid-grp-1</GUID><ALTERID>11</ALTERID>\
<NAME>Sundry Debtors</NAME><PARENT>Current Assets</PARENT></GROUP>\
<LEDGER NAME=\"Sharma Hardware\"><GUID>guid-led-1</GUID><ALTERID>22</ALTERID>\
<NAME>Sharma Hardware</NAME><PARENT>Sundry Debtors</PARENT></LEDGER>\
<STOCKITEM NAME=\"Steel Bolt\"><GUID>guid-itm-1</GUID><ALTERID>33</ALTERID>\
<NAME>Steel Bolt</NAME><PARENT>Fasteners</PARENT></STOCKITEM>\
<UNIT NAME=\"Nos\"><GUID>guid-unt-1</GUID><ALTERID>44</ALTERID><NAME>Nos</NAME></UNIT>\
<GODOWN NAME=\"Main Store\"><GUID>guid-gdn-1</GUID><ALTERID>55</ALTERID>\
<NAME>Main Store</NAME><PARENT>Warehouse</PARENT></GODOWN>\
<VOUCHERTYPE NAME=\"Sales\"><GUID>guid-vt-1</GUID><ALTERID>66</ALTERID>\
<NAME>Sales</NAME><PARENT>Sales</PARENT></VOUCHERTYPE>\
<COSTCENTRE NAME=\"North\"><GUID>guid-cc-1</GUID><ALTERID>77</ALTERID>\
<NAME>North</NAME><PARENT>Regions</PARENT></COSTCENTRE>\
<COSTCATEGORY NAME=\"Regions\"><GUID>guid-ccat-1</GUID><ALTERID>88</ALTERID>\
<NAME>Regions</NAME></COSTCATEGORY>\
<STOCKGROUP NAME=\"Fasteners\"><GUID>guid-sg-1</GUID><ALTERID>99</ALTERID>\
<NAME>Fasteners</NAME></STOCKGROUP>";

#[test]
fn all_nine_master_kinds_parse_in_document_order_without_warnings() {
    let parsed = ok(&wrap(ALL_KINDS));
    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
    assert_eq!(parsed.masters.len(), 9);
}

#[test]
fn group_ledger_and_stock_item_carry_guid_alterid_name_and_parent() {
    // Expected values: the literals in ALL_KINDS above (derived).
    let m = ok(&wrap(ALL_KINDS)).masters;
    match &m[0] {
        TallyMaster::Group(g) => {
            assert_eq!(g.tally_ref, tref("guid-grp-1", Some(11)));
            assert_eq!(g.name, "Sundry Debtors");
            assert_eq!(g.parent.as_deref(), Some("Current Assets"));
        }
        other => panic!("expected group, got {other:?}"),
    }
    match &m[1] {
        TallyMaster::Ledger(l) => {
            assert_eq!(l.tally_ref, tref("guid-led-1", Some(22)));
            assert_eq!(l.name, "Sharma Hardware");
            assert_eq!(l.parent.as_deref(), Some("Sundry Debtors"));
        }
        other => panic!("expected ledger, got {other:?}"),
    }
    match &m[2] {
        TallyMaster::StockItem(s) => {
            assert_eq!(s.tally_ref, tref("guid-itm-1", Some(33)));
            assert_eq!(s.name, "Steel Bolt");
            assert_eq!(s.parent.as_deref(), Some("Fasteners"));
        }
        other => panic!("expected stock item, got {other:?}"),
    }
}

#[test]
fn unit_godown_and_voucher_type_carry_guid_alterid_name_and_parent() {
    // Expected values: the literals in ALL_KINDS above (derived).
    let m = ok(&wrap(ALL_KINDS)).masters;
    match &m[3] {
        TallyMaster::Unit(u) => {
            assert_eq!(u.tally_ref, tref("guid-unt-1", Some(44)));
            assert_eq!(u.name, "Nos");
            assert_eq!(u.parent, None);
        }
        other => panic!("expected unit, got {other:?}"),
    }
    match &m[4] {
        TallyMaster::Godown(g) => {
            assert_eq!(g.tally_ref, tref("guid-gdn-1", Some(55)));
            assert_eq!(g.name, "Main Store");
            assert_eq!(g.parent.as_deref(), Some("Warehouse"));
        }
        other => panic!("expected godown, got {other:?}"),
    }
    match &m[5] {
        TallyMaster::VoucherType(v) => {
            assert_eq!(v.tally_ref, tref("guid-vt-1", Some(66)));
            assert_eq!(v.name, "Sales");
            assert_eq!(v.parent.as_deref(), Some("Sales"));
        }
        other => panic!("expected voucher type, got {other:?}"),
    }
}

#[test]
fn cost_centre_cost_category_and_stock_group_carry_guid_alterid_name_and_parent() {
    // Expected values: the literals in ALL_KINDS above (derived).
    let m = ok(&wrap(ALL_KINDS)).masters;
    match &m[6] {
        TallyMaster::CostCentre(c) => {
            assert_eq!(c.tally_ref, tref("guid-cc-1", Some(77)));
            assert_eq!(c.name, "North");
            assert_eq!(c.parent.as_deref(), Some("Regions"));
        }
        other => panic!("expected cost centre, got {other:?}"),
    }
    match &m[7] {
        TallyMaster::CostCategory(c) => {
            assert_eq!(c.tally_ref, tref("guid-ccat-1", Some(88)));
            assert_eq!(c.name, "Regions");
        }
        other => panic!("expected cost category, got {other:?}"),
    }
    match &m[8] {
        TallyMaster::StockGroup(s) => {
            assert_eq!(s.tally_ref, tref("guid-sg-1", Some(99)));
            assert_eq!(s.name, "Fasteners");
        }
        other => panic!("expected stock group, got {other:?}"),
    }
}

#[test]
fn collection_response_form_parses_too() {
    // Derived: the same master elements under BODY/DATA/COLLECTION (R8 section 2.2 collections).
    let text = "<ENVELOPE><BODY><DATA><COLLECTION>\
        <LEDGER NAME=\"Cash\"><GUID>g1</GUID><ALTERID>5</ALTERID></LEDGER>\
        </COLLECTION></DATA></BODY></ENVELOPE>";
    let parsed = ok(text);
    assert_eq!(parsed.masters.len(), 1);
    match &parsed.masters[0] {
        // NAME comes from the NAME attribute because there is no NAME child.
        TallyMaster::Ledger(l) => assert_eq!(l.name, "Cash"),
        other => panic!("expected ledger, got {other:?}"),
    }
}

#[test]
fn unknown_tags_and_attributes_are_kept_in_extra() {
    // Derived: PARTYGSTIN, the nested LANGUAGENAME.LIST and the ACTION attribute are not typed
    // fields, so they must survive verbatim (rule A9). The inner NAME inside NAME.LIST must not be
    // mistaken for the ledger's own NAME.
    let body = "<LEDGER NAME=\"Sharma Hardware\" ACTION=\"Create\">\
        <GUID>guid-led-1</GUID><ALTERID>22</ALTERID>\
        <LANGUAGENAME.LIST><NAME.LIST TYPE=\"String\"><NAME>Alias One</NAME></NAME.LIST></LANGUAGENAME.LIST>\
        <NAME>Sharma Hardware</NAME>\
        <PARTYGSTIN>FICTIONAL-GSTIN</PARTYGSTIN><PARENT>Sundry Debtors</PARENT>\
        <MYUDF xmlns:UDF=\"TallyUDF\"/></LEDGER>";
    let parsed = ok(&wrap(body));
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.name, "Sharma Hardware");
    assert_eq!(l.parent.as_deref(), Some("Sundry Debtors"));
    assert!(
        l.attributes
            .contains(&("ACTION".to_owned(), "Create".to_owned()))
    );
    let names: Vec<&str> = l.extra.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["LANGUAGENAME.LIST", "PARTYGSTIN", "MYUDF"]);
    assert_eq!(l.extra[1].text, "FICTIONAL-GSTIN");
    let lang: &RawElement = &l.extra[0];
    assert_eq!(lang.children.len(), 1);
    assert_eq!(lang.children[0].name, "NAME.LIST");
    assert_eq!(lang.children[0].children[0].text, "Alias One");
    assert!(
        l.extra[2]
            .attributes
            .contains(&("xmlns:UDF".to_owned(), "TallyUDF".to_owned()))
    );
}

#[test]
fn raw_control_character_is_dropped_with_a_warning_at_its_offset() {
    // Derived: the document contains one raw U+0004 inside the NAME text. The expected warning
    // offset is the position of that character in the text we hand to the parser.
    let body = "<LEDGER><GUID>g1</GUID><NAME>Shar\u{4}ma</NAME></LEDGER>";
    let text = wrap(body);
    let at = text.find('\u{4}').expect("fixture has the control char");
    let parsed = ok(&text);
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.name, "Sharma");
    assert_eq!(parsed.warnings.len(), 1);
    assert_eq!(parsed.warnings[0].offset, at);
}

#[test]
fn numeric_reference_to_control_character_is_dropped_with_a_warning() {
    // Derived: Tally can emit a stray `&#4;` (R8 section 2.1, "Dirty output"). It is not a legal
    // XML character, so it is dropped and reported at the offset of its `&`.
    let body = "<LEDGER><GUID>g1</GUID><NAME>Shar&#4;ma</NAME></LEDGER>";
    let text = wrap(body);
    let at = text.find("&#4;").expect("fixture has the reference");
    let parsed = ok(&text);
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.name, "Sharma");
    assert_eq!(parsed.warnings.len(), 1);
    assert_eq!(parsed.warnings[0].offset, at);
}

#[test]
fn predefined_entities_and_valid_char_refs_decode() {
    // Derived: XML 1.0 predefined entities: &amp; is '&', &lt; is '<', &#8377; is U+20B9.
    let body = "<LEDGER><GUID>g1</GUID><NAME>A &amp; B &lt; C &#8377;</NAME></LEDGER>";
    let parsed = ok(&wrap(body));
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.name, "A & B < C \u{20B9}");
    assert!(parsed.warnings.is_empty());
}

#[test]
fn unknown_entity_is_kept_literally_with_a_warning() {
    let body = "<LEDGER><GUID>g1</GUID><NAME>A&nbsp;B</NAME></LEDGER>";
    let text = wrap(body);
    let at = text.find("&nbsp;").expect("fixture has the entity");
    let parsed = ok(&text);
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.name, "A&nbsp;B");
    assert_eq!(parsed.warnings.len(), 1);
    assert_eq!(parsed.warnings[0].offset, at);
}

#[test]
fn utf16_le_with_bom_is_decoded() {
    // Derived: bytes are FF FE followed by the fixture's UTF-16LE code units (R8 section 2.1:
    // the loader exchanges UTF-16LE). The name contains non-ASCII text typed into the fixture.
    let text = wrap(
        "<LEDGER><GUID>g1</GUID><NAME>\u{20B9} \u{935}\u{94D}\u{92F}\u{93E}\u{92A}\u{93E}\u{930}</NAME></LEDGER>",
    );
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let parsed = parse_masters_bytes(&bytes).expect("utf-16le parses");
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(
        l.name,
        "\u{20B9} \u{935}\u{94D}\u{92F}\u{93E}\u{92A}\u{93E}\u{930}"
    );
}

#[test]
fn utf16_be_with_bom_and_utf8_with_bom_are_decoded() {
    let text = wrap("<LEDGER><GUID>g1</GUID><NAME>Cash</NAME></LEDGER>");
    let mut be = vec![0xFE, 0xFF];
    for unit in text.encode_utf16() {
        be.extend_from_slice(&unit.to_be_bytes());
    }
    let mut u8bom = vec![0xEF, 0xBB, 0xBF];
    u8bom.extend_from_slice(text.as_bytes());
    for bytes in [be, u8bom, text.clone().into_bytes()] {
        let parsed = parse_masters_bytes(&bytes).expect("parses");
        let TallyMaster::Ledger(l) = &parsed.masters[0] else {
            panic!("expected ledger");
        };
        assert_eq!(l.name, "Cash");
    }
}

#[test]
fn utf16_with_odd_length_or_lone_surrogate_is_a_decode_error() {
    // Derived: UTF-16 needs whole 2-byte units; a trailing byte cannot be decoded.
    let bytes = [0xFF, 0xFE, b'<', 0x00, b'A'];
    let err = parse_masters_bytes(&bytes).expect_err("odd length");
    assert!(matches!(err.error, TallyXmlError::Decode { .. }), "{err:?}");
    // A lone high surrogate D800 (bytes 00 D8) is not a character.
    let bytes = [0xFF, 0xFE, 0x00, 0xD8];
    let err = parse_masters_bytes(&bytes).expect_err("lone surrogate");
    assert!(matches!(err.error, TallyXmlError::Decode { .. }), "{err:?}");
}

#[test]
fn invalid_utf8_is_a_decode_error_with_offset() {
    // Derived: "<A>" is 3 bytes, then 0xFF is never valid UTF-8, so the error offset is 3.
    let err = parse_masters_bytes(b"<A>\xFF</A>").expect_err("invalid utf-8");
    assert_eq!(
        err.error,
        TallyXmlError::Decode {
            offset: 3,
            reason: "invalid UTF-8 sequence".to_owned()
        }
    );
}

#[test]
fn truncated_input_returns_offset_and_everything_parsed_so_far() {
    // Derived: the document is cut in the middle of the second ledger. The first ledger was
    // complete and must come back in `partial`; the error offset is the end of the input.
    let full = "<ENVELOPE><BODY><DATA><TALLYMESSAGE>\
        <LEDGER NAME=\"One\"><GUID>g1</GUID><ALTERID>1</ALTERID><NAME>One</NAME></LEDGER>\
        <LEDGER NAME=\"Two\"><GUID>g2</GUID><ALTERID>2</ALTERID><NAME>Tw";
    let err = parse_masters_text(full).expect_err("truncated");
    assert_eq!(err.error, TallyXmlError::Truncated { offset: full.len() });
    assert_eq!(err.partial.masters.len(), 1);
    let TallyMaster::Ledger(l) = &err.partial.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.name, "One");
    assert_eq!(l.tally_ref, tref("g1", Some(1)));
    // The half-read ledger is discarded and said so.
    assert_eq!(err.partial.warnings.len(), 1);
}

#[test]
fn missing_envelope_close_tag_is_truncation_with_all_masters_kept() {
    // Derived: both ledgers are complete; only `</ENVELOPE>` (and the wrappers) are missing.
    let text = "<ENVELOPE><BODY><DATA><TALLYMESSAGE>\
        <LEDGER><GUID>g1</GUID><NAME>One</NAME></LEDGER>\
        <LEDGER><GUID>g2</GUID><NAME>Two</NAME></LEDGER>";
    let err = parse_masters_text(text).expect_err("truncated");
    assert_eq!(err.error, TallyXmlError::Truncated { offset: text.len() });
    assert_eq!(err.partial.masters.len(), 2);
}

#[test]
fn truncated_inside_a_tag_is_truncation() {
    let text = "<ENVELOPE><BODY><DATA><TALLYMESSAGE>\
        <LEDGER><GUID>g1</GUID><NAME>One</NAME></LEDGER><LEDG";
    let err = parse_masters_text(text).expect_err("truncated");
    assert_eq!(err.error, TallyXmlError::Truncated { offset: text.len() });
    assert_eq!(err.partial.masters.len(), 1);
}

#[test]
fn mismatched_end_tag_is_a_malformed_xml_error_with_partial_results() {
    let text = "<ENVELOPE><BODY><DATA><TALLYMESSAGE>\
        <LEDGER><GUID>g1</GUID><NAME>One</NAME></LEDGER>\
        <LEDGER><GUID>g2</GUID><NAME>Two</NAME></GROUP>";
    let err = parse_masters_text(text).expect_err("mismatch");
    assert!(matches!(err.error, TallyXmlError::Xml { .. }), "{err:?}");
    assert_eq!(err.partial.masters.len(), 1);
}

#[test]
fn missing_guid_is_an_error_never_an_empty_string() {
    let err = parse_masters_text(&wrap("<LEDGER><NAME>One</NAME></LEDGER>")).expect_err("no guid");
    assert!(
        matches!(&err.error, TallyXmlError::Missing { tag, .. } if tag == "GUID"),
        "{err:?}"
    );
}

#[test]
fn missing_name_is_an_error() {
    let err = parse_masters_text(&wrap("<LEDGER><GUID>g1</GUID></LEDGER>")).expect_err("no name");
    assert!(
        matches!(&err.error, TallyXmlError::Missing { tag, .. } if tag == "NAME"),
        "{err:?}"
    );
}

#[test]
fn non_numeric_alterid_is_a_value_error_and_absent_alterid_is_none() {
    for bad in ["12x", "-5", "", "1.5"] {
        let body =
            format!("<LEDGER><GUID>g1</GUID><ALTERID>{bad}</ALTERID><NAME>A</NAME></LEDGER>");
        let err = parse_masters_text(&wrap(&body)).expect_err("bad alterid");
        assert!(
            matches!(&err.error, TallyXmlError::Value { tag, .. } if tag == "ALTERID"),
            "{bad}: {err:?}"
        );
    }
    let parsed = ok(&wrap("<LEDGER><GUID>g1</GUID><NAME>A</NAME></LEDGER>"));
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.tally_ref.alter_id, None);
    // u64::MAX fits; one more does not (derived: 18446744073709551615 = 2^64 - 1).
    let parsed = ok(&wrap(
        "<LEDGER><GUID>g1</GUID><ALTERID>18446744073709551615</ALTERID><NAME>A</NAME></LEDGER>",
    ));
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.tally_ref.alter_id, Some(u64::MAX));
    let err = parse_masters_text(&wrap(
        "<LEDGER><GUID>g1</GUID><ALTERID>18446744073709551616</ALTERID><NAME>A</NAME></LEDGER>",
    ))
    .expect_err("overflow");
    assert!(matches!(err.error, TallyXmlError::Value { .. }));
}

#[test]
fn duplicate_typed_field_and_name_mismatch_are_errors() {
    let dup = "<LEDGER><GUID>g1</GUID><GUID>g2</GUID><NAME>A</NAME></LEDGER>";
    let err = parse_masters_text(&wrap(dup)).expect_err("dup guid");
    assert!(matches!(err.error, TallyXmlError::Value { .. }), "{err:?}");
    let mismatch = "<LEDGER NAME=\"A\"><GUID>g1</GUID><NAME>B</NAME></LEDGER>";
    let err = parse_masters_text(&wrap(mismatch)).expect_err("name mismatch");
    assert!(
        matches!(&err.error, TallyXmlError::Value { tag, .. } if tag == "NAME"),
        "{err:?}"
    );
}

#[test]
fn empty_parent_means_no_parent() {
    // Derived: a primary group has an empty PARENT element; that is modelled as None.
    let body = "<GROUP><GUID>g1</GUID><NAME>Primary</NAME><PARENT></PARENT></GROUP>\
                <GROUP><GUID>g2</GUID><NAME>Other</NAME><PARENT/></GROUP>";
    let parsed = ok(&wrap(body));
    for m in &parsed.masters {
        let TallyMaster::Group(g) = m else {
            panic!("expected group");
        };
        assert_eq!(g.parent, None);
    }
}

#[test]
fn vouchers_are_skipped_silently_and_other_unsupported_messages_are_warned() {
    // Derived: vouchers belong to card X3, so they are not a dropped-field warning; a COMPANY
    // message is neither a master we model nor a voucher, so it is reported.
    let body = "<VOUCHER VCHTYPE=\"Sales\"><DATE>20261115</DATE></VOUCHER>\
                <COMPANY><NAME>Acme</NAME></COMPANY>\
                <LEDGER><GUID>g1</GUID><NAME>Cash</NAME></LEDGER>";
    let parsed = ok(&wrap(body));
    assert_eq!(parsed.masters.len(), 1);
    assert_eq!(parsed.warnings.len(), 1);
    assert!(parsed.warnings[0].message.contains("COMPANY"));
}

#[test]
fn nesting_beyond_the_limit_is_an_error_not_a_stack_overflow() {
    let mut body = String::from("<LEDGER><GUID>g1</GUID><NAME>A</NAME>");
    for _ in 0..500 {
        body.push_str("<X>");
    }
    for _ in 0..500 {
        body.push_str("</X>");
    }
    body.push_str("</LEDGER>");
    let err = parse_masters_text(&wrap(&body)).expect_err("too deep");
    assert!(matches!(err.error, TallyXmlError::Xml { .. }), "{err:?}");
}

#[test]
fn name_text_is_kept_verbatim_including_inner_spaces() {
    // Derived: leaf text is data; only indentation around child elements is formatting.
    let body = "\n  <LEDGER>\n    <GUID>g1</GUID>\n    <NAME> Two  Spaces </NAME>\n  </LEDGER>\n";
    let parsed = ok(&wrap(body));
    let TallyMaster::Ledger(l) = &parsed.masters[0] else {
        panic!("expected ledger");
    };
    assert_eq!(l.name, " Two  Spaces ");
    assert_eq!(l.tally_ref.guid, "g1");
}

proptest::proptest! {
    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512)) {
        let _ = parse_masters_bytes(&bytes);
    }

    #[test]
    fn arbitrary_text_in_a_name_never_panics(name in "\\PC{0,40}") {
        // An empty NAME is rejected by design (see missing/empty tests), so skip that input.
        proptest::prop_assume!(!name.is_empty());
        let escaped = name.replace('&', "&amp;").replace('<', "&lt;");
        let body = format!("<LEDGER><GUID>g1</GUID><NAME>{escaped}</NAME></LEDGER>");
        let parsed = parse_masters_text(&wrap(&body));
        // Derived: text without control characters round-trips exactly once XML-escaped.
        let parsed = parsed.expect("escaped text parses");
        if let Some(TallyMaster::Ledger(l)) = parsed.masters.first() {
            proptest::prop_assert_eq!(&l.name, &name);
        }
    }
}
