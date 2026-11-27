use ledgerly_core::{BankStatementId, Date, EventSeq, LedgerId, OffsetDateTime, Paise, UserId};
use ledgerly_model::{AiActivity, BankMatch, MasterSnapshot, ProposalRecord, Voucher};
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
    /// A bank statement file was imported and verified, with everything needed to rebuild it
    /// when the company file is reopened (card C3). Replaces `StatementImported` for new imports.
    BankStatementStored {
        /// The statement id.
        id: BankStatementId,
        /// The bank ledger the statement was imported for.
        ledger: LedgerId,
        /// Lower-case hex SHA-256 of the imported file, for duplicate detection.
        file_hash: String,
        /// Format registry id the file was parsed with.
        format_id: String,
        /// Account number or name found in the file, if any.
        account_hint: Option<String>,
        /// Opening balance, credit positive.
        opening: Paise,
        /// Closing balance, credit positive.
        closing: Paise,
        /// The statement's stated end date (`:62F:` date for MT940), if the file has one.
        end_date: Option<Date>,
        /// The lines in file order.
        lines: Vec<StoredStatementLine>,
    },
    /// A GSTR-2B file downloaded from the GST portal was imported for a return period (round
    /// s11). The file text is kept whole, so the reconciliation is rebuilt from it each time the
    /// company file is opened. A later import for the same GSTIN and period replaces the earlier
    /// one in the reconciliation; both stay in the log.
    Gstr2bImported {
        /// The company GSTIN the file was imported for, as the file states it.
        gstin: String,
        /// First day of the return period.
        period_start: Date,
        /// File name the user chose, for the audit trail.
        file_name: String,
        /// Lower-case hex SHA-256 of the file text.
        file_hash: String,
        /// The file text, exactly as imported.
        json: String,
    },
    /// The user's IMS action on one 2B document (round s11). The last event for a document, in
    /// sequence order, is the action in force. Ledgerly records the action; it does not send it
    /// to the portal.
    ImsActionRecorded {
        /// The company GSTIN.
        gstin: String,
        /// First day of the return period of the 2B file.
        period_start: Date,
        /// The supplier's GSTIN as written in the 2B file.
        supplier_gstin: String,
        /// The document number as written in the 2B file.
        doc_number: String,
        /// What the user chose.
        action: StoredImsAction,
    },
    /// The user marked GSTR-3B filed for a return period (round s11). IMS actions for the period
    /// are then final.
    Gstr3bFiled {
        /// The company GSTIN.
        gstin: String,
        /// First day of the return period.
        period_start: Date,
        /// The day the user says the return was filed.
        filed_on: Date,
    },
    /// An assistant event (round s21): a draft stored, accepted or rejected, a question answered,
    /// a read that failed, or a cloud consent. The AI activity log is the list of these.
    AiActivityRecorded(AiActivity),
}

/// An action on a record in the GST portal's Invoice Management System, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoredImsAction {
    /// Accept the document.
    Accept,
    /// Reject the document.
    Reject,
    /// Keep the document pending.
    Pending,
}

/// One bank statement line as stored by `Event::BankStatementStored`. Balances are credit
/// positive, as in the parsed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredStatementLine {
    /// Row or record number in the file (1-based).
    pub row: u32,
    /// Transaction date.
    pub date: Date,
    /// Value date.
    pub value_date: Option<Date>,
    /// Description.
    pub description: String,
    /// Cheque or reference number.
    pub reference: Option<String>,
    /// Money out; zero when none.
    pub debit: Paise,
    /// Money in; zero when none.
    pub credit: Paise,
    /// Running balance after the line, when the file has one.
    pub balance: Option<Paise>,
    /// The raw cells, kept for audit.
    pub raw: Vec<String>,
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
