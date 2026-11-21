//! Card B1 tests: company discovery through a recorded-fixture transport.
//!
//! Provenance. Every reply below is a fictional, hand-written document in the reply shape
//! assumed in `discover.rs` (research R8 2.3 example (b) for the request; the reply shape is
//! not yet observed, so these tests pin our reading of that shape, not Tally's behaviour).
//! Every expected value is `derived`: the literal typed into the fixture (a company name, a
//! port number, an HTTP status), never output copied from our own code.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::cell::RefCell;

use ledgerly_tally_bridge::discover::{BridgeError, list_companies};
use ledgerly_tally_bridge::{TallyTransport, TransportError};

/// Replays one canned result and records the request it was given.
struct Fixture {
    reply: Result<String, TransportError>,
    seen: RefCell<Vec<String>>,
}

impl Fixture {
    fn ok(reply: &str) -> Self {
        Self {
            reply: Ok(reply.to_owned()),
            seen: RefCell::new(Vec::new()),
        }
    }

    fn failing(error: TransportError) -> Self {
        Self {
            reply: Err(error),
            seen: RefCell::new(Vec::new()),
        }
    }
}

impl TallyTransport for Fixture {
    fn post(&self, xml_request: &str) -> Result<String, TransportError> {
        self.seen.borrow_mut().push(xml_request.to_owned());
        self.reply.clone()
    }
}

const TWO_COMPANIES: &str = "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>1</STATUS></HEADER>\
    <BODY><DATA><COLLECTION>\
    <COMPANY NAME=\"Acme Traders\"><NAME TYPE=\"String\">Acme Traders</NAME></COMPANY>\
    <COMPANY NAME=\"Beta Stores (2025-26)\"><NAME TYPE=\"String\">Beta Stores (2025-26)</NAME></COMPANY>\
    </COLLECTION></DATA></BODY></ENVELOPE>";

#[test]
fn lists_the_companies_in_the_reply() {
    let t = Fixture::ok(TWO_COMPANIES);
    let names = list_companies(&t).unwrap();
    assert_eq!(
        names,
        vec![
            "Acme Traders".to_owned(),
            "Beta Stores (2025-26)".to_owned()
        ]
    );
}

#[test]
fn sends_exactly_one_request_and_it_is_an_export() {
    let t = Fixture::ok(TWO_COMPANIES);
    list_companies(&t).unwrap();
    let seen = t.seen.borrow();
    assert_eq!(seen.len(), 1);
    let request = seen.first().unwrap();
    assert!(request.contains("<TALLYREQUEST>Export</TALLYREQUEST>"));
    assert!(request.contains("<TYPE>Company</TYPE>"));
}

#[test]
fn no_company_loaded_is_an_empty_list_not_an_error() {
    let t = Fixture::ok("<ENVELOPE><BODY><DATA><COLLECTION></COLLECTION></DATA></BODY></ENVELOPE>");
    assert_eq!(list_companies(&t).unwrap(), Vec::<String>::new());
}

#[test]
fn connection_refused_names_the_port() {
    let t = Fixture::failing(TransportError::TallyNotRunning { port: 9000 });
    let err = list_companies(&t).unwrap_err();
    assert_eq!(
        err,
        BridgeError::Transport(TransportError::TallyNotRunning { port: 9000 })
    );
    assert!(err.to_string().contains("port 9000"));
}

#[test]
fn http_error_status_is_kept() {
    let t = Fixture::failing(TransportError::Http { status: 503 });
    assert_eq!(
        list_companies(&t).unwrap_err(),
        BridgeError::Transport(TransportError::Http { status: 503 })
    );
}

#[test]
fn timeout_is_kept() {
    let t = Fixture::failing(TransportError::Timeout { seconds: 60 });
    assert_eq!(
        list_companies(&t).unwrap_err(),
        BridgeError::Transport(TransportError::Timeout { seconds: 60 })
    );
}

#[test]
fn tally_reporting_an_error_is_not_an_empty_list() {
    let t = Fixture::ok("<RESPONSE>Unknown Request, cannot be processed</RESPONSE>");
    assert_eq!(
        list_companies(&t).unwrap_err(),
        BridgeError::TallyReported {
            message: "Unknown Request, cannot be processed".to_owned()
        }
    );
}

#[test]
fn truncated_reply_is_an_error_with_a_position() {
    // The reply stops after the first company, inside <COLLECTION>: the error must carry the
    // byte position where the text ends, which is the length of the literal below.
    let reply = "<ENVELOPE><BODY><COLLECTION><COMPANY NAME=\"Acme\"/>";
    let t = Fixture::ok(reply);
    match list_companies(&t).unwrap_err() {
        BridgeError::MalformedResponse { offset, .. } => assert_eq!(offset, reply.len()),
        other @ (BridgeError::Transport(_)
        | BridgeError::Xml(_)
        | BridgeError::UnexpectedResponse { .. }
        | BridgeError::TallyReported { .. }
        | BridgeError::Cancelled
        | BridgeError::Io { .. }
        | BridgeError::Pull(_)
        | BridgeError::Report { .. }) => panic!("expected MalformedResponse, got {other:?}"),
    }
}

#[test]
fn html_error_page_is_unexpected_not_empty() {
    let t = Fixture::ok("<HTML><BODY>Bad Gateway</BODY></HTML>");
    assert!(matches!(
        list_companies(&t).unwrap_err(),
        BridgeError::UnexpectedResponse { .. }
    ));
}

#[test]
fn stray_control_entity_in_a_name_is_reported_not_dropped() {
    // Research R8 2.1: Tally can emit a stray &#4;. Dropping it would change the name.
    let t = Fixture::ok(
        "<ENVELOPE><BODY><COLLECTION><COMPANY><NAME>Ac&#4;me</NAME></COMPANY></COLLECTION></BODY></ENVELOPE>",
    );
    assert!(matches!(
        list_companies(&t).unwrap_err(),
        BridgeError::MalformedResponse { .. }
    ));
}
