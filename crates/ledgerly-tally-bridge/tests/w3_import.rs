//! Card W3 tests: the bridge import (spec 6.7 steps 2 to 4) against a fake Tally.
//!
//! Provenance. Every fixture is a fictional, hand-built Tally-shaped export served by a fake
//! transport. None was captured from TallyPrime, so nothing here is `observed`. Each expected
//! value is `derived`: arithmetic on the fixture's own amounts, a count of the events the mapping
//! must emit, or a rule from spec 6.7 or the seed (the financial year runs 1 April to 31 March).
//! Expected events are counted by hand from the fixture, never copied from the code under test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::cell::Cell;

use ledgerly_core::{CancelToken, Date, Month, Outcome, Period, VoucherTypeId};
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{BaseVoucherKind, MasterSnapshot};
use ledgerly_store::Event;
use ledgerly_tally_bridge::import::{ImportError, import_company, import_range};
use ledgerly_tally_bridge::pull::{MANIFEST_FILE, PullError};
use ledgerly_tally_bridge::{TallyTransport, TransportError};

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

/// Wraps collection children in a Tally response envelope.
fn wrap(inner: &str) -> String {
    format!(
        "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>1</STATUS></HEADER><BODY><DATA>\
         <COLLECTION>{inner}</COLLECTION></DATA></BODY></ENVELOPE>"
    )
}

/// The text between `<tag>` and `</tag>` in `text`.
fn between<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let (_, rest) = text.split_once(format!("<{tag}>").as_str())?;
    let (value, _) = rest.split_once(format!("</{tag}>").as_str())?;
    Some(value)
}

/// Groups: `Cash-in-Hand` is a default group under `Current Assets`, so it is altered, not created.
fn groups_xml() -> String {
    "<GROUP NAME=\"Current Assets\"><GUID>g-ca</GUID><ALTERID>1</ALTERID>\
     <NAME>Current Assets</NAME></GROUP>\
     <GROUP NAME=\"Cash-in-Hand\"><GUID>g-cih</GUID><ALTERID>2</ALTERID>\
     <NAME>Cash-in-Hand</NAME><PARENT>Current Assets</PARENT></GROUP>"
        .to_owned()
}

/// Ledgers with a zero opening balance, under default groups.
fn ledgers_xml() -> String {
    [
        ("Cash", "Cash-in-Hand", "l-cash"),
        ("Rent", "Indirect Expenses", "l-rent"),
        ("Capital", "Capital Account", "l-capital"),
    ]
    .iter()
    .map(|(name, parent, guid)| {
        format!(
            "<LEDGER NAME=\"{name}\"><GUID>{guid}</GUID><ALTERID>3</ALTERID>\
             <NAME>{name}</NAME><PARENT>{parent}</PARENT>\
             <OPENINGBALANCE>0</OPENINGBALANCE></LEDGER>"
        )
    })
    .collect::<Vec<String>>()
    .concat()
}

fn voucher_types_xml() -> String {
    "<VOUCHERTYPE NAME=\"Journal\"><GUID>vt-journal</GUID><ALTERID>4</ALTERID>\
     <NAME>Journal</NAME></VOUCHERTYPE>"
        .to_owned()
}

/// A journal voucher. Each entry is (ledger, `Yes` for a debit or `No` for a credit, amount in
/// Tally's signed text: negative for a debit).
fn journal(guid: &str, yyyymmdd: &str, number: &str, entries: &[(&str, &str, &str)]) -> String {
    let lines = entries
        .iter()
        .map(|(ledger, positive, amount)| {
            format!(
                "<ALLLEDGERENTRIES.LIST><LEDGERNAME>{ledger}</LEDGERNAME>\
                 <ISDEEMEDPOSITIVE>{positive}</ISDEEMEDPOSITIVE><AMOUNT>{amount}</AMOUNT>\
                 </ALLLEDGERENTRIES.LIST>"
            )
        })
        .collect::<Vec<String>>()
        .concat();
    format!(
        "<VOUCHER VCHTYPE=\"Journal\" ACTION=\"Create\"><GUID>{guid}</GUID><ALTERID>9</ALTERID>\
         <DATE>{yyyymmdd}</DATE><VOUCHERTYPENAME>Journal</VOUCHERTYPENAME>\
         <VOUCHERNUMBER>{number}</VOUCHERNUMBER>{lines}<ISCANCELLED>No</ISCANCELLED>\
         <ISOPTIONAL>No</ISOPTIONAL><ISPOSTDATED>No</ISPOSTDATED><ISINVOICE>No</ISINVOICE>\
         </VOUCHER>"
    )
}

