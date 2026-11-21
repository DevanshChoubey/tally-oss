//! Card W2 tests: the MCP server over a real company file (spec 6.8, rule A10).
//!
//! Provenance. Expected values are `derived`:
//! - Search: `ledgerly_api::search` documents that an empty (blank) query gives no hits, that a
//!   report is found by a substring of its title ("Trial balance" is in the report catalogue) and
//!   that a setting is found by a substring of its name ("GST" is one of the company feature
//!   switches). These are the rules written in the search module, not engine output.
//! - Invalid arguments: JSON-RPC invalid params is -32602 (the MCP/JSON-RPC 2.0 error codes, as in
//!   the card P1 tests).
//! - Propose never posts: the event count of the file is unchanged after a rejected proposal
//!   (`derived` from rule A10: no tool may post).
//! - The company literal is the one the facade tests use; dates are written as calendar dates.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};

use ledgerly_api::{Address, Api, Company, EntityKind, Features, NegativeStockPolicy};
use ledgerly_core::{Clock, CompanyId, Date, Month, OffsetDateTime, StateCode};
use ledgerly_mcp::server::serve;
use serde_json::{Value, json};

fn company() -> Company {
    let april_first = Date::from_calendar_date(2026, Month::April, 1).unwrap();
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Test Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: april_first,
        books_begin: april_first,
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// A fresh company file in a directory unique to `name`, under the target tmp directory.
fn fresh_api(name: &str) -> (PathBuf, Api) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("w2-serve-{name}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("company.ledgerly");
    let api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    (path, api)
}

/// Sends the lines to the server over `api` and returns the parsed replies.
fn run(api: &Api, lines: &[Value]) -> Vec<Value> {
    let input = lines
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut out = Vec::new();
    serve(api, input.as_bytes(), &mut out).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn call(id: u64, name: &str, arguments: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": name, "arguments": arguments}})
}

#[test]
fn blank_search_finds_nothing() {
    let (_path, api) = fresh_api("blank-search");
    let replies = run(&api, &[call(1, "search", &json!({"query": "   "}))]);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["result"]["structuredContent"]["hits"], json!([]));
    assert_eq!(replies[0]["result"]["isError"], json!(false));
}

#[test]
fn search_finds_a_report_by_title() {
    let (_path, api) = fresh_api("report-search");
    let replies = run(&api, &[call(1, "search", &json!({"query": "trial"}))]);
    let hits = replies[0]["result"]["structuredContent"]["hits"]
        .as_array()
        .unwrap();
    assert!(
        hits.iter()
            .any(|h| h["kind"] == "report" && h["title"] == "Trial balance"),
        "{hits:?}"
    );
}

#[test]
fn search_finds_a_setting_by_name() {
    let (_path, api) = fresh_api("setting-search");
    let replies = run(&api, &[call(1, "search", &json!({"query": "GST"}))]);
    let hits = replies[0]["result"]["structuredContent"]["hits"]
        .as_array()
        .unwrap();
    assert!(
        hits.iter()
            .any(|h| h["kind"] == "setting" && h["name"] == "GST"),
        "{hits:?}"
    );
}

#[test]
fn report_with_invalid_parameters_is_an_invalid_params_error() {
    let (_path, api) = fresh_api("bad-report");
    let replies = run(
        &api,
        &[call(
            1,
            "report",
            &json!({"params": {"not": "report params"}}),
        )],
    );
    assert_eq!(replies[0]["error"]["code"], -32602);
    assert!(replies[0].get("result").is_none());
}

#[test]
fn malformed_draft_is_refused_and_writes_nothing() {
    let (path, api) = fresh_api("bad-draft");
    let before = ledgerly_api::verify_file(&path).unwrap().events_checked;
    let replies = run(
        &api,
        &[call(
            1,
            "propose_voucher",
            &json!({"draft": {"kind": "Payment"}}),
        )],
    );
    assert_eq!(replies[0]["error"]["code"], -32602);
    assert_eq!(
        ledgerly_api::verify_file(&path).unwrap().events_checked,
        before
    );
}

#[test]
fn posting_tools_still_do_not_exist() {
    let (_path, api) = fresh_api("no-post");
    let replies = run(&api, &[call(1, "post_voucher", &json!({"draft": {}}))]);
    assert_eq!(replies[0]["error"]["code"], -32602);
}
