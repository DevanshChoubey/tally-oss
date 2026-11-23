//! The ledger list's Balance column (`design/mock-v2/index.html:1796`, `Balance
//! ${prov('eng','engine')}`): each ledger's closing balance from the engine's trial balance, run on
//! the API worker.
//!
//! The screen works nothing out (rule C4). It reads the trial balance's ledger rows and joins them
//! to ledgers by id only:
//!
//! * a ledger row with one amount shows that amount and its side;
//! * a ledger the report has no row for shows nothing, never 0.00 (the engine leaves out a nil
//!   balance, but the screen does not assume why a row is missing);
//! * a cell the engine could not compute, or one that is not an amount, shows a dash;
//! * until the engine answers, or when it cannot, every cell shows a dash and the heading says
//!   why on hover.

use std::collections::BTreeMap;

use ledgerly_api::{
    Api, ApiError, Cell, ColumnSpec, Detail, DrillKey, Report, ReportFilter, ReportKind,
    ReportParams, RowKind,
};
use ledgerly_core::{Date, LedgerId, Outcome, Period, SignedAmount};
use ledgerly_model::Scenario;

use crate::screens::report_view::amount_text;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::worker::ApiWorker;

/// What a cell shows where there is no figure.
const DASH: &str = "\u{2014}";
/// The heading's hover text before the figures are here.
const LOADING_HINT: &str =
    "The balances have not come from the engine yet. Nothing is estimated here.";
/// The heading's hover text when the engine could not give the balances.
const FAILED_HINT: &str =
    "The engine could not give the balances, so none are shown. Nothing is estimated here.";

/// One ledger's balance as the engine gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BalanceCell {
    /// The closing balance with its side.
    Amount(SignedAmount),
    /// The engine gave no usable figure; the text says why.
    Unsupported(String),
}

/// What the screen holds about the balances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BalanceState {
    /// Not asked yet, or the answer is on its way.
    NotAsked,
    /// The engine's balances, by ledger.
    Loaded(BTreeMap<LedgerId, BalanceCell>),
    /// The engine could not give them; the text says why.
    Unsupported(String),
}

/// The balances of the ledger list, and the call that fetches them.
#[derive(Debug)]
pub struct LedgerBalances {
    state: BalanceState,
    want: Option<Date>,
    latest: Latest<BalanceState>,
}

impl Default for LedgerBalances {
    fn default() -> Self {
        Self::new()
    }
}

impl LedgerBalances {
    /// No balances yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: BalanceState::NotAsked,
            want: None,
            latest: Latest::default(),
        }
    }

    /// Asks for the closing balances as of `as_of`. The call goes to the worker on the next
    /// [`LedgerBalances::tick`]; the balances shown stay until the answer comes.
    pub fn request(&mut self, as_of: Date) {
        self.want = Some(as_of);
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
                self.state = BalanceState::Unsupported(
                    "the background worker stopped before the balances were worked out".to_owned(),
                );
            }
            None => {}
        }
        let Some(handle) = worker else {
            return;
        };
        if let Some(as_of) = self.want.take() {
            self.latest.start(handle, move |api| fetch(api, as_of));
        }
    }

    /// Sets the balances directly, as an answer from the worker would.
    pub fn set(&mut self, state: BalanceState) {
        self.state = state;
    }

    /// What the screen holds now.
    #[must_use]
    pub fn state(&self) -> &BalanceState {
        &self.state
    }

    /// The text of ledger `id`'s Balance cell.
    #[must_use]
    pub fn text(&self, id: LedgerId) -> String {
        match &self.state {
            BalanceState::Loaded(map) => match map.get(&id) {
                Some(BalanceCell::Amount(amount)) => amount_text(*amount),
                Some(BalanceCell::Unsupported(_)) => DASH.to_owned(),
                None => String::new(),
            },
            BalanceState::NotAsked | BalanceState::Unsupported(_) => DASH.to_owned(),
        }
    }

    /// The Balance heading's hover text: why there are no figures, or `None` once they are here.
    #[must_use]
    pub fn heading_hint(&self) -> Option<&'static str> {
        match self.state {
            BalanceState::Loaded(_) => None,
            BalanceState::NotAsked => Some(LOADING_HINT),
            BalanceState::Unsupported(_) => Some(FAILED_HINT),
        }
    }

    /// Why the engine could not give the balances, when it could not.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        match &self.state {
            BalanceState::Unsupported(why) => Some(why),
            BalanceState::NotAsked | BalanceState::Loaded(_) => None,
        }
    }
}

/// Runs the detailed trial balance as of `as_of` on the worker's `Api`.
fn fetch(api: &Api, as_of: Date) -> BalanceState {
    let period = match Period::new(as_of, as_of) {
        Ok(period) => period,
        Err(e) => return BalanceState::Unsupported(format!("the date is not usable: {e}")),
    };
    let params = ReportParams {
        kind: ReportKind::TrialBalance,
        period,
        as_of,
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    from_answer(api.report(&params))
}

/// The state for one answer of `Api::report`.
#[must_use]
pub fn from_answer(answer: Result<Outcome<Report>, ApiError>) -> BalanceState {
    match answer {
        Ok(Outcome::Value(report)) => BalanceState::Loaded(from_report(&report)),
        Ok(Outcome::Unsupported(u)) => BalanceState::Unsupported(unsupported_text(&u)),
        Err(e) => BalanceState::Unsupported(format!("the trial balance failed: {e}")),
    }
}

/// The balance of every ledger row of a trial balance, by ledger id. Group, difference and other
/// rows belong to no ledger and are skipped.
#[must_use]
pub fn from_report(report: &Report) -> BTreeMap<LedgerId, BalanceCell> {
    let mut out = BTreeMap::new();
    for row in &report.rows {
        let id = match (&row.kind, &row.key) {
            (RowKind::Ledger, Some(DrillKey::Ledger(id))) => *id,
            (
                RowKind::Ledger
                | RowKind::Group
                | RowKind::Voucher
                | RowKind::Item
                | RowKind::Bill
                | RowKind::Difference
                | RowKind::Subtotal
                | RowKind::Text,
                _,
            ) => continue,
        };
        out.insert(id, ledger_cell(&row.cells));
    }
    out
}

/// One ledger row's Debit and Credit cells as one balance: the one amount the row holds.
fn ledger_cell(cells: &[Cell]) -> BalanceCell {
    let mut found: Option<SignedAmount> = None;
    for cell in cells {
        match cell {
            Cell::Empty => {}
            Cell::Amount(amount) => {
                if found.is_some() {
                    return BalanceCell::Unsupported(
                        "the trial balance has an amount on both sides of this ledger".to_owned(),
                    );
                }
                found = Some(*amount);
            }
            Cell::Unsupported(u) => return BalanceCell::Unsupported(unsupported_text(u)),
            Cell::Provisional { reason, .. } => {
                return BalanceCell::Unsupported(format!("the balance is provisional: {reason}"));
            }
            Cell::Signed(_) | Cell::Qty(_) | Cell::Text(_) | Cell::Date(_) => {
                return BalanceCell::Unsupported(
                    "the trial balance holds a cell that is not an amount".to_owned(),
                );
            }
        }
    }
    found.map_or_else(
        || BalanceCell::Unsupported("the trial balance row has no amount".to_owned()),
        BalanceCell::Amount,
    )
}
