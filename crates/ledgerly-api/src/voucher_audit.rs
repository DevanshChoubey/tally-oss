//! What the read-only voucher view needs beyond the stored voucher: the posting totals and the
//! audit facts (who entered it, when, how, and what changed it since), from the engine and the
//! audit log (design mock `balance-sheet.voucher`, its Total row and Audit card).
//!
//! * The totals are the sum of the voucher's debit lines and of its credit lines, by checked
//!   addition. A sum that overflows is `Unsupported`, never a wrapped figure.
//! * The audit facts are read from the hash-chained log: the `VoucherPosted` record of the
//!   voucher gives who and when (the record's own time, not the voucher date), how (typed, AI
//!   draft or import, as in the day book) and its place in the chain; each later
//!   `VoucherAltered` or `VoucherCancelled` record of the voucher is listed. Whether the period
//!   is locked is the voucher date against the lock date in force.
//!
//! What the log does not keep is not shown: which kind of AI draft a voucher came from, how many
//! edits were made before accepting it, and attachments.

use ledgerly_core::{Date, DrCr, EventSeq, OffsetDateTime, Outcome, Paise, VoucherId};
use ledgerly_store::{AuditRecord, Event};

use crate::day_book_meta::{Entry, EntryOrigin, entry};
use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// A change made to a voucher after it was posted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// The voucher was altered.
    Altered,
    /// The voucher was cancelled (number kept, amounts removed).
    Cancelled,
}

/// One later change of a voucher, from the audit log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherChange {
    /// What was done.
    pub kind: ChangeKind,
    /// Who did it (the name in the user directory, `User <id>` without one).
    pub by: String,
    /// When the record was written.
    pub at: OffsetDateTime,
    /// The record's place in the log.
    pub event: EventSeq,
}

/// Where a voucher came from, from its `VoucherPosted` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherAudit {
    /// Who entered it.
    pub entered_by: String,
    /// How it was entered.
    pub origin: EntryOrigin,
    /// When the record was written.
    pub posted_at: OffsetDateTime,
    /// The record's place in the log.
    pub event: EventSeq,
    /// The record's hash, lower-case hex.
    pub hash: String,
    /// Later alterations and cancellations, oldest first.
    pub changes: Vec<VoucherChange>,
}

/// The facts the voucher view shows besides the stored fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherDetail {
    /// The sum of the debit lines.
    pub debit_total: Paise,
    /// The sum of the credit lines.
    pub credit_total: Paise,
    /// The audit facts; `None` when the log holds no `VoucherPosted` record for the voucher.
    pub audit: Option<VoucherAudit>,
    /// The lock date in force, when there is one.
    pub lock_date: Option<Date>,
    /// The voucher's date.
    pub date: Date,
}

impl VoucherDetail {
    /// `true` when the voucher's date is on or before the lock date.
    #[must_use]
    pub fn period_locked(&self) -> bool {
        self.lock_date.is_some_and(|lock| self.date <= lock)
    }
}

impl Api {
    /// The totals and audit facts of voucher `id`.
    ///
    /// # Errors
    /// [`ApiError::Stale`] after a saved change whose reload failed; [`ApiError::Store`] when the
    /// log cannot be read.
    ///
    /// Returns `Outcome::Unsupported` when the voucher is not in the books or a total overflows.
    pub fn voucher_detail(&self, id: VoucherId) -> Result<Outcome<VoucherDetail>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let Some(voucher) = books.vouchers.get(&id) else {
            return Ok(Outcome::unsupported(
                "voucher detail",
                format!("voucher {} is not in the books", id.0),
            ));
        };
        let (mut debit, mut credit) = (Paise::ZERO, Paise::ZERO);
        for line in &voucher.ledger_lines {
            let side = match line.amount.side {
                DrCr::Dr => &mut debit,
                DrCr::Cr => &mut credit,
            };
            match side.checked_add(line.amount.amount) {
                Ok(sum) => *side = sum,
                Err(e) => {
                    return Ok(Outcome::unsupported(
                        "voucher posting total",
                        format!("the lines of voucher {} cannot be added: {e}", id.0),
                    ));
                }
            }
        }
        let records = read_all(self.store())?;
        Ok(Outcome::Value(VoucherDetail {
            debit_total: debit,
            credit_total: credit,
            audit: audit_of(&records, id),
            lock_date: self.lock(),
            date: voucher.date,
        }))
    }
}

/// The audit facts of `id` in `records`, when the log holds its `VoucherPosted` record.
fn audit_of(records: &[AuditRecord], id: VoucherId) -> Option<VoucherAudit> {
    let mut posted: Option<(&AuditRecord, Entry)> = None;
    let mut changes = Vec::new();
    for record in records {
        match &record.event {
            Event::VoucherPosted(v) if v.id == id && posted.is_none() => {
                posted = Some((record, entry(records, &record.meta.actor)));
            }
            Event::VoucherAltered { after, .. } if after.id == id && posted.is_some() => {
                changes.push(change_of(records, record, ChangeKind::Altered));
            }
            Event::VoucherCancelled { before } if before.id == id && posted.is_some() => {
                changes.push(change_of(records, record, ChangeKind::Cancelled));
            }
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
            | Event::Gstr3bFiled { .. } => {}
        }
    }
    let (record, who_how) = posted?;
    Some(VoucherAudit {
        entered_by: who_how.by,
        origin: who_how.origin,
        posted_at: record.meta.at,
        event: record.seq,
        hash: hex(&record.hash),
        changes,
    })
}

/// The change a later record made, with who made it.
fn change_of(records: &[AuditRecord], record: &AuditRecord, kind: ChangeKind) -> VoucherChange {
    VoucherChange {
        kind,
        by: entry(records, &record.meta.actor).by,
        at: record.meta.at,
        event: record.seq,
    }
}

/// Lower-case hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            out.extend(char::from_digit(u32::from(nibble), 16));
        }
    }
    out
}
