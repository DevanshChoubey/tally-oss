//! What one refresh of Home reads from the engine, on the API worker: the tiles, the sales bars
//! and the nudges, all from the same books.

use ledgerly_api::{Api, DuplicatePayment, HomeNudges};
use ledgerly_core::{Date, Outcome};

use super::Kpi;
use super::chart::{self, Chart};
use super::kpi;
use super::nudge::{self, Nudge};

/// The answer to one refresh.
#[derive(Debug, Clone)]
pub(super) struct HomeLoad {
    /// The four tiles.
    pub(super) kpis: Vec<Kpi>,
    /// The sales bars, or why there are none.
    pub(super) chart: Chart,
    /// The nudge cards.
    pub(super) nudges: Vec<Nudge>,
    /// The possible duplicate payments, in the order `NudgeAction::Compare` indexes them.
    pub(super) duplicates: Vec<DuplicatePayment>,
    /// The signed-in user's stored name, when there is one.
    pub(super) user_name: Option<String>,
}

/// Reads everything Home shows for `as_of`, with the nudges as of `today`.
pub(super) fn home_load(api: &Api, as_of: Date, today: Date) -> HomeLoad {
    let figures = kpi::figures_or_failed(api, as_of);
    let found = api.home_nudges(today).unwrap_or_else(|e| HomeNudges {
        gstr1: None,
        duplicates: Outcome::unsupported(
            "Duplicate payments",
            format!("the books could not be read: {e}"),
        ),
    });
    let duplicates = match &found.duplicates {
        Outcome::Value(pairs) => pairs.clone(),
        Outcome::Unsupported(_) => Vec::new(),
    };
    HomeLoad {
        kpis: kpi::tiles(&figures, as_of),
        chart: chart::chart_of(&figures.sales_by_month),
        nudges: nudge::nudges(&figures, &found),
        duplicates,
        user_name: api.actor_name().unwrap_or(None),
    }
}
