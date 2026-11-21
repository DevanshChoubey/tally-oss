//! Card P1: MCP server (spec 6.8, rule A10): read queries and `propose_voucher`, no posting tool.
//!
//! Provenance for every expected value here is `derived`: spec section 6.8 ("AI can only call read
//! queries and `propose_*`; never post, file or send"), the MCP specification revision 2024-11-05
//! (stdio transport: one JSON-RPC 2.0 message per line; `tools/call` for an unknown tool is a
//! JSON-RPC error; JSON-RPC 2.0 error codes -32700, -32600, -32601, -32602), and the card text
//! ("tool list snapshot; calling a non-existent `post_voucher` errors").
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::cell::RefCell;

use ledgerly_mcp::server::{
    MAX_MESSAGE_BYTES, McpError, PROTOCOL_VERSION, Tool, ToolBackend, ToolError, serve_with,
};
use serde_json::{Value, json};

/// Records every call so tests can prove a rejected request never reached the books.
#[derive(Default)]
struct Fake {
    calls: RefCell<Vec<String>>,
}

impl ToolBackend for Fake {
    fn search(&self, query: &str) -> Result<Value, ToolError> {
        self.calls.borrow_mut().push(format!("search:{query}"));
        match query {
            "fail" => Err(ToolError::Failed("the store is locked".to_owned())),
            "unsupported" => Err(ToolError::Unsupported("no such index".to_owned())),
            "badargs" => Err(ToolError::InvalidArguments("query too long".to_owned())),
            other => Ok(json!({"hits": [{"name": other}]})),
        }
    }

    fn report(&self, params: &Value) -> Result<Value, ToolError> {
        self.calls.borrow_mut().push("report".to_owned());
        Ok(json!({"echo": params}))
    }

    fn propose_voucher(&self, draft: &Value) -> Result<Value, ToolError> {
        self.calls.borrow_mut().push("propose_voucher".to_owned());
        Ok(json!({"check": "Blocked", "draft": draft}))
    }
}

/// Feed the lines to the server and return the parsed reply lines.
fn run(backend: &Fake, lines: &[&str]) -> Vec<Value> {
    let input = lines.join("\n") + "\n";
    let mut out = Vec::new();
    serve_with(backend, input.as_bytes(), &mut out).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn call(id: u32, name: &str, arguments: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": name, "arguments": arguments}})
    .to_string()
}

#[test]
fn tool_list_snapshot() {
    // provenance: derived, spec 6.8 and card P1: the tools are read queries plus propose_voucher.
    // Names and required arguments written out by hand; this is the whole surface.
    let fake = Fake::default();
    let replies = run(
        &fake,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#],
    );
    assert_eq!(replies.len(), 1);
    let tools = replies[0]["result"]["tools"].as_array().unwrap();
    let summary: Vec<(String, Vec<String>, bool)> = tools
        .iter()
        .map(|t| {
            let required = t["inputSchema"]["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.as_str().unwrap().to_owned())
                .collect();
            (
                t["name"].as_str().unwrap().to_owned(),
                required,
                t["annotations"]["readOnlyHint"].as_bool().unwrap(),
            )
        })
        .collect();
    let expected = vec![
        ("search".to_owned(), vec!["query".to_owned()], true),
        ("report".to_owned(), vec!["params".to_owned()], true),
        (
            "propose_voucher".to_owned(),
            vec!["draft".to_owned()],
            false,
        ),
    ];
    assert_eq!(summary, expected);
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object");
        assert!(t["description"].as_str().unwrap().len() > 20);
        assert_eq!(t["annotations"]["destructiveHint"], false);
    }
    assert!(fake.calls.borrow().is_empty());
}

#[test]
fn tool_names_round_trip_and_none_posts() {
    // provenance: derived, rule A10 / spec 6.8: no tool may post, file or send.
    for tool in Tool::ALL {
        assert_eq!(Tool::from_name(tool.name()), Some(tool));
        for forbidden in ["post", "file", "send", "delete", "alter", "cancel"] {
            assert!(
                !tool.name().contains(forbidden),
                "{} looks like a write tool",
                tool.name()
            );
        }
    }
    assert_eq!(Tool::from_name("Search"), None);
    assert_eq!(Tool::from_name("search "), None);
}

#[test]
fn calling_post_voucher_errors_and_never_reaches_the_books() {
    // provenance: derived, card P1 test "calling a non-existent post_voucher errors"; MCP spec:
    // an unknown tool is a JSON-RPC error (-32602 invalid params), not a tool result.
    let fake = Fake::default();
    let args = json!({"draft": {"kind": "Payment"}});
    let replies = run(
        &fake,
        &[
            &call(1, "post_voucher", &args),
            &call(2, "file_gstr1", &args),
            &call(3, "send_einvoice", &args),
            &call(4, "execute", &args),
        ],
    );
    assert_eq!(replies.len(), 4);
    for (i, reply) in replies.iter().enumerate() {
        assert_eq!(reply["id"], json!(i + 1));
        assert!(reply.get("result").is_none(), "reply {i} must be an error");
        assert_eq!(reply["error"]["code"], -32602);
    }
    assert!(
        replies[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("post_voucher")
    );
    assert!(fake.calls.borrow().is_empty());
}

#[test]
fn initialize_reports_protocol_and_tools_capability() {
    // provenance: derived, MCP spec 2024-11-05: initialize result carries protocolVersion,
    // capabilities (tools) and serverInfo (name, version).
    let fake = Fake::default();
    let replies = run(
        &fake,
        &[
            r#"{"jsonrpc":"2.0","id":"a","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
        ],
    );
    assert_eq!(PROTOCOL_VERSION, "2024-11-05");
    let r = &replies[0];
    assert_eq!(r["jsonrpc"], "2.0");
    assert_eq!(r["id"], "a");
    assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
    assert!(r["result"]["capabilities"]["tools"].is_object());
    assert_eq!(r["result"]["serverInfo"]["name"], "ledgerly-mcp");
    assert!(r["result"]["capabilities"].get("resources").is_none());
}

#[test]
fn notifications_get_no_reply_and_ping_gets_empty_result() {
    // provenance: derived, JSON-RPC 2.0 section 4.1: a notification (no id) is never answered;
    // MCP ping: the receiver responds promptly with an empty result.
    let fake = Fake::default();
    let replies = run(
        &fake,
        &[
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"x"}}}"#,
        ],
    );
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["id"], 7);
    assert_eq!(replies[0]["result"], json!({}));
    // A notification never runs a tool, even a valid one.
    assert!(fake.calls.borrow().is_empty());
}

