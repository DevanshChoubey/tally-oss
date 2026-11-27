//! Round s21, plan round E: Ask your books (spec 6.8). A question goes through the one AI gate to
//! the provider, which answers with a `ReportQuery`. The API resolves the query to a report the
//! engine knows, runs it with `Api::report`, and hands back the engine's `Report` and nothing the
//! provider wrote. Every ask that runs is logged: an answer as `Answered`, a query the engine
//! cannot run (no report carries it, or the report engine fails on it) as `NotRun`, a provider
//! failure as `ReadFailed` (coordinator, 2026-11-27: an ask that cannot run is its own kind).
//!
//! Provenance (derived). The questions and the recorded queries are written here. The company's
//! financial year starts 1 April 2026, so it runs to 31 March 2027 (Indian financial year); with
//! no voucher dated in it the default period is the whole year (`default_report_period`, the rule
//! the report screens open with). Each run report is compared with the report the engine gives
//! for the same parameters, never with figures copied from it.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ledgerly_api::{
    AiFeature, AiGate, AiOutput, AiTier, Api, ApiError, AskAnswer, CloudRequest, CloudResponse,
    CloudSettings, CloudTransport, Command, FixedAnswers, KEY_SERVICE, MemoryVault, ReportKind,
    ReportQuery, default_report_params,
};
use ledgerly_assist::AssistError;
use ledgerly_core::{
    CancelToken, Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, Period, SignedAmount,
    StateCode,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Ledger, NegativeStockPolicy, ReservedGroup,
};
use time::macros::{date, datetime};

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn ledger(id: u64, name: &str, kind: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group(kind)),
        // Only Cash opens with a balance: an opening balance on an income or expense ledger makes
        // the profit and loss Unsupported (the engine's rule), which is not what these tests ask.
        opening: if kind == ReservedGroup::CashInHand {
            SignedAmount::dr(Paise::new(100_000_000))
        } else {
            SignedAmount::dr(Paise::new(0))
        },
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn books(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    };
    let mut api = Api::create(
        &path,
        &company,
        Clock::Fixed(datetime!(2026-11-21 10:00 UTC)),
    )
    .unwrap();
    for l in [
        ledger(1, "Cash", ReservedGroup::CashInHand),
        ledger(3, "Rent", ReservedGroup::IndirectExpenses),
        ledger(4, "Rent Deposit", ReservedGroup::IndirectExpenses),
    ] {
        match api.execute(Command::CreateLedger(l)) {
            Ok(Outcome::Value(_)) => {}
            other => panic!("expected a committed ledger, got {other:?}"),
        }
    }
    api
}

fn query(report: &str, ledger_names: &[&str]) -> ReportQuery {
    ReportQuery {
        report: report.to_owned(),
        from: None,
        to: None,
        ledger_names: ledger_names.iter().map(|n| (*n).to_owned()).collect(),
    }
}

/// A file with the local tier on and a provider that answers `question` with `q`.
fn asking(name: &str, question: &str, q: ReportQuery) -> Api {
    let mut api = books(name);
    api.set_ai_tier(AiTier::Local).unwrap();
    api.attach_assistant(Box::new(FixedAnswers::new().with_question(question, q)));
    api
}

fn ask(api: &mut Api, question: &str) -> AskAnswer {
    api.ask_books(question, &CancelToken::new()).unwrap()
}

fn whole_year() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap()
}

fn engine_report(api: &Api, kind: ReportKind) -> ledgerly_api::Report {
    let books = match api.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
    let params = default_report_params(kind, &books).unwrap();
    match api.report(&params).unwrap() {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    }
}

#[test]
fn a_question_runs_the_report_the_query_names_with_the_default_period_and_is_logged() {
    let question = "show me the trial balance";
    let mut api = asking("s21-ask-tb.ledgerly", question, query("trial_balance", &[]));
    let AskAnswer::Ran(report) = ask(&mut api, question) else {
        panic!("a trial balance query runs");
    };
    assert_eq!(report.params.kind, ReportKind::TrialBalance);
    assert_eq!(report.params.period, whole_year());
    assert_eq!(*report, engine_report(&api, ReportKind::TrialBalance));
    let rows = api.ai_log_rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].feature, AiFeature::Ask);
    assert_eq!(rows[0].input_label, question);
    assert_eq!(rows[0].output, AiOutput::Answered);
}

