//! Auditor reports (headless). **Card RP10** implements this module (spec 6.6).
//!
//! Frozen interface (plan card RP10):
//!
//! ```text
//! pub fn audit_report(records: &[AuditRecord], books: &Books, kind: &AuditReportKind, params: &ReportParams) -> Result<Outcome<Report>, ReportError>;
//! pub fn integrity_certificate(verify: &VerifyReport) -> Report;
//! ```
//!
//! `integrity_certificate` returns `Result<Report, ReportError>` instead of the frozen `Report`:
//! the whole-calendar period it needs can only be built through the fallible `Period::new`, and a
//! library crate may not `expect` (rule B2). Interface request: change the frozen signature, or
//! give `Period` an infallible whole-calendar constructor.
//!
//! # Input
//!
//! `records` is a window of the audit log, as read with `Store::events`. Order does not matter
//! (the reports sort by sequence number); a sequence number that appears twice is an error. The
//! reports never read the books to decide what happened: every fact comes from the events.
//! `books` is used only to print the current name of a voucher type next to a voucher.
//!
//! # Common rules
//!
//! - `params.kind` must be `ReportKind::Audit(kind)` with the same `kind`, else
//!   [`ReportError::Invalid`].
//! - `params.period` selects records by the calendar day of `meta.at` in the offset it was
//!   recorded with (the user's own day, as the checkpoints of card S3 use).
//! - Only `ColumnSpec::Single` is supported; a period-column layout has no meaning for an event
//!   log and returns `Unsupported`. `scenario`, `detail` and `exceptions_only` do not apply to an
//!   event log and are not read.
//! - Rows are ordered by sequence number (object history: by object, then sequence number).
//!   Every record row has seven base columns (sequence, date, time, user, action, object, reason)
//!   followed by report-specific columns.
//!
//! # The reports
//!
//! - **Object history** lists every event of the selected objects with a child row per changed
//!   field (field path, value before, value after). Unchanged fields are never listed. The
//!   selection is `filter.ledgers`, `filter.groups` and `filter.items` (masters); with all three
//!   empty every object is listed. A voucher cannot be selected by id because [`ReportFilter`]
//!   has no voucher field (interface request); `filter.voucher_types` returns `Unsupported`.
//! - **Edit log** lists every event of the period. `filter.voucher_types` keeps only voucher
//!   events of those types. The other filters return `Unsupported`. The notes count events per
//!   user.
//! - **Deleted and altered vouchers** lists voucher alterations, cancellations and deletions, with
//!   the changed fields of an alteration.
//! - **Back-dated entries** lists each posted voucher whose date is strictly earlier than the
//!   latest date among all vouchers posted by earlier events (any type, any period). The first
//!   posted voucher is never back-dated. Alterations that move a date are not examined.
//! - **Edits after lock** lists a voucher event when a date it touches (the new date, and for an
//!   alteration the old date too) is on or before the lock date in force when the event was
//!   recorded. The lock date in force is the `until` of the latest earlier
//!   `Event::PeriodLocked` (the lock may be moved either way, as the lock engine allows). The
//!   comparison is `date <= lock`, the same as the engine's lock check.
//! - **Integrity certificate** needs the chain verification result: [`integrity_certificate`]
//!   renders a `VerifyReport`; asking [`audit_report`] for it returns `Unsupported`.
//!
//! The back-dated and lock reports are stateful: they follow the posted dates and lock dates from
//! the first record supplied onward. A window that omits earlier `VoucherPosted` or `PeriodLocked`
//! events starts from nothing and can list too little. When the first record supplied is not event
//! 1, both reports say so in their notes; pass the whole log (and use `params.period` to narrow
//! the rows) for a complete answer.
//!
//! Voucher events are the only ones that carry a voucher date, so the back-dated and lock reports
//! look at no other event. Filters other than `voucher_types` return `Unsupported` for the edit
//! log, the back-dated report, the lock report and the deleted/altered report.

