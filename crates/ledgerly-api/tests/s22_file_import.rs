//! S22: importing masters and transactions from an XML file into the open company.
//!
//! Provenance. Every file here is a fictional, hand-built fixture in the import form of
//! <https://help.tallysolutions.com/sample-xml/> (read 2026-11-27); none was captured from the
//! other program, so nothing is `observed`. Expected values are `derived`:
//! - the import screen (<https://help.tallysolutions.com/import-data-from-xml-or-json/>, read
//!   2026-11-27) imports masters or transactions into the open company and lists exceptions;
//! - the three choices for a master that exists (Combine Opening Balance, Ignore Duplicates,
//!   Modify with New Data) are from <https://help.tallysolutions.com/import-data-faq/>;
//! - a master's `OPENINGBALANCE` is negative for a debit (the bridge's derived convention), so
//!   `-1500.00` is Rs 1,500 Dr = 150,000 paise; an absent one on a new ledger is zero;
//! - the counts are the objects in each fixture, counted by hand.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};

use ledgerly_api::{Api, ApiError, Command, FileImportSummary, MasterBehaviour, XmlImport};
use ledgerly_core::{Clock, CompanyId, Outcome, Paise, SignedAmount, StateCode};
use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, NegativeStockPolicy, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::new_utc(
        date!(2026 - 11 - 21),
        time::Time::MIDNIGHT,
    ))
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn setup() -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir().unwrap();
    let api = Api::create(&dir.path().join("company.ledgerly"), &company(), clock()).unwrap();
    (dir, api)
}

fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    path
}

fn envelope(messages: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ENVELOPE><HEADER><TALLYREQUEST>Import Data\
         </TALLYREQUEST></HEADER><BODY><IMPORTDATA><REQUESTDESC><REPORTNAME>All Masters\
         </REPORTNAME></REQUESTDESC><REQUESTDATA>{messages}</REQUESTDATA></IMPORTDATA></BODY>\
         </ENVELOPE>"
    )
}

fn msg(inner: &str) -> String {
    format!("<TALLYMESSAGE xmlns:UDF=\"TallyUDF\">{inner}</TALLYMESSAGE>")
}

const FORMULA: &str = "=HYPERLINK(\"http://x.example\",\"pay\")";
const TELUGU: &str = "శ్రీ లక్ష్మి స్టోర్స్";

/// One group (after a ledger that uses it), four ledgers, one ledger under a missing group, a
/// stock item and the existing Sales voucher type.
fn masters_file() -> String {
    envelope(&[
        msg("<LEDGER NAME=\"Kumar Textiles\" ACTION=\"Create\"><NAME>Kumar Textiles</NAME><PARENT>Branch Debtors</PARENT></LEDGER>"),
        msg("<LEDGER NAME=\"=HYPERLINK(&quot;http://x.example&quot;,&quot;pay&quot;)\" ACTION=\"Create\"><NAME>=HYPERLINK(\"http://x.example\",\"pay\")</NAME><PARENT>Sundry Debtors</PARENT><OPENINGBALANCE>-1500.00</OPENINGBALANCE></LEDGER>"),
        msg(&format!("<LEDGER NAME=\"{TELUGU}\" ACTION=\"Create\"><NAME>{TELUGU}</NAME><PARENT>Sundry Debtors</PARENT></LEDGER>")),
        msg("<LEDGER NAME=\"Cash Box\" ACTION=\"Create\"><NAME>Cash Box</NAME><PARENT>Cash-in-Hand</PARENT><OPENINGBALANCE>-250.50</OPENINGBALANCE></LEDGER>"),
        msg("<LEDGER NAME=\"Lost Ledger\" ACTION=\"Create\"><NAME>Lost Ledger</NAME><PARENT>Missing Group</PARENT></LEDGER>"),
        msg("<GROUP NAME=\"Branch Debtors\" ACTION=\"Create\"><NAME>Branch Debtors</NAME><PARENT>Sundry Debtors</PARENT></GROUP>"),
        msg("<STOCKITEM NAME=\"Silk Saree\" ACTION=\"Create\"><NAME>Silk Saree</NAME><PARENT/></STOCKITEM>"),
        msg("<VOUCHERTYPE NAME=\"Sales\" ACTION=\"Create\"><NAME>Sales</NAME><PARENT>Sales</PARENT></VOUCHERTYPE>"),
    ]
    .concat())
}

fn books(api: &Api) -> Books {
    match api.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("books unsupported: {u:?}"),
    }
}

fn ledger_opening(api: &Api, name: &str) -> SignedAmount {
    books(api)
        .masters
        .ledgers
        .values()
        .find(|l| l.name == name)
        .unwrap_or_else(|| panic!("ledger {name} exists"))
        .opening
}

