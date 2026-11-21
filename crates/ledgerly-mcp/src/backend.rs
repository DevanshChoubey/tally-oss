//! The read and propose tools over the open company file (card W2, Wave 2 wiring).
//!
//! [`ToolBackend`] for [`Api`] maps each tool to one facade call. Every figure in a result comes
//! from the engine or the report engines (spec 4). Nothing here posts: `propose_voucher` asks the
//! engine to check the draft, and the engine's dry run commits no event.

use ledgerly_api::{Api, Report, ReportParams, SearchHit, Voucher, VoucherCheck};
use ledgerly_core::Outcome;
use serde_json::{Value, json};

use crate::server::{ToolBackend, ToolError};

impl ToolBackend for Api {
    fn search(&self, query: &str) -> Result<Value, ToolError> {
        let hits = Api::search(self, query)
            .iter()
            .map(hit_json)
            .collect::<Result<Vec<Value>, ToolError>>()?;
        Ok(json!({"hits": hits}))
    }

    fn report(&self, params: &Value) -> Result<Value, ToolError> {
        let params: ReportParams = serde_json::from_value(params.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("the report parameters are not valid: {e}"))
        })?;
        match Api::report(self, &params) {
            Ok(Outcome::Value(report)) => report_json(&report),
            Ok(Outcome::Unsupported(u)) => {
                Err(ToolError::Unsupported(format!("{}: {}", u.what, u.reason)))
            }
            Err(e) => Err(ToolError::Failed(e.to_string())),
        }
    }

    fn propose_voucher(&self, draft: &Value) -> Result<Value, ToolError> {
        let voucher: Voucher = serde_json::from_value(draft.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("the voucher draft is not valid: {e}"))
        })?;
        match Api::check_voucher(self, voucher) {
            Ok(Outcome::Value(check)) => Ok(check_json(check)),
            Ok(Outcome::Unsupported(u)) => {
                Err(ToolError::Unsupported(format!("{}: {}", u.what, u.reason)))
            }
            Err(e) => Err(ToolError::Failed(format!(
                "the books refuse this draft: {e}"
            ))),
        }
    }
}

/// The verdict of a checked draft. The count is the engine's, and nothing is posted.
fn check_json(check: VoucherCheck) -> Value {
    json!({
        "posted": false,
        "status": "checked by the engine; nothing was posted and a person must review and confirm it",
        "events_that_would_be_written": check.events_that_would_be_written,
    })
}

/// A report as JSON. The engine's report is serialised as it is.
fn report_json(report: &Report) -> Result<Value, ToolError> {
    serde_json::to_value(report)
        .map_err(|e| ToolError::Failed(format!("the report could not be encoded: {e}")))
}

/// One search hit as JSON. Every variant is named, so a new hit kind fails to compile here.
fn hit_json(hit: &SearchHit) -> Result<Value, ToolError> {
    Ok(match hit {
        SearchHit::Ledger { id, name, matched } => json!({
            "kind": "ledger",
            "id": id.0,
            "name": name,
            "matched": format!("{matched:?}"),
        }),
        SearchHit::Voucher {
            id,
            number,
            date,
            matched,
        } => json!({
            "kind": "voucher",
            "id": id.0,
            "number": number,
            "date": date.to_string(),
            "matched": format!("{matched:?}"),
        }),
        SearchHit::Report { title, kind } => json!({
            "kind": "report",
            "title": title,
            "report": serde_json::to_value(kind)
                .map_err(|e| ToolError::Failed(format!("the report kind could not be encoded: {e}")))?,
        }),
        SearchHit::Setting { name } => json!({"kind": "setting", "name": name}),
        SearchHit::Unavailable { reason } => json!({"kind": "unavailable", "reason": reason}),
    })
}