use std::collections::{BTreeMap, BTreeSet};

use ledgerly_core::{
    BankStatementId, BatchId, CostCategoryId, CostCentreId, Date, EventSeq, GodownId, GroupId,
    LedgerId, Outcome, Period, StockGroupId, StockItemId, UnitId, Unsupported, VoucherId,
    VoucherTypeId,
};
use ledgerly_model::{Books, MasterSnapshot, Voucher};
use ledgerly_store::{Actor, AuditRecord, Event, StoredImsAction};
use time::OffsetDateTime;

mod certificate;
mod diff;

pub use certificate::integrity_certificate;

use diff::{Change, changes_of};

use crate::{
    AuditReportKind, Cell, ColumnHeader, ColumnSpec, DrillKey, Report, ReportError, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};

/// Labels of the columns every event report starts with.
const BASE_COLUMNS: [&str; 7] = ["Seq", "Date", "Time", "User", "Action", "Object", "Reason"];

/// Labels of the field-diff columns of the history, log and deleted/altered reports.
const DIFF_COLUMNS: [&str; 3] = ["Field", "Before", "After"];

/// What an event is about, ordered so that object history groups by object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ObjectKey {
    Voucher(VoucherId),
    Group(GroupId),
    Ledger(LedgerId),
    VoucherType(VoucherTypeId),
    Unit(UnitId),
    StockGroup(StockGroupId),
    StockItem(StockItemId),
    Godown(GodownId),
    Batch(BatchId),
    CostCategory(CostCategoryId),
    CostCentre(CostCentreId),
    Setting(String),
    PeriodLock,
    Statement(BankStatementId),
    AiProposal(EventSeq),
    /// A GST return period of one registration (a 2B import, an IMS action, a filing).
    GstReturn(String),
}

/// One record ready to become rows.
struct Item<'a> {
    rec: &'a AuditRecord,
    key: ObjectKey,
    label: String,
    action: String,
    extra: Vec<Cell>,
    changes: Vec<Change>,
}

/// Builds an auditor report.
///
/// See the module documentation for what each [`AuditReportKind`] lists and which filters it
/// understands.
///
/// # Errors
/// [`ReportError::Invalid`] when `params.kind` is not `Audit(kind)`, when two records share a
/// sequence number, or when an event snapshot cannot be read as JSON for the field diff.
///
/// Unsupported requests (integrity certificate, period columns, filters that do not apply)
/// return `Outcome::Unsupported`, not an error.
pub fn audit_report(
    records: &[AuditRecord],
    books: &Books,
    kind: &AuditReportKind,
    params: &ReportParams,
) -> Result<Outcome<Report>, ReportError> {
    if params.kind != ReportKind::Audit(kind.clone()) {
        return Err(ReportError::Invalid {
            field: "params.kind".to_owned(),
            reason: "the parameters were built for a different report than the one requested"
                .to_owned(),
        });
    }
    if params.columns != ColumnSpec::Single {
        return Ok(unsupported(
            "audit report with period columns",
            "an event log has one fixed column layout; use a single-column layout",
        ));
    }
    let ordered = ordered_records(records)?;
    match kind {
        AuditReportKind::ObjectHistory => object_history(&ordered, books, params),
        AuditReportKind::EditLog => Ok(edit_log(&ordered, books, params)),
        AuditReportKind::DeletedAltered => deleted_altered(&ordered, books, params),
        AuditReportKind::BackDated => Ok(back_dated(&ordered, books, params)),
        AuditReportKind::EditsAfterLock => Ok(edits_after_lock(&ordered, books, params)),
        AuditReportKind::IntegrityCertificate => Ok(unsupported(
            "integrity certificate from audit_report",
            "the certificate needs the chain verification result; call integrity_certificate \
             with the report from verify_chain",
        )),
    }
}

fn unsupported(what: &str, reason: &str) -> Outcome<Report> {
    Outcome::Unsupported(Unsupported {
        what: what.to_owned(),
        reason: reason.to_owned(),
    })
}

