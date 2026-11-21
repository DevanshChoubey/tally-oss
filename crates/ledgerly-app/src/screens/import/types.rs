//! Plain data of the import wizard: steps, requests, pull, map and reconciliation types.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ledgerly_core::{CancelToken, Outcome, Period, PeriodError, SignedAmount};
use thiserror::Error;

/// The port Tally listens on for its XML interface (spec 6.7 step 1).
pub const TALLY_PORT: u16 = 9000;

/// The most vouchers one pull request may ask for (spec 6.7 step 2). The pull itself (card B2)
/// splits the date range; the screen only states the limit to the user.
pub const MAX_VOUCHERS_PER_REQUEST: u32 = 5000;

// ---------------------------------------------------------------------------------------------
// Steps
// ---------------------------------------------------------------------------------------------

/// A step of the wizard, in the order of spec 6.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    /// Find Tally (or pick an exported file).
    Detect,
    /// Choose the company and the period.
    ChooseCompany,
    /// Pull masters and vouchers.
    Pull,
    /// Review how Tally items map to ours.
    Map,
    /// Commit and compare with Tally's own reports.
    Reconcile,
}

impl Step {
    /// Every step, in order.
    pub const ALL: [Step; 5] = [
        Step::Detect,
        Step::ChooseCompany,
        Step::Pull,
        Step::Map,
        Step::Reconcile,
    ];

    /// The name shown in the step list.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Step::Detect => "Detect",
            Step::ChooseCompany => "Choose company",
            Step::Pull => "Pull",
            Step::Map => "Map",
            Step::Reconcile => "Reconcile",
        }
    }
}

/// Why a wizard call was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImportError {
    /// The call does not fit the current step or state (for example a late reply from the host).
    #[error(
        "cannot {action} now: the import is at the \"{step}\" step in a state that does not allow it"
    )]
    WrongStep {
        /// What was attempted.
        action: &'static str,
        /// The step the wizard is at.
        step: &'static str,
    },
    /// No company was chosen.
    #[error("choose a company first")]
    NoCompany,
    /// The company number is not in the list.
    #[error("company number {index} is not in the list of {count} companies")]
    CompanyIndex {
        /// The number asked for.
        index: usize,
        /// How many companies the list has.
        count: usize,
    },
    /// No exported file was named.
    #[error("type the path of the exported XML file first")]
    NoExportPath,
    /// The chosen financial years do not make a period.
    #[error("the chosen years do not make a period: {0}")]
    Period(#[from] PeriodError),
    /// Some Tally items have no mapping; the import cannot continue.
    #[error("{count} items from the other app have no mapping; resolve them before importing")]
    UnmappedItems {
        /// How many items are unmapped.
        count: usize,
    },
    /// The reconciliation report covers another period than the one the wizard asked for (a late
    /// or wrong reply). It is refused: the figures of another year must never show as a match.
    #[error(
        "the reconciliation report is for another period than the one asked for, so it is refused"
    )]
    WrongPeriod {
        /// The period the wizard asked the host to compare.
        asked: Period,
        /// The period the report covers.
        got: Period,
    },
}

// ---------------------------------------------------------------------------------------------
// Requests the host runs
// ---------------------------------------------------------------------------------------------

/// Where the data comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSource {
    /// A running Tally on `localhost` at this port.
    Live {
        /// The port to call.
        port: u16,
    },
    /// An XML file the user exported from Tally (the offline path, spec 6.7 step 5).
    Export(PathBuf),
}

/// Work the wizard asks the host to do. The host runs it off the UI thread and answers through
/// the wizard's reply methods.
#[derive(Debug, Clone)]
pub enum ImportRequest {
    /// List the companies the source holds. Answer with [`super::ImportWizard::detect_result`].
    Detect {
        /// Where to look.
        source: ImportSource,
    },
    /// Pull the company's masters and vouchers for the period, capturing raw XML to disk first.
    /// Answer with [`super::ImportWizard::pull_progress`], then one of [`super::ImportWizard::pull_finished`],
    /// [`super::ImportWizard::pull_failed`] or [`super::ImportWizard::pull_cancelled`]. Stop promptly when the
    /// token is cancelled (rule D2).
    Pull {
        /// Where the data comes from.
        source: ImportSource,
        /// The company name as Tally lists it.
        company: String,
        /// The dates to pull vouchers for.
        period: Period,
        /// Set when the user cancels.
        cancel: CancelToken,
    },
    /// Stage the pulled data and map it. Answer with [`super::ImportWizard::map_ready`] or
    /// [`super::ImportWizard::map_failed`].
    Map {
        /// The company name.
        company: String,
    },
    /// Write the mapped data to the books in one transaction (rule D3). Answer with
    /// [`super::ImportWizard::commit_done`] or [`super::ImportWizard::commit_failed`].
    Commit {
        /// The company name.
        company: String,
    },
    /// Compare our reports with Tally's own for the period. Answer with
    /// [`super::ImportWizard::recon_ready`] or [`super::ImportWizard::recon_failed`].
    Reconcile {
        /// The company name.
        company: String,
        /// The period to compare.
        period: Period,
        /// Set when the user leaves the wizard.
        cancel: CancelToken,
    },
}

