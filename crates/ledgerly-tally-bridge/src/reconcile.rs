//! Reconciliation report: ours vs Tally's. **Card B4** implements this module.
//!
//! Frozen interface (plan card B4):
//!
//! ```text
//! pub struct ReconReport { .. }
//! pub fn reconcile(ours: &Books, theirs: &TallyReports, period: Period) -> Result<ReconReport, BridgeError>;
//! ```
//!
//! # What is compared
//!
//! Only the **trial balance** is compared. Our side is the engine's own trial balance
//! ([`ledgerly_reports::trial_balance::trial_balance`], detailed, as of `period.to()`); the
//! Tally side is `theirs.tb` as parsed by card X5. Every Tally row is resolved to one of our
//! masters (by Tally GUID first, then by exact name) and the debit and credit columns are
//! compared to the paisa. **Every row is listed with both values, matching or not**; a
//! difference is never summarised away or hidden (spec 6.7).
//!
//! The other four exports (`bs`, `pl`, `stock`, `outstanding`) are **not compared**. If `theirs`
//! carries rows for one of them its section is [`SectionStatus::Unsupported`] and the report is
//! not green. The reasons: X5 keeps those amounts as written (`TReportAmount::Signed`) because
//! the side a sign means is not established, and a comparison would have to assume it; the
//! balance sheet, profit and loss and stock summary also need stock valuations that `reconcile`
//! is not given and this crate cannot compute (`ledgerly-stock` is not a dependency); and bills
//! outstanding (card RP7) is not a dependency of this card. A section with no rows in `theirs`
//! is [`SectionStatus::NotSupplied`] and does not block green.
//!
//! # Never a wrong number (rule A1)
//!
//! * A Tally row that names no master of ours, or one name that matches several masters, or a
//!   master listed twice, is listed with its own status and is not green.
//! * A Tally trial balance amount without a column side (`TReportAmount::Signed` or
//!   `TReportAmount::Nil`) is [`LineStatus::Unreadable`]; its side is never guessed.
//! * A ledger of ours with a non-zero closing balance that neither a Tally row nor a Tally
//!   group row above it accounts for is listed as [`LineStatus::OnlyInOurs`].
//! * If our own trial balance is unsupported or fails, the section says so and lists no lines.
//! * A Tally export without a totals row is not an error: no totals line is invented.
//! * Totals are recognised by label only (`pending` an observed capture of Tally's layout). A row
//!   with that label that names one of our masters is that master: it is listed with the
//!   master's comparison and marked [`LineStatus::Ambiguous`], so it is never green and is never
//!   compared as the grand total.
//!
//! Provenance of the Tally layout: recalled, not observed (see `ledgerly_tally_xml::reports`).

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{DrCr, GroupId, LedgerId, Outcome, Paise, Period, Unsupported};
use ledgerly_model::{Books, Scenario};
use ledgerly_reports::{
    Cell, ColumnSpec, Detail, DrillKey, Report, ReportFilter, ReportKind, ReportParams, RowKind,
    trial_balance::trial_balance,
};
use ledgerly_tally_xml::reports::{TReportAmount, TReportRow, TallyReports};

use crate::discover::BridgeError;

/// Deepest group nesting followed when walking from a ledger to its ancestors.
const MAX_GROUP_DEPTH: usize = 256;

/// A pair of debit and credit magnitudes (never negative).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconAmount {
    /// Debit column.
    pub debit: Paise,
    /// Credit column.
    pub credit: Paise,
}

impl ReconAmount {
    /// Nothing on either side.
    pub const ZERO: ReconAmount = ReconAmount {
        debit: Paise::ZERO,
        credit: Paise::ZERO,
    };

    fn is_zero(self) -> bool {
        self.debit.is_zero() && self.credit.is_zero()
    }
}

/// What a listed line stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// One of our ledgers.
    Ledger,
    /// One of our groups.
    Group,
    /// The report's grand total.
    Total,
    /// The explicit line for unequal opening balances.
    OpeningDifference,
    /// A Tally row that resolves to none of our masters.
    Unmatched,
}

/// How a line came out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineStatus {
    /// Both sides agree to the paisa.
    Match,
    /// The amounts differ. Each field is ours minus Tally's for that column.
    Differs {
        /// Our debit minus Tally's debit.
        debit_difference: Paise,
        /// Our credit minus Tally's credit.
        credit_difference: Paise,
    },
    /// Tally has the row; none of our masters matches it.
    NotInOurBooks,
    /// We have a non-zero closing balance that nothing in Tally's export accounts for.
    OnlyInOurs,
    /// The row cannot be matched to exactly one master (or is listed twice by Tally).
    Ambiguous {
        /// Why.
        reason: String,
    },
    /// An amount on one side cannot be read as a debit and a credit.
    Unreadable {
        /// Why.
        reason: String,
    },
}

