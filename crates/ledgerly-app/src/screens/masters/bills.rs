//! The Open bills card beside an altered bill-wise party (`design/mock-v2/index.html:1806`: each
//! open bill's reference, date, open amount and an overdue or "Not due" badge): the party's lines
//! of the engine's outstanding report, run on the API worker.
//!
//! The screen works nothing out (rule C4). It runs the detailed outstanding report, receivable and
//! payable, and keeps the rows of the one party:
//!
//! * a bill row gives the bill's name, its date, its pending amount, and its age as the report's
//!   bucket ("0-30 days" and so on); the screen never counts days itself;
//! * the "On account" row gives the party's unallocated amount, which is never aged;
//! * a party the reports do not list has nothing open (the report leaves out a party with nothing
//!   open);
//! * a report the engine cannot give, when the party is not in the other one, leaves no bills
//!   and says why.

use ledgerly_api::{
    Api, ApiError, Cell, ColumnSpec, Detail, DrillKey, Report, ReportFilter, ReportKind,
    ReportParams, RowKind, days_overdue,
};
use ledgerly_core::{Date, LedgerId, Outcome, Period, SignedAmount};
use ledgerly_model::Scenario;

use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::worker::ApiWorker;

/// The outstanding report's heading of a bill's date.
const DATE_HEADING: &str = "Bill date";
/// The outstanding report's heading of a bill's open amount.
const PENDING_HEADING: &str = "Pending";
/// The outstanding report's heading of the unallocated amount.
const ON_ACCOUNT_HEADING: &str = "On account";
/// The outstanding report's heading of a bill not yet due.
const NOT_DUE_HEADING: &str = "Not yet due";
/// The outstanding report's heading of a bill with no due date.
const NO_DUE_HEADING: &str = "No due date";
/// The outstanding report's heading of a bill's due date.
const DUE_HEADING: &str = "Due date";
/// The outstanding report's overdue buckets, youngest first.
const OVERDUE_HEADINGS: [&str; 4] = ["0-30 days", "31-60 days", "61-90 days", "Over 90 days"];

/// How old an open line is, as the report buckets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillAge {
    /// The due date is after the report date.
    NotDue,
    /// Overdue: the whole days past the due date as of the report date (the engine's count,
    /// `ledgerly_api::days_overdue`) and the report's bucket (its heading, such as "31-60 days").
    Overdue {
        /// Days past the due date.
        days: i64,
        /// The report's age column the amount sits in.
        bucket: String,
    },
    /// The bill has no due date, so it is not aged.
    NoDueDate,
    /// The party's unallocated amount, which is never aged.
    OnAccount,
    /// The report gave no bucket the screen can read; the text says why.
    Unknown(String),
}

/// One open line of the party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillLine {
    /// The bill's name, or "On account".
    pub name: String,
    /// The bill's date; `None` for the on-account line.
    pub date: Option<Date>,
    /// The open amount with its side, or why there is none.
    pub pending: Result<SignedAmount, String>,
    /// How old it is.
    pub age: BillAge,
}

/// What the screen holds about the party's open bills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillsState {
    /// Not asked yet, or the answer is on its way.
    NotAsked,
    /// The party's open lines; empty when nothing is open.
    Loaded(Vec<BillLine>),
    /// The engine could not give them; the text says why.
    Unsupported(String),
}

/// What one request asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ask {
    ledger: LedgerId,
    period: Period,
}

/// One party's open bills, and the call that fetches them.
#[derive(Debug)]
pub struct PartyBills {
    state: BillsState,
    want: Option<Ask>,
    latest: Latest<BillsState>,
}

impl Default for PartyBills {
    fn default() -> Self {
        Self::new()
    }
}

