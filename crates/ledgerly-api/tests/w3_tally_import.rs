//! Card W3 tests: importing a Tally company into a company file through the facade (spec 6.7).
//!
//! Provenance. Every Tally export here is a fictional, hand-built fixture served by a fake
//! transport; none was captured from TallyPrime, so nothing is `observed`. Expected values are
//! `derived`: the amounts in each fixture voucher, added by hand. Paise: Rs 1,000 is `100_000`.
//! The trial balance is read through the report engine, as the W1 tests do.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::cell::Cell;
use std::path::PathBuf;

use ledgerly_api::{Api, ApiError, Command, ImportSummary};
use ledgerly_core::{
    CancelToken, Clock, CompanyId, Date, Outcome, Paise, Period, SignedAmount, StateCode,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Ledger, NegativeStockPolicy, Scenario,
};
use ledgerly_reports::{
    Cell as ReportCell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams,
};
use ledgerly_tally_bridge::import::ImportError;
use ledgerly_tally_bridge::pull::PullError;
use ledgerly_tally_bridge::{TallyTransport, TransportError};
use time::OffsetDateTime;
use time::macros::date;

/// The clock for the tests: 21 November 2026, so the import period ends 31 March 2027.
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

/// A new company file, seeded as `Api::create` does, with the clock above.
fn setup() -> (tempfile::TempDir, PathBuf, Api) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let api = Api::create(&path, &company(), clock()).unwrap();
    (dir, path, api)
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

