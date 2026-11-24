//! The four KPI tiles, from the engine's [`HomeFigures`]. The words under each figure state
//! what the figure counts; the counts come from the engine too (rule C4). The mock's tiles are
//! `SCREENS.home`, `design/mock-v2/index.html` lines 1061 to 1065.

use ledgerly_api::{Api, CUSTOMER_LATE_DAYS, HomeFigures, LateBills};
use ledgerly_core::{Date, Outcome};

use super::{HomeTarget, Kpi};
use crate::ui::format;

/// Why the GST tile has no figure: the engine has the GSTR-3B working but not the payable
/// after set-off, which needs the input credit accepted from GSTR-2B, and it has no reader for
/// that file.
const GST_REASON: &str = "the set-off needs the input credit accepted from GSTR-2B, and the engine cannot read GSTR-2B yet";

/// "s" for any count but one.
fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// The line under the cash tile: how many bank accounts the figure includes
/// ("Cash and 2 bank accounts").
#[must_use]
pub fn cash_footnote(bank_accounts: usize) -> String {
    format!(
        "Cash and {bank_accounts} bank account{}",
        plural(bank_accounts)
    )
}

/// The line under the receivables tile ("3 bills overdue more than 30 days"), or the engine's
/// reason when it cannot count them. The count is never shown as zero when it is not known.
#[must_use]
pub fn receivables_footnote(late: &Outcome<LateBills>) -> String {
    match late {
        Outcome::Value(l) => format!(
            "{} bill{} overdue more than {CUSTOMER_LATE_DAYS} days",
            l.count,
            plural(l.count)
        ),
        Outcome::Unsupported(u) => format!("Overdue bills not counted: {}", u.reason),
    }
}

/// The line under the payables tile ("3 supplier bills past due"), or the engine's reason.
#[must_use]
pub fn payables_footnote(late: &Outcome<LateBills>) -> String {
    match late {
        Outcome::Value(l) => format!("{} supplier bill{} past due", l.count, plural(l.count)),
        Outcome::Unsupported(u) => format!("Past-due bills not counted: {}", u.reason),
    }
}

/// The four tiles for `figures`, as of `as_of`: cash and bank, receivables, payables, and the
/// GST cash payable of the month (which stays unsupported, see `GST_REASON`).
#[must_use]
pub fn tiles(figures: &HomeFigures, as_of: Date) -> Vec<Kpi> {
    let banks = match &figures.cash_and_bank {
        Outcome::Value(c) => cash_footnote(c.bank_accounts),
        Outcome::Unsupported(_) => String::new(),
    };
    vec![
        Kpi {
            label: "Cash and bank".to_owned(),
            value: figures.cash_and_bank.clone().map(|c| c.total),
            footnote: banks,
            target: HomeTarget::BalanceSheet {
                node: "Bank accounts".to_owned(),
            },
        },
        Kpi {
            label: "Receivables".to_owned(),
            value: figures.receivables.clone(),
            footnote: receivables_footnote(&figures.receivables_overdue),
            target: HomeTarget::BalanceSheet {
                node: "Sundry debtors".to_owned(),
            },
        },
        Kpi {
            label: "Payables".to_owned(),
            value: figures.payables.clone(),
            footnote: payables_footnote(&figures.payables_past_due),
            target: HomeTarget::BalanceSheet {
                node: "Sundry creditors".to_owned(),
            },
        },
        gst_tile(as_of),
    ]
}

/// The GST tile: titled with the month of `as_of` ("GST cash payable, Nov"), in the same slot
/// and size as the others, saying why it has no figure.
fn gst_tile(as_of: Date) -> Kpi {
    Kpi {
        label: format!("GST cash payable, {}", format::month_abbrev(as_of.month())),
        value: Outcome::unsupported("Payable after set-off", GST_REASON),
        footnote: "GSTR-3B working, after accepted ITC".to_owned(),
        target: HomeTarget::GstReturns {
            tab: "gstr3b".to_owned(),
        },
    }
}

/// An `Unsupported` outcome for `what`, with `reason`.
fn none<T>(what: &str, reason: &str) -> Outcome<T> {
    Outcome::unsupported(what, reason)
}

/// The engine's figures for `as_of`, or, when they could not be read, figures that all say so:
/// each is `Unsupported` with the error as its reason, so no tile or nudge shows a guess.
#[must_use]
pub fn figures_or_failed(api: &Api, as_of: Date) -> HomeFigures {
    match api.home_figures(as_of) {
        Ok(figures) => figures,
        Err(e) => {
            let reason = format!("the figures could not be read: {e}");
            HomeFigures {
                cash_and_bank: none("Cash and bank", &reason),
                receivables: none("Receivables", &reason),
                payables: none("Payables", &reason),
                receivables_overdue: none("Overdue customer bills", &reason),
                payables_past_due: none("Past-due supplier bills", &reason),
                sales_by_month: none("Sales by month", &reason),
            }
        }
    }
}

/// The home KPI tiles as of `as_of`. Runs on the API worker. A tile whose figure the engine
/// cannot give carries `Unsupported` with the reason; no tile ever carries a guessed figure.
#[must_use]
pub fn home_kpis(api: &Api, as_of: Date) -> Vec<Kpi> {
    tiles(&figures_or_failed(api, as_of), as_of)
}
