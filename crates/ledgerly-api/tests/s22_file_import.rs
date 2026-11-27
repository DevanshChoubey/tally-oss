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

use ledgerly_api::{
    Api, ApiError, Command, FileImportSummary, MasterBehaviour, VoucherBehaviour, XmlImport,
};
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
    // A company as the setup screen makes it, with its predefined Cash ledger (S22 round 2).
    let api = match Api::create_company(&dir.path().join("company.ledgerly"), &company(), clock())
        .unwrap()
    {
        Outcome::Value(api) => api,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
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
    let s = import(&mut api, XmlImport::default_transactions(), &path);
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
    let s = import(&mut api, XmlImport::default_transactions(), &path);
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
            .import_tally_xml_file(XmlImport::default_transactions(), &path)
            .unwrap_err();
        assert!(matches!(err, ApiError::FileImport { .. }), "{name}: {err}");
    }
}

#[test]
fn a_missing_file_is_an_error() {
    let (dir, mut api) = setup();
    let err = api
        .import_tally_xml_file(
            XmlImport::default_transactions(),
            &dir.path().join("none.xml"),
        )
        .unwrap_err();
    assert!(matches!(err, ApiError::FileImport { .. }), "{err}");
}

// Bill-wise details from a file (S22 round 2).
//
// provenance: derived.
// - The sample import file of <https://help.tallysolutions.com/sample-xml/> (read 2026-11-27)
//   creates the party ledger "Customer ABC" with only NAME and PARENT (Sundry Debtors), and its
//   sales voucher carries BILLALLOCATIONS on that ledger, which the page says are mandatory "only
//   for party ledger". So a ledger under Sundry Debtors or Sundry Creditors (or a group under
//   them) that a file creates without ISBILLWISEON keeps its balances bill-by-bill. An explicit
//   ISBILLWISEON wins. Any other ledger is not bill-wise unless the file says so.
// - The TDL reference appendix
//   (<https://help.tallysolutions.com/developer-reference/tally-definition-language/appendix/>,
//   read 2026-11-27): the Bill Allocations collection "won't be available" when the ledger does
//   not maintain bill-wise details. So bills on such a line are not imported; the voucher is, and
//   the summary says what was left out.
// - "Receivables and Payables tracking methods"
//   (<https://help.tallysolutions.com/receivables-payables-tracking-methods/>, read 2026-11-27):
//   On Account records "an amount without linking it to a specific invoice or bill". A line on a
//   bill-wise ledger with no bills in the file is recorded On Account for the whole line.

fn party_masters() -> String {
    envelope(&[
        msg("<LEDGER NAME=\"Customer ABC\" ACTION=\"Create\"><NAME>Customer ABC</NAME><PARENT>Sundry Debtors</PARENT></LEDGER>"),
        msg("<LEDGER NAME=\"Weaver Co\" ACTION=\"Create\"><NAME>Weaver Co</NAME><PARENT>Sundry Creditors</PARENT></LEDGER>"),
        msg("<LEDGER NAME=\"Branch Party\" ACTION=\"Create\"><NAME>Branch Party</NAME><PARENT>Branch Debtors</PARENT></LEDGER>"),
        msg("<GROUP NAME=\"Branch Debtors\" ACTION=\"Create\"><NAME>Branch Debtors</NAME><PARENT>Sundry Debtors</PARENT></GROUP>"),
        msg("<LEDGER NAME=\"Walk-in\" ACTION=\"Create\"><NAME>Walk-in</NAME><PARENT>Sundry Debtors</PARENT><ISBILLWISEON>No</ISBILLWISEON></LEDGER>"),
        msg("<LEDGER NAME=\"Sales\" ACTION=\"Create\"><NAME>Sales</NAME><PARENT>Sales Accounts</PARENT></LEDGER>"),
        msg("<LEDGER NAME=\"Rent\" ACTION=\"Create\"><NAME>Rent</NAME><PARENT>Indirect Expenses</PARENT><ISBILLWISEON>Yes</ISBILLWISEON></LEDGER>"),
    ]
    .concat())
}

