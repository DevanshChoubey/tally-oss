//! Card X1 tests: decoding and the envelope parser.
//!
//! Provenance. Every fixture is a fictional, hand-written document whose shape follows the
//! documented envelope in `docs/research/08-data-interop-editions.md` sections 2.1 to 2.3. Every
//! expected value is `derived`: it is a literal typed into the fixture (a tag, a name, a status),
//! a byte offset found by searching the fixture for the marker it points at, or a rule of the
//! XML 1.0 and UTF-16 specifications written out in the test. No fixture was captured from a
//! TallyPrime release, so none is `observed`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_tally_xml::envelope::{
    Envelope, Header, decode, parse_envelope, parse_envelope_partial,
};
use ledgerly_tally_xml::masters::RawElement;
use ledgerly_tally_xml::{TallyXmlError, XmlWarning};
use proptest::prelude::*;

fn no_warnings() -> Vec<XmlWarning> {
    Vec::new()
}

fn ok(text: &str) -> (Envelope, Vec<XmlWarning>) {
    parse_envelope(text).unwrap()
}

fn names(elements: &[RawElement]) -> Vec<&str> {
    elements.iter().map(|e| e.name.as_str()).collect()
}

fn utf16(text: &str, little_endian: bool) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| {
            if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Envelope structure
// ---------------------------------------------------------------------------------------------

#[test]
fn export_response_header_and_messages() {
    // provenance: derived: research 08 section 2.2 (response header carries VERSION and STATUS;
    // messages sit in BODY/DATA/TALLYMESSAGE); every expected value is a literal of the fixture.
    let text = concat!(
        "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>1</STATUS></HEADER><BODY><DATA>",
        "<TALLYMESSAGE xmlns:UDF=\"TallyUDF\">",
        "<LEDGER NAME=\"A\"><NAME>A</NAME></LEDGER><GROUP NAME=\"G\"><NAME>G</NAME></GROUP>",
        "</TALLYMESSAGE>",
        "<TALLYMESSAGE><UNIT NAME=\"Nos\"/></TALLYMESSAGE>",
        "</DATA></BODY></ENVELOPE>"
    );
    let (envelope, warnings) = ok(text);
    assert_eq!(warnings, no_warnings());
    assert_eq!(
        envelope.header,
        Header {
            version: Some("1".to_owned()),
            status: Some("1".to_owned()),
            ..Header::default()
        }
    );
    assert_eq!(envelope.extras.len(), 0);
    let [first, second] = envelope.messages.as_slice() else {
        panic!("expected two messages, got {}", envelope.messages.len());
    };
    assert_eq!(first.tag, "TALLYMESSAGE");
    assert_eq!(
        first.attributes,
        [("xmlns:UDF".to_owned(), "TallyUDF".to_owned())]
    );
    assert_eq!(names(&first.elements), ["LEDGER", "GROUP"]);
    let ledger = first.elements.first().unwrap();
    assert_eq!(ledger.attributes, [("NAME".to_owned(), "A".to_owned())]);
    assert_eq!(ledger.children.first().unwrap().text, "A");
    assert_eq!(first.offset, text.find("<TALLYMESSAGE xmlns").unwrap());
    assert_eq!(second.tag, "TALLYMESSAGE");
    assert_eq!(second.attributes.len(), 0);
    assert_eq!(names(&second.elements), ["UNIT"]);
    assert_eq!(second.offset, text.rfind("<TALLYMESSAGE>").unwrap());
}

#[test]
fn collection_is_a_message_too() {
    // provenance: derived: card X2 reads masters whose parent is TALLYMESSAGE (import form) or
    // COLLECTION (export form), so both become messages.
    let text = "<ENVELOPE><HEADER/><BODY><DATA><COLLECTION>\
                <LEDGER NAME=\"A\"><GUID>g1</GUID></LEDGER></COLLECTION></DATA></BODY></ENVELOPE>";
    let (envelope, warnings) = ok(text);
    assert_eq!(warnings, no_warnings());
    let [message] = envelope.messages.as_slice() else {
        panic!("expected one message");
    };
    assert_eq!(message.tag, "COLLECTION");
    assert_eq!(names(&message.elements), ["LEDGER"]);
}

#[test]
fn import_form_is_read_through_importdata_and_requestdata() {
    // provenance: derived: research 08 section 2.3 example d (BODY/IMPORTDATA/REQUESTDESC and
    // REQUESTDATA/TALLYMESSAGE).
    let text = "<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER><BODY>\
        <IMPORTDATA><REQUESTDESC><REPORTNAME>All Masters</REPORTNAME><STATICVARIABLES>\
        <SVCURRENTCOMPANY>Acme Traders</SVCURRENTCOMPANY></STATICVARIABLES></REQUESTDESC>\
        <REQUESTDATA><TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\
        <LEDGER NAME=\"Sharma Hardware\" ACTION=\"Create\"><NAME>Sharma Hardware</NAME></LEDGER>\
        </TALLYMESSAGE></REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>";
    let (envelope, warnings) = ok(text);
    assert_eq!(warnings, no_warnings());
    assert_eq!(envelope.header.request.as_deref(), Some("Import Data"));
    assert_eq!(envelope.messages.len(), 1);
    let [desc] = envelope.extras.as_slice() else {
        panic!("expected REQUESTDESC as the only extra");
    };
    assert_eq!(desc.name, "REQUESTDESC");
    assert_eq!(desc.children.first().unwrap().text, "All Masters");
    let message = envelope.messages.first().unwrap();
    let ledger = message.elements.first().unwrap();
    assert_eq!(
        ledger.attributes,
        [
            ("NAME".to_owned(), "Sharma Hardware".to_owned()),
            ("ACTION".to_owned(), "Create".to_owned())
        ]
    );
}

#[test]
fn other_body_and_envelope_elements_are_kept_whole_as_extras() {
    // provenance: derived: rule A9 (unknown content is preserved); extras are listed in the order
    // their elements close: DESC, then IMPORTRESULT, then FOOTER.
    let text = "<ENVELOPE><HEADER/><BODY><DESC><X>1</X></DESC><DATA><TALLYMESSAGE/></DATA>\
        <IMPORTRESULT><CREATED>2</CREATED><ERRORS>0</ERRORS></IMPORTRESULT></BODY>\
        <FOOTER>end</FOOTER></ENVELOPE>";
    let (envelope, warnings) = ok(text);
    assert_eq!(warnings, no_warnings());
    assert_eq!(names(&envelope.extras), ["DESC", "IMPORTRESULT", "FOOTER"]);
    let result = envelope.extras.get(1).unwrap();
    assert_eq!(result.children.len(), 2);
    assert_eq!(result.children.first().unwrap().text, "2");
    assert_eq!(envelope.extras.get(2).unwrap().text, "end");
    assert_eq!(envelope.messages.len(), 1);
}

#[test]
fn header_fields_are_trimmed_and_unknown_children_kept() {
    // provenance: derived: research 08 section 2.2 lists VERSION, TALLYREQUEST, TYPE, SUBTYPE, ID.
    let text = "<ENVELOPE><HEADER><VERSION> 1 </VERSION><TALLYREQUEST>Export</TALLYREQUEST>\
        <TYPE>Data</TYPE><SUBTYPE>s</SUBTYPE><ID>Trial Balance</ID><COMPANYHINT>c</COMPANYHINT>\
        </HEADER><BODY/></ENVELOPE>";
    let (envelope, warnings) = ok(text);
    assert_eq!(warnings, no_warnings());
    let header = envelope.header;
    assert_eq!(header.version.as_deref(), Some("1"));
    assert_eq!(header.request.as_deref(), Some("Export"));
    assert_eq!(header.kind.as_deref(), Some("Data"));
    assert_eq!(header.sub_type.as_deref(), Some("s"));
    assert_eq!(header.id.as_deref(), Some("Trial Balance"));
    assert_eq!(header.status, None);
    assert_eq!(names(&header.extra), ["COMPANYHINT"]);
}

#[test]
fn repeated_or_nested_header_fields_are_errors() {
    // provenance: derived: a header field holds one value; two VERSIONs would make the first
    // silently win, so the document is rejected.
    let repeated = "<ENVELOPE><HEADER><VERSION>1</VERSION><VERSION>2</VERSION></HEADER></ENVELOPE>";
    let TallyXmlError::Value { tag, value, .. } = parse_envelope(repeated).unwrap_err() else {
        panic!("expected a Value error");
    };
    assert_eq!((tag.as_str(), value.as_str()), ("VERSION", "2"));
    let nested = "<ENVELOPE><HEADER><ID><X/></ID></HEADER></ENVELOPE>";
    assert!(matches!(
        parse_envelope(nested).unwrap_err(),
        TallyXmlError::Value { ref tag, .. } if tag == "ID"
    ));
    let twice = "<ENVELOPE><HEADER/><HEADER/></ENVELOPE>";
    assert!(matches!(
        parse_envelope(twice).unwrap_err(),
        TallyXmlError::Value { ref tag, .. } if tag == "HEADER"
    ));
}

#[test]
fn missing_header_is_an_empty_header_with_a_warning() {
    // provenance: derived: no value is invented for a header the document lacks; the absence is
    // reported at offset 0.
    let (envelope, warnings) = ok("<ENVELOPE><BODY/></ENVELOPE>");
    assert_eq!(envelope.header, Header::default());
    assert_eq!(
        warnings,
        [XmlWarning {
            offset: 0,
            message: "the envelope has no HEADER".to_owned()
        }]
    );
}

#[test]
fn self_closing_header_is_an_empty_header_without_warning() {
    // provenance: derived: `<HEADER/>` is a HEADER with no fields.
    let (envelope, warnings) = ok("<ENVELOPE><HEADER/></ENVELOPE>");
    assert_eq!(envelope.header, Header::default());
    assert_eq!(warnings, no_warnings());
}

#[test]
fn declaration_comments_cdata_and_indentation_are_handled() {
    // provenance: derived: XML 1.0 (declaration and comments are not content; CDATA is literal
    // text); element text keeps its whitespace for a leaf and drops it between children.
    let text = "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n<ENVELOPE>\n  <!-- note -->\n  \
        <HEADER>\n    <VERSION>1</VERSION>\n  </HEADER>\n  <BODY>\n    <DATA>\n      \
        <TALLYMESSAGE>\n        <LEDGER NAME=\"A\">\n          <NAME> A </NAME>\n          \
        <NARRATION><![CDATA[a < b & c]]></NARRATION>\n        </LEDGER>\n      </TALLYMESSAGE>\n    \
        </DATA>\n  </BODY>\n</ENVELOPE>\n";
    let (envelope, warnings) = ok(text);
    assert_eq!(warnings, no_warnings());
    let message = envelope.messages.first().unwrap();
    let ledger = message.elements.first().unwrap();
    assert_eq!(ledger.text, "");
    assert_eq!(ledger.children.first().unwrap().text, " A ");
    assert_eq!(ledger.children.get(1).unwrap().text, "a < b & c");
}

#[test]
fn predefined_and_numeric_references_decode() {
    // provenance: derived: XML 1.0 section 4.6 predefined entities; &#65; is U+0041 'A' and
    // &#x42; is U+0042 'B'.
    let text = "<ENVELOPE><HEADER/><BODY><DATA><TALLYMESSAGE><V>\
        &amp;&lt;&gt;&quot;&apos;&#65;&#x42;</V></TALLYMESSAGE></DATA></BODY></ENVELOPE>";
    let (envelope, warnings) = ok(text);
    assert_eq!(warnings, no_warnings());
    let value = envelope.messages.first().unwrap().elements.first().unwrap();
    assert_eq!(value.text, "&<>\"'AB");
}

#[test]
fn attribute_values_decode_references() {
    // provenance: derived: XML 1.0 section 3.3.3: references in attribute values are expanded.
    let text = "<ENVELOPE><HEADER/><BODY><DATA><TALLYMESSAGE>\
        <LEDGER NAME='Smith &amp; Sons'/></TALLYMESSAGE></DATA></BODY></ENVELOPE>";
    let (envelope, _) = ok(text);
    let ledger = envelope.messages.first().unwrap().elements.first().unwrap();
    assert_eq!(
        ledger.attributes,
        [("NAME".to_owned(), "Smith & Sons".to_owned())]
    );
}

#[test]
fn envelope_and_body_attributes_and_stray_text_are_reported() {
    // provenance: derived: rule A9: content that has nowhere to go is a warning, not a silent
    // drop. Warnings point at the start tag.
    let text = "<ENVELOPE v=\"1\"><HEADER/><BODY>stray<DATA/></BODY></ENVELOPE>";
    let (_, warnings) = ok(text);
    assert_eq!(
        warnings,
        [
            XmlWarning {
                offset: 0,
                message: "attributes of <ENVELOPE> ignored".to_owned()
            },
            XmlWarning {
                offset: text.find("<BODY>").unwrap(),
                message: "text inside <BODY> ignored".to_owned()
            }
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Invalid characters
// ---------------------------------------------------------------------------------------------

#[test]
fn invalid_characters_and_references_are_dropped_with_warnings() {
    // provenance: derived: research 08 section 2.1 (Tally output carries stray &#4; and raw
    // control characters); XML 1.0 production Char forbids U+0004. Expected text is the fixture
    // text with the forbidden characters and the unknown parts removed or kept as the doc says.
    let text = "<ENVELOPE><HEADER/><BODY><DATA><TALLYMESSAGE><LEDGER NAME=\"A\">\
        <NAME>A\u{4}B&#4;C&amp;D&foo;E&F</NAME></LEDGER></TALLYMESSAGE></DATA></BODY></ENVELOPE>";
    let (envelope, warnings) = ok(text);
    let ledger = envelope.messages.first().unwrap().elements.first().unwrap();
    assert_eq!(ledger.children.first().unwrap().text, "ABC&D&foo;E&F");
    let offsets: Vec<usize> = warnings.iter().map(|w| w.offset).collect();
    assert_eq!(
        offsets,
        [
            text.find('\u{4}').unwrap(),
            text.find("&#4;").unwrap(),
            text.find("&foo;").unwrap(),
            text.find("&F").unwrap()
        ]
    );
    assert!(warnings.first().unwrap().message.contains("U+0004"));
    assert!(warnings.get(1).unwrap().message.contains("&#4;"));
    assert!(warnings.get(2).unwrap().message.contains("&foo;"));
    assert!(warnings.get(3).unwrap().message.contains("bare '&'"));
}

#[test]
fn invalid_characters_in_attributes_and_cdata_are_dropped_too() {
    // provenance: derived: XML 1.0 production Char applies everywhere in the document.
    let text = "<ENVELOPE><HEADER/><BODY><DATA><TALLYMESSAGE><LEDGER NAME=\"A\u{1}B\">\
        <N><![CDATA[x\u{2}y&#4;]]></N></LEDGER></TALLYMESSAGE></DATA></BODY></ENVELOPE>";
    let (envelope, warnings) = ok(text);
    let ledger = envelope.messages.first().unwrap().elements.first().unwrap();
    assert_eq!(ledger.attributes, [("NAME".to_owned(), "AB".to_owned())]);
    // A CDATA section holds literal text, so `&#4;` is five characters there, not a reference.
    assert_eq!(ledger.children.first().unwrap().text, "xy&#4;");
    assert_eq!(warnings.len(), 2);
}

// ---------------------------------------------------------------------------------------------
// Truncation and errors
// ---------------------------------------------------------------------------------------------

#[test]
fn truncated_document_reports_the_offset_and_keeps_everything_parsed() {
    // provenance: derived: card X2/X3 contract and review focus 4: a missing </ENVELOPE> is an
    // error at the end of the text, and completed messages are kept; the open one is reported.
    let text = "<ENVELOPE><HEADER><VERSION>1</VERSION></HEADER><BODY><DATA>\
        <TALLYMESSAGE><LEDGER NAME=\"A\"><NAME>A</NAME></LEDGER></TALLYMESSAGE>\
        <TALLYMESSAGE><LEDGER NAME=\"B\"><NAME>B";
    let want = TallyXmlError::Truncated { offset: text.len() };
    assert_eq!(parse_envelope(text).unwrap_err(), want);

    let parsed = parse_envelope_partial(text);
    assert_eq!(parsed.error, Some(want));
    assert_eq!(parsed.envelope.header.version.as_deref(), Some("1"));
    let [kept] = parsed.envelope.messages.as_slice() else {
        panic!("expected exactly the first message");
    };
    assert_eq!(
        kept.elements.first().unwrap().attributes,
        [("NAME".to_owned(), "A".to_owned())]
    );
    assert_eq!(
        parsed.warnings,
        [XmlWarning {
            offset: text.rfind("<TALLYMESSAGE>").unwrap(),
            message: "incomplete <TALLYMESSAGE> discarded".to_owned()
        }]
    );
}

#[test]
fn only_the_envelope_close_missing_keeps_every_message() {
    // provenance: derived: the document ends after </BODY>; both messages are complete.
    let text = "<ENVELOPE><HEADER/><BODY><DATA><TALLYMESSAGE><A/></TALLYMESSAGE>\
        <TALLYMESSAGE><B/></TALLYMESSAGE></DATA></BODY>";
    let parsed = parse_envelope_partial(text);
    assert_eq!(
        parsed.error,
        Some(TallyXmlError::Truncated { offset: text.len() })
    );
    assert_eq!(parsed.envelope.messages.len(), 2);
    assert_eq!(parsed.warnings, no_warnings());
}

#[test]
fn truncation_inside_a_tag_or_attribute_or_comment() {
    // provenance: derived: every one of these ends before the construct closes.
    for text in [
        "<ENVELOPE><HEADER",
        "<ENVELOPE a=\"1",
        "<ENVELOPE a=",
        "<ENVELOPE><!-- open",
        "<ENVELOPE><![CDATA[open",
        "<ENVELOPE><?pi",
        "<ENVELOPE></HEADER",
        "<ENVELOPE><X/",
    ] {
        assert_eq!(
            parse_envelope(text).unwrap_err(),
            TallyXmlError::Truncated { offset: text.len() },
            "{text}"
        );
    }
    assert_eq!(
        parse_envelope("").unwrap_err(),
        TallyXmlError::Truncated { offset: 0 }
    );
    assert_eq!(
        parse_envelope(" \n ").unwrap_err(),
        TallyXmlError::Truncated { offset: 3 }
    );
}

#[test]
fn mismatched_end_tag_is_an_error_at_its_offset() {
    // provenance: derived: XML 1.0 well-formedness: an end tag closes the innermost open element.
    let text = "<ENVELOPE><HEADER></BODY></HEADER></ENVELOPE>";
    let TallyXmlError::Xml { offset, reason } = parse_envelope(text).unwrap_err() else {
        panic!("expected an Xml error");
    };
    assert_eq!(offset, text.find("</BODY>").unwrap());
    assert!(reason.contains("</BODY>") && reason.contains("<HEADER>"));
}

#[test]
fn an_end_tag_with_nothing_open_is_an_error() {
    // provenance: derived: XML 1.0 well-formedness.
    let TallyXmlError::Xml { offset, .. } = parse_envelope("</ENVELOPE>").unwrap_err() else {
        panic!("expected an Xml error");
    };
    assert_eq!(offset, 0);
}

#[test]
fn a_root_other_than_envelope_is_missing_envelope() {
    // provenance: derived: the frozen contract parses an ENVELOPE; X2 reports the same.
    assert_eq!(
        parse_envelope("<RESPONSE><CREATED>1</CREATED></RESPONSE>").unwrap_err(),
        TallyXmlError::Missing {
            tag: "ENVELOPE".to_owned(),
            path: "/".to_owned()
        }
    );
}

#[test]
fn text_before_the_root_is_an_error() {
    // provenance: derived: XML 1.0: character data cannot precede the root element.
    let TallyXmlError::Xml { offset, .. } = parse_envelope("abc<ENVELOPE/>").unwrap_err() else {
        panic!("expected an Xml error");
    };
    assert_eq!(offset, 0);
}

#[test]
fn doctype_is_rejected() {
    // provenance: derived: declarations other than comments, CDATA and processing instructions
    // are not supported (no entity definitions are ever expanded).
    let text = "<!DOCTYPE ENVELOPE [<!ENTITY x \"y\">]><ENVELOPE/>";
    let TallyXmlError::Xml { offset, .. } = parse_envelope(text).unwrap_err() else {
        panic!("expected an Xml error");
    };
    assert_eq!(offset, 0);
}

#[test]
fn malformed_attributes_are_errors() {
    // provenance: derived: XML 1.0 well-formedness: attribute names are unique and values quoted.
    for text in [
        "<ENVELOPE a=\"1\" a=\"2\"/>",
        "<ENVELOPE a=1/>",
        "<ENVELOPE a/>",
        "<ENVELOPE 1a=\"x\"/>",
    ] {
        assert!(
            matches!(parse_envelope(text).unwrap_err(), TallyXmlError::Xml { .. }),
            "{text}"
        );
    }
}

#[test]
fn nesting_beyond_128_levels_is_an_error_not_a_stack_overflow() {
    // provenance: derived: the limit is 128 open elements. ENVELOPE is the first, so 127
    // `<A>` fit and the 128th `<A>` (the 129th element) is refused at offset
    // len("<ENVELOPE>") + 127 * len("<A>") = 10 + 381.
    let text = format!("<ENVELOPE>{}", "<A>".repeat(200));
    let TallyXmlError::Xml { offset, reason } = parse_envelope(&text).unwrap_err() else {
        panic!("expected an Xml error");
    };
    assert_eq!(offset, 10 + 127 * 3);
    assert!(reason.contains("nested deeper than 128"));
}

#[test]
fn content_after_the_envelope_closes_is_a_warning() {
    // provenance: derived: rule A9: nothing is dropped silently. Trailing whitespace is fine.
    let (_, quiet) = ok("<ENVELOPE><HEADER/></ENVELOPE>\n  \n");
    assert_eq!(quiet, no_warnings());
    let text = "<ENVELOPE><HEADER/></ENVELOPE><ENVELOPE/>";
    let (envelope, warnings) = ok(text);
    assert_eq!(envelope.messages.len(), 0);
    assert_eq!(
        warnings,
        [XmlWarning {
            offset: text.rfind("<ENVELOPE/>").unwrap(),
            message: "content after the closing </ENVELOPE> ignored".to_owned()
        }]
    );
}

#[test]
fn a_byte_order_mark_left_in_the_text_is_skipped() {
    // provenance: derived: U+FEFF at the start is a byte order mark, not content.
    let (envelope, _) = ok("\u{FEFF}<ENVELOPE><HEADER><VERSION>1</VERSION></HEADER></ENVELOPE>");
    assert_eq!(envelope.header.version.as_deref(), Some("1"));
}

// ---------------------------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------------------------

#[test]
fn utf8_with_and_without_a_byte_order_mark() {
    // provenance: derived: the BOM bytes EF BB BF are not part of the text (Unicode 3.10).
    let (plain, w) = decode(b"<A>x</A>").unwrap();
    assert_eq!((plain.as_str(), w), ("<A>x</A>", no_warnings()));
    let (with_bom, w) = decode(b"\xEF\xBB\xBF<A>x</A>").unwrap();
    assert_eq!((with_bom.as_str(), w), ("<A>x</A>", no_warnings()));
    let (empty, w) = decode(b"").unwrap();
    assert_eq!((empty.as_str(), w), ("", no_warnings()));
}

#[test]
fn non_ascii_names_survive_utf8_and_utf16() {
    // provenance: derived: research 08 section 2.1 asks to test Hindi ledger names and the
    // rupee sign; the expected text is the literal that was encoded.
    let text = "<NAME>शर्मा हार्डवेयर ₹ 😀</NAME>";
    let (utf8, _) = decode(text.as_bytes()).unwrap();
    assert_eq!(utf8, text);
    let mut le = vec![0xFF, 0xFE];
    le.extend(utf16(text, true));
    assert_eq!(decode(&le).unwrap().0, text);
    let mut be = vec![0xFE, 0xFF];
    be.extend(utf16(text, false));
    assert_eq!(decode(&be).unwrap().0, text);
}

#[test]
fn utf16_without_a_byte_order_mark_is_detected_from_the_first_character() {
    // provenance: derived: XML 1.0 appendix F.1: `3C 00` starts UTF-16LE and `00 3C` starts
    // UTF-16BE for a document that begins with `<`.
    let text = "<ENVELOPE/>";
    assert_eq!(decode(&utf16(text, true)).unwrap().0, text);
    assert_eq!(decode(&utf16(text, false)).unwrap().0, text);
    assert_eq!(utf16("<", true), vec![0x3C_u8, 0x00]);
    assert_eq!(utf16("<", false), vec![0x00_u8, 0x3C]);
}

#[test]
fn utf16_with_an_odd_number_of_bytes_is_a_decode_error() {
    // provenance: derived: UTF-16 code units are two bytes; after BOM (2) and one unit (2) the
    // stray byte starts at offset 4.
    assert_eq!(
        decode(&[0xFF, 0xFE, 0x3C, 0x00, 0x41]).unwrap_err(),
        TallyXmlError::Decode {
            offset: 4,
            reason: "UTF-16 input has an odd number of bytes".to_owned()
        }
    );
}

#[test]
fn unpaired_utf16_surrogates_are_decode_errors() {
    // provenance: derived: Unicode 3.9: a high surrogate must be followed by a low one, and a
    // low surrogate cannot stand alone. The unit starts at offset 2, after the BOM.
    let high = decode(&[0xFF, 0xFE, 0x00, 0xD8]).unwrap_err();
    assert!(matches!(high, TallyXmlError::Decode { offset: 2, .. }));
    let low = decode(&[0xFF, 0xFE, 0x00, 0xDC]).unwrap_err();
    assert!(matches!(low, TallyXmlError::Decode { offset: 2, .. }));
    // High surrogate followed by an ordinary unit.
    let broken = decode(&[0xFF, 0xFE, 0x00, 0xD8, 0x41, 0x00]).unwrap_err();
    assert!(matches!(broken, TallyXmlError::Decode { offset: 2, .. }));
}

#[test]
fn invalid_utf8_is_a_decode_error_at_the_first_bad_byte() {
    // provenance: derived: 0xFF never occurs in UTF-8; `<A>` is three valid bytes first, and
    // after a three-byte BOM plus one valid byte the bad byte is at offset 4.
    assert_eq!(
        decode(b"<A>\xFF</A>").unwrap_err(),
        TallyXmlError::Decode {
            offset: 3,
            reason: "invalid UTF-8 sequence".to_owned()
        }
    );
    assert!(matches!(
        decode(b"\xEF\xBB\xBFA\xFF").unwrap_err(),
        TallyXmlError::Decode { offset: 4, .. }
    ));
}

#[test]
fn raw_control_characters_are_dropped_and_reported_by_decode() {
    // provenance: derived: XML 1.0 production Char excludes U+0001 and U+0004 but allows tab,
    // line feed and carriage return. Offsets are in the decoded text: `<A>x` is bytes 0 to 3, so
    // U+0001 is at 4, `y` at 5 and U+0004 at 6.
    let (text, warnings) = decode(b"<A>x\x01y\x04z\t\n\r</A>").unwrap();
    assert_eq!(text, "<A>xyz\t\n\r</A>");
    assert_eq!(
        warnings,
        [
            XmlWarning {
                offset: 4,
                message: "dropped invalid character U+0001".to_owned()
            },
            XmlWarning {
                offset: 6,
                message: "dropped invalid character U+0004".to_owned()
            }
        ]
    );
}

#[test]
fn utf16_document_with_stray_references_decodes_then_parses_with_warnings() {
    // provenance: derived: research 08 section 2.1: Tally sends UTF-16LE and may carry `&#4;`.
    let doc = "<ENVELOPE><HEADER><VERSION>1</VERSION></HEADER><BODY><DATA><TALLYMESSAGE>\
        <LEDGER NAME=\"Sharma\u{4}\"><NAME>Sharma &#4;Hardware ₹</NAME></LEDGER>\
        </TALLYMESSAGE></DATA></BODY></ENVELOPE>";
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(utf16(doc, true));
    let (text, decode_warnings) = decode(&bytes).unwrap();
    assert_eq!(decode_warnings.len(), 1);
    assert_eq!(
        decode_warnings.first().unwrap().offset,
        doc.find('\u{4}').unwrap()
    );
    let (envelope, warnings) = ok(&text);
    let ledger = envelope.messages.first().unwrap().elements.first().unwrap();
    assert_eq!(
        ledger.attributes,
        [("NAME".to_owned(), "Sharma".to_owned())]
    );
    assert_eq!(ledger.children.first().unwrap().text, "Sharma Hardware ₹");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings.first().unwrap().offset, text.find("&#4;").unwrap());
}

// ---------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------

fn soup() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            "<ENVELOPE>",
            "</ENVELOPE>",
            "<HEADER>",
            "</HEADER>",
            "<VERSION>",
            "</VERSION>",
            "<BODY>",
            "</BODY>",
            "<DATA>",
            "</DATA>",
            "<TALLYMESSAGE>",
            "</TALLYMESSAGE>",
            "<A b=\"1\">",
            "<A b='x&amp;'>",
            "</A>",
            "<A/>",
            "&#4;",
            "&#x41;",
            "&amp;",
            "&",
            "x",
            "\u{4}",
            "<!--",
            "-->",
            "<![CDATA[",
            "]]>",
            "<?",
            "?>",
            "<!DOCTYPE",
            " ",
            "é",
        ]),
        0..40,
    )
    .prop_map(|parts| parts.concat())
}