impl PartyBills {
    /// No bills yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: BillsState::NotAsked,
            want: None,
            latest: Latest::default(),
        }
    }

    /// Asks for the open bills of `ledger` as of the end of `period`. The call goes to the worker
    /// on the next [`PartyBills::tick`]. The bills shown are cleared, since they may be another
    /// party's.
    pub fn request(&mut self, ledger: LedgerId, period: Period) {
        self.want = Some(Ask { ledger, period });
        self.state = BillsState::NotAsked;
    }

    /// `true` while a request waits for the worker or runs on it.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.want.is_some() || self.latest.is_pending()
    }

    /// Takes a finished answer, then sends a waiting request to `worker`. With no worker the
    /// request waits.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        match self.latest.poll() {
            Some(Done::Ready(state)) => self.state = state,
            Some(Done::Lost) => {
                self.state = BillsState::Unsupported(
                    "the background worker stopped before the open bills were worked out"
                        .to_owned(),
                );
            }
            None => {}
        }
        let Some(handle) = worker else {
            return;
        };
        if let Some(ask) = self.want.take() {
            self.latest.start(handle, move |api| fetch(api, ask));
        }
    }

    /// Sets the bills directly, as an answer from the worker would.
    pub fn set(&mut self, state: BillsState) {
        self.state = state;
    }

    /// What the screen holds now.
    #[must_use]
    pub fn state(&self) -> &BillsState {
        &self.state
    }
}

/// Runs the detailed outstanding report, receivable then payable, on the worker's `Api`, and
/// keeps the party's lines.
fn fetch(api: &Api, ask: Ask) -> BillsState {
    let run = |receivable: bool| {
        api.report(&ReportParams {
            kind: ReportKind::Outstanding { receivable },
            period: ask.period,
            as_of: ask.period.to(),
            scenario: Scenario::Actual,
            detail: Detail::Detailed,
            columns: ColumnSpec::Single,
            filter: ReportFilter::default(),
            exceptions_only: false,
        })
    };
    from_answers(ask.ledger, [run(true), run(false)])
}

/// The state for the answers of the receivable and payable reports, for `ledger`. The party's
/// lines come from the report that lists it. When neither lists it, nothing is open, unless a
/// report could not be given: the party may be in that one, so the reason is shown instead.
#[must_use]
pub fn from_answers(
    ledger: LedgerId,
    answers: [Result<Outcome<Report>, ApiError>; 2],
) -> BillsState {
    let mut failed: Option<String> = None;
    for answer in answers {
        match answer {
            Ok(Outcome::Value(report)) => {
                if let Some(lines) = party_lines(&report, ledger) {
                    return BillsState::Loaded(lines);
                }
            }
            Ok(Outcome::Unsupported(u)) => {
                failed.get_or_insert_with(|| unsupported_text(&u));
            }
            Err(e) => {
                failed.get_or_insert_with(|| format!("the outstanding report failed: {e}"));
            }
        }
    }
    failed.map_or_else(|| BillsState::Loaded(Vec::new()), BillsState::Unsupported)
}

/// The open lines of `ledger` in a detailed outstanding report, or `None` when the report does not
/// list the party.
#[must_use]
pub fn party_lines(report: &Report, ledger: LedgerId) -> Option<Vec<BillLine>> {
    let at = |label: &str| report.columns.iter().position(|c| c.label == label);
    let mut listed = false;
    let mut lines = Vec::new();
    for row in &report.rows {
        match (&row.kind, &row.key) {
            (RowKind::Ledger, Some(DrillKey::Ledger(id))) if *id == ledger => listed = true,
            (RowKind::Bill, Some(DrillKey::Bill { ledger: id, name })) if *id == ledger => {
                lines.push(BillLine {
                    name: name.clone(),
                    date: match at(DATE_HEADING).and_then(|i| row.cells.get(i)) {
                        Some(Cell::Date(d)) => Some(*d),
                        Some(_) | None => None,
                    },
                    pending: amount(at(PENDING_HEADING).and_then(|i| row.cells.get(i))),
                    age: age(report, &row.cells, due_date(report, &row.cells)),
                });
            }
            (RowKind::Bill, Some(DrillKey::Ledger(id))) if *id == ledger => {
                lines.push(BillLine {
                    name: row.label.clone(),
                    date: None,
                    pending: amount(at(ON_ACCOUNT_HEADING).and_then(|i| row.cells.get(i))),
                    age: BillAge::OnAccount,
                });
            }
            (
                RowKind::Ledger
                | RowKind::Bill
                | RowKind::Group
                | RowKind::Voucher
                | RowKind::Item
                | RowKind::Difference
                | RowKind::Subtotal
                | RowKind::Text,
                _,
            ) => {}
        }
    }
    listed.then_some(lines)
}

