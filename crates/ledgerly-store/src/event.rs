use ledgerly_core::{BankStatementId, Date, EventSeq, OffsetDateTime, UserId};
use ledgerly_model::{BankMatch, MasterSnapshot, ProposalRecord, Voucher};
use serde::{Deserialize, Serialize};

/// One change to a company. Every create, alter, cancel and delete of masters, vouchers and
/// settings is an event (spec 6.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[expect(
    clippy::large_enum_variant,
    reason = "events are built once per command and serialised; boxing would only add noise"
)]
pub enum Event {
    /// A master was created.
    MasterCreated(MasterSnapshot),
    /// A master was altered.
    MasterAltered {
        /// State before.
        before: MasterSnapshot,
        /// State after.
        after: MasterSnapshot,
    },
    /// A master was deleted.
    MasterDeleted(MasterSnapshot),
    /// A voucher was posted.
    VoucherPosted(Voucher),
    /// A voucher was altered.
    VoucherAltered {
        /// State before.
        before: Voucher,
        /// State after.
        after: Voucher,
    },
    /// A voucher was cancelled (number kept, amounts removed).
    VoucherCancelled {
        /// State before.
        before: Voucher,
    },
    /// A voucher was deleted.
    VoucherDeleted {
        /// State before.
        before: Voucher,
    },
    /// A setting changed.
    SettingChanged {
        /// Setting key.
        key: String,
        /// Value before.
        before: String,
        /// Value after.
        after: String,
    },
    /// Books were locked up to and including this date.
    PeriodLocked {
        /// Last locked date.
        until: Date,
    },
    /// A bank statement was imported.
    StatementImported(BankStatementId),
    /// The user confirmed a bank match.
    MatchConfirmed(BankMatch),
    /// An assistant proposal and the user's response.
    AiProposalRecorded(ProposalRecord),
}

/// Who did it, when and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditMeta {
    /// Actor.
    pub actor: Actor,
    /// Time, with the user's UTC offset.
    pub at: OffsetDateTime,
    /// Reason, when given (required for some actions such as unlocking).
    pub reason: Option<String>,
    /// Ledgerly version that wrote the event.
    pub app_version: String,
}

/// Who made a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Actor {
    /// A signed-in user.
    User(UserId),
    /// An import job acting for a user.
    Import {
        /// Job description, such as `tally-import 2026-11-20T10:00`.
        job: String,
        /// User who started it.
        on_behalf_of: UserId,
    },
    /// The assistant acting for a user (only ever via confirmed proposals).
    Assistant {
        /// Model name.
        model: String,
        /// User who confirmed.
        on_behalf_of: UserId,
    },
}

/// A stored event with its place in the hash chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Sequence number (1-based, gapless).
    pub seq: EventSeq,
    /// Hash of the previous record (all zero for the first).
    pub prev_hash: [u8; 32],
    /// SHA-256 over the canonical encoding of `(seq, prev_hash, meta, event)`.
    pub hash: [u8; 32],
    /// Who, when, why.
    pub meta: AuditMeta,
    /// What changed.
    pub event: Event,
}

/// Result of a successful commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitReceipt {
    /// Sequence numbers assigned, in order.
    pub seqs: Vec<EventSeq>,
    /// Hash of the last record written.
    pub last_hash: [u8; 32],
}
