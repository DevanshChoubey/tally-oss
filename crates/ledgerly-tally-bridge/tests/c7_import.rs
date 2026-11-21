//! Card C7 tests: opening balances and out-of-period vouchers in the bridge import.
//!
//! Provenance. Every fixture is a fictional, hand-built Tally-shaped export served by a fake
//! transport; none was captured from TallyPrime, so nothing is `observed`. Expected values are
//! `derived`. The sign rule (negative `OPENINGBALANCE` is a debit, positive is a credit) is derived
//! from Tally's developer reference, Case Study I, which imports a bank ledger with
//! `<OPENINGBALANCE>-12500</OPENINGBALANCE>`
//! (<https://help.tallysolutions.com/article/DeveloperReference/integration-capabilities/case_study_1.htm>,
//! read 2026-11-21) and from Tally's import FAQ ("negative ... imported as debit",
//! <https://help.tallysolutions.com/import-data-faq/>, read 2026-11-21). An observed capture of a
//! master's `OPENINGBALANCE` is pending. "Difference in opening balances" is Tally's name for
//! imbalanced openings (<https://help.tallysolutions.com/trial-balance-tally/>, read 2026-11-21).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{CancelToken, Date, Month, Outcome, Paise, Period, SignedAmount};
use ledgerly_model::MasterSnapshot;
use ledgerly_store::Event;
use ledgerly_tally_bridge::import::{ImportedTally, import_company, import_range};
use ledgerly_tally_bridge::{TallyTransport, TransportError};

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

fn journal(guid: &str, yyyymmdd: &str) -> String {
    format!(
        "<VOUCHER VCHTYPE=\"Journal\" ACTION=\"Create\"><GUID>{guid}</GUID><ALTERID>9</ALTERID>\
         <DATE>{yyyymmdd}</DATE><VOUCHERTYPENAME>Journal</VOUCHERTYPENAME>\
         <VOUCHERNUMBER>{guid}</VOUCHERNUMBER>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Rent</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>\
         <AMOUNT>-100.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>\
         <AMOUNT>100.00</AMOUNT></ALLLEDGERENTRIES.LIST><ISCANCELLED>No</ISCANCELLED>\
         <ISOPTIONAL>No</ISOPTIONAL><ISPOSTDATED>No</ISPOSTDATED><ISINVOICE>No</ISINVOICE>\
         </VOUCHER>"
    )
}

struct Fake {
    ledgers: String,
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
        let groups = "<GROUP NAME=\"Current Assets\"><GUID>g-ca</GUID><ALTERID>1</ALTERID>\
                      <NAME>Current Assets</NAME></GROUP>\
                      <GROUP NAME=\"Cash-in-Hand\"><GUID>g-cih</GUID><ALTERID>2</ALTERID>\
                      <NAME>Cash-in-Hand</NAME><PARENT>Current Assets</PARENT></GROUP>";
        let vts = "<VOUCHERTYPE NAME=\"Journal\"><GUID>vt-j</GUID><ALTERID>4</ALTERID>\
                   <NAME>Journal</NAME></VOUCHERTYPE>";
        let body = if xml_request.contains("<ID>LedgerlyGroups</ID>") {
            groups.to_owned()
        } else if xml_request.contains("<ID>LedgerlyLedgers</ID>") {
            self.ledgers.clone()
        } else if xml_request.contains("<ID>LedgerlyVoucherTypes</ID>") {
            vts.to_owned()
        } else {
            String::new()
        };
        Ok(wrap(&body))
    }
}

fn period() -> Period {
    import_range(
        Date::from_calendar_date(2026, Month::April, 1).unwrap(),
        Date::from_calendar_date(2026, Month::November, 21).unwrap(),
    )
    .unwrap()
}

fn run(fake: &Fake) -> Outcome<ImportedTally> {
    let dir = tempfile::tempdir().unwrap();
    import_company(fake, "Test Traders", period(), dir.path(), &CancelToken::new()).unwrap()
}