/// Rent of 1,000.00 paid from cash on 10 April 2026, and 500.00 paid into cash on 15 June 2026
/// from capital.
fn two_journals() -> Vec<(&'static str, String)> {
    vec![
        (
            "20260410",
            journal(
                "v-1",
                "20260410",
                "J-1",
                &[("Rent", "Yes", "-1000.00"), ("Cash", "No", "1000.00")],
            ),
        ),
        (
            "20260615",
            journal(
                "v-2",
                "20260615",
                "J-2",
                &[("Cash", "Yes", "-500.00"), ("Capital", "No", "500.00")],
            ),
        ),
    ]
}

/// A fake Tally: masters are served by collection id, vouchers by the requested date window.
struct FakeTally {
    masters: Vec<(&'static str, String)>,
    vouchers: Vec<(&'static str, String)>,
    requests: Cell<usize>,
}

impl FakeTally {
    fn standard() -> Self {
        FakeTally {
            masters: vec![
                ("LedgerlyGroups", groups_xml()),
                ("LedgerlyLedgers", ledgers_xml()),
                ("LedgerlyVoucherTypes", voucher_types_xml()),
            ],
            vouchers: two_journals(),
            requests: Cell::new(0),
        }
    }
}

impl TallyTransport for FakeTally {
    fn post(&self, xml_request: &str) -> Result<String, TransportError> {
        self.requests.set(self.requests.get() + 1);
        if let (Some(from), Some(to)) = (
            between(xml_request, "SVFROMDATE"),
            between(xml_request, "SVTODATE"),
        ) {
            let body: String = self
                .vouchers
                .iter()
                .filter(|(day, _)| *day >= from && *day <= to)
                .map(|(_, xml)| xml.as_str())
                .collect();
            return Ok(wrap(&body));
        }
        let body = self
            .masters
            .iter()
            .find(|(id, _)| xml_request.contains(&format!("<ID>{id}</ID>")))
            .map_or_else(String::new, |(_, xml)| xml.clone());
        Ok(wrap(&body))
    }
}

/// Tally is not running.
struct Down;

impl TallyTransport for Down {
    fn post(&self, _xml_request: &str) -> Result<String, TransportError> {
        Err(TransportError::TallyNotRunning { port: 9000 })
    }
}

/// The voucher period for a books-begin of 1 April 2026 and a current date of 21 November 2026:
/// the financial year that holds the current date ends on 31 March 2027.
fn period() -> Period {
    import_range(d(2026, Month::April, 1), d(2026, Month::November, 21)).unwrap()
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn import_range_is_the_books_beginning_to_the_financial_year_end() {
    // provenance: derived — the financial year runs 1 April to 31 March (spec global constraint
    // "Indian FY starts 1 April"). 21 November 2026 is in FY 2026-27, which ends 31 March 2027.
    assert_eq!(
        import_range(d(2026, Month::April, 1), d(2026, Month::November, 21)).unwrap(),
        Period::new(d(2026, Month::April, 1), d(2027, Month::March, 31)).unwrap()
    );
    // provenance: derived — 1 March 2027 is still in FY 2026-27 (before 1 April 2027).
    assert_eq!(
        import_range(d(2026, Month::April, 1), d(2027, Month::March, 1)).unwrap(),
        Period::new(d(2026, Month::April, 1), d(2027, Month::March, 31)).unwrap()
    );
    // provenance: derived — 1 April 2027 starts FY 2027-28, which ends 31 March 2028.
    assert_eq!(
        import_range(d(2026, Month::April, 1), d(2027, Month::April, 1)).unwrap(),
        Period::new(d(2026, Month::April, 1), d(2028, Month::March, 31)).unwrap()
    );
}

#[test]
fn import_range_refuses_books_that_begin_after_the_year_end() {
    // provenance: derived — books beginning 1 April 2030 cannot be imported into FY 2026-27.
    assert_eq!(
        import_range(d(2030, Month::April, 1), d(2026, Month::November, 21)),
        Err(ImportError::DateRange)
    );
}

#[test]
fn imported_events_match_a_hand_derivation_of_the_fixture() {
    let dir = tmp();
    let tally = FakeTally::standard();
    let imported = match import_company(
        &tally,
        "Test Traders",
        period(),
        dir.path(),
        &CancelToken::new(),
    )
    .unwrap()
    {
        Outcome::Value(imported) => imported,
        Outcome::Unsupported(u) => panic!("expected an import, got {u:?}"),
    };

    // provenance: derived — the fixture holds 2 groups, 3 ledgers, 1 voucher type and 2 vouchers.
    assert_eq!(imported.counts.masters.get("group"), Some(&2));
    assert_eq!(imported.counts.masters.get("ledger"), Some(&3));
    assert_eq!(imported.counts.masters.get("voucher type"), Some(&1));
    assert_eq!(imported.counts.vouchers.get("Journal"), Some(&2));
    // provenance: derived — the financial year 2026-27 holds 365 days (February 2027 has 28 days).
    // Windows of 31 days: 11 x 31 = 341, then one window of the remaining 24 days: 12 windows.
    // Windows are accepted whether or not they hold vouchers.
    assert_eq!(imported.counts.voucher_windows, 12);

    // provenance: derived — 2 group alterations (both default groups exist), 3 ledger creations,
    // 1 voucher type alteration (Journal is a default type), 2 voucher postings: 8 events.
    assert_eq!(imported.events.len(), 8);

    let journal_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Journal)
        .unwrap()
        .id;
    let posted: Vec<VoucherTypeId> = imported
        .events
        .iter()
        .filter_map(|e| match e {
            Event::VoucherPosted(v) => Some(v.type_id),
            Event::MasterCreated(_)
            | Event::MasterAltered { .. }
            | Event::MasterDeleted(_)
            | Event::VoucherAltered { .. }
            | Event::VoucherCancelled { .. }
            | Event::VoucherDeleted { .. }
            | Event::SettingChanged { .. }
            | Event::PeriodLocked { .. }
            | Event::StatementImported(_)
            | Event::BankStatementStored { .. }
            | Event::Gstr2bImported { .. }
            | Event::ImsActionRecorded { .. }
            | Event::Gstr3bFiled { .. }
            | Event::MatchConfirmed(_)
            | Event::AiProposalRecorded(_) => None,
        })
        .collect();
    // provenance: derived — both vouchers are Journal vouchers, so they point at the default
    // Journal type's id (the mapping numbered the Tally type 1; the import re-points it).
    assert_eq!(posted, vec![journal_id, journal_id]);

    // The Journal type is altered in place: its before is the default, its after keeps the id.
    let altered: Vec<&MasterSnapshot> = imported
        .events
        .iter()
        .filter_map(|e| match e {
            Event::MasterAltered { after, .. } => Some(after),
            Event::MasterCreated(_)
            | Event::VoucherPosted(_)
            | Event::MasterDeleted(_)
            | Event::VoucherAltered { .. }
            | Event::VoucherCancelled { .. }
            | Event::VoucherDeleted { .. }
            | Event::SettingChanged { .. }
            | Event::PeriodLocked { .. }
            | Event::StatementImported(_)
            | Event::BankStatementStored { .. }
            | Event::Gstr2bImported { .. }
            | Event::ImsActionRecorded { .. }
            | Event::Gstr3bFiled { .. }
            | Event::MatchConfirmed(_)
            | Event::AiProposalRecorded(_) => None,
        })
        .collect();
    assert!(altered.iter().any(|after| matches!(
        after,
        MasterSnapshot::VoucherType(vt)
            if vt.id == journal_id && vt.base == BaseVoucherKind::Journal
    )));
}

#[test]
fn a_tally_voucher_type_with_a_default_name_and_other_behaviour_is_refused() {
    let dir = tmp();
    let mut tally = FakeTally::standard();
    // Keep the Journal type the vouchers use, and add "Sales" under Journal: Ledgerly's default
    // "Sales" is a Sales type, so the two cannot merge.
    tally.masters[2].1.push_str(
        "<VOUCHERTYPE NAME=\"Sales\"><GUID>vt-sales</GUID><ALTERID>4</ALTERID>\
         <NAME>Sales</NAME><PARENT>Journal</PARENT></VOUCHERTYPE>",
    );
    match import_company(
        &tally,
        "Test Traders",
        period(),
        dir.path(),
        &CancelToken::new(),
    )
    .unwrap()
    {
        Outcome::Unsupported(u) => assert!(u.what.contains("Sales"), "{}", u.what),
        Outcome::Value(_) => panic!("a clashing voucher type must refuse the import"),
    }
}

#[test]
fn an_inventory_line_refuses_the_whole_import() {
    let dir = tmp();
    let mut tally = FakeTally::standard();
    let with_stock = String::from(
        "<VOUCHER VCHTYPE=\"Journal\" ACTION=\"Create\"><GUID>v-stock</GUID><ALTERID>9</ALTERID>\
         <DATE>20260410</DATE><VOUCHERTYPENAME>Journal</VOUCHERTYPENAME>\
         <VOUCHERNUMBER>J-9</VOUCHERNUMBER>\
         <ALLINVENTORYENTRIES.LIST><STOCKITEMNAME>Widget</STOCKITEMNAME>\
         <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>100.00</AMOUNT>\
         </ALLINVENTORYENTRIES.LIST>\
         <ISCANCELLED>No</ISCANCELLED><ISOPTIONAL>No</ISOPTIONAL>\
         <ISPOSTDATED>No</ISPOSTDATED><ISINVOICE>No</ISINVOICE></VOUCHER>",
    );
    tally.vouchers.push(("20260410", with_stock));
    match import_company(
        &tally,
        "Test Traders",
        period(),
        dir.path(),
        &CancelToken::new(),
    )
    .unwrap()
    {
        Outcome::Unsupported(u) => {
            assert!(u.what.contains("inventory"), "{}", u.what);
            assert!(u.reason.contains("Nothing was imported"), "{}", u.reason);
        }
        Outcome::Value(_) => panic!("an inventory voucher must refuse the whole import"),
    }
}

#[test]
fn a_cancel_before_the_first_request_stops_the_pull_with_nothing_read() {
    let dir = tmp();
    let tally = FakeTally::standard();
    let cancel = CancelToken::new();
    cancel.cancel();
    let err = import_company(&tally, "Test Traders", period(), dir.path(), &cancel).unwrap_err();
    // provenance: derived — the pull checks cancellation before its first request, so no master
    // collection and no voucher window has been accepted.
    assert_eq!(
        err,
        ImportError::Pull(PullError::Cancelled {
            master_files_done: 0,
            batches_done: 0,
        })
    );
    assert_eq!(tally.requests.get(), 0);
}

#[test]
fn tally_not_running_is_an_error_naming_the_port() {
    let dir = tmp();
    let err = import_company(
        &Down,
        "Test Traders",
        period(),
        dir.path(),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ImportError::Pull(PullError::Transport(TransportError::TallyNotRunning {
            port: 9000
        }))
    );
}

#[test]
fn raw_responses_are_captured_beside_the_import() {
    let dir = tmp();
    let tally = FakeTally::standard();
    let _ = import_company(
        &tally,
        "Test Traders",
        period(),
        dir.path(),
        &CancelToken::new(),
    )
    .unwrap();
    // provenance: derived — spec 6.7 step 2: the capture directory holds the manifest that lists
    // the accepted files.
    assert!(dir.path().join(MANIFEST_FILE).is_file());
    assert!(dir.path().join("masters-02-ledgers.xml").is_file());
}