fn text_row(depth: u8, label: &str, value: String) -> ReportRow {
    ReportRow {
        key: None,
        label: label.to_owned(),
        depth,
        kind: RowKind::Text,
        cells: vec![Cell::Text(label.to_owned()), Cell::Text(value)],
    }
}

fn columns(labels: &[&str]) -> Vec<ColumnHeader> {
    labels
        .iter()
        .map(|l| ColumnHeader {
            label: (*l).to_owned(),
            period: None,
        })
        .collect()
}

/// Records sorted by sequence number; a repeated number is an error.
fn ordered_records(records: &[AuditRecord]) -> Result<Vec<&AuditRecord>, ReportError> {
    let mut ordered: Vec<&AuditRecord> = records.iter().collect();
    ordered.sort_by_key(|r| r.seq);
    let repeated = ordered.windows(2).find_map(|w| match w {
        [a, b] if a.seq == b.seq => Some(a.seq),
        _ => None,
    });
    match repeated {
        Some(seq) => Err(ReportError::Invalid {
            field: "records".to_owned(),
            reason: format!("event sequence number {} appears more than once", seq.0),
        }),
        None => Ok(ordered),
    }
}

fn in_period(rec: &AuditRecord, period: Period) -> bool {
    period.contains(rec.meta.at.date())
}

/// `Some(unsupported)` when a filter other than `voucher_types` is set.
fn non_type_filter(filter: &ReportFilter, report: &str) -> Option<Outcome<Report>> {
    if filter.ledgers.is_empty() && filter.groups.is_empty() && filter.items.is_empty() {
        None
    } else {
        Some(unsupported(
            &format!("{report} filtered by ledger, group or item"),
            "this report can be narrowed by voucher type only",
        ))
    }
}

fn vouchers_of(event: &Event) -> Vec<&Voucher> {
    match event {
        Event::VoucherPosted(v) => vec![v],
        Event::VoucherAltered { before, after } => vec![before, after],
        Event::VoucherCancelled { before } | Event::VoucherDeleted { before } => vec![before],
        Event::MasterCreated(_)
        | Event::MasterAltered { .. }
        | Event::MasterDeleted(_)
        | Event::SettingChanged { .. }
        | Event::PeriodLocked { .. }
        | Event::StatementImported(_)
        | Event::BankStatementStored { .. }
        | Event::Gstr2bImported { .. }
        | Event::ImsActionRecorded { .. }
        | Event::Gstr3bFiled { .. }
        | Event::AiActivityRecorded(_)
        | Event::MatchConfirmed(_)
        | Event::AiProposalRecorded(_) => Vec::new(),
    }
}

fn type_matches(event: &Event, types: &[VoucherTypeId]) -> bool {
    types.is_empty()
        || vouchers_of(event)
            .iter()
            .any(|v| types.contains(&v.type_id))
}

fn actor_label(actor: &Actor) -> String {
    match actor {
        Actor::User(u) => format!("user {}", u.0),
        Actor::Import { job, on_behalf_of } => {
            format!("import '{job}' for user {}", on_behalf_of.0)
        }
        Actor::Assistant {
            model,
            on_behalf_of,
        } => format!("assistant {model} for user {}", on_behalf_of.0),
    }
}

fn time_label(at: OffsetDateTime) -> String {
    let seconds = at.offset().whole_seconds();
    let sign = if seconds < 0 { '-' } else { '+' };
    let abs = seconds.unsigned_abs();
    format!(
        "{:02}:{:02}:{:02} {sign}{:02}:{:02}",
        at.hour(),
        at.minute(),
        at.second(),
        abs / 3600,
        (abs % 3600) / 60
    )
}

