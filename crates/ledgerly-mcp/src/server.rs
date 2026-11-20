//! JSON-RPC over stdio. **Card P1** implements this module.
//!
//! Frozen interface (plan card P1):
//!
//! ```text
//! pub fn serve(api: &Api, input: impl BufRead, output: impl Write) -> Result<(), McpError>;
//! ```
//!
//! `ledgerly_api::facade::Api` is built by card W1 (Wave 2) and does not exist in this tree yet, so
//! `serve` cannot be written against it. This module implements everything that does not need it:
//! the MCP protocol loop, [`serve_with`], and the tool list, over the [`ToolBackend`] seam. The
//! Wave 2 wiring adds `impl ToolBackend for Api` and `serve` as a one-line call to [`serve_with`]
//! (recorded as an interface request).
//!
//! The wire format is MCP revision 2024-11-05 over stdio: one JSON-RPC 2.0 message per line.
//!
//! The tool set is closed and has no posting tool (spec 6.8, rule A10): `search` and `report`
//! read, `propose_voucher` returns a proposal for a person to review. A request for any other
//! tool name, `post_voucher` included, is a JSON-RPC error and never reaches the backend.

use std::io::{BufRead, Read, Write};

use serde_json::{Map, Value, json};

/// MCP protocol revision this server speaks.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Largest accepted message line, in bytes. A longer line is untrusted input (rule D4).
pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

/// Why the server loop stopped. Protocol-level problems in a single message are answered with a
/// JSON-RPC error and do not stop the loop.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Reading from the client failed, or the client sent bytes that are not UTF-8.
    #[error("could not read a message from the MCP client: {0}")]
    Read(#[source] std::io::Error),
    /// Writing to the client failed.
    #[error("could not write a message to the MCP client: {0}")]
    Write(#[source] std::io::Error),
    /// One line was longer than [`MAX_MESSAGE_BYTES`]. The client must send smaller messages.
    #[error("an MCP message is longer than 4 MiB")]
    MessageTooLarge,
}

/// A tool call that the backend could not answer with a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    /// The arguments are well-formed JSON but not valid for the query. Reported to the client as a
    /// JSON-RPC invalid-params error.
    #[error("{0}")]
    InvalidArguments(String),
    /// The query ran and failed. Reported to the model as a tool error it can read.
    #[error("{0}")]
    Failed(String),
    /// The engine does not support this query (rule A1: never a guess). Reported to the model as a
    /// tool error that starts with "unsupported".
    #[error("unsupported: {0}")]
    Unsupported(String),
}

/// What the server may ask of the books. The implementation for the real `Api` lands with card W1;
/// tests use a fake. Every method is a read or a proposal: no method posts, files or sends.
pub trait ToolBackend {
    /// Global search over masters and vouchers. The returned JSON is the list of hits.
    ///
    /// # Errors
    /// Returns a [`ToolError`] when the search cannot run.
    fn search(&self, query: &str) -> Result<Value, ToolError>;

    /// Run a report. `params` is the JSON form of `ReportParams`; the engine computes every figure
    /// and the result is the report with its `Outcome` (a value or an unsupported reason).
    ///
    /// # Errors
    /// Returns a [`ToolError`] when the parameters are invalid or the report fails.
    fn report(&self, params: &Value) -> Result<Value, ToolError>;

    /// Check a proposed voucher draft against the books and the rule set and return the check
    /// result for a person to review. This never writes to the store (rule A10).
    ///
    /// # Errors
    /// Returns a [`ToolError`] when the draft cannot be read or checked.
    fn propose_voucher(&self, draft: &Value) -> Result<Value, ToolError>;
}

/// The closed set of tools this server offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Search masters and vouchers.
    Search,
    /// Run a report.
    Report,
    /// Propose a voucher for review.
    ProposeVoucher,
}

impl Tool {
    /// Every tool, in the order `tools/list` returns them.
    pub const ALL: [Tool; 3] = [Tool::Search, Tool::Report, Tool::ProposeVoucher];

