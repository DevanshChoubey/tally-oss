//! Who entered each voucher, and how, for the day book (design mock `day-book.default`).
//!
//! The report engine knows the books, not the audit log, so the facade adds three columns to the
//! day book after the engine has built it: `Narration` (the voucher's own text), `Entered by` and
//! `Origin`. Both of the last two are read from the `VoucherPosted` record of each voucher:
//!
//! * `Entered by` is the actor's name: a user's name from the company's user directory
//!   (`SettingChanged` under `user.<id>.name`, set with [`Api::set_user_name`]); a user with no
//!   name shows as `User <id>`; an import shows the user who started it; the assistant shows the
//!   user who confirmed the proposal.
//! * `Origin` is `Typed` for a user, `AI draft` for the assistant (a person accepted the draft)
//!   and `Imported` for an import job.
//!
//! What the log does not record is not shown: no attachment is stored with a voucher, and no bank
//! rule posts one, so `Image attached` and `Bank rule` (both in the mock) are never produced, and
//! which kind of AI draft a voucher came from is not kept.

use std::collections::BTreeMap;

use ledgerly_core::{UserId, VoucherId};
use ledgerly_model::{AiActivityKind, AiFeature, Books};
use ledgerly_reports::{Cell, ColumnHeader, DrillKey, Report};
use ledgerly_store::{Actor, AuditRecord, Event};

use crate::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// The setting key that holds the name of `user`.
fn user_key(user: UserId) -> String {
    format!("user.{}.name", user.0)
}

/// The name stored for `user` in `records`: the last `SettingChanged` for its key.
fn stored_name(records: &[AuditRecord], user: UserId) -> Option<String> {
    let key = user_key(user);
    records.iter().rev().find_map(|record| match &record.event {
        Event::SettingChanged { key: k, after, .. } if *k == key => Some(after.clone()),
        Event::SettingChanged { .. }
        | Event::MasterCreated(_)
        | Event::MasterAltered { .. }
        | Event::MasterDeleted(_)
        | Event::VoucherPosted(_)
        | Event::VoucherAltered { .. }
        | Event::VoucherCancelled { .. }
        | Event::VoucherDeleted { .. }
        | Event::PeriodLocked { .. }
        | Event::StatementImported(_)
        | Event::MatchConfirmed(_)
        | Event::AiProposalRecorded(_)
        | Event::BankStatementStored { .. }
        | Event::Gstr2bImported { .. }
        | Event::ImsActionRecorded { .. }
        | Event::Gstr3bFiled { .. }
        | Event::AiActivityRecorded(_) => None,
    })
}

impl Api {
    /// Records the display name of `user`, as an audited setting change. The day book shows it
    /// under `Entered by`. Setting the name already stored writes nothing.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the log cannot be read or the change cannot be committed.
    pub fn set_user_name(&mut self, user: UserId, name: &str) -> Result<(), ApiError> {
        let records = read_all(self.store())?;
        let before = stored_name(&records, user);
        if before.as_deref() == Some(name) {
            return Ok(());
        }
        let event = Event::SettingChanged {
            key: user_key(user),
            before: before.unwrap_or_default(),
            after: name.to_owned(),
        };
        let actor = self.actor().clone();
        self.commit_as(&[event], &actor)?;
        Ok(())
    }
}

impl Api {
    /// The stored name of the signed-in user (the actor of this Api), or `None` when the
    /// directory holds no name for them. A user the directory does not name has no name: nothing
    /// is made up here.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the log cannot be read.
    pub fn actor_name(&self) -> Result<Option<String>, ApiError> {
        let user = match self.actor() {
            Actor::User(user) => *user,
            Actor::Import { on_behalf_of, .. } | Actor::Assistant { on_behalf_of, .. } => {
                *on_behalf_of
            }
        };
        let records = read_all(self.store())?;
        Ok(stored_name(&records, user))
    }
}

/// How a voucher came to be entered, from the actor of its `VoucherPosted` record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryOrigin {
    /// A user typed it.
    Typed,
    /// The assistant drafted it and a person accepted the draft.
    AiDraft,
    /// A person accepted a draft of this assistant feature; the accept event names the voucher.
    AcceptedDraft(AiFeature),
    /// An import job posted it.
    Imported,
}