fn import(api: &mut Api, what: XmlImport, path: &Path) -> FileImportSummary {
    match api.import_tally_xml_file(what, path).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("import unsupported: {u:?}"),
    }
}

fn counts(s: &FileImportSummary) -> [usize; 5] {
    [s.created, s.altered, s.combined, s.ignored, s.errors]
}

#[test]
fn masters_go_into_the_open_company_and_every_exception_is_listed() {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "masters.xml", &masters_file());
    let s = import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
        &path,
    );
    // created: Branch Debtors and four ledgers; ignored: the Sales voucher type; errors: the
    // ledger under Missing Group and the stock item.
    assert_eq!(counts(&s), [5, 0, 0, 1, 2], "{s}");
    let objects: Vec<&str> = s.exceptions.iter().map(|e| e.object.as_str()).collect();
    assert_eq!(
        objects,
        vec!["ledger 'Lost Ledger'", "stock item 'Silk Saree'"]
    );
    assert!(s.exceptions[0].reason.contains("Missing Group"), "{s}");
    assert_eq!(
        ledger_opening(&api, FORMULA),
        SignedAmount::dr(Paise::new(150_000))
    );
    assert_eq!(
        ledger_opening(&api, "Cash Box"),
        SignedAmount::dr(Paise::new(25_050))
    );
    assert_eq!(ledger_opening(&api, TELUGU), SignedAmount::dr(Paise::ZERO));
    let b = books(&api);
    let branch = b
        .masters
        .groups
        .values()
        .find(|g| g.name == "Branch Debtors")
        .unwrap();
    let kumar = b
        .masters
        .ledgers
        .values()
        .find(|l| l.name == "Kumar Textiles")
        .unwrap();
    assert_eq!(kumar.group, branch.id);
}

#[test]
fn ignore_duplicates_keeps_what_exists() {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "masters.xml", &masters_file());
    let behaviour = XmlImport::Masters(MasterBehaviour::IgnoreDuplicates);
    import(&mut api, behaviour, &path);
    let again = import(&mut api, behaviour, &path);
    // Branch Debtors, four ledgers and the voucher type exist; the two exceptions stay.
    assert_eq!(counts(&again), [0, 0, 0, 6, 2], "{again}");
    assert_eq!(
        ledger_opening(&api, FORMULA),
        SignedAmount::dr(Paise::new(150_000))
    );
}

#[test]
fn combine_adds_the_files_opening_to_the_existing_one() {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "masters.xml", &masters_file());
    import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
        &path,
    );
    let s = import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::CombineOpeningBalance),
        &path,
    );
    // combined: the two ledgers with an opening; ignored: the group, the two ledgers without
    // one and the voucher type.
    assert_eq!(counts(&s), [0, 0, 2, 4, 2], "{s}");
    assert_eq!(
        ledger_opening(&api, FORMULA),
        SignedAmount::dr(Paise::new(300_000))
    );
    assert_eq!(
        ledger_opening(&api, "Cash Box"),
        SignedAmount::dr(Paise::new(50_100))
    );
}

#[test]
fn modify_replaces_the_group_and_the_opening() {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "masters.xml", &masters_file());
    import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
        &path,
    );
    let change = envelope(&msg(
        "<LEDGER NAME=\"Kumar Textiles\"><NAME>Kumar Textiles</NAME><PARENT>Sundry Debtors</PARENT><OPENINGBALANCE>100.00</OPENINGBALANCE></LEDGER>",
    ));
    let path = write(dir.path(), "change.xml", &change);
    let s = import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::ModifyWithNewData),
        &path,
    );
    assert_eq!(counts(&s), [0, 1, 0, 0, 0], "{s}");
    let b = books(&api);
    let debtors = b
        .masters
        .groups
        .values()
        .find(|g| g.name == "Sundry Debtors")
        .unwrap();
    let kumar = b
        .masters
        .ledgers
        .values()
        .find(|l| l.name == "Kumar Textiles")
        .unwrap();
    assert_eq!(kumar.group, debtors.id);
    // 100.00 positive is a credit of Rs 100.
    assert_eq!(kumar.opening, SignedAmount::cr(Paise::new(10_000)));
}

