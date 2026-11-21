//! Reading bills and invoices. **Card A3** implements this module.
//!
//! Frozen interface (plan card A3):
//!
//! ```text
//! pub fn ocr_to_draft(ocr: &OcrOutput, masters: &Masters) -> Result<VoucherDraft, AssistError>;
//! ```
//!
//! An OCR engine (`PaddleOCR` or Tesseract, spec 6.8) turns a bill image into text lines with a
//! bounding box and a confidence each. That output is [`OcrOutput`]. This module reads it into a
//! [`VoucherDraft`]: a purchase bill from a supplier. It is a transcription step, not a
//! computation (rule A10):
//!
//! - Every value is a [`Transcribed`] value with the document hash, page and the bounding box of
//!   the OCR line it came from, and that line's confidence. A value read with low confidence is
//!   kept with its confidence; the validator blanks it against the caller's threshold.
//! - No number is derived. The document total and the document tax are only ever the figures
//!   printed on the bill (`Total`, `Total Tax`); a tax total is never the sum of the tax lines.
//!   The validator recomputes the figures through the engine and blocks a mismatch.
//! - A GSTIN that fails its format or check character is **rejected**: the draft carries no
//!   supplier GSTIN and [`OcrReading::rejected`] says why. Nothing is auto-corrected (rule A1), so
//!   an OCR confusion such as `O` for `0` is rejected, not repaired.
//! - Ledger and party ids are never invented. They are resolved against the masters only when
//!   exactly one ledger fits; a name with no match goes to `create_ledgers`, and an ambiguous one
//!   is left unresolved for the person to choose.
//!
//! # Layout the reader understands
//!
//! Lines of the form `Label value`, with an optional `:` after the label: `Invoice No`, `Date`,
//! `GSTIN` (or `Supplier GSTIN`), `Supplier` (or `Sold by`, `Vendor`), `Taxable Value`, `CGST`,
//! `SGST`, `UTGST`, `IGST`, `Cess` (each optionally with `@ 9%` before the amount), `Total Tax`
//! and `Total`. Lines that do not start with one of these labels (the buyer's block, item rows,
//! amount in words) are ignored.
//!
//! - A label that occurs on more than one line is **rejected as ambiguous**; the reader neither
//!   picks one nor adds them up (a bill with a tax table per rate needs the item and tax-table
//!   reader, which is pending).
//! - A supplier name is read only from a labelled line; guessing it from the top of the page is
//!   pending.
//! - Dates are day first (`15/03/2026`, `15-03-2026`, `15.03.2026`, `15 Mar 2026`). A two-digit
//!   year or an impossible date is rejected.
//! - Amounts are rupees with at most two decimals, optional `Rs`, `Rs.`, `INR` or the rupee sign,
//!   and commas only in valid Indian or Western grouping. Anything else is rejected, never
//!   rounded.
//! - Item rows are **not** read (pending: no layout provenance yet), so `items` is empty.
//! - A credit note, debit note, proforma invoice, quotation or estimate is
//!   [`AssistError::Unsupported`].

use ledgerly_core::{Date, Decimal, DrCr, Gstin, LedgerId, Paise, Rate, Unsupported};
use ledgerly_model::{BaseVoucherKind, Masters, TaxLedgerKind};
use serde::{Deserialize, Serialize};

use crate::proposal::{DraftLine, SourceRef, Transcribed, VoucherDraft};
use crate::seam::AssistError;

mod values;
use values::{parse_amount_tail, parse_date};

/// One line of text found by an OCR engine, with where it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrLine {
    /// Page (1-based).
    pub page: u32,
    /// The recognised text.
    pub text: String,
    /// Bounding box `[x, y, width, height]` in pixels.
    pub bbox: [u32; 4],
    /// The engine's confidence in per-mille (0..=1000).
    pub confidence_permille: u16,
}

/// What an OCR engine returned for one document. Recorded outputs of this shape are the test
/// fixtures; the engine itself sits behind the `Proposer` seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrOutput {
    /// SHA-256 of the document, hex.
    pub doc_hash: String,
    /// Recognised lines, in any order.
    pub lines: Vec<OcrLine>,
}

/// A field that was found on the document but not used, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedField {
    /// Which field of the draft it would have filled, such as `supplier_gstin`.
    pub field: String,
    /// The text on the document.
    pub text: String,
    /// Why it was not used.
    pub reason: String,
}

/// The draft read from a document, with what was rejected on the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrReading {
    /// The draft to review. Fields that were rejected are absent from it.
    pub draft: VoucherDraft,
    /// Fields found but rejected, in the order they were read.
    pub rejected: Vec<RejectedField>,
}