proptest! {
    #[test]
    fn parsing_never_panics_and_warnings_are_ordered(text in soup()) {
        // provenance: derived: invariants of the contract (never panic; warnings ordered by
        // offset; a truncation error points at the end of the text).
        let parsed = parse_envelope_partial(&text);
        let offsets: Vec<usize> = parsed.warnings.iter().map(|w| w.offset).collect();
        let mut sorted = offsets.clone();
        sorted.sort_unstable();
        prop_assert_eq!(offsets, sorted);
        if let Some(TallyXmlError::Truncated { offset }) = parsed.error {
            prop_assert_eq!(offset, text.len());
        }
        prop_assert_eq!(parse_envelope(&text).is_ok(), parsed.error.is_none());
    }

    #[test]
    fn decoding_arbitrary_bytes_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..64)) {
        // provenance: derived: rule B2/D4, untrusted input never panics.
        let _ = decode(&bytes);
    }

    #[test]
    fn decoded_arbitrary_bytes_always_parse_without_panic(
        bytes in prop::collection::vec(any::<u8>(), 0..64)
    ) {
        if let Ok((text, _)) = decode(&bytes) {
            let _ = parse_envelope_partial(&text);
        }
    }

    #[test]
    fn text_without_control_characters_decodes_unchanged(text in "\\PC*") {
        // provenance: derived: `\PC` excludes control, format, surrogate and unassigned code
        // points, so every character is a valid XML character and nothing may be dropped.
        prop_assert_eq!(decode(text.as_bytes()), Ok((text.clone(), Vec::new())));
        let mut le = vec![0xFF, 0xFE];
        le.extend(utf16(&text, true));
        prop_assert_eq!(decode(&le), Ok((text.clone(), Vec::new())));
        let mut be = vec![0xFE, 0xFF];
        be.extend(utf16(&text, false));
        prop_assert_eq!(decode(&be), Ok((text, Vec::new())));
    }
}
