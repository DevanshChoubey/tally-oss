use ledgerly_core::{Date, DrCr, LedgerId, Paise, Qty, Rate, VoucherId};
use ledgerly_model::BaseVoucherKind;
use serde::{Deserialize, Serialize};

/// What the assistant proposes. Proposals are never posted directly: the engine recomputes and a
/// person confirms (spec 6.8, rule A10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Proposal {
    /// A voucher to review.
    VoucherDraft(Box<VoucherDraft>),
    /// A bank line match.
    MatchSuggestion(MatchSuggestion),
    /// A report to run.
    ReportQuery(ReportQuery),
    /// An import column mapping.
    MappingSuggestion(MappingSuggestion),
}

/// A value read or inferred by the assistant, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcribed<T> {
    /// The value.
    pub value: T,
    /// Where it came from.
    pub source: SourceRef,
    /// Confidence in per-mille (0..=1000). No floats (rule A3).
    pub confidence_permille: u16,
}

impl<T> Transcribed<T> {
    /// Whether the confidence reaches `threshold_permille` (equal counts). Below it the value is
    /// blank: callers treat it as not read.
    pub fn is_confident(&self, threshold_permille: u16) -> bool {
        self.confidence_permille >= threshold_permille
    }
}

/// Where a transcribed value came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceRef {
    /// Text the user typed.
    Typed {
        /// Character range in the input.
        start: usize,
        /// End (exclusive).
        end: usize,
    },
    /// A region of a document.
    Document {
        /// SHA-256 of the document, hex.
        doc_hash: String,
        /// Page (1-based).
        page: u32,
        /// Bounding box `[x, y, width, height]` in pixels.
        bbox: [u32; 4],
    },
    /// A bank statement line.
    StatementLine {
        /// Row (1-based).
        row: u32,
    },
}

/// A proposed voucher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherDraft {
    /// Suggested voucher kind.
    pub kind: BaseVoucherKind,
    /// Date.
    pub date: Option<Transcribed<Date>>,
    /// Party name as read.
    pub party_name: Option<Transcribed<String>>,
    /// Party resolved against masters, when it exists.
    pub party: Option<LedgerId>,
    /// Supplier invoice number.
    pub invoice_number: Option<Transcribed<String>>,
    /// Supplier GSTIN as read (validated before use).
    pub supplier_gstin: Option<Transcribed<String>>,
    /// Accounting lines.
    pub lines: Vec<DraftLine>,
    /// Item lines.
    pub items: Vec<DraftItem>,
    /// Document total as read; checked against the engine's recomputation.
    pub document_total: Option<Transcribed<Paise>>,
    /// Document tax total as read; checked against the engine's recomputation.
    pub document_tax: Option<Transcribed<Paise>>,
    /// Ledgers the user would need to create (never invented ids).
    pub create_ledgers: Vec<String>,
    /// Narration.
    pub narration: String,
}

/// A proposed accounting line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftLine {
    /// Ledger name as read or inferred.
    pub ledger_name: Transcribed<String>,
    /// Ledger resolved against masters, when it exists.
    pub ledger: Option<LedgerId>,
    /// Side.
    pub side: DrCr,
    /// Amount.
    pub amount: Transcribed<Paise>,
}

/// A proposed item line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftItem {
    /// Item name as read.
    pub name: Transcribed<String>,
    /// Quantity.
    pub qty: Option<Transcribed<Qty>>,
    /// Rate per unit, rupees.
    pub rate: Option<Transcribed<Rate>>,
    /// Line amount as read.
    pub amount: Option<Transcribed<Paise>>,
}

/// A proposed bank match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSuggestion {
    /// Statement row (1-based).
    pub statement_row: u32,
    /// Candidate vouchers, best first.
    pub vouchers: Vec<VoucherId>,
    /// Ledger to post to when no voucher exists.
    pub ledger_name: Option<String>,
    /// Why.
    pub reason: String,
    /// Confidence in per-mille.
    pub confidence_permille: u16,
}

/// A report the user asked for in words. The engine runs it; the answer shows this query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportQuery {
    /// Report name, such as `trial_balance` or `ledger_vouchers`.
    pub report: String,
    /// From date.
    pub from: Option<Date>,
    /// To date.
    pub to: Option<Date>,
    /// Ledger names mentioned.
    pub ledger_names: Vec<String>,
}

/// A proposed column mapping for an import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingSuggestion {
    /// `(source column, target field)` pairs.
    pub columns: Vec<(String, String)>,
    /// Confidence in per-mille.
    pub confidence_permille: u16,
}