/// Reads OCR output into a purchase-bill draft, discarding the list of rejected fields. Use
/// [`read_bill`] when the rejections must be shown.
///
/// # Errors
/// [`AssistError::Output`] when the OCR output is malformed (no text, no document hash, or a
/// confidence above 1000 per-mille); [`AssistError::Unsupported`] for a document that is not a
/// tax invoice from a supplier (credit note, debit note, proforma, quotation, estimate).
pub fn ocr_to_draft(ocr: &OcrOutput, masters: &Masters) -> Result<VoucherDraft, AssistError> {
    read_bill(ocr, masters).map(|reading| reading.draft)
}

/// Reads OCR output into a purchase-bill draft and reports every rejected field.
///
/// # Errors
/// As [`ocr_to_draft`].
pub fn read_bill(ocr: &OcrOutput, masters: &Masters) -> Result<OcrReading, AssistError> {
    check_output(ocr)?;
    let mut reader = Reader::new(ocr);
    let invoice_number = reader.text(Field::InvoiceNumber, "invoice_number");
    let date = reader.date();
    let supplier_gstin = reader.gstin();
    let party_name = reader.text(Field::Party, "party_name");
    let taxable = reader.amount(Field::Taxable, "taxable_value");
    let document_total = reader.amount(Field::Total, "document_total");
    let document_tax = reader.amount(Field::TotalTax, "document_tax");
    let taxes = reader.taxes();

    let mut create_ledgers = Vec::new();
    let party = resolve_party(
        masters,
        party_name.as_ref(),
        supplier_gstin.as_ref(),
        &mut reader.rejected,
        &mut create_ledgers,
    );
    let mut lines: Vec<DraftLine> = taxable.map(taxable_line).into_iter().collect();
    lines.extend(tax_lines(masters, taxes, &mut create_ledgers));
    if let (Some(name), Some(total)) = (&party_name, &document_total) {
        lines.push(DraftLine {
            ledger_name: name.clone(),
            ledger: party,
            side: DrCr::Cr,
            amount: total.clone(),
        });
    }
    let narration = invoice_number.as_ref().map_or_else(
        || "Purchase bill".to_owned(),
        |n| format!("Purchase bill {}", n.value),
    );
    let draft = VoucherDraft {
        kind: BaseVoucherKind::Purchase,
        date,
        party_name,
        party,
        invoice_number,
        supplier_gstin,
        lines,
        items: Vec::new(),
        document_total,
        document_tax,
        create_ledgers,
        narration,
    };
    Ok(OcrReading {
        draft,
        rejected: reader.rejected,
    })
}

/// The debit line for the taxable value. Its ledger is left for the person to choose: nothing on
/// the bill says which purchase ledger it belongs to.
fn taxable_line(amount: Transcribed<Paise>) -> DraftLine {
    DraftLine {
        ledger_name: Transcribed {
            value: "Taxable Value".to_owned(),
            source: amount.source.clone(),
            confidence_permille: amount.confidence_permille,
        },
        ledger: None,
        side: DrCr::Dr,
        amount,
    }
}