impl EntryOrigin {
    /// The word the day book's Origin column shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Typed => "Typed",
            Self::AiDraft => "AI draft",
            Self::AcceptedDraft(feature) => match feature {
                AiFeature::BillReading => "Bill photo draft, accepted",
                AiFeature::PlainWords => "Plain-words draft, accepted",
                AiFeature::BankSuggestion => "Bank suggestion, accepted",
                AiFeature::Ask => "Ask answer, accepted",
                AiFeature::Gstr2bExplanation => "GSTR-2B explanation, accepted",
            },
            Self::Imported => "Imported",
        }
    }
}

/// Entered by and origin of one voucher.
pub(crate) struct Entry {
    pub(crate) by: String,
    pub(crate) origin: EntryOrigin,
}

fn who(records: &[AuditRecord], user: UserId) -> String {
    stored_name(records, user).unwrap_or_else(|| format!("User {}", user.0))
}

pub(crate) fn entry(records: &[AuditRecord], actor: &Actor) -> Entry {
    match actor {
        Actor::User(user) => Entry {
            by: who(records, *user),
            origin: EntryOrigin::Typed,
        },
        Actor::Import { on_behalf_of, .. } => Entry {
            by: who(records, *on_behalf_of),
            origin: EntryOrigin::Imported,
        },
        Actor::Assistant { on_behalf_of, .. } => Entry {
            by: who(records, *on_behalf_of),
            origin: EntryOrigin::AiDraft,
        },
    }
}

/// The vouchers a person posted by accepting an assistant draft, with the draft's feature. The
/// posting's actor is the person; the accept event recorded in the same commit names the voucher.
pub(crate) fn accepted_drafts(records: &[AuditRecord]) -> BTreeMap<VoucherId, AiFeature> {
    records
        .iter()
        .filter_map(|record| match &record.event {
            Event::AiActivityRecorded(activity) => match &activity.kind {
                AiActivityKind::DraftAccepted {
                    voucher: Some(voucher),
                    ..
                } => Some((*voucher, activity.feature)),
                AiActivityKind::DraftAccepted { voucher: None, .. }
                | AiActivityKind::DraftStored { .. }
                | AiActivityKind::DraftRejected { .. }
                | AiActivityKind::Answered { .. }
                | AiActivityKind::ReadFailed { .. }
                | AiActivityKind::CloudConsent { .. }
                | AiActivityKind::NotRun { .. } => None,
            },
            Event::MasterCreated(_)
            | Event::MasterAltered { .. }
            | Event::MasterDeleted(_)
            | Event::VoucherPosted(_)
            | Event::VoucherAltered { .. }
            | Event::VoucherCancelled { .. }
            | Event::VoucherDeleted { .. }
            | Event::SettingChanged { .. }
            | Event::PeriodLocked { .. }
            | Event::StatementImported(_)
            | Event::MatchConfirmed(_)
            | Event::AiProposalRecorded(_)
            | Event::BankStatementStored { .. }
            | Event::Gstr2bImported { .. }
            | Event::ImsActionRecorded { .. }
            | Event::Gstr3bFiled { .. } => None,
        })
        .collect()
}

/// `report` (the day book) with the narration, entered-by and origin columns added.
pub(crate) fn annotate(
    store: &ledgerly_store::SqliteStore,
    books: &Books,
    mut report: Report,
) -> Result<Report, ApiError> {
    let records = read_all(store)?;
    let accepted = accepted_drafts(&records);
    let mut posted: BTreeMap<VoucherId, Entry> = BTreeMap::new();
    for record in &records {
        if let Event::VoucherPosted(v) = &record.event {
            posted.entry(v.id).or_insert_with(|| {
                let mut e = entry(&records, &record.meta.actor);
                if let Some(feature) = accepted.get(&v.id) {
                    e.origin = EntryOrigin::AcceptedDraft(*feature);
                }
                e
            });
        }
    }
    for label in ["Narration", "Entered by", "Origin"] {
        report.columns.push(ColumnHeader {
            label: label.to_owned(),
            period: None,
        });
        report.totals.push(Cell::Empty);
    }
    for row in &mut report.rows {
        let Some(DrillKey::Voucher(id)) = &row.key else {
            row.cells.extend([Cell::Empty, Cell::Empty, Cell::Empty]);
            continue;
        };
        let narration = books
            .vouchers
            .get(id)
            .map_or(Cell::Empty, |v| Cell::Text(v.narration.clone()));
        let (by, origin) = match posted.get(id) {
            Some(e) => (
                Cell::Text(e.by.clone()),
                Cell::Text(e.origin.label().to_owned()),
            ),
            None => (Cell::Empty, Cell::Empty),
        };
        row.cells.extend([narration, by, origin]);
    }
    Ok(report)
}
