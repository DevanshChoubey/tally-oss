//! S22: masters in the import form a user exports to a file, where `GUID` is optional.
//!
//! Provenance (derived): the help page's sample XML for importing a ledger
//! (<https://help.tallysolutions.com/sample-xml/>, read 2026-11-27) lists only `NAME` and `PARENT`
//! as mandatory and carries no `GUID`; the fixtures below are fictional and follow that shape.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_tally_xml::masters::{
    ImportMasterKind, parse_import_masters_bytes, parse_masters_text,
};

const FILE: &str = r#"<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER>
<BODY><IMPORTDATA><REQUESTDESC><REPORTNAME>All Masters</REPORTNAME></REQUESTDESC><REQUESTDATA>
<TALLYMESSAGE xmlns:UDF="TallyUDF">
 <LEDGER NAME="Kumar Textiles" ACTION="Create"><NAME>Kumar Textiles</NAME><PARENT>Sundry Debtors</PARENT></LEDGER>
</TALLYMESSAGE>
<TALLYMESSAGE xmlns:UDF="TallyUDF">
 <LEDGER NAME="=HYPERLINK(&quot;http://x.example&quot;,&quot;pay&quot;)" ACTION="Create"><NAME>=HYPERLINK("http://x.example","pay")</NAME><PARENT>Sundry Debtors</PARENT><OPENINGBALANCE>-1500.00</OPENINGBALANCE></LEDGER>
</TALLYMESSAGE>
<TALLYMESSAGE xmlns:UDF="TallyUDF">
 <GROUP NAME="Branch Debtors" ACTION="Create"><GUID>g-1</GUID><ALTERID>7</ALTERID><PARENT>Sundry Debtors</PARENT></GROUP>
</TALLYMESSAGE>
<TALLYMESSAGE xmlns:UDF="TallyUDF">
 <LEDGER NAME="శ్రీ లక్ష్మి స్టోర్స్"><PARENT>Sundry Debtors</PARENT></LEDGER>
</TALLYMESSAGE>
</REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>"#;

#[test]
fn masters_without_a_guid_are_read_in_document_order() {
    let parsed = parse_import_masters_bytes(FILE.as_bytes()).unwrap();
    let got: Vec<(ImportMasterKind, &str, Option<&str>, Option<&str>)> = parsed
        .masters
        .iter()
        .map(|m| {
            (
                m.kind,
                m.name.as_str(),
                m.parent.as_deref(),
                m.guid.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (
                ImportMasterKind::Ledger,
                "Kumar Textiles",
                Some("Sundry Debtors"),
                None
            ),
            (
                ImportMasterKind::Ledger,
                "=HYPERLINK(\"http://x.example\",\"pay\")",
                Some("Sundry Debtors"),
                None
            ),
            (
                ImportMasterKind::Group,
                "Branch Debtors",
                Some("Sundry Debtors"),
                Some("g-1")
            ),
            (
                ImportMasterKind::Ledger,
                "శ్రీ లక్ష్మి స్టోర్స్",
                Some("Sundry Debtors"),
                None
            ),
        ]
    );
    assert_eq!(parsed.masters[2].alter_id, Some(7));
    let opening = parsed.masters[1]
        .extra
        .iter()
        .find(|e| e.name == "OPENINGBALANCE")
        .unwrap();
    assert_eq!(opening.text, "-1500.00");
}

#[test]
fn the_strict_parser_still_needs_a_guid() {
    assert!(parse_masters_text(FILE).is_err());
}

#[test]
fn a_master_without_a_name_is_still_refused() {
    let text = r"<ENVELOPE><BODY><TALLYMESSAGE><LEDGER><PARENT>Sundry Debtors</PARENT></LEDGER></TALLYMESSAGE></BODY></ENVELOPE>";
    assert!(parse_import_masters_bytes(text.as_bytes()).is_err());
}