/// Debit lines for the tax heads read, each on the one tax ledger that fits. A head with no
/// ledger at all is suggested for creation.
fn tax_lines(
    masters: &Masters,
    taxes: Vec<TaxRead>,
    create_ledgers: &mut Vec<String>,
) -> Vec<DraftLine> {
    taxes
        .into_iter()
        .map(|tax| {
            let ledger = match resolve_tax(masters, tax.kind, tax.rate) {
                TaxMatch::One(id) => Some(id),
                TaxMatch::Ambiguous => None,
                TaxMatch::NoMatch => {
                    add_unique(create_ledgers, tax.create_name());
                    None
                }
            };
            DraftLine {
                ledger_name: tax.name,
                ledger,
                side: DrCr::Dr,
                amount: tax.amount,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Checking the OCR output
// ---------------------------------------------------------------------------------------------

/// Document headings that mean the document is not a supplier's tax invoice.
const UNSUPPORTED_HEADINGS: [&str; 5] = [
    "credit note",
    "debit note",
    "proforma",
    "quotation",
    "estimate",
];

/// `lower` starts with `word` and the word ends there (not in the middle of a longer word).
fn starts_with_word(lower: &str, word: &str) -> bool {
    lower.starts_with(word)
        && lower
            .get(word.len()..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|c| !c.is_alphanumeric())
}

fn check_output(ocr: &OcrOutput) -> Result<(), AssistError> {
    if ocr.doc_hash.trim().is_empty() {
        return Err(output_error("the OCR output has no document hash"));
    }
    if ocr.lines.iter().all(|l| l.text.trim().is_empty()) {
        return Err(output_error("the OCR engine found no text on the document"));
    }
    if let Some(bad) = ocr.lines.iter().find(|l| l.confidence_permille > 1000) {
        return Err(output_error(format!(
            "an OCR line has confidence {} per-mille, above 1000",
            bad.confidence_permille
        )));
    }
    for line in &ocr.lines {
        let lower = line.text.trim().to_ascii_lowercase();
        if let Some(heading) = UNSUPPORTED_HEADINGS
            .iter()
            .find(|h| starts_with_word(&lower, h))
        {
            return Err(AssistError::Unsupported(Unsupported {
                what: format!("reading a document headed '{heading}'"),
                reason: "only a supplier's tax invoice is read into a purchase draft; enter this \
                         document in the voucher form"
                    .to_owned(),
            }));
        }
    }
    Ok(())
}

fn output_error(reason: impl Into<String>) -> AssistError {
    AssistError::Output {
        reason: reason.into(),
    }
}

// ---------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------

/// What a labelled line holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    InvoiceNumber,
    Date,
    SupplierGstin,
    Party,
    Taxable,
    Tax(TaxLedgerKind),
    TotalTax,
    Total,
}

/// Label (lower case) to field. The longest matching label wins, so `total tax` beats `total`.
const LABELS: &[(&str, Field)] = &[
    ("invoice no", Field::InvoiceNumber),
    ("invoice number", Field::InvoiceNumber),
    ("inv no", Field::InvoiceNumber),
    ("bill no", Field::InvoiceNumber),
    ("invoice date", Field::Date),
    ("bill date", Field::Date),
    ("dated", Field::Date),
    ("date", Field::Date),
    ("gstin", Field::SupplierGstin),
    ("gstin/uin", Field::SupplierGstin),
    ("gst no", Field::SupplierGstin),
    ("supplier gstin", Field::SupplierGstin),
    ("supplier", Field::Party),
    ("sold by", Field::Party),
    ("vendor", Field::Party),
    ("taxable value", Field::Taxable),
    ("taxable amount", Field::Taxable),
    ("total taxable value", Field::Taxable),
    ("cgst", Field::Tax(TaxLedgerKind::Cgst)),
    ("sgst", Field::Tax(TaxLedgerKind::Sgst)),
    ("utgst", Field::Tax(TaxLedgerKind::Utgst)),
    ("igst", Field::Tax(TaxLedgerKind::Igst)),
    ("cess", Field::Tax(TaxLedgerKind::Cess)),
    ("total tax", Field::TotalTax),
    ("total tax amount", Field::TotalTax),
    ("total gst", Field::TotalTax),
    ("total", Field::Total),
    ("total amount", Field::Total),
    ("grand total", Field::Total),
    ("invoice total", Field::Total),
    ("invoice value", Field::Total),
];

/// The tax heads read, in draft order: kind, draft field name, label used in suggestions.
const TAX_HEADS: [(TaxLedgerKind, &str, &str); 5] = [
    (TaxLedgerKind::Cgst, "cgst", "CGST"),
    (TaxLedgerKind::Sgst, "sgst", "SGST"),
    (TaxLedgerKind::Utgst, "utgst", "UTGST"),
    (TaxLedgerKind::Igst, "igst", "IGST"),
    (TaxLedgerKind::Cess, "cess", "Cess"),
];

/// A line that starts with a known label.
#[derive(Debug, Clone, Copy)]
struct Hit<'a> {
    field: Field,
    line: &'a OcrLine,
    /// The label as printed on the document.
    label: &'a str,
    /// Text after the label, separators removed.
    tail: &'a str,
}

/// The field a line is labelled as, with the label as printed and the text after it. A minus sign
/// is never stripped: a negative amount must reach the amount parser and be rejected there.
fn match_label(text: &str) -> Option<(Field, &str, &str)> {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    let mut best: Option<(Field, usize)> = None;
    for &(label, field) in LABELS {
        let longer = best.is_none_or(|(_, len)| label.len() > len);
        if longer && starts_with_word(&lower, label) {
            best = Some((field, label.len()));
        }
    }
    let (field, len) = best?;
    let label = text.get(..len)?;
    let tail = text
        .get(len..)?
        .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ':' | '.' | '#'))
        .trim_end();
    Some((field, label, tail))
}

// ---------------------------------------------------------------------------------------------
// Reading fields
// ---------------------------------------------------------------------------------------------

/// A tax line read from the document.
struct TaxRead {
    kind: TaxLedgerKind,
    label: &'static str,
    rate: Option<Decimal>,
    name: Transcribed<String>,
    amount: Transcribed<Paise>,
}