fn voucher_label(v: &Voucher, books: &Books) -> String {
    let type_name = books
        .masters
        .voucher_types
        .get(&v.type_id)
        .map_or_else(|| format!("type {}", v.type_id.0), |t| t.name.clone());
    let number = v.number.as_deref().unwrap_or("(no number)");
    format!("{type_name} {number} dated {} [voucher {}]", v.date, v.id.0)
}

fn master_kind(s: &MasterSnapshot) -> &'static str {
    match s {
        MasterSnapshot::Group(_) => "group",
        MasterSnapshot::Ledger(_) => "ledger",
        MasterSnapshot::VoucherType(_) => "voucher type",
        MasterSnapshot::Unit(_) => "unit",
        MasterSnapshot::StockGroup(_) => "stock group",
        MasterSnapshot::StockItem(_) => "stock item",
        MasterSnapshot::Godown(_) => "godown",
        MasterSnapshot::Batch(_) => "batch",
        MasterSnapshot::CostCategory(_) => "cost category",
        MasterSnapshot::CostCentre(_) => "cost centre",
    }
}

/// Key and display label of a master.
fn master_key(s: &MasterSnapshot) -> (ObjectKey, String) {
    let (key, name) = match s {
        MasterSnapshot::Group(x) => (ObjectKey::Group(x.id), x.name.clone()),
        MasterSnapshot::Ledger(x) => (ObjectKey::Ledger(x.id), x.name.clone()),
        MasterSnapshot::VoucherType(x) => (ObjectKey::VoucherType(x.id), x.name.clone()),
        MasterSnapshot::Unit(x) => (ObjectKey::Unit(x.id), x.symbol.clone()),
        MasterSnapshot::StockGroup(x) => (ObjectKey::StockGroup(x.id), x.name.clone()),
        MasterSnapshot::StockItem(x) => (ObjectKey::StockItem(x.id), x.name.clone()),
        MasterSnapshot::Godown(x) => (ObjectKey::Godown(x.id), x.name.clone()),
        MasterSnapshot::Batch(x) => (ObjectKey::Batch(x.id), x.name.clone()),
        MasterSnapshot::CostCategory(x) => (ObjectKey::CostCategory(x.id), x.name.clone()),
        MasterSnapshot::CostCentre(x) => (ObjectKey::CostCentre(x.id), x.name.clone()),
    };
    (key, format!("{} '{name}'", master_kind(s)))
}

/// Object key, object label and action text of a record.
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive match over every event kind, with no catch-all arm"
)]
fn describe(rec: &AuditRecord, books: &Books) -> (ObjectKey, String, String) {
    match &rec.event {
        Event::MasterCreated(snap) => {
            let (key, label) = master_key(snap);
            (key, label, format!("Created {}", master_kind(snap)))
        }
        Event::MasterAltered { after, .. } => {
            let (key, label) = master_key(after);
            (key, label, format!("Altered {}", master_kind(after)))
        }
        Event::MasterDeleted(snap) => {
            let (key, label) = master_key(snap);
            (key, label, format!("Deleted {}", master_kind(snap)))
        }
        Event::VoucherPosted(v) => voucher_description(v, books, "Posted voucher"),
        Event::VoucherAltered { after, .. } => voucher_description(after, books, "Altered voucher"),
        Event::VoucherCancelled { before } => {
            voucher_description(before, books, "Cancelled voucher")
        }
        Event::VoucherDeleted { before } => voucher_description(before, books, "Deleted voucher"),
        Event::SettingChanged { key, .. } => (
            ObjectKey::Setting(key.clone()),
            format!("setting '{key}'"),
            "Changed setting".to_owned(),
        ),
        Event::PeriodLocked { until } => (
            ObjectKey::PeriodLock,
            "period lock".to_owned(),
            format!("Locked books until {until}"),
        ),
        Event::StatementImported(id) | Event::BankStatementStored { id, .. } => (
            ObjectKey::Statement(*id),
            format!("bank statement {}", id.0),
            "Imported bank statement".to_owned(),
        ),
        Event::MatchConfirmed(matched) => (
            ObjectKey::Statement(matched.statement),
            format!("bank statement {}", matched.statement.0),
            "Confirmed bank match".to_owned(),
        ),
        Event::Gstr2bImported {
            gstin,
            period_start,
            file_name,
            ..
        } => (
            gst_return_key(gstin, *period_start),
            gst_return_label(gstin, *period_start),
            format!("Imported GSTR-2B file {file_name}"),
        ),
        Event::ImsActionRecorded {
            gstin,
            period_start,
            supplier_gstin,
            doc_number,
            action,
        } => (
            gst_return_key(gstin, *period_start),
            gst_return_label(gstin, *period_start),
            format!(
                "Recorded IMS action {} on {doc_number} of {supplier_gstin}",
                ims_action_word(*action)
            ),
        ),
        Event::Gstr3bFiled {
            gstin,
            period_start,
            filed_on,
        } => (
            gst_return_key(gstin, *period_start),
            gst_return_label(gstin, *period_start),
            format!("Marked GSTR-3B filed on {filed_on}"),
        ),
        Event::AiActivityRecorded(activity) => ai_activity_row(rec.seq, activity),
        Event::AiProposalRecorded(proposal) => (
            ObjectKey::AiProposal(rec.seq),
            format!("assistant proposal by {}", proposal.model),
            if proposal.accepted {
                "Assistant proposal accepted".to_owned()
            } else {
                "Assistant proposal not accepted".to_owned()
            },
        ),
    }
}

