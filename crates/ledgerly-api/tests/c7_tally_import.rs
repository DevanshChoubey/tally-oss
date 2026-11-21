//! Card C7 tests: Tally opening balances and out-of-period vouchers through the facade.
//!
//! Provenance. The Tally export is a fictional, hand-built fixture served by a fake transport;
//! nothing is `observed`. The sign rule (negative `OPENINGBALANCE` is a debit) is derived from
//! Tally's developer reference, Case Study I
//! (<https://help.tallysolutions.com/article/DeveloperReference/integration-capabilities/case_study_1.htm>,
//! read 2026-11-21). Expected amounts are hand derivations written in each test. Paise: Rs 1 is 100.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{Api, ApiError, ImportSummary};
use ledgerly_core::{
    CancelToken, Clock, CompanyId, Outcome, Paise, Period, SignedAmount, StateCode,
};
use ledgerly_model::{
    Address, Company, EntityKind, Features, NegativeStockPolicy, Scenario,
};
use ledgerly_reports::{
    Cell as ReportCell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams,
};
use ledgerly_tally_bridge::{TallyTransport, TransportError};
use time::OffsetDateTime;
use time::macros::date;

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::new_utc(
        date!(2026 - 11 - 21),
        time::Time::MIDNIGHT,
    ))
}

fn setup() -> (tempfile::TempDir, Api) {
    let company = Company {
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
    };
    let dir = tempfile::tempdir().unwrap();
    let api = Api::create(&dir.path().join("company.ledgerly"), &company, clock()).unwrap();
    (dir, api)
}

fn wrap(inner: &str) -> String {
    format!(
        "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>1</STATUS></HEADER><BODY><DATA>\
         <COLLECTION>{inner}</COLLECTION></DATA></BODY></ENVELOPE>"
    )
}

fn between<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let (_, rest) = text.split_once(format!("<{tag}>").as_str())?;
    let (value, _) = rest.split_once(format!("</{tag}>").as_str())?;
    Some(value)
}

fn ledger(name: &str, parent: &str, opening: &str) -> String {
    format!(
        "<LEDGER NAME=\"{name}\"><GUID>l-{name}</GUID><ALTERID>3</ALTERID><NAME>{name}</NAME>\
         <PARENT>{parent}</PARENT><OPENINGBALANCE>{opening}</OPENINGBALANCE></LEDGER>"
    )
}

/// Rent paid from cash, 1,000.00.
fn journal(guid: &str, yyyymmdd: &str) -> String {
    format!(
        "<VOUCHER VCHTYPE=\"Journal\" ACTION=\"Create\"><GUID>{guid}</GUID><ALTERID>9</ALTERID>\
         <DATE>{yyyymmdd}</DATE><VOUCHERTYPENAME>Journal</VOUCHERTYPENAME>\
         <VOUCHERNUMBER>{guid}</VOUCHERNUMBER>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Rent</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>\
         <AMOUNT>-1000.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>\
         <AMOUNT>1000.00</AMOUNT></ALLLEDGERENTRIES.LIST><ISCANCELLED>No</ISCANCELLED>\
         <ISOPTIONAL>No</ISOPTIONAL><ISPOSTDATED>No</ISPOSTDATED><ISINVOICE>No</ISINVOICE>\
         </VOUCHER>"
    )
}