/// A cell as an amount with its side.
fn amount(cell: Option<&Cell>) -> Result<SignedAmount, String> {
    match cell {
        Some(Cell::Amount(a)) => Ok(*a),
        Some(Cell::Unsupported(u)) => Err(unsupported_text(u)),
        Some(Cell::Provisional { reason, .. }) => {
            Err(format!("the amount is provisional: {reason}"))
        }
        Some(Cell::Empty | Cell::Signed(_) | Cell::Qty(_) | Cell::Text(_) | Cell::Date(_))
        | None => Err("the outstanding report gives no amount here".to_owned()),
    }
}

/// The due date cell of a bill row, when it holds a date.
fn due_date(report: &Report, cells: &[Cell]) -> Option<Date> {
    let at = report.columns.iter().position(|c| c.label == DUE_HEADING)?;
    match cells.get(at) {
        Some(Cell::Date(d)) => Some(*d),
        Some(_) | None => None,
    }
}

/// A bill's age: the bucket column that holds its amount, and for an overdue bill the engine's
/// count of days past the due date as of the report date (`ledgerly_api::days_overdue`, the rule
/// the buckets are cut from). An overdue bill with no due date cell, or one the count calls not
/// overdue, is not given an age (rule A1).
fn age(report: &Report, cells: &[Cell], due: Option<Date>) -> BillAge {
    let filled = |label: &str| {
        report
            .columns
            .iter()
            .position(|c| c.label == label)
            .and_then(|i| cells.get(i))
            .is_some_and(|c| !matches!(c, Cell::Empty))
    };
    let mut found: Vec<BillAge> = Vec::new();
    if filled(NOT_DUE_HEADING) {
        found.push(BillAge::NotDue);
    }
    for label in OVERDUE_HEADINGS {
        if filled(label) {
            match days_overdue(report.params.as_of, due) {
                Some(days) => found.push(BillAge::Overdue {
                    days,
                    bucket: label.to_owned(),
                }),
                None => {
                    return BillAge::Unknown(
                        "the outstanding report puts the bill in an overdue column but gives no due date to count the days from"
                            .to_owned(),
                    );
                }
            }
        }
    }
    if filled(NO_DUE_HEADING) {
        found.push(BillAge::NoDueDate);
    }
    match found.as_slice() {
        [one] => one.clone(),
        [] => BillAge::Unknown("the outstanding report puts the bill in no age column".to_owned()),
        [_, _, ..] => BillAge::Unknown(
            "the outstanding report puts the bill in more than one age column".to_owned(),
        ),
    }
}

/// The badge text of an age, with whether it calls for attention; `None` for no badge.
#[must_use]
pub fn age_badge(age: &BillAge) -> Option<(String, bool)> {
    match age {
        BillAge::NotDue => Some((crate::i18n::t("masters.bills.not_due"), false)),
        BillAge::Overdue { days: 1, .. } => {
            Some((crate::i18n::t("masters.bills.overdue_one"), true))
        }
        BillAge::Overdue { days, .. } => Some((
            crate::i18n::tf("masters.bills.overdue_many", &[("days", &days.to_string())]),
            true,
        )),
        BillAge::NoDueDate => Some((crate::i18n::t("masters.bills.no_due"), false)),
        BillAge::OnAccount => None,
        BillAge::Unknown(_) => Some(("\u{2014}".to_owned(), false)),
    }
}
