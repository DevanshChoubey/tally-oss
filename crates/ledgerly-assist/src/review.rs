//! Values that need review (coordinator decision 2026-11-27).
//!
//! `ocrs` gives no confidence, so every value it reads carries [`NEEDS_REVIEW_PERMILLE`], the
//! lowest confidence. Such a value is shown with its source but kept by nothing until the person
//! confirms it: [`needs_review`] lists the fields still waiting, and [`confirm`] marks one as the
//! person's (confidence 1000, the source left as the document box it was read from). The value
//! itself is never changed. Confirmation is a human edit; callers log it as one.

use crate::proposal::{Transcribed, VoucherDraft};

/// The confidence of a value that needs review: the lowest there is.
pub const NEEDS_REVIEW_PERMILLE: u16 = 0;

/// The confidence of a value the person confirmed.
const CONFIRMED_PERMILLE: u16 = 1000;

/// A field of a draft voucher that can need review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DraftField {
    /// The voucher date.
    Date,
    /// The party's name as read.
    PartyName,
    /// The supplier's invoice number.
    InvoiceNumber,
    /// The supplier's GSTIN.
    SupplierGstin,
    /// A line's ledger name, by the line's index.
    LineLedger(usize),
    /// A line's amount, by the line's index.
    LineAmount(usize),
    /// The document's printed total.
    DocumentTotal,
    /// The document's printed tax.
    DocumentTax,
}

impl DraftField {
    /// The field's name as the validator reports it in `DraftCheck::Blocked`.
    pub fn name(self) -> String {
        match self {
            DraftField::Date => "date".to_owned(),
            DraftField::PartyName => "party_name".to_owned(),
            DraftField::InvoiceNumber => "invoice_number".to_owned(),
            DraftField::SupplierGstin => "supplier_gstin".to_owned(),
            DraftField::LineLedger(i) => format!("lines[{i}].ledger"),
            DraftField::LineAmount(i) => format!("lines[{i}].amount"),
            DraftField::DocumentTotal => "document_total".to_owned(),
            DraftField::DocumentTax => "document_tax".to_owned(),
        }
    }
}

fn waiting<T>(t: Option<&Transcribed<T>>) -> bool {
    t.is_some_and(|t| t.confidence_permille == NEEDS_REVIEW_PERMILLE)
}

/// The fields of `draft` that need review, in the order a person reads a bill: date, party,
/// invoice number, GSTIN, each line's ledger and amount, then the printed total and tax.
pub fn needs_review(draft: &VoucherDraft) -> Vec<DraftField> {
    let mut out = Vec::new();
    let heads = [
        (waiting(draft.date.as_ref()), DraftField::Date),
        (waiting(draft.party_name.as_ref()), DraftField::PartyName),
        (
            waiting(draft.invoice_number.as_ref()),
            DraftField::InvoiceNumber,
        ),
        (
            waiting(draft.supplier_gstin.as_ref()),
            DraftField::SupplierGstin,
        ),
    ];
    out.extend(heads.into_iter().filter_map(|(w, f)| w.then_some(f)));
    for (i, line) in draft.lines.iter().enumerate() {
        if waiting(Some(&line.ledger_name)) {
            out.push(DraftField::LineLedger(i));
        }
        if waiting(Some(&line.amount)) {
            out.push(DraftField::LineAmount(i));
        }
    }
    if waiting(draft.document_total.as_ref()) {
        out.push(DraftField::DocumentTotal);
    }
    if waiting(draft.document_tax.as_ref()) {
        out.push(DraftField::DocumentTax);
    }
    out
}

fn mark<T>(t: Option<&mut Transcribed<T>>) -> bool {
    match t {
        Some(t) => {
            t.confidence_permille = CONFIRMED_PERMILLE;
            true
        }
        None => false,
    }
}

/// Marks `field` of `draft` as confirmed by the person. Returns `false`, changing nothing, when
/// the draft has no such field.
pub fn confirm(draft: &mut VoucherDraft, field: DraftField) -> bool {
    match field {
        DraftField::Date => mark(draft.date.as_mut()),
        DraftField::PartyName => mark(draft.party_name.as_mut()),
        DraftField::InvoiceNumber => mark(draft.invoice_number.as_mut()),
        DraftField::SupplierGstin => mark(draft.supplier_gstin.as_mut()),
        DraftField::LineLedger(i) => mark(draft.lines.get_mut(i).map(|l| &mut l.ledger_name)),
        DraftField::LineAmount(i) => mark(draft.lines.get_mut(i).map(|l| &mut l.amount)),
        DraftField::DocumentTotal => mark(draft.document_total.as_mut()),
        DraftField::DocumentTax => mark(draft.document_tax.as_mut()),
    }
}