fn gst_return_key(gstin: &str, period_start: Date) -> ObjectKey {
    ObjectKey::GstReturn(format!("{gstin} {period_start}"))
}

fn gst_return_label(gstin: &str, period_start: Date) -> String {
    format!(
        "GST return {gstin} for {:04}-{:02}",
        period_start.year(),
        u8::from(period_start.month())
    )
}

fn ims_action_word(action: StoredImsAction) -> &'static str {
    match action {
        StoredImsAction::Accept => "Accept",
        StoredImsAction::Reject => "Reject",
        StoredImsAction::Pending => "Pending",
    }
}

fn voucher_description(v: &Voucher, books: &Books, action: &str) -> (ObjectKey, String, String) {
    (
        ObjectKey::Voucher(v.id),
        voucher_label(v, books),
        action.to_owned(),
    )
}

fn row_kind(key: &ObjectKey) -> RowKind {
    match key {
        ObjectKey::Voucher(_) => RowKind::Voucher,
        ObjectKey::Ledger(_) => RowKind::Ledger,
        ObjectKey::Group(_) => RowKind::Group,
        ObjectKey::StockItem(_) => RowKind::Item,
        ObjectKey::VoucherType(_)
        | ObjectKey::Unit(_)
        | ObjectKey::StockGroup(_)
        | ObjectKey::Godown(_)
        | ObjectKey::Batch(_)
        | ObjectKey::CostCategory(_)
        | ObjectKey::CostCentre(_)
        | ObjectKey::Setting(_)
        | ObjectKey::PeriodLock
        | ObjectKey::Statement(_)
        | ObjectKey::GstReturn(_)
        | ObjectKey::AiProposal(_) => RowKind::Text,
    }
}