// ---------------------------------------------------------------------------------------------
// Detect
// ---------------------------------------------------------------------------------------------

/// What the detect step knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectState {
    /// Nothing was tried yet.
    NotChecked,
    /// A request is out.
    Checking,
    /// Companies were found; the wizard moved to the next step.
    Found,
    /// Nothing is listening.
    NotRunning {
        /// The port tried.
        port: u16,
    },
    /// Any other problem, with the reason to show.
    Failed(String),
}

/// How a detect request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectError {
    /// Tally is not running or not accepting connections.
    NotRunning {
        /// The port tried.
        port: u16,
    },
    /// Any other problem, with the reason to show.
    Other(String),
}

// ---------------------------------------------------------------------------------------------
// Pull
// ---------------------------------------------------------------------------------------------

/// A stage of the pull, in dependency order (spec 6.7 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PullStage {
    /// Account groups.
    Groups,
    /// Ledgers.
    Ledgers,
    /// Units of measure.
    Units,
    /// Stock groups and categories.
    StockGroupsAndCategories,
    /// Stock items.
    StockItems,
    /// Godowns.
    Godowns,
    /// Cost categories and centres.
    CostCategoriesAndCentres,
    /// Voucher types.
    VoucherTypes,
    /// Vouchers, in date batches.
    Vouchers,
}

impl PullStage {
    /// Every stage, in the order the pull runs them.
    pub const ALL: [PullStage; 9] = [
        PullStage::Groups,
        PullStage::Ledgers,
        PullStage::Units,
        PullStage::StockGroupsAndCategories,
        PullStage::StockItems,
        PullStage::Godowns,
        PullStage::CostCategoriesAndCentres,
        PullStage::VoucherTypes,
        PullStage::Vouchers,
    ];

    /// The name shown in the progress list.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PullStage::Groups => "Groups",
            PullStage::Ledgers => "Ledgers",
            PullStage::Units => "Units",
            PullStage::StockGroupsAndCategories => "Stock groups and categories",
            PullStage::StockItems => "Stock items",
            PullStage::Godowns => "Godowns",
            PullStage::CostCategoriesAndCentres => "Cost categories and centres",
            PullStage::VoucherTypes => "Voucher types",
            PullStage::Vouchers => "Vouchers",
        }
    }
}

/// Where the pull has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PullProgress {
    /// The stage being pulled.
    pub stage: PullStage,
    /// Voucher batches finished.
    pub batches_done: u32,
    /// Voucher batches in all, once known.
    pub batches_total: Option<u32>,
    /// Vouchers received so far.
    pub vouchers_pulled: u64,
}

impl PullProgress {
    /// The state before the first reply: the first stage, nothing done.
    #[must_use]
    pub const fn starting() -> Self {
        PullProgress {
            stage: PullStage::Groups,
            batches_done: 0,
            batches_total: None,
            vouchers_pulled: 0,
        }
    }

    /// Voucher batches done as a whole percentage, rounded down so 100 means every batch is done.
    /// `None` while the batch count is unknown or zero.
    #[must_use]
    pub fn batch_percent(&self) -> Option<u8> {
        let total = self.batches_total.filter(|t| *t > 0)?;
        let done = self.batches_done.min(total);
        let percent = u64::from(done).saturating_mul(100) / u64::from(total);
        Some(u8::try_from(percent).unwrap_or(100))
    }
}

/// What a finished pull produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullSummary {
    /// Vouchers received.
    pub vouchers: u64,
    /// Voucher batches fetched.
    pub batches: u32,
    /// Where the raw XML was saved.
    pub capture_dir: PathBuf,
}