impl TaxRead {
    /// The name suggested when no tax ledger exists: the label, with the rate when it was read.
    fn create_name(&self) -> String {
        match self.rate {
            Some(rate) => format!("{} @ {}%", self.label, rate.normalize()),
            None => self.label.to_owned(),
        }
    }
}

struct Reader<'a> {
    doc_hash: &'a str,
    hits: Vec<Hit<'a>>,
    rejected: Vec<RejectedField>,
}

impl<'a> Reader<'a> {
    fn new(ocr: &'a OcrOutput) -> Self {
        let hits = ocr
            .lines
            .iter()
            .filter_map(|line| {
                let (field, label, tail) = match_label(&line.text)?;
                Some(Hit {
                    field,
                    line,
                    label,
                    tail,
                })
            })
            .collect();
        Reader {
            doc_hash: &ocr.doc_hash,
            hits,
            rejected: Vec::new(),
        }
    }

    fn reject(&mut self, field: &str, text: &str, reason: impl Into<String>) {
        self.rejected.push(RejectedField {
            field: field.to_owned(),
            text: text.to_owned(),
            reason: reason.into(),
        });
    }

    fn transcribe<T>(&self, value: T, line: &OcrLine) -> Transcribed<T> {
        Transcribed {
            value,
            source: SourceRef::Document {
                doc_hash: self.doc_hash.to_owned(),
                page: line.page,
                bbox: line.bbox,
            },
            confidence_permille: line.confidence_permille,
        }
    }

    /// The only line with this field. Two or more lines are rejected, never chosen between.
    fn pick(&mut self, field: Field, name: &str) -> Option<Hit<'a>> {
        let mut found = self.hits.iter().filter(|h| h.field == field);
        let first = found.next().copied()?;
        let count = 1 + found.count();
        if count > 1 {
            self.reject(
                name,
                first.line.text.as_str(),
                format!("{count} lines carry this label; the reader does not choose between them"),
            );
            return None;
        }
        Some(first)
    }

    fn text(&mut self, field: Field, name: &str) -> Option<Transcribed<String>> {
        let hit = self.pick(field, name)?;
        if hit.tail.is_empty() {
            self.reject(name, hit.line.text.as_str(), "nothing follows the label");
            return None;
        }
        Some(self.transcribe(hit.tail.to_owned(), hit.line))
    }

    fn date(&mut self) -> Option<Transcribed<Date>> {
        let hit = self.pick(Field::Date, "date")?;
        match parse_date(hit.tail) {
            Ok(date) => Some(self.transcribe(date, hit.line)),
            Err(reason) => {
                self.reject("date", hit.line.text.as_str(), reason);
                None
            }
        }
    }

    fn gstin(&mut self) -> Option<Transcribed<String>> {
        let hit = self.pick(Field::SupplierGstin, "supplier_gstin")?;
        match Gstin::parse(hit.tail) {
            Ok(gstin) => Some(self.transcribe(gstin.as_str().to_owned(), hit.line)),
            Err(e) => {
                self.reject(
                    "supplier_gstin",
                    hit.line.text.as_str(),
                    format!("not a valid GSTIN: {e}"),
                );
                None
            }
        }
    }

    fn amount(&mut self, field: Field, name: &str) -> Option<Transcribed<Paise>> {
        let hit = self.pick(field, name)?;
        match parse_amount_tail(hit.tail, false) {
            Ok((paise, _)) => Some(self.transcribe(paise, hit.line)),
            Err(reason) => {
                self.reject(name, hit.line.text.as_str(), reason);
                None
            }
        }
    }

    fn taxes(&mut self) -> Vec<TaxRead> {
        let mut taxes = Vec::new();
        for (kind, name, label) in TAX_HEADS {
            let Some(hit) = self.pick(Field::Tax(kind), name) else {
                continue;
            };
            match parse_amount_tail(hit.tail, true) {
                Ok((paise, rate)) => taxes.push(TaxRead {
                    kind,
                    label,
                    rate,
                    name: self.transcribe(hit.label.to_owned(), hit.line),
                    amount: self.transcribe(paise, hit.line),
                }),
                Err(reason) => self.reject(name, hit.line.text.as_str(), reason),
            }
        }
        taxes
    }
}

// ---------------------------------------------------------------------------------------------
// Resolving against masters
// ---------------------------------------------------------------------------------------------

fn add_unique(list: &mut Vec<String>, name: String) {
    if !list.contains(&name) {
        list.push(name);
    }
}

