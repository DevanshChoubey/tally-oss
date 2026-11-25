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
use ledgerly_model::Books;
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
        | Event::Gstr3bFiled { .. } => None,
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

/// Entered by and origin of one voucher.
struct Entry {
    by: String,
    origin: &'static str,
}

fn who(records: &[AuditRecord], user: UserId) -> String {
    stored_name(records, user).unwrap_or_else(|| format!("User {}", user.0))
}

fn entry(records: &[AuditRecord], actor: &Actor) -> Entry {
    match actor {
        Actor::User(user) => Entry {
            by: who(records, *user),
            origin: "Typed",
        },
        Actor::Import { on_behalf_of, .. } => Entry {
            by: who(records, *on_behalf_of),
            origin: "Imported",
        },
        Actor::Assistant { on_behalf_of, .. } => Entry {
            by: who(records, *on_behalf_of),
            origin: "AI draft",
        },
    }
}

/// `report` (the day book) with the narration, entered-by and origin columns added.
pub(crate) fn annotate(
    store: &ledgerly_store::SqliteStore,
    books: &Books,
    mut report: Report,
) -> Result<Report, ApiError> {
    let records = read_all(store)?;
    let mut posted: BTreeMap<VoucherId, Entry> = BTreeMap::new();
    for record in &records {
        if let Event::VoucherPosted(v) = &record.event {
            posted
                .entry(v.id)
                .or_insert_with(|| entry(&records, &record.meta.actor));
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
            Some(e) => (Cell::Text(e.by.clone()), Cell::Text(e.origin.to_owned())),
            None => (Cell::Empty, Cell::Empty),
        };
        row.cells.extend([narration, by, origin]);
    }
    Ok(report)
}