fn bill_wise(api: &Api, name: &str) -> bool {
    books(api)
        .masters
        .ledgers
        .values()
        .find(|l| l.name == name)
        .unwrap_or_else(|| panic!("ledger {name} exists"))
        .bill_wise
        .is_some()
}

fn with_parties() -> (tempfile::TempDir, Api) {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "parties.xml", &party_masters());
    let s = import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
        &path,
    );
    assert_eq!(counts(&s), [7, 0, 0, 0, 0], "{s}");
    (dir, api)
}

#[test]
fn a_party_ledger_from_a_file_keeps_balances_bill_by_bill_unless_the_file_says_no() {
    let (_dir, api) = with_parties();
    assert!(bill_wise(&api, "Customer ABC"));
    assert!(bill_wise(&api, "Weaver Co"));
    assert!(bill_wise(&api, "Branch Party"));
    assert!(!bill_wise(&api, "Walk-in"));
    assert!(!bill_wise(&api, "Sales"));
    assert!(bill_wise(&api, "Rent"));
}

/// A sales voucher as the sample file's: Customer ABC Dr 1,050 with a New Ref bill "1" of 1,050,
/// Sales Cr 1,050.
fn sale(party: &str, guid: &str, narration: &str) -> String {
    msg(&format!(
        "<VOUCHER VCHTYPE=\"Sales\" ACTION=\"Create\"><DATE>20261010</DATE><GUID>{guid}</GUID>\
         <NARRATION>{narration}</NARRATION><VOUCHERTYPENAME>Sales</VOUCHERTYPENAME><VOUCHERNUMBER>1</VOUCHERNUMBER>\
         <PARTYLEDGERNAME>{party}</PARTYLEDGERNAME>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>{party}</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-1050.00</AMOUNT>\
         <BILLALLOCATIONS.LIST><NAME>1</NAME><BILLTYPE>New Ref</BILLTYPE><AMOUNT>-1050.00</AMOUNT></BILLALLOCATIONS.LIST></ALLLEDGERENTRIES.LIST>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Sales</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>1050.00</AMOUNT></ALLLEDGERENTRIES.LIST></VOUCHER>"
    ))
}

#[test]
fn a_sale_with_bills_on_a_party_ledger_from_a_file_is_imported_with_its_bill() {
    let (dir, mut api) = with_parties();
    let path = write(
        dir.path(),
        "sale.xml",
        &envelope(&sale("Customer ABC", "g-sale-1", "first")),
    );
    let s = import(&mut api, XmlImport::default_transactions(), &path);
    assert_eq!(counts(&s), [1, 0, 0, 0, 0], "{s}");
    let b = books(&api);
    let v = b.vouchers.values().next().unwrap();
    assert_eq!(v.ledger_lines[0].bills.len(), 1);
    assert_eq!(v.ledger_lines[0].bills[0].name, "1");
    assert_eq!(
        v.ledger_lines[0].bills[0].kind,
        ledgerly_model::BillRefKind::New
    );
}

#[test]
fn bills_on_a_ledger_that_is_not_bill_wise_are_left_out_and_said() {
    let (dir, mut api) = with_parties();
    let path = write(
        dir.path(),
        "sale.xml",
        &envelope(&sale("Walk-in", "g-sale-2", "walk-in")),
    );
    let s = import(&mut api, XmlImport::default_transactions(), &path);
    assert_eq!(counts(&s), [1, 0, 0, 0, 0], "{s}");
    let b = books(&api);
    let v = b.vouchers.values().next().unwrap();
    assert_eq!(v.ledger_lines[0].bills, Vec::new());
    assert!(
        s.warnings
            .iter()
            .any(|w| w.to_string().contains("Walk-in") && w.to_string().contains("bill-by-bill")),
        "{:?}",
        s.warnings
    );
}