fn fake(cash: &str, capital: &str, vouchers: Vec<(&'static str, String)>) -> Fake {
    Fake {
        ledgers: [
            ledger("Cash", "Cash-in-Hand", cash),
            ledger("Rent", "Indirect Expenses", "0"),
            ledger("Capital", "Capital Account", capital),
        ]
        .concat(),
        vouchers,
    }
}

fn opening_of(imported: &ImportedTally, name: &str) -> SignedAmount {
    imported
        .events
        .iter()
        .find_map(|e| {
            if let Event::MasterCreated(MasterSnapshot::Ledger(l)) = e
                && l.name == name
            {
                Some(l.opening)
            } else {
                None
            }
        })
        .unwrap()
}

#[test]
fn a_negative_opening_is_a_debit_and_a_positive_one_a_credit() {
    // provenance: derived — Cash -10000.00 is Dr 10,000.00 = 1,000,000 paise; Capital 10000.00 is
    // Cr 10,000.00 (sign rule in the file header). They net to zero: no difference warning.
    let Outcome::Value(imported) = run(&fake("-10000.00", "10000.00", Vec::new())) else {
        panic!("expected an import");
    };
    assert_eq!(
        opening_of(&imported, "Cash"),
        SignedAmount::dr(Paise::new(1_000_000))
    );
    assert_eq!(
        opening_of(&imported, "Capital"),
        SignedAmount::cr(Paise::new(1_000_000))
    );
    assert_eq!(
        opening_of(&imported, "Rent"),
        SignedAmount::dr(Paise::ZERO)
    );
    let difference: Vec<_> = imported
        .warnings
        .iter()
        .filter(|w| w.message.contains("do not balance"))
        .collect();
    assert_eq!(difference, Vec::new());
}

#[test]
fn imbalanced_openings_are_imported_and_the_difference_is_warned() {
    // provenance: derived — Dr 10,000.00 less Cr 7,500.50 leaves a debit difference of 2,499.50.
    let Outcome::Value(imported) = run(&fake("-10000.00", "7500.50", Vec::new())) else {
        panic!("expected an import");
    };
    assert_eq!(
        opening_of(&imported, "Capital"),
        SignedAmount::cr(Paise::new(750_050))
    );
    assert!(
        imported
            .warnings
            .iter()
            .any(|w| w.message.contains("2499.50 debit")),
        "{:?}",
        imported.warnings
    );
}

#[test]
fn an_opening_with_more_than_two_decimals_refuses_the_import() {
    // provenance: derived — a paisa is the smallest amount; 10.005 cannot be stored exactly, so
    // the import refuses instead of rounding (never a wrong number).
    match run(&fake("-10.005", "0", Vec::new())) {
        Outcome::Unsupported(u) => assert!(u.what.contains("Cash"), "{u:?}"),
        Outcome::Value(_) => panic!("must refuse"),
    }
}

#[test]
fn a_ledger_without_an_opening_is_still_refused() {
    // provenance: derived — an absent opening is unknown, not zero (unchanged behaviour).
    let mut f = fake("0", "0", Vec::new());
    f.ledgers = f.ledgers.replace("<OPENINGBALANCE>0</OPENINGBALANCE>", "");
    assert!(matches!(run(&f), Outcome::Unsupported(_)));
}

#[test]
fn vouchers_outside_the_period_are_reported_not_skipped_silently() {
    // provenance: derived — the period is 1 April 2026 to 31 March 2027. One voucher is dated
    // 15 March 2026 (before) and one 10 April 2027 (after); one is inside. Two are reported.
    let vouchers = vec![
        ("20260315", journal("v-before", "20260315")),
        ("20260410", journal("v-in", "20260410")),
        ("20270410", journal("v-after", "20270410")),
    ];
    let Outcome::Value(imported) = run(&fake("0", "0", vouchers)) else {
        panic!("expected an import");
    };
    let posted = imported
        .events
        .iter()
        .filter(|e| matches!(e, Event::VoucherPosted(_)))
        .count();
    assert_eq!(posted, 1);
    let outside: Vec<&str> = imported
        .warnings
        .iter()
        .filter(|w| w.message.contains("outside the import period"))
        .map(|w| w.message.as_str())
        .collect();
    assert_eq!(outside.len(), 2, "{:?}", imported.warnings);
    assert!(outside.iter().any(|m| m.contains("1 voucher(s) dated 2026-03-15")));
    assert!(outside.iter().any(|m| m.contains("1 voucher(s) dated 2027-04-10")));
}

#[test]
fn nothing_outside_the_period_means_no_such_warning() {
    // provenance: derived — the only voucher is inside the period.
    let vouchers = vec![("20260410", journal("v-in", "20260410"))];
    let Outcome::Value(imported) = run(&fake("0", "0", vouchers)) else {
        panic!("expected an import");
    };
    let outside: Vec<_> = imported
        .warnings
        .iter()
        .filter(|w| w.message.contains("outside the import period"))
        .collect();
    assert_eq!(outside, Vec::new());
}