/// One listed line of the trial balance comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconLine {
    /// Tally's label, or our name for a line only we have.
    pub label: String,
    /// What the line stands for.
    pub kind: LineKind,
    /// Our amounts, when the line exists on our side.
    pub ours: Option<ReconAmount>,
    /// Tally's amounts, when its row carried debit and credit columns.
    pub theirs: Option<ReconAmount>,
    /// The outcome.
    pub status: LineStatus,
}

/// What happened to one section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionStatus {
    /// Compared; the lines say how it came out.
    Compared,
    /// Tally supplied no rows for it, so there is nothing to compare.
    NotSupplied,
    /// Not compared, with what and why (rule A1).
    Unsupported(Unsupported),
    /// Our side could not be produced; nothing was compared.
    Failed {
        /// Why, from our own report engine.
        reason: String,
    },
}

/// One section of the reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionReport {
    /// Whether it was compared.
    pub status: SectionStatus,
    /// Every compared line (empty unless [`SectionStatus::Compared`]).
    pub lines: Vec<ReconLine>,
}

/// The reconciliation of our books against Tally's exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconReport {
    /// The period reconciled; balances are as of its last day.
    pub period: Period,
    /// Trial balance, the only compared section.
    pub trial_balance: SectionReport,
    /// Balance sheet (never compared yet).
    pub balance_sheet: SectionReport,
    /// Profit and loss (never compared yet).
    pub profit_loss: SectionReport,
    /// Stock summary (never compared yet).
    pub stock: SectionReport,
    /// Bills outstanding (never compared yet).
    pub outstanding: SectionReport,
}

impl ReconReport {
    fn sections(&self) -> [&SectionReport; 5] {
        [
            &self.trial_balance,
            &self.balance_sheet,
            &self.profit_loss,
            &self.stock,
            &self.outstanding,
        ]
    }

    /// Every listed line that is not a match, across sections, in report order.
    pub fn mismatches(&self) -> Vec<&ReconLine> {
        self.sections()
            .into_iter()
            .flat_map(|s| s.lines.iter())
            .filter(|l| l.status != LineStatus::Match)
            .collect()
    }

    /// `true` only when the trial balance was compared, every line matches, and no other
    /// section is unsupported or failed. A section Tally did not supply does not block green.
    pub fn is_green(&self) -> bool {
        let tb_compared = self.trial_balance.status == SectionStatus::Compared;
        let sections_ok = self.sections().into_iter().all(|s| {
            matches!(
                s.status,
                SectionStatus::Compared | SectionStatus::NotSupplied
            )
        });
        tb_compared && sections_ok && self.mismatches().is_empty()
    }
}

/// Reconciles our trial balance with Tally's exports for `period` (spec 6.7).
///
/// Balances are compared as of `period.to()`. See the module documentation for exactly what is
/// compared and what is reported as unsupported.
///
/// # Errors
/// None today: a failure on our side is carried in the report as
/// [`SectionStatus::Failed`] so that the lines already compared are not lost. The `Result` is
/// the frozen interface; a later card may add cancellation.
pub fn reconcile(
    ours: &Books,
    theirs: &TallyReports,
    period: Period,
) -> Result<ReconReport, BridgeError> {
    let _span = tracing::info_span!("reconcile").entered();
    Ok(ReconReport {
        period,
        trial_balance: trial_balance_section(ours, &theirs.tb, period),
        balance_sheet: not_compared(
            &theirs.bs,
            "reconciliation: balance sheet",
            "The other program's balance sheet amounts carry a sign whose side is not established, and our balance sheet needs stock valuations this reconciliation is not given",
        ),
        profit_loss: not_compared(
            &theirs.pl,
            "reconciliation: profit and loss",
            "The other program's profit and loss amounts carry a sign whose side is not established, and our profit and loss needs stock valuations this reconciliation is not given",
        ),
        stock: not_compared(
            &theirs.stock,
            "reconciliation: stock summary",
            "The other program's stock values carry a sign whose side is not established, and our stock values need valuations this reconciliation is not given",
        ),
        outstanding: not_compared(
            &theirs.outstanding,
            "reconciliation: bills outstanding",
            "The other program's bill amounts carry a sign whose side is not established, and the bills outstanding report is not part of this comparison yet",
        ),
    })
}