#[test]
fn a_line_on_a_bill_wise_ledger_with_no_bills_is_on_account() {
    let (dir, mut api) = with_parties();
    // Cash Dr 400, Customer ABC Cr 400, no bills: the whole 400 (40,000 paise) is On Account.
    let receipt = msg(
        "<VOUCHER VCHTYPE=\"Receipt\"><DATE>20261012</DATE><GUID>g-rcpt</GUID><VOUCHERTYPENAME>Receipt</VOUCHERTYPENAME>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-400.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Customer ABC</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>400.00</AMOUNT></ALLLEDGERENTRIES.LIST></VOUCHER>",
    );
    let path = write(dir.path(), "receipt.xml", &envelope(&receipt));
    let s = import(&mut api, XmlImport::default_transactions(), &path);
    assert_eq!(counts(&s), [1, 0, 0, 0, 0], "{s}");
    let b = books(&api);
    let v = b.vouchers.values().next().unwrap();
    let party = &v.ledger_lines[1];
    assert_eq!(party.bills.len(), 1);
    assert_eq!(party.bills[0].kind, ledgerly_model::BillRefKind::OnAccount);
    assert_eq!(party.bills[0].amount, SignedAmount::cr(Paise::new(40_000)));
    assert!(
        s.warnings
            .iter()
            .any(|w| w.to_string().contains("Customer ABC") && w.to_string().contains("On Account")),
        "{:?}",
        s.warnings
    );
}

// Importing the same vouchers again (S22 round 2).
//
// provenance: derived. <https://help.tallysolutions.com/import-data-faq/> (read 2026-11-27), on
// "Overwrite voucher when a voucher with same GUID exists" (Alt+O, Import, Configuration):
// - Yes: "the existing voucher gets overwritten";
// - No: "a duplicate voucher gets created. The subsequent import of the same voucher will
//   overwrite the previously duplicated/created voucher".
// The default is No (the coordinator's brief; the FAQ states none). Counts by hand from the files.

fn vouchers(api: &Api) -> Vec<ledgerly_model::Voucher> {
    books(api).vouchers.values().cloned().collect()
}

#[test]
fn with_overwrite_off_a_second_import_duplicates_and_a_third_overwrites_the_duplicate() {
    let (dir, mut api) = with_parties();
    let first = write(
        dir.path(),
        "a.xml",
        &envelope(&sale("Walk-in", "g-1", "first")),
    );
    let behaviour = XmlImport::default_transactions();
    assert_eq!(
        behaviour,
        XmlImport::Transactions(VoucherBehaviour {
            overwrite_same_guid: false
        })
    );
    assert_eq!(
        counts(&import(&mut api, behaviour, &first)),
        [1, 0, 0, 0, 0]
    );
    let again = import(&mut api, behaviour, &first);
    assert_eq!(counts(&again), [1, 0, 0, 0, 0], "{again}");
    assert!(
        again
            .warnings
            .iter()
            .any(|w| w.to_string().contains("duplicate")),
        "{:?}",
        again.warnings
    );
    assert_eq!(vouchers(&api).len(), 2);
    let changed = write(
        dir.path(),
        "b.xml",
        &envelope(&sale("Walk-in", "g-1", "third")),
    );
    let third = import(&mut api, behaviour, &changed);
    assert_eq!(counts(&third), [0, 1, 0, 0, 0], "{third}");
    let after = vouchers(&api);
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].narration, "first");
    assert_eq!(after[1].narration, "third");
}