fn vouchers_file() -> String {
    envelope(&[
        // A receipt with no GUID and no status flags: Cash Box Dr 1,000, Kumar Textiles Cr 1,000.
        msg("<VOUCHER VCHTYPE=\"Receipt\" ACTION=\"Create\"><DATE>20261005</DATE><VOUCHERTYPENAME>Receipt</VOUCHERTYPENAME><VOUCHERNUMBER>R-7</VOUCHERNUMBER><NARRATION>Part payment</NARRATION>\
             <ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash Box</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-1000.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
             <ALLLEDGERENTRIES.LIST><LEDGERNAME>Kumar Textiles</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>1000.00</AMOUNT></ALLLEDGERENTRIES.LIST></VOUCHER>"),
        // A ledger the company does not have.
        msg("<VOUCHER VCHTYPE=\"Receipt\"><DATE>20261006</DATE><GUID>v-2</GUID><VOUCHERTYPENAME>Receipt</VOUCHERTYPENAME>\
             <ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash Box</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-50.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
             <ALLLEDGERENTRIES.LIST><LEDGERNAME>Nobody</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>50.00</AMOUNT></ALLLEDGERENTRIES.LIST></VOUCHER>"),
        // A voucher type the company does not have.
        msg("<VOUCHER VCHTYPE=\"Shop Sales\"><DATE>20261007</DATE><VOUCHERTYPENAME>Shop Sales</VOUCHERTYPENAME>\
             <ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash Box</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-20.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
             <ALLLEDGERENTRIES.LIST><LEDGERNAME>Kumar Textiles</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>20.00</AMOUNT></ALLLEDGERENTRIES.LIST></VOUCHER>"),
    ]
    .concat())
}

fn with_masters() -> (tempfile::TempDir, Api) {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "masters.xml", &masters_file());
    import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
        &path,
    );
    (dir, api)
}

#[test]
fn transactions_need_their_masters_and_the_rest_are_exceptions() {
    let (dir, mut api) = with_masters();
    let path = write(dir.path(), "vouchers.xml", &vouchers_file());
    let s = import(&mut api, XmlImport::Transactions, &path);
    assert_eq!(counts(&s), [1, 0, 0, 0, 2], "{s}");
    assert!(s.exceptions[0].reason.contains("Nobody"), "{s}");
    assert!(s.exceptions[1].reason.contains("Shop Sales"), "{s}");
    let b = books(&api);
    let v = b.vouchers.values().next().unwrap();
    assert_eq!(v.status, VoucherStatus::Regular);
    assert_eq!(v.narration, "Part payment");
    // The Receipt type numbers automatically, so the file's number is kept aside.
    assert_eq!(
        v.extra.get("VOUCHERNUMBER").map(String::as_str),
        Some("R-7")
    );
    assert_eq!(v.ledger_lines.len(), 2);
    assert_eq!(
        v.ledger_lines[0].amount,
        SignedAmount::dr(Paise::new(100_000))
    );
}

#[test]
fn a_voucher_in_the_locked_period_is_an_exception() {
    let (dir, mut api) = with_masters();
    api.execute(Command::LockPeriod(date!(2026 - 10 - 31)))
        .unwrap();
    let path = write(dir.path(), "vouchers.xml", &vouchers_file());
    let s = import(&mut api, XmlImport::Transactions, &path);
    assert_eq!(s.created, 0, "{s}");
    assert_eq!(s.errors, 3, "{s}");
    assert!(books(&api).vouchers.is_empty());
}

#[test]
fn a_file_that_is_not_xml_is_an_error_and_imports_nothing() {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "notes.xml", "this is not xml");
    let before = books(&api).masters.ledgers.len();
    let err = api
        .import_tally_xml_file(XmlImport::Masters(MasterBehaviour::IgnoreDuplicates), &path)
        .unwrap_err();
    assert!(matches!(err, ApiError::FileImport { .. }), "{err}");
    assert_eq!(books(&api).masters.ledgers.len(), before);
}

#[test]
fn a_transactions_file_that_is_not_xml_is_an_error() {
    // derived: an import file is an ENVELOPE document (https://help.tallysolutions.com/sample-xml/,
    // read 2026-11-27); text with no element is not one, so it is refused, not read as "no vouchers".
    let (dir, mut api) = setup();
    for (name, body) in [
        ("plain.xml", "this is not xml"),
        ("other.xml", "<NOTES><VOUCHER/></NOTES>"),
    ] {
        let path = write(dir.path(), name, body);
        let err = api
            .import_tally_xml_file(XmlImport::Transactions, &path)
            .unwrap_err();
        assert!(matches!(err, ApiError::FileImport { .. }), "{name}: {err}");
    }
}

#[test]
fn a_missing_file_is_an_error() {
    let (dir, mut api) = setup();
    let err = api
        .import_tally_xml_file(XmlImport::Transactions, &dir.path().join("none.xml"))
        .unwrap_err();
    assert!(matches!(err, ApiError::FileImport { .. }), "{err}");
}