fn not_compared(rows: &[TReportRow], what: &str, reason: &str) -> SectionReport {
    let status = if rows.is_empty() {
        SectionStatus::NotSupplied
    } else {
        SectionStatus::Unsupported(Unsupported {
            what: what.to_owned(),
            reason: reason.to_owned(),
        })
    };
    SectionReport {
        status,
        lines: Vec::new(),
    }
}

fn failed(reason: String) -> SectionReport {
    SectionReport {
        status: SectionStatus::Failed { reason },
        lines: Vec::new(),
    }
}

fn trial_balance_section(books: &Books, theirs: &[TReportRow], period: Period) -> SectionReport {
    if theirs.is_empty() {
        return SectionReport {
            status: SectionStatus::NotSupplied,
            lines: Vec::new(),
        };
    }
    let params = ReportParams {
        kind: ReportKind::TrialBalance,
        period,
        as_of: period.to(),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    let report = match trial_balance(books, &params) {
        Ok(Outcome::Value(report)) => report,
        Ok(Outcome::Unsupported(u)) => {
            return SectionReport {
                status: SectionStatus::Unsupported(u),
                lines: Vec::new(),
            };
        }
        Err(e) => return failed(format!("our trial balance could not be produced: {e}")),
    };
    match compare_trial_balance(books, &report, theirs) {
        Ok(lines) => SectionReport {
            status: SectionStatus::Compared,
            lines,
        },
        Err(reason) => failed(reason),
    }
}

// ---------------------------------------------------------------------------------------------
// Our side, read back from the report
// ---------------------------------------------------------------------------------------------

/// Our trial balance as amounts keyed by master.
struct OurTb {
    by_ledger: BTreeMap<LedgerId, ReconAmount>,
    by_group: BTreeMap<GroupId, ReconAmount>,
    /// Label and amounts of the explicit opening-difference row, when there is one.
    difference: Option<(String, ReconAmount)>,
    totals: ReconAmount,
}

fn side(cell: &Cell, expected: DrCr, column: &str) -> Result<Paise, String> {
    match cell {
        Cell::Empty => Ok(Paise::ZERO),
        Cell::Amount(a) if a.side == expected && !a.amount.is_negative() => Ok(a.amount),
        Cell::Amount(_) => Err(format!(
            "our trial balance has an amount on the wrong side in the {column} column"
        )),
        Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Provisional { .. }
        | Cell::Unsupported(_) => Err(format!(
            "our trial balance has an unexpected cell in the {column} column"
        )),
    }
}

fn amount_of(cells: &[Cell]) -> Result<ReconAmount, String> {
    let [dr, cr] = cells else {
        return Err("our trial balance row does not have a debit and a credit cell".to_owned());
    };
    Ok(ReconAmount {
        debit: side(dr, DrCr::Dr, "debit")?,
        credit: side(cr, DrCr::Cr, "credit")?,
    })
}

fn read_ours(report: &Report) -> Result<OurTb, String> {
    let mut ours = OurTb {
        by_ledger: BTreeMap::new(),
        by_group: BTreeMap::new(),
        difference: None,
        totals: amount_of(&report.totals)?,
    };
    for row in &report.rows {
        let amount = amount_of(&row.cells)?;
        match &row.key {
            Some(DrillKey::Ledger(id)) => {
                ours.by_ledger.insert(*id, amount);
            }
            Some(DrillKey::Group(id)) => {
                ours.by_group.insert(*id, amount);
            }
            Some(DrillKey::Voucher(_) | DrillKey::Item(_) | DrillKey::Bill { .. }) => {
                return Err(format!(
                    "our trial balance row {} drills to something other than a group or ledger",
                    row.label
                ));
            }
            None if row.kind == RowKind::Difference => {
                ours.difference = Some((row.label.clone(), amount));
            }
            None => {
                return Err(format!(
                    "our trial balance row {} has no group or ledger behind it",
                    row.label
                ));
            }
        }
    }
    Ok(ours)
}

// ---------------------------------------------------------------------------------------------
// Resolving Tally rows to masters
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Ledger(LedgerId),
    Group(GroupId),
    Difference,
}

enum Resolution {
    Found(Target),
    Ambiguous(String),
    Unknown,
}

/// Lookup tables over our masters.
struct Index<'a> {
    by_guid: BTreeMap<&'a str, Vec<Target>>,
    by_name: BTreeMap<&'a str, Vec<Target>>,
}