fn drill_key(key: &ObjectKey) -> Option<DrillKey> {
    match key {
        ObjectKey::Voucher(id) => Some(DrillKey::Voucher(*id)),
        ObjectKey::Ledger(id) => Some(DrillKey::Ledger(*id)),
        ObjectKey::Group(id) => Some(DrillKey::Group(*id)),
        ObjectKey::StockItem(id) => Some(DrillKey::Item(*id)),
        ObjectKey::VoucherType(_)
        | ObjectKey::Unit(_)
        | ObjectKey::StockGroup(_)
        | ObjectKey::Godown(_)
        | ObjectKey::Batch(_)
        | ObjectKey::CostCategory(_)
        | ObjectKey::CostCentre(_)
        | ObjectKey::Setting(_)
        | ObjectKey::PeriodLock
        | ObjectKey::Statement(_)
        | ObjectKey::GstReturn(_)
        | ObjectKey::AiProposal(_) => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------------------------

fn base_cells(item: &Item<'_>) -> Vec<Cell> {
    let rec = item.rec;
    vec![
        Cell::Text(rec.seq.0.to_string()),
        Cell::Date(rec.meta.at.date()),
        Cell::Text(time_label(rec.meta.at)),
        Cell::Text(actor_label(&rec.meta.actor)),
        Cell::Text(item.action.clone()),
        Cell::Text(item.label.clone()),
        rec.meta
            .reason
            .as_ref()
            .map_or(Cell::Empty, |r| Cell::Text(r.clone())),
    ]
}

/// Blank field, before and after cells for the event row of the diff layout.
fn diff_pad() -> Vec<Cell> {
    vec![Cell::Empty, Cell::Empty, Cell::Empty]
}

fn item<'a>(rec: &'a AuditRecord, books: &Books, extra: Vec<Cell>) -> Item<'a> {
    let (key, label, action) = describe(rec, books);
    Item {
        rec,
        key,
        label,
        action,
        extra,
        changes: Vec::new(),
    }
}