#[test]
fn with_overwrite_on_the_voucher_with_the_same_guid_is_overwritten() {
    let (dir, mut api) = with_parties();
    let overwrite = XmlImport::Transactions(VoucherBehaviour {
        overwrite_same_guid: true,
    });
    let first = write(
        dir.path(),
        "a.xml",
        &envelope(&sale("Customer ABC", "g-1", "first")),
    );
    assert_eq!(
        counts(&import(&mut api, overwrite, &first)),
        [1, 0, 0, 0, 0]
    );
    let changed = write(
        dir.path(),
        "b.xml",
        &envelope(&sale("Customer ABC", "g-1", "second")),
    );
    let s = import(&mut api, overwrite, &changed);
    assert_eq!(counts(&s), [0, 1, 0, 0, 0], "{s}");
    let after = vouchers(&api);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].narration, "second");
    // The same file a third time leaves one voucher.
    assert_eq!(
        counts(&import(&mut api, overwrite, &changed)),
        [0, 1, 0, 0, 0]
    );
    assert_eq!(vouchers(&api).len(), 1);
}

#[test]
fn a_duplicate_of_a_sale_that_opens_a_bill_is_refused_by_the_bill_check() {
    // derived: with overwrite off the second import posts the sale again, and its New Ref bill
    // "1" on Customer ABC is then opened twice; the engine refuses a bill name opened twice on one
    // ledger, so the duplicate is an exception and the books keep one sale.
    let (dir, mut api) = with_parties();
    let path = write(
        dir.path(),
        "a.xml",
        &envelope(&sale("Customer ABC", "g-1", "first")),
    );
    let behaviour = XmlImport::default_transactions();
    import(&mut api, behaviour, &path);
    let again = import(&mut api, behaviour, &path);
    assert_eq!(counts(&again), [0, 0, 0, 0, 1], "{again}");
    assert!(
        again.exceptions[0].reason.contains("already exists"),
        "{again}"
    );
    assert_eq!(vouchers(&api).len(), 1);
}

// A backup before the import (S22 round 3).
//
// provenance: derived. <https://help.tallysolutions.com/import-data-from-xml-or-json/> (read
// 2026-11-27), masters step 4 and transactions step 3: "Select Yes to take a backup of the
// company data before importing." The backup is taken before anything is imported, so it holds
// the books as they were; a backup that cannot be written stops the import (nothing to restore
// from otherwise). Where it goes is this app's choice (no folder is documented): a `Backups`
// folder beside the company file.

fn back_up_and_import(api: &mut Api, what: XmlImport, path: &Path) -> FileImportSummary {
    match api.back_up_and_import_tally_xml_file(what, path).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("import unsupported: {u:?}"),
    }
}

#[test]
fn a_backup_before_the_import_holds_the_books_as_they_were() {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "masters.xml", &masters_file());
    let s = back_up_and_import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
        &path,
    );
    let backup = s.backup.clone().unwrap();
    assert_eq!(backup.parent(), Some(dir.path().join("Backups").as_path()));
    assert!(
        books(&api)
            .masters
            .ledgers
            .values()
            .any(|l| l.name == "Kumar Textiles")
    );
    let copy = Api::open(&backup).unwrap();
    let before = match copy.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
    assert!(
        !before
            .masters
            .ledgers
            .values()
            .any(|l| l.name == "Kumar Textiles")
    );
    assert!(before.masters.ledgers.values().any(|l| l.name == "Cash"));
}

#[test]
fn an_import_without_a_backup_says_none_was_taken() {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "masters.xml", &masters_file());
    let s = import(
        &mut api,
        XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
        &path,
    );
    assert_eq!(s.backup, None);
    assert!(!dir.path().join("Backups").exists());
}

#[test]
fn a_backup_that_cannot_be_written_stops_the_import() {
    let (dir, mut api) = setup();
    let path = write(dir.path(), "masters.xml", &masters_file());
    // A file where the Backups folder would go: the folder cannot be made.
    std::fs::write(dir.path().join("Backups"), "not a folder").unwrap();
    let err = api
        .back_up_and_import_tally_xml_file(
            XmlImport::Masters(MasterBehaviour::IgnoreDuplicates),
            &path,
        )
        .unwrap_err();
    assert!(matches!(err, ApiError::Store(_)), "{err:?}");
    assert!(
        !books(&api)
            .masters
            .ledgers
            .values()
            .any(|l| l.name == "Kumar Textiles")
    );
}