impl<'a> Index<'a> {
    fn new(books: &'a Books) -> Self {
        let mut by_guid: BTreeMap<&str, Vec<Target>> = BTreeMap::new();
        let mut by_name: BTreeMap<&str, Vec<Target>> = BTreeMap::new();
        for ledger in books.masters.ledgers.values() {
            let target = Target::Ledger(ledger.id);
            by_name
                .entry(ledger.name.as_str())
                .or_default()
                .push(target);
            if let Some(t) = &ledger.tally {
                by_guid.entry(t.guid.as_str()).or_default().push(target);
            }
        }
        for group in books.masters.groups.values() {
            let target = Target::Group(group.id);
            by_name.entry(group.name.as_str()).or_default().push(target);
            if let Some(t) = &group.tally {
                by_guid.entry(t.guid.as_str()).or_default().push(target);
            }
        }
        Index { by_guid, by_name }
    }

    /// Tally GUID first, then exact name; the opening-difference label last.
    fn resolve(&self, row: &TReportRow, difference_label: Option<&str>) -> Resolution {
        if let Some(guid) = row.guid.as_deref()
            && let Some(hits) = self.by_guid.get(guid)
        {
            return pick(hits, &format!("GUID {guid}"));
        }
        match self.by_name.get(row.label.as_str()) {
            Some(hits) => pick(hits, &format!("name {}", row.label)),
            None if difference_label == Some(row.label.as_str()) => {
                Resolution::Found(Target::Difference)
            }
            None => Resolution::Unknown,
        }
    }
}

