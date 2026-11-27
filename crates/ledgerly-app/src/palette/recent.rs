//! Live details for the palette's Recent rows (`design/mock-v2/index.html:1115`): a recent
//! balance sheet says the date it opens at, and a recent ledger its group and its balance.
//!
//! Runs on the API thread. Every figure comes from an engine report (the ledger's vouchers, run
//! with the parameters the ledger would open with); nothing here adds or rounds a figure. A
//! report the engine cannot compute shows its reason, never a figure.

use ledgerly_api::{Api, Books, ReportKind, ReportRow};
use ledgerly_core::Outcome;

use super::PaletteTarget;
use crate::screens::report_view::engine_cell_text;
use crate::shell::navigate::{default_params, report_from_key};
use crate::ui::format;

/// The live detail of each target that has one, in the order given. Targets with no live
/// detail (commands, vouchers, settings, other reports) are left out, so their rows keep the
/// detail they had when chosen. With no books loaded, nothing has a live detail.
pub(super) fn live_details(api: &Api, targets: &[PaletteTarget]) -> Vec<(PaletteTarget, String)> {
    let Ok(Outcome::Value(books)) = api.books() else {
        return Vec::new();
    };
    targets
        .iter()
        .filter_map(|target| live_detail(api, &books, target).map(|d| (target.clone(), d)))
        .collect()
}

fn live_detail(api: &Api, books: &Books, target: &PaletteTarget) -> Option<String> {
    match target {
        PaletteTarget::Report(key) => match report_from_key(key)? {
            ReportKind::BalanceSheet => {
                let params = default_params(ReportKind::BalanceSheet, books).ok()?;
                Some(format!("As at {}", format::date(params.as_of)))
            }
            ReportKind::TrialBalance
            | ReportKind::ProfitAndLoss
            | ReportKind::GroupSummary(_)
            | ReportKind::GroupLedgers(_)
            | ReportKind::LedgerVouchers(_)
            | ReportKind::DayBook
            | ReportKind::Register(_)
            | ReportKind::Outstanding { .. }
            | ReportKind::StockSummary
            | ReportKind::Audit(_) => None,
        },
        PaletteTarget::Master(key) => match report_from_key(key)? {
            ReportKind::LedgerVouchers(id) => {
                let ledger = books.masters.ledgers.get(&id)?;
                let group = books.masters.groups.get(&ledger.group?)?;
                let params = default_params(ReportKind::LedgerVouchers(id), books).ok()?;
                let balance = match api.report(&params) {
                    Ok(Outcome::Value(report)) => closing_text(&report.rows),
                    Ok(Outcome::Unsupported(u)) => format!("not computed: {}", u.reason),
                    Err(e) => format!("not computed: {e}"),
                };
                Some(format!("Ledger \u{00B7} {} \u{00B7} {balance}", group.name))
            }
            ReportKind::TrialBalance
            | ReportKind::BalanceSheet
            | ReportKind::ProfitAndLoss
            | ReportKind::GroupSummary(_)
            | ReportKind::GroupLedgers(_)
            | ReportKind::DayBook
            | ReportKind::Register(_)
            | ReportKind::Outstanding { .. }
            | ReportKind::StockSummary
            | ReportKind::Audit(_) => None,
        },
        PaletteTarget::Command(_)
        | PaletteTarget::Run(_)
        | PaletteTarget::Voucher(_)
        | PaletteTarget::Setting(_)
        | PaletteTarget::AiIntent(_)
        | PaletteTarget::Fill(_) => None,
    }
}

/// The ledger's closing balance as the engine's report shows it: the last cell of its
/// `Closing Balance` row.
fn closing_text(rows: &[ReportRow]) -> String {
    rows.iter()
        .rev()
        .find(|row| row.label == "Closing Balance")
        .and_then(|row| row.cells.last())
        .map_or_else(
            || "not computed: the ledger report has no closing balance".to_owned(),
            engine_cell_text,
        )
}