    /// The name a client uses in `tools/call`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Tool::Search => "search",
            Tool::Report => "report",
            Tool::ProposeVoucher => "propose_voucher",
        }
    }

    /// The tool with this exact name, or `None` (there is no fuzzy matching).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.name() == name)
    }

    /// The `tools/list` entry: name, description, JSON schema of the arguments, hints.
    #[must_use]
    pub fn definition(self) -> Value {
        match self {
            Tool::Search => json!({
                "name": self.name(),
                "description": "Search ledgers, groups, stock items and vouchers by name, alias, GSTIN, voucher number, party or amount. Read only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Text to search for."}
                    },
                    "required": ["query"],
                    "additionalProperties": false
                },
                "annotations": {"readOnlyHint": true, "destructiveHint": false}
            }),
            Tool::Report => json!({
                "name": self.name(),
                "description": "Run a report. The engine computes every figure; the answer is the report with its query parameters. A report the engine does not support comes back as unsupported with the reason. Read only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "params": {"type": "object", "description": "Report parameters: report kind, period, scenario, columns and filters."}
                    },
                    "required": ["params"],
                    "additionalProperties": false
                },
                "annotations": {"readOnlyHint": true, "destructiveHint": false}
            }),
            Tool::ProposeVoucher => json!({
                "name": self.name(),
                "description": "Propose a voucher draft. The books check it and return the result for a person to review. Nothing is posted: there is no tool that posts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "draft": {"type": "object", "description": "The voucher draft: kind, date, party and lines."}
                    },
                    "required": ["draft"],
                    "additionalProperties": false
                },
                "annotations": {"readOnlyHint": false, "destructiveHint": false}
            }),
        }
    }
}

/// All tool definitions, in `tools/list` order.
#[must_use]
pub fn tool_definitions() -> Vec<Value> {
    Tool::ALL.into_iter().map(Tool::definition).collect()
}

struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn ok_response(id: &Value, result: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "ledgerly-mcp", "version": env!("CARGO_PKG_VERSION")},
        "instructions": "Read queries and propose_voucher only. Nothing here posts, files or sends; a person reviews every proposal and the engine recomputes every figure."
    })
}

fn required_str<'a>(args: &'a Map<String, Value>, key: &str) -> Result<&'a str, RpcError> {
    args.get(key).and_then(Value::as_str).ok_or_else(|| {
        RpcError::new(
            INVALID_PARAMS,
            format!("argument `{key}` is required and must be a string"),
        )
    })
}

fn required_object<'a>(args: &'a Map<String, Value>, key: &str) -> Result<&'a Value, RpcError> {
    args.get(key).filter(|v| v.is_object()).ok_or_else(|| {
        RpcError::new(
            INVALID_PARAMS,
            format!("argument `{key}` is required and must be an object"),
        )
    })
}

fn tool_result(result: Result<Value, ToolError>) -> Result<Value, RpcError> {
    match result {
        Ok(value) => {
            let text = serde_json::to_string(&value).map_err(|e| {
                RpcError::new(INTERNAL_ERROR, format!("could not encode the result: {e}"))
            })?;
            let mut out = Map::new();
            out.insert(
                "content".to_owned(),
                json!([{"type": "text", "text": text}]),
            );
            if value.is_object() {
                out.insert("structuredContent".to_owned(), value);
            }
            out.insert("isError".to_owned(), Value::Bool(false));
            Ok(Value::Object(out))
        }
        Err(ToolError::InvalidArguments(message)) => Err(RpcError::new(INVALID_PARAMS, message)),
        Err(e @ (ToolError::Failed(_) | ToolError::Unsupported(_))) => Ok(json!({
            "content": [{"type": "text", "text": e.to_string()}],
            "isError": true
        })),
    }
}