/// A fake Tally: masters are served by collection id, vouchers by the requested date window.
struct FakeTally {
    masters: Vec<(&'static str, String)>,
    vouchers: Vec<(&'static str, String)>,
    requests: Cell<usize>,
}

impl FakeTally {
    /// Groups `Current Assets` and `Cash-in-Hand`, ledgers Cash, Rent and Capital, the Journal
    /// voucher type, and two Journal vouchers.
    ///
    /// Hand derivation of the trial balance at 31 March 2027:
    /// - v-1, 10 April 2026: Dr Rent 1,000.00, Cr Cash 1,000.00.
    /// - v-2, 15 June 2026: Dr Cash 500.00, Cr Capital 500.00.
    /// - Rent: Dr 1,000.00 = 100,000 paise.
    /// - Cash: Cr 1,000.00 less Dr 500.00 = Cr 500.00 = 50,000 paise.
    /// - Capital: Cr 500.00 = 50,000 paise.
    /// - Totals: Dr 100,000 and Cr 100,000 (Cash 50,000 plus Capital 50,000).
    fn standard() -> Self {
        let groups = "<GROUP NAME=\"Current Assets\"><GUID>g-ca</GUID><ALTERID>1</ALTERID>\
                      <NAME>Current Assets</NAME></GROUP>\
                      <GROUP NAME=\"Cash-in-Hand\"><GUID>g-cih</GUID><ALTERID>2</ALTERID>\
                      <NAME>Cash-in-Hand</NAME><PARENT>Current Assets</PARENT></GROUP>";
        let ledgers = [
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
        .concat();
        let voucher_types = "<VOUCHERTYPE NAME=\"Journal\"><GUID>vt-journal</GUID>\
                             <ALTERID>4</ALTERID><NAME>Journal</NAME></VOUCHERTYPE>";
        FakeTally {
            masters: vec![
                ("LedgerlyGroups", groups.to_owned()),
                ("LedgerlyLedgers", ledgers),
                ("LedgerlyVoucherTypes", voucher_types.to_owned()),
            ],
            vouchers: vec![
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
            ],
            requests: Cell::new(0),
        }
    }

    fn request_count(&self) -> usize {
        self.requests.get()
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

/// Tally is not running on port 9000.
struct Down;

impl TallyTransport for Down {
    fn post(&self, _xml_request: &str) -> Result<String, TransportError> {
        Err(TransportError::TallyNotRunning { port: 9000 })
    }
}

fn params(kind: ReportKind, as_of: Date) -> ReportParams {
    ReportParams {
        kind,
        period: Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap(),
        as_of,
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn trial_balance(api: &Api) -> Report {
    api.report(&params(ReportKind::TrialBalance, date!(2027 - 03 - 31)))
        .unwrap()
        .value()
        .unwrap()
}

fn row_cells(report: &Report, label: &str) -> Option<Vec<ReportCell>> {
    report
        .rows
        .iter()
        .find(|row| row.label == label)
        .map(|row| row.cells.clone())
}

/// The summary of an import that must have gone through.
fn committed(out: Result<Outcome<ImportSummary>, ApiError>) -> ImportSummary {
    match out {
        Ok(Outcome::Value(summary)) => summary,
        other => panic!("expected a committed import, got {other:?}"),
    }
}

#[test]
fn import_into_an_empty_company_matches_the_hand_trial_balance() {
    let (_dir, _path, mut api) = setup();
    let tally = FakeTally::standard();
    let summary = committed(api.import_tally_with(&tally, "Test Traders", &CancelToken::new()));

    // provenance: derived — 2 group alterations, 3 ledger creations, 1 voucher type alteration
    // and 2 voucher postings: 8 events.
    assert_eq!(summary.events, 8);
    assert_eq!(summary.masters.get("ledger"), Some(&3));
    assert_eq!(summary.vouchers.get("Journal"), Some(&2));
    assert_eq!(summary.period.from(), date!(2026 - 04 - 01));
    assert_eq!(summary.period.to(), date!(2027 - 03 - 31));
    assert!(summary.to_string().contains("2 Journal voucher(s)"));

    // provenance: derived — the hand trial balance in FakeTally::standard.
    let tb = trial_balance(&api);
    assert_eq!(
        row_cells(&tb, "Rent"),
        Some(vec![
            ReportCell::Amount(SignedAmount::dr(Paise::new(100_000))),
            ReportCell::Empty
        ])
    );
    assert_eq!(
        row_cells(&tb, "Cash"),
        Some(vec![
            ReportCell::Empty,
            ReportCell::Amount(SignedAmount::cr(Paise::new(50_000)))
        ])
    );
    assert_eq!(
        row_cells(&tb, "Capital"),
        Some(vec![
            ReportCell::Empty,
            ReportCell::Amount(SignedAmount::cr(Paise::new(50_000)))
        ])
    );
    assert_eq!(
        tb.totals,
        vec![
            ReportCell::Amount(SignedAmount::dr(Paise::new(100_000))),
            ReportCell::Amount(SignedAmount::cr(Paise::new(100_000)))
        ]
    );
}

#[test]
fn a_second_import_into_the_same_company_is_refused_and_changes_nothing() {
    let (_dir, _path, mut api) = setup();
    let tally = FakeTally::standard();
    committed(api.import_tally_with(&tally, "Test Traders", &CancelToken::new()));
    let before = trial_balance(&api);
    let requests_before = tally.request_count();

    let second = api
        .import_tally_with(&tally, "Test Traders", &CancelToken::new())
        .unwrap();
    assert!(matches!(second, Outcome::Unsupported(_)), "{second:?}");
    // provenance: derived — the company holds data, so Tally is not contacted at all.
    assert_eq!(tally.request_count(), requests_before);
    assert_eq!(trial_balance(&api), before);
}

#[test]
fn a_company_with_a_ledger_is_refused_before_tally_is_contacted() {
    let (_dir, _path, mut api) = setup();
    let group = reserved_groups()
        .into_iter()
        .find(|g| g.name == "Cash-in-Hand")
        .unwrap()
        .id;
    let cash = Ledger {
        ineligible_itc: None,
        id: ledgerly_core::LedgerId(1),
        name: "Cash".to_owned(),
        aliases: Vec::new(),
        group: Some(group),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    };
    api.execute(Command::CreateLedger(cash)).unwrap();

    let tally = FakeTally::standard();
    let out = api
        .import_tally_with(&tally, "Test Traders", &CancelToken::new())
        .unwrap();
    assert!(matches!(out, Outcome::Unsupported(_)), "{out:?}");
    // provenance: derived — the refusal is decided on the company's books, before any pull.
    assert_eq!(tally.request_count(), 0);
}

#[test]
fn an_inventory_line_refuses_the_whole_import_and_commits_nothing() {
    let (_dir, _path, mut api) = setup();
    let mut tally = FakeTally::standard();
    let with_stock = "<VOUCHER VCHTYPE=\"Journal\" ACTION=\"Create\"><GUID>v-stock</GUID>\
         <ALTERID>9</ALTERID><DATE>20260410</DATE>\
         <VOUCHERTYPENAME>Journal</VOUCHERTYPENAME><VOUCHERNUMBER>J-9</VOUCHERNUMBER>\
         <ALLINVENTORYENTRIES.LIST><STOCKITEMNAME>Widget</STOCKITEMNAME>\
         <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>100.00</AMOUNT>\
         </ALLINVENTORYENTRIES.LIST><ISCANCELLED>No</ISCANCELLED><ISOPTIONAL>No</ISOPTIONAL>\
         <ISPOSTDATED>No</ISPOSTDATED><ISINVOICE>No</ISINVOICE></VOUCHER>";
    tally.vouchers.push(("20260410", with_stock.to_owned()));

    let out = api
        .import_tally_with(&tally, "Test Traders", &CancelToken::new())
        .unwrap();
    assert!(matches!(out, Outcome::Unsupported(_)), "{out:?}");
    // provenance: derived — the mapping refuses the whole import on the first inventory line, so
    // none of the ledger postings in the fixture are committed.
    let tb = trial_balance(&api);
    assert_eq!(row_cells(&tb, "Rent"), None);
    assert_eq!(row_cells(&tb, "Cash"), None);
}

#[test]
fn a_cancelled_import_commits_nothing() {
    let (_dir, _path, mut api) = setup();
    let tally = FakeTally::standard();
    let cancel = CancelToken::new();
    cancel.cancel();
    match api.import_tally_with(&tally, "Test Traders", &cancel) {
        Err(ApiError::Import(ImportError::Pull(PullError::Cancelled { .. }))) => {}
        other => panic!("expected a cancelled pull, got {other:?}"),
    }
    assert_eq!(row_cells(&trial_balance(&api), "Rent"), None);
}

#[test]
fn tally_not_running_is_reported_and_nothing_is_committed() {
    let (_dir, _path, mut api) = setup();
    match api.import_tally_with(&Down, "Test Traders", &CancelToken::new()) {
        Err(ApiError::Import(ImportError::Pull(PullError::Transport(
            TransportError::TallyNotRunning { port: 9000 },
        )))) => {}
        other => panic!("expected Tally not running on port 9000, got {other:?}"),
    }
    assert_eq!(row_cells(&trial_balance(&api), "Rent"), None);
}

#[test]
fn raw_tally_responses_are_kept_beside_the_company_file() {
    let (dir, _path, mut api) = setup();
    let tally = FakeTally::standard();
    committed(api.import_tally_with(&tally, "Test Traders", &CancelToken::new()));
    // The folder is named after the company file with `.tally-capture` appended.
    let capture = dir.path().join("company.ledgerly.tally-capture");
    assert!(capture.join("manifest.tsv").is_file());
    assert!(capture.join("masters-02-ledgers.xml").is_file());
}

#[test]
fn the_summary_names_the_company_and_lists_warnings() {
    let (_dir, _path, mut api) = setup();
    let tally = FakeTally::standard();
    let summary = committed(api.import_tally_with(&tally, "Test Traders", &CancelToken::new()));
    assert_eq!(summary.company, "Test Traders");
    // The voucher type numbering is not imported, and the summary says so.
    assert!(
        summary
            .warnings
            .iter()
            .any(|w| w.contains("numbering settings are not imported")),
        "{:?}",
        summary.warnings
    );
    assert!(summary.to_string().contains("Test Traders"));
}

#[test]
fn a_new_company_with_its_predefined_cash_takes_the_import() {
    // provenance: derived. A company made as the setup screen makes it has the predefined Cash
    // ledger (TallyPrime help, "Ledgers in TallyPrime", read 2026-11-27) and nothing else, so it
    // is still a new company to import into. The other program's export carries its own Cash
    // under Cash-in-Hand, which takes the predefined one's place: one Cash, the same hand trial
    // balance as `import_into_an_empty_company_matches_the_hand_trial_balance`.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wizard.ledgerly");
    let mut api = new_company(&path, &company(), clock());
    let tally = FakeTally::standard();
    committed(api.import_tally_with(&tally, "Test Traders", &CancelToken::new()));
    let books = match api.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
    let cash: Vec<_> = books
        .masters
        .ledgers
        .values()
        .filter(|l| l.name == "Cash")
        .collect();
    assert_eq!(cash.len(), 1, "{cash:?}");
    assert_eq!(cash[0].reserved, Some(ledgerly_model::ReservedLedger::Cash));
    let tb = trial_balance(&api);
    assert_eq!(
        row_cells(&tb, "Cash"),
        Some(vec![
            ReportCell::Empty,
            ReportCell::Amount(SignedAmount::cr(Paise::new(50_000)))
        ])
    );
    assert_eq!(
        tb.totals,
        vec![
            ReportCell::Amount(SignedAmount::dr(Paise::new(100_000))),
            ReportCell::Amount(SignedAmount::cr(Paise::new(100_000)))
        ]
    );
}

/// A company made as the setup screen makes it.
fn new_company(path: &std::path::Path, company: &Company, clock: Clock) -> Api {
    match Api::create_company(path, company, clock).unwrap() {
        Outcome::Value(api) => api,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    }
}