struct Fake {
    vouchers: Vec<(&'static str, String)>,
}

impl TallyTransport for Fake {
    fn post(&self, xml_request: &str) -> Result<String, TransportError> {
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
        let body = if xml_request.contains("<ID>LedgerlyGroups</ID>") {
            "<GROUP NAME=\"Current Assets\"><GUID>g-ca</GUID><ALTERID>1</ALTERID>\
             <NAME>Current Assets</NAME></GROUP>\
             <GROUP NAME=\"Cash-in-Hand\"><GUID>g-cih</GUID><ALTERID>2</ALTERID>\
             <NAME>Cash-in-Hand</NAME><PARENT>Current Assets</PARENT></GROUP>"
                .to_owned()
        } else if xml_request.contains("<ID>LedgerlyLedgers</ID>") {
            [
                ledger("Cash", "Cash-in-Hand", "-10000.00"),
                ledger("Rent", "Indirect Expenses", "0"),
                ledger("Capital", "Capital Account", "10000.00"),
            ]
            .concat()
        } else if xml_request.contains("<ID>LedgerlyVoucherTypes</ID>") {
            "<VOUCHERTYPE NAME=\"Journal\"><GUID>vt-j</GUID><ALTERID>4</ALTERID>\
             <NAME>Journal</NAME></VOUCHERTYPE>"
                .to_owned()
        } else {
            String::new()
        };
        Ok(wrap(&body))
    }
}

fn trial_balance(api: &Api) -> Report {
    api.report(&ReportParams {
        kind: ReportKind::TrialBalance,
        period: Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap(),
        as_of: date!(2027 - 03 - 31),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    })
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

fn committed(out: Result<Outcome<ImportSummary>, ApiError>) -> ImportSummary {
    match out {
        Ok(Outcome::Value(summary)) => summary,
        other => panic!("expected a committed import, got {other:?}"),
    }
}

#[test]
fn opening_balances_import_and_the_trial_balance_matches_the_hand_derivation() {
    // Hand derivation at 31 March 2027:
    // - Cash: opening Dr 10,000.00, journal Cr 1,000.00 => Dr 9,000.00 = 900,000 paise.
    // - Rent: Dr 1,000.00 = 100,000 paise.
    // - Capital: opening Cr 10,000.00 = 1,000,000 paise.
    // - Totals: Dr 900,000 + 100,000 = 1,000,000; Cr 1,000,000.
    let (_dir, mut api) = setup();
    let tally = Fake {
        vouchers: vec![("20260410", journal("v-1", "20260410"))],
    };
    let summary = committed(api.import_tally_with(&tally, "Test Traders", &CancelToken::new()));
    assert_eq!(summary.warnings.iter().filter(|w| w.contains("do not balance")).count(), 0);
    let tb = trial_balance(&api);
    assert_eq!(
        row_cells(&tb, "Cash"),
        Some(vec![
            ReportCell::Amount(SignedAmount::dr(Paise::new(900_000))),
            ReportCell::Empty
        ])
    );
    assert_eq!(
        row_cells(&tb, "Rent"),
        Some(vec![
            ReportCell::Amount(SignedAmount::dr(Paise::new(100_000))),
            ReportCell::Empty
        ])
    );
    assert_eq!(
        row_cells(&tb, "Capital"),
        Some(vec![
            ReportCell::Empty,
            ReportCell::Amount(SignedAmount::cr(Paise::new(1_000_000)))
        ])
    );
    assert_eq!(
        tb.totals,
        vec![
            ReportCell::Amount(SignedAmount::dr(Paise::new(1_000_000))),
            ReportCell::Amount(SignedAmount::cr(Paise::new(1_000_000)))
        ]
    );
}

#[test]
fn a_voucher_outside_the_period_is_in_the_summary_warnings_and_not_imported() {
    // provenance: derived — the period is 1 April 2026 to 31 March 2027; a voucher dated
    // 5 May 2027 is outside it, so one voucher is imported and one warning names the period.
    let (_dir, mut api) = setup();
    let tally = Fake {
        vouchers: vec![
            ("20260410", journal("v-1", "20260410")),
            ("20270505", journal("v-2", "20270505")),
        ],
    };
    let summary = committed(api.import_tally_with(&tally, "Test Traders", &CancelToken::new()));
    assert_eq!(summary.vouchers.get("Journal"), Some(&1));
    assert!(
        summary
            .warnings
            .iter()
            .any(|w| w.contains("1 voucher(s) dated 2027-05-05") && w.contains("were not imported")),
        "{:?}",
        summary.warnings
    );
}