fn call_tool<B: ToolBackend + ?Sized>(
    backend: &B,
    params: Option<&Value>,
) -> Result<Value, RpcError> {
    let params = params.and_then(Value::as_object).ok_or_else(|| {
        RpcError::new(
            INVALID_PARAMS,
            "tools/call needs a params object with a tool `name`",
        )
    })?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "tools/call needs a string `name`"))?;
    let Some(tool) = Tool::from_name(name) else {
        return Err(RpcError::new(
            INVALID_PARAMS,
            format!(
                "unknown tool `{name}`: this server offers only read queries and propose_voucher, and has no tool that posts"
            ),
        ));
    };
    let empty = Map::new();
    let args = match params.get("arguments") {
        None => &empty,
        Some(v) => v
            .as_object()
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, "`arguments` must be an object"))?,
    };
    let result = match tool {
        Tool::Search => backend.search(required_str(args, "query")?),
        Tool::Report => backend.report(required_object(args, "params")?),
        Tool::ProposeVoucher => backend.propose_voucher(required_object(args, "draft")?),
    };
    tool_result(result)
}

/// Answer one decoded JSON-RPC message. Returns `None` for notifications and for responses from
/// the client, which get no reply.
pub fn handle_message<B: ToolBackend + ?Sized>(backend: &B, msg: &Value) -> Option<Value> {
    let Some(obj) = msg.as_object() else {
        return Some(error_response(
            &Value::Null,
            INVALID_REQUEST,
            "a message must be a JSON object; batches are not supported",
        ));
    };
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        if obj.contains_key("result") || obj.contains_key("error") {
            return None;
        }
        return Some(error_response(
            &Value::Null,
            INVALID_REQUEST,
            "a request needs a string `method`",
        ));
    };
    let id = match obj.get("id") {
        None => {
            tracing::debug!(method, "notification ignored");
            return None;
        }
        Some(v) if v.is_string() || v.is_number() => v.clone(),
        Some(_) => {
            return Some(error_response(
                &Value::Null,
                INVALID_REQUEST,
                "`id` must be a string or a number",
            ));
        }
    };
    tracing::debug!(method, "request");
    let outcome = match method {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tool_definitions()})),
        "tools/call" => call_tool(backend, obj.get("params")),
        other => Err(RpcError::new(
            METHOD_NOT_FOUND,
            format!("method `{other}` is not supported"),
        )),
    };
    Some(match outcome {
        Ok(result) => ok_response(&id, &result),
        Err(e) => error_response(&id, e.code, &e.message),
    })
}

/// Read one line of at most [`MAX_MESSAGE_BYTES`]. `Ok(false)` at end of input.
fn read_line(input: &mut impl BufRead, line: &mut String) -> Result<bool, McpError> {
    line.clear();
    let limit = u64::try_from(MAX_MESSAGE_BYTES.saturating_add(1)).unwrap_or(u64::MAX);
    let read = input
        .by_ref()
        .take(limit)
        .read_line(line)
        .map_err(McpError::Read)?;
    if read == 0 {
        return Ok(false);
    }
    if line.len() > MAX_MESSAGE_BYTES && !line.ends_with('\n') {
        return Err(McpError::MessageTooLarge);
    }
    Ok(true)
}

fn write_message(output: &mut impl Write, message: &Value) -> Result<(), McpError> {
    let text = serde_json::to_string(message)
        .map_err(|e| McpError::Write(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    output
        .write_all(text.as_bytes())
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(McpError::Write)
}

/// Run the MCP loop until the client closes its input: one JSON-RPC message per line in, one
/// reply per request line out. A line that is not valid JSON gets a parse-error reply and the loop
/// carries on.
///
/// # Errors
/// Returns [`McpError`] when reading or writing the stream fails, or a line is over
/// [`MAX_MESSAGE_BYTES`].
pub fn serve_with<B: ToolBackend + ?Sized>(
    backend: &B,
    mut input: impl BufRead,
    mut output: impl Write,
) -> Result<(), McpError> {
    let mut line = String::new();
    while read_line(&mut input, &mut line)? {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(text) {
            Ok(msg) => handle_message(backend, &msg),
            Err(e) => Some(error_response(
                &Value::Null,
                PARSE_ERROR,
                &format!("the message is not valid JSON: {e}"),
            )),
        };
        if let Some(reply) = reply {
            write_message(&mut output, &reply)?;
        }
    }
    Ok(())
}
