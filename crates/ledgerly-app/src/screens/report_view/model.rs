//! The report data the screen shows, and the requests it makes of the API or the host.

use ledgerly_core::{
    Date, GroupId, LedgerId, Paise, Period, Qty, SignedAmount, StockItemId, Unsupported, VoucherId,
};
use thiserror::Error;

use crate::Screen;

/// Opaque handle the host uses to re-run a report. The screen stores it and hands it back; it
/// never reads it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReportToken(pub String);

/// Condensed (primary groups) or detailed (down to ledgers), as in `ledgerly-reports`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewDetail {
    /// Primary groups only.
    Condensed,
    /// Down to ledgers.
    Detailed,
}

impl ViewDetail {
    /// The other level.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            ViewDetail::Condensed => ViewDetail::Detailed,
            ViewDetail::Detailed => ViewDetail::Condensed,
        }
    }

    /// Name shown in the header.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ViewDetail::Condensed => "Condensed",
            ViewDetail::Detailed => "Detailed",
        }
    }
}

/// The parameters the user can change on screen. The host keeps the rest (scenario, filters,
/// as-of date) behind the [`ReportToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewParams {
    /// Period of the report.
    pub period: Period,
    /// Condensed or detailed.
    pub detail: ViewDetail,
}

/// Where a row drills to; mirrors `ledgerly_reports::DrillKey`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ViewKey {
    /// A group.
    Group(GroupId),
    /// A ledger.
    Ledger(LedgerId),
    /// A voucher.
    Voucher(VoucherId),
    /// A stock item.
    Item(StockItemId),
    /// A bill.
    Bill {
        /// Ledger the bill belongs to.
        ledger: LedgerId,
        /// Bill name.
        name: String,
    },
}

/// Row kind; mirrors `ledgerly_reports::RowKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewRowKind {
    /// A group.
    Group,
    /// A ledger.
    Ledger,
    /// A voucher.
    Voucher,
    /// A stock item.
    Item,
    /// A bill.
    Bill,
    /// An explicit difference line.
    Difference,
    /// A subtotal.
    Subtotal,
    /// Text only.
    Text,
}

/// One cell; mirrors `ledgerly_reports::Cell`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewCell {
    /// An amount with its side.
    Amount(SignedAmount),
    /// A plain signed amount (negative is a loss or a credit balance, as the report defines it).
    Signed(Paise),
    /// A quantity.
    Qty(Qty),
    /// Text.
    Text(String),
    /// A date.
    Date(Date),
    /// Empty.
    Empty,
    /// A value that is not final.
    Provisional {
        /// The amount.
        amount: SignedAmount,
        /// Why it is provisional.
        reason: String,
    },
    /// The engine cannot compute this cell.
    Unsupported(Unsupported),
}

/// One row of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewRow {
    /// What Enter opens, if anything.
    pub key: Option<ViewKey>,
    /// Label.
    pub label: String,
    /// Indent level, 0 at the top.
    pub depth: u8,
    /// Row kind.
    pub kind: ViewRowKind,
    /// One cell per column.
    pub cells: Vec<ViewCell>,
}

/// A computed report, ready to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewReport {
    /// Handle for re-running this report.
    pub token: ReportToken,
    /// Title.
    pub title: String,
    /// The parameters it was run with.
    pub params: ViewParams,
    /// Column headings.
    pub columns: Vec<String>,
    /// Rows in display order.
    pub rows: Vec<ViewRow>,
    /// Total cells, one per column, or none when the report has no totals.
    pub totals: Vec<ViewCell>,
    /// Notes shown under the table.
    pub notes: Vec<String>,
}

/// Why a report could not be shown.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ViewError {
    /// A row does not have one cell per column.
    #[error("row {row} ({label}) has {found} cells but the report has {expected} columns")]
    RowShape {
        /// Position of the row, from 1.
        row: usize,
        /// Row label.
        label: String,
        /// Cells the row has.
        found: usize,
        /// Columns the report has.
        expected: usize,
    },
    /// The totals do not have one cell per column.
    #[error("the totals have {found} cells but the report has {expected} columns")]
    TotalsShape {
        /// Cells the totals have.
        found: usize,
        /// Columns the report has.
        expected: usize,
    },
    /// The host could not produce the report.
    #[error("{message}")]
    Source {
        /// What the host reported.
        message: String,
    },
}

/// What a drill opens.
pub enum DrillTarget {
    /// Another report; it is shown in a new report screen.
    Report(ViewReport),
    /// Any other screen, for example the voucher form for a voucher row.
    Screen(Box<dyn Screen>),
}

/// What the screen asks the host for. A request is answered with [`ReportViewScreen::answer_refresh`]
/// or [`ReportViewScreen::answer_drill`], or run on the API worker when the screen has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestKind {
    /// Re-run the report shown, with the period or detail the user chose.
    Refresh,
    /// Open what a row drills to.
    Drill(ViewKey),
}

/// One request from a report screen. `params` is what the API runs (a drilled report carries
/// the period and detail of its parent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRequest {
    /// Number of the request. Only the latest one's answer is shown.
    pub seq: u64,
    /// Refresh or drill.
    pub kind: RequestKind,
    /// The parameters to run.
    pub params: ledgerly_api::ReportParams,
}

/// The report a screen shows was run with these parameters. A refresh and a drill are both
/// built from them. The screen holds it, so the host needs to store nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSource {
    /// The parameters the shown report was run with.
    base: ledgerly_api::ReportParams,
}

impl ReportSource {
    /// A source for a report run with `base`.
    #[must_use]
    pub fn new(base: ledgerly_api::ReportParams) -> Self {
        Self { base }
    }

    /// The parameters the shown report was run with.
    #[must_use]
    pub fn base(&self) -> &ledgerly_api::ReportParams {
        &self.base
    }
}

impl ViewReport {
    /// Checks that every row, and the totals when present, has one cell per column.
    ///
    /// # Errors
    /// [`ViewError::RowShape`] or [`ViewError::TotalsShape`] for the first mismatch.
    pub fn check_shape(&self) -> Result<(), ViewError> {
        let expected = self.columns.len();
        for (i, row) in self.rows.iter().enumerate() {
            if row.cells.len() != expected {
                return Err(ViewError::RowShape {
                    row: i.saturating_add(1),
                    label: row.label.clone(),
                    found: row.cells.len(),
                    expected,
                });
            }
        }
        if !self.totals.is_empty() && self.totals.len() != expected {
            return Err(ViewError::TotalsShape {
                found: self.totals.len(),
                expected,
            });
        }
        Ok(())
    }
}