/// The pull step's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullState {
    /// Not started.
    Idle,
    /// Running.
    Running(PullProgress),
    /// Cancel was asked for and the pull has not stopped yet.
    Cancelling(PullProgress),
    /// Finished; the data is captured and nothing is imported yet.
    Done(PullSummary),
    /// Stopped by the user. Nothing was imported; the batches already captured stay on disk.
    Cancelled {
        /// Raw batches kept on disk.
        batches_kept: u32,
    },
    /// Failed, with the reason to show.
    Failed(String),
}

// ---------------------------------------------------------------------------------------------
// Map
// ---------------------------------------------------------------------------------------------

/// The kind of a Tally item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MapKind {
    /// Account group.
    Group,
    /// Ledger.
    Ledger,
    /// Unit of measure.
    Unit,
    /// Stock group.
    StockGroup,
    /// Stock category.
    StockCategory,
    /// Stock item.
    StockItem,
    /// Godown.
    Godown,
    /// Cost category.
    CostCategory,
    /// Cost centre.
    CostCentre,
    /// Voucher type.
    VoucherType,
    /// Voucher.
    Voucher,
}

impl MapKind {
    /// The name shown in the mapping table.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            MapKind::Group => "Group",
            MapKind::Ledger => "Ledger",
            MapKind::Unit => "Unit",
            MapKind::StockGroup => "Stock group",
            MapKind::StockCategory => "Stock category",
            MapKind::StockItem => "Stock item",
            MapKind::Godown => "Godown",
            MapKind::CostCategory => "Cost category",
            MapKind::CostCentre => "Cost centre",
            MapKind::VoucherType => "Voucher type",
            MapKind::Voucher => "Voucher",
        }
    }
}

/// How one Tally item maps to ours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapStatus {
    /// Mapped without remark.
    Mapped,
    /// Mapped, with something the user should know (for example a field kept opaque).
    Warning(String),
    /// No mapping; the reason says why. Blocks the import.
    Unmapped(String),
}

/// One Tally item and its mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRow {
    /// What kind of item.
    pub kind: MapKind,
    /// Its name in Tally.
    pub tally_name: String,
    /// Its Tally GUID, when it has one.
    pub guid: Option<String>,
    /// How it maps.
    pub status: MapStatus,
}

/// The result of staging and mapping.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MapReport {
    /// Every item, in the order the host lists them.
    pub rows: Vec<MapRow>,
    /// Remarks that belong to no single row (for example a dropped field, rule A9).
    pub warnings: Vec<String>,
}

impl MapReport {
    /// Items with no mapping.
    #[must_use]
    pub fn unmapped_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r.status, MapStatus::Unmapped(_)))
            .count()
    }

    /// Items mapped with a remark, plus the report-level remarks.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        let on_rows = self
            .rows
            .iter()
            .filter(|r| matches!(r.status, MapStatus::Warning(_)))
            .count();
        on_rows.saturating_add(self.warnings.len())
    }

    /// How many items of each kind, in kind order.
    #[must_use]
    pub fn kind_counts(&self) -> BTreeMap<MapKind, usize> {
        let mut counts = BTreeMap::new();
        for row in &self.rows {
            let slot: &mut usize = counts.entry(row.kind).or_insert(0);
            *slot = slot.saturating_add(1);
        }
        counts
    }
}

/// The map step's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapState {
    /// Not started.
    NotStarted,
    /// The host is staging and mapping.
    Working,
    /// Ready for review.
    Ready(MapReport),
    /// Failed, with the reason to show.
    Failed(String),
}

// ---------------------------------------------------------------------------------------------
// Reconciliation
// ---------------------------------------------------------------------------------------------

/// A part of the reconciliation (spec 6.7 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReconSection {
    /// Trial balance.
    TrialBalance,
    /// Profit and loss.
    ProfitAndLoss,
    /// Balance sheet.
    BalanceSheet,
    /// Stock summary.
    StockSummary,
    /// Party outstandings.
    PartyOutstandings,
    /// GST summaries.
    GstSummary,
}

impl ReconSection {
    /// Every section a complete report has, in display order.
    pub const ALL: [ReconSection; 6] = [
        ReconSection::TrialBalance,
        ReconSection::ProfitAndLoss,
        ReconSection::BalanceSheet,
        ReconSection::StockSummary,
        ReconSection::PartyOutstandings,
        ReconSection::GstSummary,
    ];

    /// The name shown in the report.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ReconSection::TrialBalance => "Trial balance",
            ReconSection::ProfitAndLoss => "Profit and loss",
            ReconSection::BalanceSheet => "Balance sheet",
            ReconSection::StockSummary => "Stock summary",
            ReconSection::PartyOutstandings => "Party outstandings",
            ReconSection::GstSummary => "GST summary",
        }
    }
}

