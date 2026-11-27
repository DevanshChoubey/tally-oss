//! Command and query facade used by the app, the CLI and the MCP server (card W1, Wave 2).
//!
//! The entry point is [`Api`]: [`Api::execute`] runs a [`Command`], [`Api::report`] runs a
//! [`ledgerly_reports::ReportParams`] and [`Api::search`] finds ledgers, vouchers, reports and
//! settings. Every figure comes from the engine or the report engines (spec section 4).

mod ai;
mod ai_drafts;
mod ai_features;
mod ai_settings;
mod as_voucher;
mod ask_books;
mod backup;
mod balance;
mod bank;
mod bank_ai;
mod brs_statement;
mod command;
mod day_book_meta;
mod einvoice;
mod error;
mod events;
pub mod facade;
mod gst;
mod gst_filing;
mod gstr1_sections;
mod gstr3b_payment;
mod home;
mod itc_draft;
mod maintenance;
mod nudges;
mod print;
mod recon;
mod report;
mod return_against;
mod review;
mod search;
mod tally_file_import;
mod tally_import;
mod upgrade;
mod voucher_audit;
mod voucher_gst;

pub use ai::{AiLogEntry, AiTier};
pub use ai_drafts::{
    AI_CONFIDENCE_THRESHOLD_PERMILLE, Accepted, AiActivityEntry, AiDecision, AiDraft, AiLogRow,
    AiOutput, DraftEdits, NewAiDraft,
};
// The assistant's typed proposals, for the app (which reaches the assistant only through here).
pub use ai_features::{AiGate, BillRead, WordsRead};
pub use ai_settings::{
    ANTHROPIC_MESSAGES_URL, AiSettings, CLOUD_FORMATS, CloudAssistant, CloudSettings,
    DEFAULT_BILL_MODEL, DEFAULT_WORDS_MODEL, KEY_SERVICE, KeyState, LOCAL_MODEL_FILE_KEY,
    cloud_model_for, cloud_request_url,
};
pub use ask_books::{AskAnswer, default_report_params, default_report_period};
pub use backup::{
    BACKUP_FOLDER_KEY, BACKUP_FREQUENCY_KEY, BACKUP_KEEP_KEY, BackupFrequency, BackupSettings,
    BackupTaken, DEFAULT_KEEP, MAX_KEEP, Restored, restore_backup,
};
pub use balance::BalanceChange;
pub use bank::{BalanceRefusal, balance_refusal};
pub use bank_ai::{AiBankMatch, BankAiAnswer};
pub use command::{Command, CommandResult, CreatedId};
pub use gst_filing::GstFiling;
pub use ledgerly_assist::AssistError;
pub use ledgerly_assist::cloud::{
    CloudRequest, CloudResponse, CloudTransport, KeyVault, KeyringStore, MemoryVault, UreqTransport,
};
pub use ledgerly_assist::ocr::{OcrLine, OcrOutput, RejectedField};
pub use ledgerly_assist::ocr_models::{
    BillOcr, ModelFetch, OCR_MODEL_FILES, OCRS_VERSION, OcrModelsError, OcrsReader, UreqModelFetch,
    download_ocr_models,
};
pub use ledgerly_assist::provider::{
    AiProvider, CloudModel, FIXED_ANSWERS_NAME, FixedAnswers, LocalModel, LocalModelFile,
    ProviderInfo, ProviderPlace, ProviderStatus,
};
pub use ledgerly_assist::review::{DraftField, NEEDS_REVIEW_PERMILLE, needs_review};
pub use ledgerly_assist::validate::DraftCheck;
pub use ledgerly_assist::{
    DraftItem, DraftLine, MappingSuggestion, MatchSuggestion, Proposal, ReportQuery, SourceRef,
    Transcribed, VoucherDraft,
};
pub use ledgerly_gst::recon2b::{DocAmounts, ImsAction, MatchLevel, ReconState};
pub use ledgerly_model::{AiActivity, AiActivityKind, AiFeature};
pub use recon::{Gstr2bReconciliation, Gstr2bRow};
// Report types the app names when it builds and reads reports (card W7). The app depends on this
// crate only, so the report model is re-exported here instead of adding a manifest dependency.
pub use as_voucher::is_gst_tax_ledger;
pub use brs_statement::{BrsLine, BrsStatement};
pub use day_book_meta::EntryOrigin;
pub use error::ApiError;
pub use facade::Api;
pub use gst::{
    Bucket, BucketedVoucher, Gstr1, Gstr1Return, Gstr3bTaxAmounts, Gstr3bWorking,
    InwardReverseCharge, ItcReverseCharge, RulePack,
};
pub use gstr1_sections::{Gstr1Section, TaxableAgreement};
pub use gstr3b_payment::{Gstr3bPayment, HomeGst};
pub use home::{
    CUSTOMER_LATE_DAYS, CashAndBank, HomeFigures, LateBills, MonthSales, SUPPLIER_LATE_DAYS,
};
pub use ledgerly_engine::bills::OpenBill;
pub use ledgerly_engine::upgrade::PredefinedStep;
pub use ledgerly_gst::compute::GstBreakup;
pub use ledgerly_gst::setoff::SetOff;
pub use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, NegativeStockPolicy, Voucher,
};
pub use ledgerly_print::pdf::{PageItem, PageLayout, PrintedDocument};
pub use ledgerly_print::setup::{MAX_COPIES, PageSelection, Paper, PrintSetup};
pub use ledgerly_reports::outstanding::days_overdue;
pub use ledgerly_reports::{
    AuditReportKind, Cell, ColumnHeader, ColumnSpec, Detail, DrillKey, Report, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};
pub use ledgerly_store::backup::{BackupFile, BackupKind, RebuildOutcome, RebuildReport};
pub use ledgerly_store::verify::{ChainProblem, VerifyReport};
pub use ledgerly_tally_bridge::file_import::MasterBehaviour;
pub use maintenance::{VoucherCheck, verify_file};
pub use nudges::{DuplicatePayment, Gstr1Due, HomeNudges, PaymentRef, gstr1_next_due};
pub use report::ExportFormat;
pub use return_against::{InvoicedAgainst, InvoicedLine};
pub use search::{MatchedOn, SearchHit};
pub use tally_file_import::{
    BACKUP_FOLDER, FileImportSummary, ImportException, ImportNote, VoucherBehaviour, XmlImport,
};
pub use tally_import::ImportSummary;
pub use upgrade::{APP_VERSION_KEY, UpgradeReport};
pub use voucher_audit::{ChangeKind, VoucherAudit, VoucherChange, VoucherDetail};
pub use voucher_gst::{EligibleItc, InvoiceGst, PartyBill, RateRow};

pub use ledgerly_bank::brs::Brs;
pub use ledgerly_bank::matcher::{Candidate, LineDirection, MatchRule};
pub use ledgerly_bank::statement::BankStatement;
pub use review::{DraftFigure, ReviewDraft, ReviewItem, ReviewQueue, ReviewSource, UnlistedSource};