#[test]
fn protocol_errors_use_json_rpc_codes_and_the_loop_carries_on() {
    // provenance: derived, JSON-RPC 2.0 section 5.1: -32700 parse error (id null), -32600 invalid
    // request, -32601 method not found.
    let fake = Fake::default();
    let replies = run(
        &fake,
        &[
            "{not json",
            "[1,2]",
            r#"{"jsonrpc":"2.0","id":true,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#,
            "",
            r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#,
        ],
    );
    let codes: Vec<(Value, i64)> = replies
        .iter()
        .map(|r| (r["id"].clone(), r["error"]["code"].as_i64().unwrap_or(0)))
        .collect();
    assert_eq!(
        codes,
        vec![
            (Value::Null, -32700),
            (Value::Null, -32600),
            (Value::Null, -32600),
            (json!(3), -32601),
            (json!(4), 0),
        ]
    );
    assert_eq!(replies[4]["result"], json!({}));
}

#[test]
fn search_call_returns_backend_value_as_text_and_structured_content() {
    // provenance: derived, MCP spec 2024-11-05 tools/call result: content array of text items,
    // isError flag. The fake backend returns {"hits":[{"name":"sharma"}]} for "sharma".
    let fake = Fake::default();
    let replies = run(&fake, &[&call(1, "search", &json!({"query": "sharma"}))]);
    let r = &replies[0]["result"];
    assert_eq!(r["isError"], false);
    assert_eq!(
        r["structuredContent"],
        json!({"hits": [{"name": "sharma"}]})
    );
    assert_eq!(r["content"][0]["type"], "text");
    let text: Value = serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, json!({"hits": [{"name": "sharma"}]}));
    assert_eq!(*fake.calls.borrow(), vec!["search:sharma".to_owned()]);
}