/// How one figure compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    /// Both sides give the same figure.
    Match,
    /// Both sides give a figure and they differ.
    Differs,
    /// Tally has a figure and we have none.
    OnlyInTally,
    /// We have a figure and Tally has none.
    OnlyInOurs,
    /// Neither side has a figure, so nothing was compared.
    NotReported,
}

impl RowStatus {
    /// The words shown in the report; never colour alone (spec 7).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            RowStatus::Match => "Matches",
            RowStatus::Differs => "DIFFERS",
            RowStatus::OnlyInTally => "DIFFERS: missing here",
            RowStatus::OnlyInOurs => "DIFFERS: missing in the other app",
            RowStatus::NotReported => "NOT COMPARED: no figure on either side",
        }
    }
}

/// One figure, ours against Tally's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconRow {
    /// What the figure is (a ledger, a stock item, a party, a GST head).
    pub label: String,
    /// Our figure from the reports crate.
    pub ours: Option<SignedAmount>,
    /// Tally's own figure from its exported report.
    pub theirs: Option<SignedAmount>,
}

impl ReconRow {
    /// Compares the two figures. Amounts are equal when they are the same paise on the same
    /// side; zero on either side counts as the same figure.
    #[must_use]
    pub fn status(&self) -> RowStatus {
        match (self.ours, self.theirs) {
            (Some(a), Some(b)) => {
                if same_figure(a, b) {
                    RowStatus::Match
                } else {
                    RowStatus::Differs
                }
            }
            (None, Some(_)) => RowStatus::OnlyInTally,
            (Some(_), None) => RowStatus::OnlyInOurs,
            (None, None) => RowStatus::NotReported,
        }
    }
}

fn same_figure(a: SignedAmount, b: SignedAmount) -> bool {
    (a.amount.is_zero() && b.amount.is_zero()) || a == b
}

/// One section of the report. An `Unsupported` outcome is shown as such.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconSectionReport {
    /// Which section.
    pub section: ReconSection,
    /// The rows, or why the section could not be compared.
    pub outcome: Outcome<Vec<ReconRow>>,
}

/// The reconciliation report handed in by the host (card B4's output, shaped for display).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconReport {
    /// The period compared.
    pub period: Period,
    /// The sections the host compared.
    pub sections: Vec<ReconSectionReport>,
}

/// What a reconciliation report adds up to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconSummary {
    /// Rows compared in all.
    pub rows: usize,
    /// Rows that are not a match.
    pub differing_rows: usize,
    /// Sections that were not compared: unsupported, missing from the report, or without rows.
    pub not_compared: Vec<ReconSection>,
}

impl ReconSummary {
    /// `true` only when every section was compared and every row matches. An empty report is not
    /// a match: nothing was compared.
    #[must_use]
    pub fn all_match(&self) -> bool {
        self.differing_rows == 0 && self.not_compared.is_empty()
    }
}

impl ReconReport {
    /// Counts rows and finds the sections that were not compared.
    #[must_use]
    pub fn summary(&self) -> ReconSummary {
        let mut rows = 0_usize;
        let mut differing_rows = 0_usize;
        for section in &self.sections {
            if let Outcome::Value(list) = &section.outcome {
                for row in list {
                    rows = rows.saturating_add(1);
                    if row.status() != RowStatus::Match {
                        differing_rows = differing_rows.saturating_add(1);
                    }
                }
            }
        }
        let not_compared = ReconSection::ALL
            .iter()
            .copied()
            .filter(|wanted| !self.section_was_compared(*wanted))
            .collect();
        ReconSummary {
            rows,
            differing_rows,
            not_compared,
        }
    }

    /// A section counts as compared when the report holds it with at least one row.
    fn section_was_compared(&self, wanted: ReconSection) -> bool {
        self.sections.iter().any(|s| match &s.outcome {
            Outcome::Value(list) => s.section == wanted && !list.is_empty(),
            Outcome::Unsupported(_) => false,
        })
    }
}

/// The reconcile step's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconState {
    /// Not reached.
    NotStarted,
    /// The host is writing the mapped data to the books.
    Committing,
    /// The write failed and nothing was written (rule D3); the user can go back.
    CommitFailed(String),
    /// The data is in the books; the host is comparing reports.
    Reconciling,
    /// The comparison failed. The data is in the books; the user can run it again.
    Failed(String),
    /// The report.
    Ready(ReconReport),
}