fn pick(hits: &[Target], what: &str) -> Resolution {
    match hits {
        [] => Resolution::Unknown,
        [one] => Resolution::Found(*one),
        [_, _, ..] => Resolution::Ambiguous(format!(
            "{what} matches {} of our ledgers and groups; rename one so the match is unique",
            hits.len()
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------------------------

fn their_amount(row: &TReportRow) -> Result<ReconAmount, String> {
    match row.amount {
        TReportAmount::Columns { debit, credit } => Ok(ReconAmount { debit, credit }),
        TReportAmount::Signed(_) => Err(format!(
            "The other program's row {} has one signed amount with no debit or credit column; its side is not established",
            row.label
        )),
        TReportAmount::Nil => Err(format!(
            "The other program's row {} has no amount",
            row.label
        )),
    }
}

fn compare(ours: ReconAmount, theirs: ReconAmount) -> LineStatus {
    if ours == theirs {
        return LineStatus::Match;
    }
    match (
        ours.debit.checked_sub(theirs.debit),
        ours.credit.checked_sub(theirs.credit),
    ) {
        (Ok(debit_difference), Ok(credit_difference)) => LineStatus::Differs {
            debit_difference,
            credit_difference,
        },
        (Err(e), _) | (_, Err(e)) => LineStatus::Unreadable {
            reason: format!("the difference cannot be computed: {e}"),
        },
    }
}

/// One Tally detail row with the master it resolves to, or why it does not.
struct Resolved<'a> {
    row: &'a TReportRow,
    resolution: Resolution,
}

fn line(
    row: &TReportRow,
    kind: LineKind,
    ours: Option<ReconAmount>,
    theirs: Option<ReconAmount>,
    status: LineStatus,
) -> ReconLine {
    ReconLine {
        label: row.label.clone(),
        kind,
        ours,
        theirs,
        status,
    }
}

/// A line for a Tally row whose own amount is read here; unreadable amounts say so.
fn their_line(row: &TReportRow, kind: LineKind, ours: ReconAmount) -> ReconLine {
    match their_amount(row) {
        Ok(t) => line(row, kind, Some(ours), Some(t), compare(ours, t)),
        Err(reason) => line(
            row,
            kind,
            Some(ours),
            None,
            LineStatus::Unreadable { reason },
        ),
    }
}

fn compare_trial_balance(
    books: &Books,
    report: &Report,
    theirs: &[TReportRow],
) -> Result<Vec<ReconLine>, String> {
    let ours = read_ours(report)?;
    let index = Index::new(books);
    let difference_label = ours.difference.as_ref().map(|(label, _)| label.as_str());

    // Every row is resolved, totals included: a row that names one of our masters accounts for it
    // whatever its label says.
    let resolved: Vec<Resolved<'_>> = theirs
        .iter()
        .map(|row| Resolved {
            row,
            resolution: index.resolve(row, difference_label),
        })
        .collect();
    let mut uses: BTreeMap<Target, usize> = BTreeMap::new();
    for r in &resolved {
        if let Resolution::Found(t) = &r.resolution {
            let n = uses.entry(*t).or_insert(0);
            *n = n.saturating_add(1);
        }
    }

    let mut lines = Vec::new();
    for r in &resolved {
        match (r.row.is_total, &r.resolution) {
            // Totals are recognised by label only (pending an observed capture). A row with that
            // label that names one of our masters is that master, not the grand total: it is
            // listed with the master's comparison, marked ambiguous (never green).
            (true, Resolution::Found(_)) => {
                let mut listed = detail_line(r, &ours, &uses);
                listed.status = LineStatus::Ambiguous {
                    reason: format!(
                        "The other program's row '{}' is recognised as a total by its label only, and it \
                         also names one of our masters; totals detection is pending an observed \
                         capture, so it is not green",
                        r.row.label
                    ),
                };
                lines.push(listed);
            }
            (true, Resolution::Ambiguous(_) | Resolution::Unknown) => {
                lines.push(their_line(r.row, LineKind::Total, ours.totals));
            }
            (false, _) => lines.push(detail_line(r, &ours, &uses)),
        }
    }
    lines.extend(only_in_ours(books, &ours, &resolved));
    Ok(lines)
}

fn detail_line(r: &Resolved<'_>, ours: &OurTb, uses: &BTreeMap<Target, usize>) -> ReconLine {
    let row = r.row;
    match &r.resolution {
        Resolution::Unknown => line(
            row,
            LineKind::Unmatched,
            None,
            their_amount(row).ok(),
            LineStatus::NotInOurBooks,
        ),
        Resolution::Ambiguous(reason) => line(
            row,
            LineKind::Unmatched,
            None,
            their_amount(row).ok(),
            LineStatus::Ambiguous {
                reason: reason.clone(),
            },
        ),
        Resolution::Found(target) => {
            let (kind, our_amount) = match target {
                Target::Ledger(id) => (
                    LineKind::Ledger,
                    ours.by_ledger.get(id).copied().unwrap_or(ReconAmount::ZERO),
                ),
                Target::Group(id) => (
                    LineKind::Group,
                    ours.by_group.get(id).copied().unwrap_or(ReconAmount::ZERO),
                ),
                Target::Difference => (
                    LineKind::OpeningDifference,
                    ours.difference.as_ref().map_or(ReconAmount::ZERO, |d| d.1),
                ),
            };
            if uses.get(target).copied().unwrap_or(0) > 1 {
                return line(
                    row,
                    kind,
                    Some(our_amount),
                    their_amount(row).ok(),
                    LineStatus::Ambiguous {
                        reason: format!(
                            "The other program lists {} more than once, so no row can be matched to it",
                            row.label
                        ),
                    },
                );
            }
            their_line(row, kind, our_amount)
        }
    }
}

/// Lines for our non-zero balances that nothing in Tally's export accounts for.
fn only_in_ours(books: &Books, ours: &OurTb, resolved: &[Resolved<'_>]) -> Vec<ReconLine> {
    let mut ledgers: BTreeSet<LedgerId> = BTreeSet::new();
    let mut groups: BTreeSet<GroupId> = BTreeSet::new();
    let mut difference_matched = false;
    for r in resolved {
        match &r.resolution {
            Resolution::Found(Target::Ledger(id)) => {
                ledgers.insert(*id);
            }
            Resolution::Found(Target::Group(id)) => {
                groups.insert(*id);
            }
            Resolution::Found(Target::Difference) => difference_matched = true,
            Resolution::Ambiguous(_) | Resolution::Unknown => {}
        }
    }
    let mut out = Vec::new();
    for (id, amount) in &ours.by_ledger {
        if amount.is_zero() || ledgers.contains(id) {
            continue;
        }
        let Some(ledger) = books.masters.ledgers.get(id) else {
            continue;
        };
        if under_matched_group(books, ledger.group, &groups) {
            continue;
        }
        out.push(ReconLine {
            label: ledger.name.clone(),
            kind: LineKind::Ledger,
            ours: Some(*amount),
            theirs: None,
            status: LineStatus::OnlyInOurs,
        });
    }
    if let Some((label, amount)) = &ours.difference
        && !difference_matched
    {
        out.push(ReconLine {
            label: label.clone(),
            kind: LineKind::OpeningDifference,
            ours: Some(*amount),
            theirs: None,
            status: LineStatus::OnlyInOurs,
        });
    }
    out
}

/// `true` when `group` or one of its ancestors is among the matched groups.
fn under_matched_group(books: &Books, group: GroupId, matched: &BTreeSet<GroupId>) -> bool {
    let mut current = Some(group);
    for _ in 0..MAX_GROUP_DEPTH {
        let Some(id) = current else {
            return false;
        };
        if matched.contains(&id) {
            return true;
        }
        current = books.masters.groups.get(&id).and_then(|g| g.parent);
    }
    false
}
