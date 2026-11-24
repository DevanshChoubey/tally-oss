//! M5 tests: `ledgerly mcp --company <file>` serves the MCP server over stdio. Each test runs the
//! built binary, writes JSON-RPC lines to its standard input, closes it, and reads the replies from
//! its standard output.
//!
//! Provenance. Expected values are `derived`:
//! - The protocol revision (2024-11-05), the server name (`ledgerly-mcp`) and the tool set
//!   (`search`, `report`, `propose_voucher`, and no posting tool) are written in
//!   `crates/ledgerly-mcp/src/server.rs` (spec 6.8, rule A10). They are read from that source, not
//!   from engine output.
//! - "Trial balance" is a report in the report catalogue, found by a substring of its title. The
//!   same rule is asserted by the card W2 tests in `w2_cli.rs` and `ledgerly-mcp/tests/w2_serve.rs`.
//! - A missing company file exits with code 2 ("the command could not run"), as in `main.rs`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use ledgerly_api::{Address, Api, Company, EntityKind, Features, NegativeStockPolicy};
use ledgerly_core::{Clock, CompanyId, Date, Month, OffsetDateTime, StateCode};
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
fn fixture(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m5-mcp-stdio-{name}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("company.ledgerly");
    Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    path
}

/// Runs `ledgerly mcp --company <company>` with `lines` on standard input.
fn mcp(company: &Path, lines: &[Value]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ledgerly"))
        .arg("mcp")
        .arg("--company")
        .arg(company)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for line in lines {
            writeln!(stdin, "{line}").unwrap();
        }
    }
    child.wait_with_output().unwrap()
}

/// The replies on standard output, one parsed JSON message per line.
fn replies(out: &Output) -> Vec<Value> {
    String::from_utf8(out.stdout.clone())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn initialize(id: u64) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "initialize",
           "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                      "clientInfo": {"name": "test", "version": "0"}}})
}

#[test]
fn initialize_answers_with_the_protocol_and_server_name() {
    let company = fixture("initialize");
    let out = mcp(&company, &[initialize(1)]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let replies = replies(&out);
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["id"], json!(1));
    assert_eq!(replies[0]["result"]["protocolVersion"], json!("2024-11-05"));
    assert_eq!(
        replies[0]["result"]["serverInfo"]["name"],
        json!("ledgerly-mcp")
    );
}

#[test]
fn tools_list_offers_only_read_and_propose_tools() {
    let company = fixture("tools-list");
    let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
    let out = mcp(&company, &[initialize(1), list]);
    let replies = replies(&out);
    assert_eq!(replies.len(), 2, "{replies:?}");
    let names: Vec<&str> = replies[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["search", "report", "propose_voucher"]);
}

#[test]
fn search_call_reads_the_company_file_and_finds_the_trial_balance() {
    let company = fixture("search");
    let call = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                      "params": {"name": "search", "arguments": {"query": "trial"}}});
    let out = mcp(&company, &[initialize(1), call]);
    let replies = replies(&out);
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_eq!(replies[1]["result"]["isError"], json!(false));
    let hits = replies[1]["result"]["structuredContent"]["hits"]
        .as_array()
        .unwrap();
    assert!(
        hits.iter()
            .any(|h| h["kind"] == "report" && h["title"] == "Trial balance"),
        "{hits:?}"
    );
}

#[test]
fn a_posting_tool_name_is_refused_over_stdio() {
    let company = fixture("no-post");
    let call = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                      "params": {"name": "post_voucher", "arguments": {"draft": {}}}});
    let out = mcp(&company, &[initialize(1), call]);
    let replies = replies(&out);
    assert_eq!(replies[1]["error"]["code"], json!(-32602), "{replies:?}");
}

#[test]
fn a_missing_company_file_exits_2_and_creates_nothing() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("m5-mcp-stdio-missing");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let missing = dir.join("absent.ledgerly");
    let out = mcp(&missing, &[initialize(1)]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        !missing.exists(),
        "the command must not create a company file"
    );
    assert!(
        out.stdout.is_empty(),
        "no protocol output on a failed start"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("there is no company file"), "{stderr}");
}
