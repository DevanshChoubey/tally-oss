//! The audit chain integrity certificate (part of the auditor reports, card RP10).

use std::fmt::Write as _;

use ledgerly_core::{Date, Period};
use ledgerly_model::Scenario;
use ledgerly_store::verify::VerifyReport;

use super::{columns, text_row};
use crate::{
    AuditReportKind, ColumnSpec, Detail, Report, ReportError, ReportFilter, ReportKind,
    ReportParams,
};

/// Renders the chain verification result as a report: PASS or FAIL, the event count, the head,
/// the first broken event, the checkpoints and every problem found.
///
/// The report covers the whole log, so its period spans every date the calendar supports.
///
/// # Errors
/// [`ReportError::Period`] if the whole-calendar period cannot be built. That cannot happen with
/// the `time` crate's `Date::MIN` and `Date::MAX`, but the core crate has no infallible
/// constructor for a period (interface request), so the error is passed on rather than hidden.
pub fn integrity_certificate(verify: &VerifyReport) -> Result<Report, ReportError> {
    let params = ReportParams {
        kind: ReportKind::Audit(AuditReportKind::IntegrityCertificate),
        period: Period::new(Date::MIN, Date::MAX)?,
        as_of: Date::MAX,
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    let verdict = if verify.is_ok() { "PASS" } else { "FAIL" };
    let head = match &verify.head {
        Some((seq, hash)) => format!("event {} hash {}", seq.0, hex(hash)),
        None => "none (the log is empty)".to_owned(),
    };
    let first_broken = verify
        .first_broken_seq()
        .map_or_else(|| "none".to_owned(), |s| s.0.to_string());
    let mut rows = vec![
        text_row(0, "Result", verdict.to_owned()),
        text_row(0, "Events checked", verify.events_checked.to_string()),
        text_row(0, "Head", head),
        text_row(0, "First broken event", first_broken),
        text_row(0, "Checkpoints", verify.checkpoints.len().to_string()),
    ];
    rows.extend(verify.checkpoints.iter().map(|c| {
        text_row(
            1,
            &format!("Checkpoint {}", c.day),
            format!("event {} hash {}", c.seq.0, hex(&c.hash)),
        )
    }));
    rows.extend(
        verify
            .problems
            .iter()
            .map(|p| text_row(1, "Problem", p.describe())),
    );
    Ok(Report {
        title: "Audit chain integrity certificate".to_owned(),
        params,
        columns: columns(&["Item", "Value"]),
        rows,
        totals: Vec::new(),
        notes: vec![
            "The check covers the events and checkpoints stored in this file. Keep a copy of the \
             checkpoints outside the file (for example in a backup) to expose a rewritten tail."
                .to_owned(),
        ],
    })
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in bytes {
        // Writing to a String cannot fail.
        let _ = write!(out, "{b:02x}");
    }
    out
}
