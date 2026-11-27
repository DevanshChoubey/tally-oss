//! Ask your books (round s21, plan round E; spec 6.8).
//!
//! A question goes through the one gate ([`Api::ai_gate`] for [`AiFeature::Ask`]) to the
//! provider, which answers with a [`ReportQuery`]. The query is checked against the question (its
//! dates and ledger names must be ones the question states), resolved to a report the engine
//! knows, and run with [`Api::report`]. The answer is the engine's [`Report`] and nothing the
//! provider wrote: the screen draws the query from the report's own parameters.
//!
//! Every ask that runs is logged. An answer is `Answered` with the query as JSON; a query no
//! report can carry, or one the engine cannot run, is `ReadFailed` with a reason that starts
//! "Unsupported"; a provider failure is `ReadFailed`. A refused gate runs and logs nothing.

use ledgerly_assist::cloud::check_report_query;
use ledgerly_assist::{AssistError, REPORT_NAMES, ReportQuery, UNSUPPORTED_REPORT};
use ledgerly_core::{CancelToken, FinancialYear, LedgerId, Outcome, Period, PeriodError};
use ledgerly_model::{AiActivity, AiActivityKind, AiFeature, Books, Scenario};
use ledgerly_reports::{ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams};
use sha2::{Digest, Sha256};

use crate::ai_features::{AiGate, hex};
use crate::error::ApiError;
use crate::facade::Api;

/// The version recorded in place of a prompt template for a question: the cloud client's
/// instructions are fixed, and a test provider has none.
const ASK_TEMPLATE: &str = "ask/1";

/// What asking the books gave.
#[derive(Debug, Clone, PartialEq)]
pub enum AskAnswer {
    /// The engine ran the report the query names. Every figure in it is the engine's.
    Ran(Box<Report>),
    /// The query names no report the engine can run as asked; the reason says why. Logged.
    Unsupported(String),
    /// The gate refused; nothing ran and nothing was logged.
    Refused(AiGate),
    /// The provider could not turn the question into a query, or gave one the question does not
    /// support; logged as a failed read.
    Failed(String),
}

/// The period a report opens with: the financial year from its start to the last voucher dated in
/// it, or the whole year when there is none.
///
/// # Errors
/// [`PeriodError`] when the company's financial year is not a valid period.
pub fn default_report_period(books: &Books) -> Result<Period, PeriodError> {
    let year = FinancialYear {
        start: books.company.fy_start,
    }
    .period()?;
    let last = books
        .vouchers
        .values()
        .map(|voucher| voucher.date)
        .filter(|date| year.contains(*date))
        .max();
    Period::new(year.from(), last.unwrap_or_else(|| year.to()))
}

/// The parameters a report of `kind` opens with in `books`: [`default_report_period`] (one day,
/// its last, for the day book), balances as at the period's end, actual vouchers, one column, no
/// filter, and detailed only for a group's or a ledger's own report.
///
/// # Errors
/// [`PeriodError`] when the company's financial year is not a valid period.
pub fn default_report_params(kind: ReportKind, books: &Books) -> Result<ReportParams, PeriodError> {
    let year_to_date = default_report_period(books)?;
    let period = if kind == ReportKind::DayBook {
        Period::new(year_to_date.to(), year_to_date.to())?
    } else {
        year_to_date
    };
    let detail = if opens_detailed(&kind) {
        Detail::Detailed
    } else {
        Detail::Condensed
    };
    Ok(ReportParams {
        kind,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    })
}

/// Whether a report opens at the detailed level: a group summary or a ledger's vouchers, as the
/// drills of `ledgerly-reports` do. Every other report opens condensed.
fn opens_detailed(kind: &ReportKind) -> bool {
    match kind {
        ReportKind::GroupSummary(_)
        | ReportKind::GroupLedgers(_)
        | ReportKind::LedgerVouchers(_) => true,
        ReportKind::TrialBalance
        | ReportKind::BalanceSheet
        | ReportKind::ProfitAndLoss
        | ReportKind::DayBook
        | ReportKind::Register(_)
        | ReportKind::Outstanding { .. }
        | ReportKind::StockSummary
        | ReportKind::Audit(_) => false,
    }
}