#[test]
fn each_report_name_maps_to_its_engine_report() {
    let cases = [
        ("balance_sheet", ReportKind::BalanceSheet),
        ("profit_and_loss", ReportKind::ProfitAndLoss),
        ("day_book", ReportKind::DayBook),
        ("receivables", ReportKind::Outstanding { receivable: true }),
        ("payables", ReportKind::Outstanding { receivable: false }),
        ("stock_summary", ReportKind::StockSummary),
    ];
    for (name, kind) in cases {
        let question = format!("show the {name}");
        let mut api = asking(
            &format!("s21-ask-{name}.ledgerly"),
            &question,
            query(name, &[]),
        );
        let AskAnswer::Ran(report) = ask(&mut api, &question) else {
            panic!("{name} runs: {:?}", ask(&mut api, &question));
        };
        assert_eq!(report.params.kind, kind, "{name}");
        assert_eq!(*report, engine_report(&api, kind.clone()), "{name}");
    }
}

#[test]
fn a_ledgers_vouchers_need_exactly_one_ledger_named_in_full() {
    let question = "show the Rent ledger";
    let mut api = asking(
        "s21-ask-ledger.ledgerly",
        question,
        query("ledger_vouchers", &["Rent"]),
    );
    let AskAnswer::Ran(report) = ask(&mut api, question) else {
        panic!("one ledger named: its vouchers run");
    };
    assert_eq!(report.params.kind, ReportKind::LedgerVouchers(LedgerId(3)));

    // A name no ledger has, two names, or no name: no report is run.
    for (file, question, q) in [
        (
            "none",
            "show the Electricity ledger",
            query("ledger_vouchers", &["Electricity"]),
        ),
        (
            "two",
            "show the Rent and Cash ledgers",
            query("ledger_vouchers", &["Rent", "Cash"]),
        ),
        ("empty", "show a ledger", query("ledger_vouchers", &[])),
    ] {
        let mut api = asking(&format!("s21-ask-ledger-{file}.ledgerly"), question, q);
        let answer = ask(&mut api, question);
        assert!(
            matches!(answer, AskAnswer::Unsupported(_)),
            "{file}: {answer:?}"
        );
    }
}

#[test]
fn a_ledger_named_for_a_whole_books_report_is_refused_not_dropped() {
    // Dropping the name would answer a question about Rent with the whole trial balance.
    let question = "trial balance for Rent";
    let mut api = asking(
        "s21-ask-tb-ledger.ledgerly",
        question,
        query("trial_balance", &["Rent"]),
    );
    assert!(matches!(ask(&mut api, question), AskAnswer::Unsupported(_)));
}

#[test]
fn stated_dates_set_the_period_and_a_lone_date_keeps_the_other_end_of_the_default() {
    let question = "day book from 01-11-2026 to 21-11-2026";
    let mut q = query("day_book", &[]);
    q.from = Some(date!(2026 - 11 - 01));
    q.to = Some(date!(2026 - 11 - 21));
    let mut api = asking("s21-ask-dates.ledgerly", question, q);
    let AskAnswer::Ran(report) = ask(&mut api, question) else {
        panic!("stated dates run");
    };
    assert_eq!(
        report.params.period,
        Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 21)).unwrap()
    );
    assert_eq!(report.params.as_of, date!(2026 - 11 - 21));

    let question = "profit and loss from 01-10-2026";
    let mut q = query("profit_and_loss", &[]);
    q.from = Some(date!(2026 - 10 - 01));
    let mut api = asking("s21-ask-from.ledgerly", question, q);
    let AskAnswer::Ran(report) = ask(&mut api, question) else {
        panic!("a from date runs");
    };
    assert_eq!(
        report.params.period,
        Period::new(date!(2026 - 10 - 01), date!(2027 - 03 - 31)).unwrap()
    );
}

#[test]
fn a_backwards_period_is_unsupported() {
    let question = "day book from 21-11-2026 to 01-11-2026";
    let mut q = query("day_book", &[]);
    q.from = Some(date!(2026 - 11 - 21));
    q.to = Some(date!(2026 - 11 - 01));
    let mut api = asking("s21-ask-backwards.ledgerly", question, q);
    assert!(matches!(ask(&mut api, question), AskAnswer::Unsupported(_)));
}

#[test]
fn a_date_or_name_the_question_does_not_state_fails_and_is_logged() {
    // The provider moved the period: the question states no date at all.
    let question = "show the day book";
    let mut q = query("day_book", &[]);
    q.from = Some(date!(2026 - 06 - 01));
    let mut api = asking("s21-ask-moved.ledgerly", question, q);
    assert!(matches!(ask(&mut api, question), AskAnswer::Failed(_)));
    let rows = api.ai_log_rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert!(matches!(rows[0].output, AiOutput::ReadFailed(_)));
}