fn item_rows(items: &[Item<'_>]) -> Vec<ReportRow> {
    let mut rows = Vec::new();
    for it in items {
        let mut cells = base_cells(it);
        cells.extend(it.extra.iter().cloned());
        rows.push(ReportRow {
            key: drill_key(&it.key),
            label: format!("#{} {} {}", it.rec.seq.0, it.action, it.label),
            depth: 0,
            kind: row_kind(&it.key),
            cells,
        });
        for change in &it.changes {
            let mut cells = vec![Cell::Empty; BASE_COLUMNS.len()];
            cells.push(Cell::Text(change.field.clone()));
            cells.push(Cell::Text(change.before.clone()));
            cells.push(Cell::Text(change.after.clone()));
            rows.push(ReportRow {
                key: drill_key(&it.key),
                label: change.field.clone(),
                depth: 1,
                kind: RowKind::Text,
                cells,
            });
        }
    }
    rows
}

fn make_report(
    title: &str,
    params: &ReportParams,
    extra_columns: &[&str],
    items: &[Item<'_>],
    notes: Vec<String>,
) -> Report {
    let labels: Vec<&str> = BASE_COLUMNS
        .iter()
        .chain(extra_columns.iter())
        .copied()
        .collect();
    let mut totals = vec![Cell::Text(count_events(items.len()))];
    totals.resize(labels.len(), Cell::Empty);
    Report {
        title: title.to_owned(),
        params: params.clone(),
        columns: columns(&labels),
        rows: item_rows(items),
        totals,
        notes,
    }
}

// ---------------------------------------------------------------------------------------------
// The reports
// ---------------------------------------------------------------------------------------------

fn object_history(
    ordered: &[&AuditRecord],
    books: &Books,
    params: &ReportParams,
) -> Result<Outcome<Report>, ReportError> {
    let filter = &params.filter;
    if !filter.voucher_types.is_empty() {
        return Ok(unsupported(
            "object history filtered by voucher type",
            "the history of vouchers cannot be selected yet: the report filter has no voucher \
             field; leave the filter empty to list every object",
        ));
    }
    let mut wanted: BTreeSet<ObjectKey> = BTreeSet::new();
    wanted.extend(filter.ledgers.iter().map(|id| ObjectKey::Ledger(*id)));
    wanted.extend(filter.groups.iter().map(|id| ObjectKey::Group(*id)));
    wanted.extend(filter.items.iter().map(|id| ObjectKey::StockItem(*id)));
    let mut items = Vec::new();
    for rec in ordered.iter().copied() {
        if !in_period(rec, params.period) {
            continue;
        }
        let mut it = item(rec, books, diff_pad());
        if wanted.is_empty() || wanted.contains(&it.key) {
            it.changes = changes_of(rec)?;
            items.push(it);
        }
    }
    // Stable sort: records of one object stay in sequence order.
    items.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(Outcome::Value(make_report(
        "Object history",
        params,
        &DIFF_COLUMNS,
        &items,
        Vec::new(),
    )))
}

fn edit_log(ordered: &[&AuditRecord], books: &Books, params: &ReportParams) -> Outcome<Report> {
    if let Some(u) = non_type_filter(&params.filter, "edit log") {
        return u;
    }
    let types = &params.filter.voucher_types;
    let items: Vec<Item<'_>> = ordered
        .iter()
        .copied()
        .filter(|r| in_period(r, params.period) && type_matches(&r.event, types))
        .map(|r| item(r, books, diff_pad()))
        .collect();
    Outcome::Value(make_report(
        "Edit log",
        params,
        &DIFF_COLUMNS,
        &items,
        user_counts(&items),
    ))
}

/// `1 event`, `3 events`.
fn count_events(n: usize) -> String {
    if n == 1 {
        "1 event".to_owned()
    } else {
        format!("{n} events")
    }
}

/// Note for the stateful reports when the records supplied do not start at event 1.
fn window_note(ordered: &[&AuditRecord]) -> Option<String> {
    let first = ordered.first()?;
    if first.seq.0 > 1 {
        Some(format!(
            "The records supplied start at event {}. Earlier events were not read, so postings \
             and period locks before it are unknown to this report and it may list too little.",
            first.seq.0
        ))
    } else {
        None
    }
}

/// One note per user with the number of events, in user-label order.
fn user_counts(items: &[Item<'_>]) -> Vec<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for it in items {
        let n = counts.entry(actor_label(&it.rec.meta.actor)).or_insert(0);
        *n = n.saturating_add(1);
    }
    counts
        .into_iter()
        .map(|(user, n)| format!("{user}: {}", count_events(n)))
        .collect()
}

fn deleted_altered(
    ordered: &[&AuditRecord],
    books: &Books,
    params: &ReportParams,
) -> Result<Outcome<Report>, ReportError> {
    if let Some(u) = non_type_filter(&params.filter, "deleted and altered vouchers") {
        return Ok(u);
    }
    let types = &params.filter.voucher_types;
    let mut items = Vec::new();
    for rec in ordered.iter().copied() {
        let wanted = match &rec.event {
            Event::VoucherAltered { .. }
            | Event::VoucherCancelled { .. }
            | Event::VoucherDeleted { .. } => true,
            Event::VoucherPosted(_)
            | Event::MasterCreated(_)
            | Event::MasterAltered { .. }
            | Event::MasterDeleted(_)
            | Event::SettingChanged { .. }
            | Event::PeriodLocked { .. }
            | Event::StatementImported(_)
            | Event::BankStatementStored { .. }
            | Event::Gstr2bImported { .. }
            | Event::ImsActionRecorded { .. }
            | Event::Gstr3bFiled { .. }
            | Event::AiActivityRecorded(_)
            | Event::MatchConfirmed(_)
            | Event::AiProposalRecorded(_) => false,
        };
        if wanted && in_period(rec, params.period) && type_matches(&rec.event, types) {
            let mut it = item(rec, books, diff_pad());
            it.changes = changes_of(rec)?;
            items.push(it);
        }
    }
    Ok(Outcome::Value(make_report(
        "Deleted and altered vouchers",
        params,
        &DIFF_COLUMNS,
        &items,
        Vec::new(),
    )))
}

fn back_dated(ordered: &[&AuditRecord], books: &Books, params: &ReportParams) -> Outcome<Report> {
    if let Some(u) = non_type_filter(&params.filter, "back-dated entries") {
        return u;
    }
    let types = &params.filter.voucher_types;
    let mut latest: Option<Date> = None;
    let mut items = Vec::new();
    for rec in ordered.iter().copied() {
        let Event::VoucherPosted(v) = &rec.event else {
            continue;
        };
        if let Some(max) = latest
            && v.date < max
            && in_period(rec, params.period)
            && type_matches(&rec.event, types)
        {
            let days = (max - v.date).whole_days();
            items.push(item(
                rec,
                books,
                vec![
                    Cell::Date(v.date),
                    Cell::Date(max),
                    Cell::Text(days.to_string()),
                ],
            ));
        }
        latest = Some(latest.map_or(v.date, |m| m.max(v.date)));
    }
    let mut notes = vec![
        "A voucher is back-dated when its date is earlier than the latest date among vouchers \
         posted before it. Only the records supplied are examined: earlier postings outside the \
         window are not known to this report."
            .to_owned(),
    ];
    notes.extend(window_note(ordered));
    Outcome::Value(make_report(
        "Back-dated entries",
        params,
        &["Voucher date", "Latest earlier date", "Days back"],
        &items,
        notes,
    ))
}

fn edits_after_lock(
    ordered: &[&AuditRecord],
    books: &Books,
    params: &ReportParams,
) -> Outcome<Report> {
    if let Some(u) = non_type_filter(&params.filter, "edits after lock") {
        return u;
    }
    let types = &params.filter.voucher_types;
    let mut lock: Option<Date> = None;
    let mut items = Vec::new();
    for rec in ordered.iter().copied() {
        if let Some(until) = lock {
            let offending = vouchers_of(&rec.event)
                .iter()
                .map(|v| v.date)
                .filter(|d| *d <= until)
                .min();
            if let Some(date) = offending
                && in_period(rec, params.period)
                && type_matches(&rec.event, types)
            {
                items.push(item(rec, books, vec![Cell::Date(date), Cell::Date(until)]));
            }
        }
        if let Event::PeriodLocked { until } = &rec.event {
            lock = Some(*until);
        }
    }
    let mut notes = vec![
        "Listed: voucher events that touch a date on or before the lock date in force when the \
         event was recorded. The lock date in force starts from the first record supplied: a \
         lock recorded before the window is not known to this report."
            .to_owned(),
    ];
    notes.extend(window_note(ordered));
    Outcome::Value(make_report(
        "Edits after lock date",
        params,
        &["Voucher date", "Lock date"],
        &items,
        notes,
    ))
}

/// The object, its label and the description of an assistant event. A decision is filed under
/// the draft it decides, so a draft's history reads stored, then accepted or rejected.
fn ai_activity_row(
    seq: EventSeq,
    activity: &ledgerly_model::AiActivity,
) -> (ObjectKey, String, String) {
    use ledgerly_model::AiActivityKind;
    let label = format!(
        "assistant {} by {}",
        activity.input_label, activity.provider
    );
    match &activity.kind {
        AiActivityKind::DraftStored { .. } => (
            ObjectKey::AiProposal(seq),
            label,
            "Assistant draft stored for review".to_owned(),
        ),
        AiActivityKind::DraftAccepted { draft, .. } => (
            ObjectKey::AiProposal(*draft),
            label,
            "Assistant draft accepted".to_owned(),
        ),
        AiActivityKind::DraftRejected { draft, .. } => (
            ObjectKey::AiProposal(*draft),
            label,
            "Assistant draft rejected".to_owned(),
        ),
        AiActivityKind::Answered { .. } => (
            ObjectKey::AiProposal(seq),
            label,
            "Assistant question answered by a report query".to_owned(),
        ),
        AiActivityKind::ReadFailed { reason } => (
            ObjectKey::AiProposal(seq),
            label,
            format!("Assistant could not read the input: {reason}"),
        ),
        AiActivityKind::CloudConsent { granted } => (
            ObjectKey::AiProposal(seq),
            label,
            if *granted {
                "Cloud send agreed for one document".to_owned()
            } else {
                "Cloud send declined; kept on this computer".to_owned()
            },
        ),
    }
}