fn normalise(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// How the party read from the bill relates to the party ledgers.
#[derive(Debug, PartialEq, Eq)]
enum PartyMatch {
    Resolved(LedgerId),
    /// The name and the GSTIN point at different ledgers.
    Conflict,
    /// More than one ledger fits.
    Ambiguous,
    NoMatch,
}

fn classify_party(by_gstin: &[LedgerId], by_name: &[LedgerId]) -> PartyMatch {
    match (by_gstin, by_name) {
        ([g], names) if names.is_empty() || names.contains(g) => PartyMatch::Resolved(*g),
        ([_], _) => PartyMatch::Conflict,
        ([], [n]) => PartyMatch::Resolved(*n),
        ([], []) => PartyMatch::NoMatch,
        _ => PartyMatch::Ambiguous,
    }
}

/// Party ledgers whose GSTIN on file is the one read from the bill.
fn party_ids_by_gstin(masters: &Masters, gstin: &Gstin) -> Vec<LedgerId> {
    masters
        .ledgers
        .iter()
        .filter(|(_, l)| {
            l.party
                .as_ref()
                .and_then(|p| p.gstin.as_ref())
                .is_some_and(|on_file| on_file == gstin)
        })
        .map(|(id, _)| *id)
        .collect()
}

/// Party ledgers whose name or alias is the name read from the bill (case and spacing ignored).
fn party_ids_by_name(masters: &Masters, name: &str) -> Vec<LedgerId> {
    let wanted = normalise(name);
    masters
        .ledgers
        .iter()
        .filter(|(_, l)| l.party.is_some())
        .filter(|(_, l)| {
            normalise(&l.name) == wanted || l.aliases.iter().any(|a| normalise(a) == wanted)
        })
        .map(|(id, _)| *id)
        .collect()
}

/// Finds the party ledger. A valid GSTIN on file is a stronger key than a name. Nothing is
/// invented: no match means the party name is suggested for creation.
fn resolve_party(
    masters: &Masters,
    name: Option<&Transcribed<String>>,
    gstin: Option<&Transcribed<String>>,
    rejected: &mut Vec<RejectedField>,
    create_ledgers: &mut Vec<String>,
) -> Option<LedgerId> {
    let by_gstin = gstin
        .and_then(|g| Gstin::parse(&g.value).ok())
        .map_or_else(Vec::new, |g| party_ids_by_gstin(masters, &g));
    let by_name = name.map_or_else(Vec::new, |n| party_ids_by_name(masters, &n.value));
    match classify_party(&by_gstin, &by_name) {
        PartyMatch::Resolved(id) => Some(id),
        PartyMatch::Conflict => {
            rejected.push(RejectedField {
                field: "party".to_owned(),
                text: name.map_or_else(String::new, |n| n.value.clone()),
                reason: "the party name and the GSTIN on the bill match different ledgers; \
                         choose the party yourself"
                    .to_owned(),
            });
            None
        }
        PartyMatch::Ambiguous => None,
        PartyMatch::NoMatch => {
            if let Some(n) = name {
                add_unique(create_ledgers, n.value.clone());
            }
            None
        }
    }
}

/// How a tax head on the bill relates to the tax ledgers.
#[derive(Debug, PartialEq, Eq)]
enum TaxMatch {
    One(LedgerId),
    Ambiguous,
    NoMatch,
}

/// Finds the tax ledger of this kind. With a rate read from the bill, a ledger fixed to that rate
/// is preferred, else a ledger with no fixed rate; a ledger fixed to another rate never fits.
fn resolve_tax(masters: &Masters, kind: TaxLedgerKind, rate: Option<Decimal>) -> TaxMatch {
    let of_kind: Vec<(LedgerId, Option<Rate>)> = masters
        .ledgers
        .iter()
        .filter_map(|(id, l)| {
            l.tax
                .filter(|t| t.kind == kind)
                .map(|t| (*id, t.rate_percent))
        })
        .collect();
    let ids_where = |keep: &dyn Fn(Option<Rate>) -> bool| -> Vec<LedgerId> {
        of_kind
            .iter()
            .filter(|entry| keep(entry.1))
            .map(|entry| entry.0)
            .collect()
    };
    let pool = match rate {
        Some(wanted) => {
            let exact = ids_where(&|r| r == Some(Rate(wanted)));
            if exact.is_empty() {
                ids_where(&|r| r.is_none())
            } else {
                exact
            }
        }
        None => ids_where(&|_| true),
    };
    match pool.as_slice() {
        [one] => TaxMatch::One(*one),
        [] => TaxMatch::NoMatch,
        _ => TaxMatch::Ambiguous,
    }
}