#[test]
fn a_report_no_one_has_is_unsupported_and_logged_as_such() {
    let question = "what will my profit be next quarter?";
    let mut api = asking(
        "s21-ask-forecast.ledgerly",
        question,
        query("unsupported", &[]),
    );
    let AskAnswer::Unsupported(reason) = ask(&mut api, question) else {
        panic!("no report forecasts");
    };
    assert!(reason.contains("does not estimate figures"), "{reason}");
    let rows = api.ai_log_rows().unwrap();
    assert_eq!(rows.len(), 1);
    let AiOutput::NotRun(logged) = &rows[0].output else {
        panic!(
            "an unsupported ask is logged as not run: {:?}",
            rows[0].output
        );
    };
    assert!(logged.contains("does not estimate figures"), "{logged}");

    let mut api = asking(
        "s21-ask-unknown.ledgerly",
        question,
        query("cash_forecast", &[]),
    );
    assert!(matches!(ask(&mut api, question), AskAnswer::Unsupported(_)));
}

#[test]
fn a_refused_gate_runs_and_logs_nothing() {
    let question = "show me the trial balance";
    let mut api = asking(
        "s21-ask-off.ledgerly",
        question,
        query("trial_balance", &[]),
    );
    api.set_ai_feature(AiFeature::Ask, false).unwrap();
    assert_eq!(
        ask(&mut api, question),
        AskAnswer::Refused(AiGate::FeatureOff)
    );
    assert_eq!(api.ai_log_rows().unwrap().len(), 0);
}

/// The books begin on 1 April 2026; a balance sheet as of 31 March 2026 is one the report engine
/// refuses with an error (`balance_sheet.rs`: as of a date before the books begin). The error
/// goes back to the caller, and the ask is logged as not run with the engine's reason.
#[test]
fn a_report_engine_error_is_returned_and_logged_as_not_run() {
    let question = "balance sheet from 01-01-2026 to 31-03-2026";
    let mut q = query("balance_sheet", &[]);
    q.from = Some(date!(2026 - 01 - 01));
    q.to = Some(date!(2026 - 03 - 31));
    let mut api = asking("s21-ask-engine-error.ledgerly", question, q);
    let out = api.ask_books(question, &CancelToken::new());
    assert!(matches!(out, Err(ApiError::Report(_))), "{out:?}");
    let rows = api.ai_log_rows().unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let AiOutput::NotRun(logged) = &rows[0].output else {
        panic!("the failed run is logged as not run: {:?}", rows[0].output);
    };
    assert!(logged.contains("before the books begin"), "{logged}");
}

#[test]
fn a_provider_failure_is_logged() {
    let mut api = asking(
        "s21-ask-fail.ledgerly",
        "recorded",
        query("trial_balance", &[]),
    );
    assert!(matches!(
        ask(&mut api, "not recorded"),
        AskAnswer::Failed(_)
    ));
    assert_eq!(api.ai_log_rows().unwrap().len(), 1);
}

const PROSE_FIGURE: &str = "4,321,987";

/// Answers every request in the Messages shape with a figure in prose around the query.
#[derive(Clone, Default)]
struct ProseAnswer {
    sent: Arc<Mutex<usize>>,
}

impl CloudTransport for ProseAnswer {
    fn post(&self, _request: &CloudRequest) -> Result<CloudResponse, AssistError> {
        *self.sent.lock().unwrap() += 1;
        let text = format!(
            "Your profit is Rs {PROSE_FIGURE}. {{\"ReportQuery\": {{\"report\": \"profit_and_loss\", \"from\": null, \"to\": null, \"ledger_names\": []}}}}"
        );
        let body = serde_json::json!({"content": [{"type": "text", "text": text}]});
        Ok(CloudResponse {
            status: 200,
            body: body.to_string(),
        })
    }
}

#[test]
fn a_figure_the_cloud_writes_in_prose_never_reaches_the_answer() {
    let mut api = books("s21-ask-cloud.ledgerly");
    api.attach_key_vault(Box::new(MemoryVault::with_key(
        KEY_SERVICE,
        "company-1-Fictional Traders",
        "fictional-key-ask-s21",
    )));
    let settings = CloudSettings {
        format: Some("messages".to_owned()),
        model: Some("model-ask".to_owned()),
        ..CloudSettings::default()
    };
    assert_eq!(
        api.set_cloud_settings(&settings).unwrap(),
        Outcome::Value(())
    );
    api.set_ai_tier(AiTier::Cloud).unwrap();
    let transport = ProseAnswer::default();
    let Outcome::Value(cloud) = api.cloud_assistant(Box::new(transport.clone())).unwrap() else {
        panic!("a cloud assistant");
    };
    api.attach_assistant(Box::new(cloud));
    let question = "how is my profit and loss";
    let AskAnswer::Ran(report) = ask(&mut api, question) else {
        panic!("the query inside the prose runs");
    };
    assert_eq!(*transport.sent.lock().unwrap(), 1);
    assert_eq!(*report, engine_report(&api, ReportKind::ProfitAndLoss));
    let shown = format!("{report:?}");
    assert!(!shown.contains("4321987") && !shown.contains(PROSE_FIGURE));
}