#[test]
fn report_and_propose_pass_their_object_through_unchanged() {
    // provenance: derived, the server must not alter, default or compute anything (rule A1, A10).
    let fake = Fake::default();
    let params =
        json!({"kind": "TrialBalance", "period": {"from": "2025-04-01", "to": "2026-03-31"}});
    let draft = json!({"kind": "Payment", "lines": [{"amount_paise": 2_500_000}]});
    let replies = run(
        &fake,
        &[
            &call(1, "report", &json!({"params": params})),
            &call(2, "propose_voucher", &json!({"draft": draft})),
        ],
    );
    assert_eq!(replies[0]["result"]["structuredContent"]["echo"], params);
    assert_eq!(replies[1]["result"]["structuredContent"]["draft"], draft);
    assert_eq!(
        *fake.calls.borrow(),
        vec!["report".to_owned(), "propose_voucher".to_owned()]
    );
}

#[test]
fn backend_failures_become_tool_errors_not_values() {
    // provenance: derived, MCP spec: a tool that ran and failed reports isError true in the result;
    // rule A1: Unsupported is named as such, never turned into a value.
    let fake = Fake::default();
    let replies = run(
        &fake,
        &[
            &call(1, "search", &json!({"query": "fail"})),
            &call(2, "search", &json!({"query": "unsupported"})),
            &call(3, "search", &json!({"query": "badargs"})),
        ],
    );
    assert_eq!(replies[0]["result"]["isError"], true);
    assert_eq!(
        replies[0]["result"]["content"][0]["text"],
        "the store is locked"
    );
    assert!(replies[0]["result"].get("structuredContent").is_none());
    assert_eq!(replies[1]["result"]["isError"], true);
    assert_eq!(
        replies[1]["result"]["content"][0]["text"],
        "unsupported: no such index"
    );
    // Invalid arguments reported by the backend are a protocol error.
    assert_eq!(replies[2]["error"]["code"], -32602);
    assert_eq!(replies[2]["error"]["message"], "query too long");
}

#[test]
fn missing_or_mistyped_arguments_are_invalid_params_and_do_not_reach_the_books() {
    // provenance: derived, JSON-RPC 2.0: -32602 invalid params; the schemas in tool_list_snapshot
    // mark these arguments required.
    let fake = Fake::default();
    let replies = run(
        &fake,
        &[
            &call(1, "search", &json!({})),
            &call(2, "search", &json!({"query": 5})),
            &call(3, "report", &json!({"params": "TrialBalance"})),
            &call(4, "propose_voucher", &json!({"draft": [1]})),
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"search","arguments":[]}}"#,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"arguments":{}}}"#,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call"}"#,
        ],
    );
    assert_eq!(replies.len(), 7);
    for reply in &replies {
        assert_eq!(reply["error"]["code"], -32602, "{reply}");
    }
    assert!(fake.calls.borrow().is_empty());
}

#[test]
fn every_reply_is_exactly_one_line() {
    // provenance: derived, MCP stdio transport: messages are newline-delimited and must not
    // contain embedded newlines. A query containing a newline must not break the framing.
    let fake = Fake::default();
    let input = format!("{}\n", call(1, "search", &json!({"query": "a\nb"})));
    let mut out = Vec::new();
    serve_with(&fake, input.as_bytes(), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.matches('\n').count(), 1);
    assert!(text.ends_with('\n'));
}

#[test]
fn a_line_over_the_limit_stops_the_server_with_a_typed_error() {
    // provenance: derived, rule D4: untrusted input is bounded; the limit is MAX_MESSAGE_BYTES.
    let fake = Fake::default();
    let input = vec![b'a'; MAX_MESSAGE_BYTES + 10];
    let mut out = Vec::new();
    let err = serve_with(&fake, input.as_slice(), &mut out).unwrap_err();
    assert!(matches!(err, McpError::MessageTooLarge));
    assert_eq!(out, [] as [u8; 0]);
}

#[test]
fn non_utf8_input_is_a_read_error() {
    // provenance: derived, JSON text is UTF-8 (RFC 8259 section 8.1).
    let fake = Fake::default();
    let mut out = Vec::new();
    let err = serve_with(&fake, &[0xff_u8, 0xfe, b'\n'][..], &mut out).unwrap_err();
    assert!(matches!(err, McpError::Read(_)));
}

#[test]
fn last_line_without_newline_is_still_served() {
    // provenance: derived, a client may close stdin right after its last message.
    let fake = Fake::default();
    let mut out = Vec::new();
    serve_with(
        &fake,
        &br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#[..],
        &mut out,
    )
    .unwrap();
    assert!(String::from_utf8(out).unwrap().contains("\"result\":{}"));
}