impl Api {
    /// Asks the books `question` through the gate for Ask (see the module documentation).
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the audit log cannot be read or the log entry cannot be
    ///   committed.
    /// - [`ApiError::AiBooksBlocked`] when the books are blocked.
    pub fn ask_books(
        &mut self,
        question: &str,
        cancel: &CancelToken,
    ) -> Result<AskAnswer, ApiError> {
        let info = match self.ai_gate(AiFeature::Ask)? {
            AiGate::Ready(info) => info,
            refused @ (AiGate::NotChosen
            | AiGate::Off
            | AiGate::FeatureOff
            | AiGate::NotInstalled(_)
            | AiGate::NotConfigured(_)) => return Ok(AskAnswer::Refused(refused)),
        };
        let books = match self.books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Err(ApiError::AiBooksBlocked { reason: u.reason }),
        };
        let asked = match self.assistant() {
            Some(provider) => provider.ask(question, cancel),
            None => Err(AssistError::Output {
                reason: "no provider is attached".to_owned(),
            }),
        }
        .and_then(|query| check_report_query(&query, question).map(|()| query));
        let log = |kind| AiActivity {
            feature: AiFeature::Ask,
            provider: info.name.clone(),
            model_version: info.version.clone(),
            prompt_template_version: ASK_TEMPLATE.to_owned(),
            input_label: question.to_owned(),
            input_hash: hex(&Sha256::digest(question.as_bytes())),
            kind,
        };
        let query = match asked {
            Ok(query) => query,
            Err(e) => {
                let reason = e.to_string();
                self.record_ai_activity(log(AiActivityKind::ReadFailed {
                    reason: reason.clone(),
                }))?;
                return Ok(AskAnswer::Failed(reason));
            }
        };
        let run = match resolve(&query, &books) {
            Ok(params) => match self.report(&params)? {
                Outcome::Value(report) => Ok(report),
                Outcome::Unsupported(u) => Err(format!("{}: {}", u.what, u.reason)),
            },
            Err(why) => Err(why),
        };
        match run {
            Ok(report) => {
                let output_json =
                    serde_json::to_string(&query).map_err(|e| ApiError::AiDraftUnreadable {
                        id: 0,
                        reason: e.to_string(),
                    })?;
                self.record_ai_activity(log(AiActivityKind::Answered { output_json }))?;
                Ok(AskAnswer::Ran(Box::new(report)))
            }
            Err(why) => {
                self.record_ai_activity(log(AiActivityKind::ReadFailed {
                    reason: format!("Unsupported: {why}"),
                }))?;
                Ok(AskAnswer::Unsupported(why))
            }
        }
    }
}

/// The report parameters `query` asks for in `books`, or why no report can carry it.
fn resolve(query: &ReportQuery, books: &Books) -> Result<ReportParams, String> {
    let kind = report_kind(query, books)?;
    let defaults = default_report_params(kind, books).map_err(|e| e.to_string())?;
    let period = match (query.from, query.to) {
        (None, None) => defaults.period,
        (from, to) => Period::new(
            from.unwrap_or_else(|| defaults.period.from()),
            to.unwrap_or_else(|| defaults.period.to()),
        )
        .map_err(|e| format!("the period asked for is not a period: {e}"))?,
    };
    Ok(ReportParams {
        period,
        as_of: period.to(),
        ..defaults
    })
}

/// The report `query` names. A ledger's vouchers need exactly one ledger, named in full; every
/// other report covers the whole books, so a ledger named for one is refused rather than dropped.
fn report_kind(query: &ReportQuery, books: &Books) -> Result<ReportKind, String> {
    let name = query.report.as_str();
    if name == UNSUPPORTED_REPORT {
        return Err(
            "no report in Ledgerly answers this question, and the assistant does not estimate figures"
                .to_owned(),
        );
    }
    if !REPORT_NAMES.contains(&name) {
        return Err(format!("no report in Ledgerly is called \"{name}\""));
    }
    if name == "ledger_vouchers" {
        return one_ledger(&query.ledger_names, books).map(ReportKind::LedgerVouchers);
    }
    if !query.ledger_names.is_empty() {
        return Err(format!(
            "the {name} report covers the whole books and cannot be narrowed to {}",
            query.ledger_names.join(", ")
        ));
    }
    match name {
        "trial_balance" => Ok(ReportKind::TrialBalance),
        "balance_sheet" => Ok(ReportKind::BalanceSheet),
        "profit_and_loss" => Ok(ReportKind::ProfitAndLoss),
        "day_book" => Ok(ReportKind::DayBook),
        "receivables" => Ok(ReportKind::Outstanding { receivable: true }),
        "payables" => Ok(ReportKind::Outstanding { receivable: false }),
        "stock_summary" => Ok(ReportKind::StockSummary),
        other => Err(format!("no report in Ledgerly is called \"{other}\"")),
    }
}

/// The one ledger `names` names in full (its name or an alias, ignoring case).
fn one_ledger(names: &[String], books: &Books) -> Result<LedgerId, String> {
    let [name] = names else {
        return Err(format!(
            "a ledger's vouchers need exactly one ledger named; the question gave {}",
            names.len()
        ));
    };
    let mut found = books.masters.ledgers.values().filter(|l| {
        l.name.eq_ignore_ascii_case(name) || l.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    });
    match (found.next(), found.next()) {
        (Some(ledger), None) => Ok(ledger.id),
        (None, _) => Err(format!("no ledger is called \"{name}\"")),
        (Some(_), Some(_)) => Err(format!("more than one ledger is called \"{name}\"")),
    }
}
