//! GSTR-2B / IMS reconciliation. **Card G11** implements this module.
//!
//! Frozen interface (plan card G11):
//!
//! ```text
//! pub enum MatchLevel { Exact, Normalised, Tolerance, Unmatched }
//! pub enum ImsAction { Accept, Reject, Pending }
//! pub struct ReconLine { pub portal_index: usize, pub voucher: Option<VoucherId>, pub level: MatchLevel, pub suggested: ImsAction, pub confirmed: bool }
//! pub fn match_2b(books: &Books, gstin: &Gstin, data: &Gstr2b, tolerance: Paise) -> Result<Vec<ReconLine>, GstError>;
//! ```
//!
//! # What is matched
//!
//! Each portal document (a GSTR-2B invoice, credit note or debit note) is matched against the
//! regular purchase-side vouchers of the books (spec 6.3: exact, then fuzzy invoice number
//! normalisation, then amount/date tolerance). A book voucher is a candidate only when its
//! party ledger carries the supplier's GSTIN, so a number match under another supplier is never
//! a match. The supplier's document number is the voucher's `reference`; the date is
//! `reference_date`, or the voucher date when that is absent.
//!
//! Which book voucher kinds pair with which portal documents is `pending` observed (policy,
//! not statute): an invoice pairs with a purchase voucher, a supplier credit note with a debit
//! note voucher, a supplier debit note with a credit note or purchase voucher.
//!
//! The amount compared is the **total tax** (IGST + CGST + SGST/UTGST + cess) of the portal
//! document against the tax lines stored on the voucher. When either side has no tax split (a
//! portal document without item lines, a voucher without stored GST) the amounts are not
//! comparable, and the pair is never reported as a match: it stays `Unmatched`.
//!
//! # Levels, in the order tried
//!
//! 1. [`MatchLevel::Exact`]: same supplier, same document number byte for byte, tax equal to the
//!    paisa.
//! 2. [`MatchLevel::Normalised`]: numbers equal after upper-casing and dropping every character
//!    that is not an ASCII letter or digit (so `INV-001` and `INV/001` agree), tax equal.
//! 3. [`MatchLevel::Tolerance`]: normalised numbers agree and the tax differs by no more than
//!    the tolerance; or the numbers disagree but the dates are equal and the tax differs by no
//!    more than the tolerance.
//!
//! A level is assigned only when the pair is unambiguous. When one portal document fits several
//! book vouchers, or several portal documents fit one book voucher, none of them is matched. A
//! document that is ambiguous at one level still counts toward the claims on every book it fits at
//! the later levels, so no other document can take one of those books uniquely and be suggested
//! Accept.
//!
//! # Suggestions
//!
//! `Exact` and `Normalised` suggest [`ImsAction::Accept`]; `Tolerance` suggests
//! [`ImsAction::Pending`] because the user must look at the difference; `Unmatched` suggests
//! [`ImsAction::Pending`] (a document missing from the books may simply be unbooked), except
//! that a document whose number matches a **cancelled** voucher of the same supplier suggests
//! [`ImsAction::Reject`]; that line carries the cancelled voucher's id with level `Unmatched`.
//! Every line has `confirmed == false`: nothing is accepted without the user (spec 6.3).

use ledgerly_core::Outcome;
use ledgerly_core::{Date, Gstin, Paise, VoucherId};
use ledgerly_gst_formats::gstr2b::{Gstr2b, InwardDoc, InwardDocKind};
use ledgerly_model::{BaseVoucherKind, Books, TaxLedgerKind, VoucherStatus};

use crate::GstError;

/// How well a portal document matched a book voucher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchLevel {
    /// Same supplier, same number, same tax to the paisa.
    Exact,
    /// Same supplier and tax; numbers equal after normalisation only.
    Normalised,
    /// Within the tolerance on tax, by normalised number or by date.
    Tolerance,
    /// No unambiguous book voucher.
    Unmatched,
}

/// The action suggested on the portal's Invoice Management System. Never applied without the
/// user's confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImsAction {
    /// Accept the document.
    Accept,
    /// Reject the document.
    Reject,
    /// Leave the document pending.
    Pending,
}

/// The reconciliation outcome for one portal document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconLine {
    /// Position of the document in [`portal_documents`].
    pub portal_index: usize,
    /// The book voucher matched, or for a rejection suggestion the cancelled voucher.
    pub voucher: Option<VoucherId>,
    /// How well it matched.
    pub level: MatchLevel,
    /// The suggested action.
    pub suggested: ImsAction,
    /// Whether the user has confirmed the suggestion. Always `false` as returned here.
    pub confirmed: bool,
}

/// The portal documents in the order [`ReconLine::portal_index`] refers to: every `b2b` invoice
/// (supplier by supplier, in file order), then every `cdnr` note. Each is paired with its
/// supplier's GSTIN as written in the file.
#[must_use]
pub fn portal_documents(data: &Gstr2b) -> Vec<(&str, &InwardDoc)> {
    data.b2b
        .iter()
        .chain(data.cdnr.iter())
        .flat_map(|group| group.docs.iter().map(move |d| (group.ctin.as_str(), d)))
        .collect()
}

/// A portal document prepared for comparison.
struct PortalDoc {
    supplier: String,
    kind: InwardDocKind,
    number: String,
    norm: Option<String>,
    date: Date,
    taxable: Option<Paise>,
    tax: Option<Paise>,
}

/// A book voucher prepared for comparison.
struct BookDoc {
    id: VoucherId,
    kind: BaseVoucherKind,
    supplier: String,
    number: Option<String>,
    norm: Option<String>,
    date: Date,
    taxable: Option<Paise>,
    tax: Option<Paise>,
}

#[derive(Clone, Copy)]
enum Rank {
    Exact,
    Normalised,
    NumberTolerance,
    DateTolerance,
}

const RANKS: [Rank; 4] = [
    Rank::Exact,
    Rank::Normalised,
    Rank::NumberTolerance,
    Rank::DateTolerance,
];

fn level_of(rank: Rank) -> MatchLevel {
    match rank {
        Rank::Exact => MatchLevel::Exact,
        Rank::Normalised => MatchLevel::Normalised,
        Rank::NumberTolerance | Rank::DateTolerance => MatchLevel::Tolerance,
    }
}

fn suggestion_for(level: MatchLevel) -> ImsAction {
    match level {
        MatchLevel::Exact | MatchLevel::Normalised => ImsAction::Accept,
        MatchLevel::Tolerance | MatchLevel::Unmatched => ImsAction::Pending,
    }
}

/// Upper-cases and keeps only ASCII letters and digits; `None` when nothing is left.
fn normalise(number: &str) -> Option<String> {
    let s: String = number
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if s.is_empty() { None } else { Some(s) }
}

fn same_gstin(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn kind_pairs(portal: InwardDocKind, book: BaseVoucherKind) -> bool {
    match portal {
        InwardDocKind::Invoice => matches!(book, BaseVoucherKind::Purchase),
        InwardDocKind::CreditNote => matches!(book, BaseVoucherKind::DebitNote),
        InwardDocKind::DebitNote => {
            matches!(
                book,
                BaseVoucherKind::CreditNote | BaseVoucherKind::Purchase
            )
        }
    }
}

fn is_purchase_side(kind: BaseVoucherKind) -> bool {
    matches!(
        kind,
        BaseVoucherKind::Purchase | BaseVoucherKind::DebitNote | BaseVoucherKind::CreditNote
    )
}

fn is_gst_head(kind: TaxLedgerKind) -> bool {
    matches!(
        kind,
        TaxLedgerKind::Cgst
            | TaxLedgerKind::Sgst
            | TaxLedgerKind::Utgst
            | TaxLedgerKind::Igst
            | TaxLedgerKind::Cess
    )
}

/// Total of the GST heads stored on a voucher, `None` when the voucher has no stored GST.
fn voucher_tax(v: &ledgerly_model::Voucher) -> Result<Option<Paise>, GstError> {
    let Some(gst) = &v.gst else {
        return Ok(None);
    };
    let mut total = Paise::ZERO;
    for line in &gst.lines {
        if is_gst_head(line.kind) {
            total = total.checked_add(line.tax)?;
        }
    }
    Ok(Some(total))
}

/// Taxable value stored on a voucher: the taxable value of its IGST and CGST lines (a CGST line
/// and its SGST or UTGST twin carry the same value, so only one of the pair is counted). `None`
/// when the voucher has no stored GST or no such line, because then no taxable value is known.
fn voucher_taxable(v: &ledgerly_model::Voucher) -> Result<Option<Paise>, GstError> {
    let Some(gst) = &v.gst else {
        return Ok(None);
    };
    let mut total = Paise::ZERO;
    let mut any = false;
    for line in &gst.lines {
        if matches!(line.kind, TaxLedgerKind::Igst | TaxLedgerKind::Cgst) {
            total = total.checked_add(line.taxable)?;
            any = true;
        }
    }
    Ok(any.then_some(total))
}

/// The books side: regular vouchers as candidates, cancelled ones kept apart.
fn book_docs(books: &Books) -> Result<(Vec<BookDoc>, Vec<BookDoc>), GstError> {
    let mut active = Vec::new();
    let mut cancelled = Vec::new();
    for v in books.vouchers.values() {
        let target = match v.status {
            VoucherStatus::Regular => &mut active,
            VoucherStatus::Cancelled => &mut cancelled,
            // Optional and post-dated vouchers are not in the books yet.
            VoucherStatus::Optional | VoucherStatus::PostDated => continue,
        };
        let vt = books
            .masters
            .voucher_types
            .get(&v.type_id)
            .ok_or_else(|| GstError::NotFound {
                what: format!("voucher type {} of voucher {}", v.type_id.0, v.id.0),
            })?;
        if !is_purchase_side(vt.base) {
            continue;
        }
        let Some(party_id) = v.party else {
            continue;
        };
        let ledger = books
            .masters
            .ledgers
            .get(&party_id)
            .ok_or_else(|| GstError::NotFound {
                what: format!("party ledger {} of voucher {}", party_id.0, v.id.0),
            })?;
        let Some(gstin) = ledger.party.as_ref().and_then(|p| p.gstin.as_ref()) else {
            continue;
        };
        target.push(BookDoc {
            id: v.id,
            kind: vt.base,
            supplier: gstin.as_str().to_owned(),
            norm: v.reference.as_deref().and_then(normalise),
            number: v.reference.clone(),
            date: v.reference_date.unwrap_or(v.date),
            taxable: voucher_taxable(v)?,
            tax: voucher_tax(v)?,
        });
    }
    Ok((active, cancelled))
}

fn portal_doc(ctin: &str, d: &InwardDoc) -> Result<PortalDoc, GstError> {
    let (taxable, tax) = match d.tax_totals()? {
        Outcome::Value(t) => (Some(t.taxable), Some(t.checked_total_tax()?)),
        Outcome::Unsupported(_) => (None, None),
    };
    Ok(PortalDoc {
        supplier: ctin.to_owned(),
        kind: d.kind,
        number: d.number.clone(),
        norm: normalise(&d.number),
        date: d.date,
        taxable,
        tax,
    })
}

/// Absolute tax difference in paise, `None` when either side has no tax split.
fn tax_difference(doc: &PortalDoc, book: &BookDoc) -> Result<Option<i64>, GstError> {
    match (doc.tax, book.tax) {
        (Some(a), Some(b)) => Ok(Some(a.checked_sub(b)?.checked_abs()?.get())),
        (None, _) | (_, None) => Ok(None),
    }
}

fn fits(rank: Rank, doc: &PortalDoc, book: &BookDoc, tolerance: Paise) -> Result<bool, GstError> {
    if !same_gstin(&doc.supplier, &book.supplier) || !kind_pairs(doc.kind, book.kind) {
        return Ok(false);
    }
    let Some(diff) = tax_difference(doc, book)? else {
        return Ok(false);
    };
    let norm_equal = doc.norm.is_some() && doc.norm == book.norm;
    let within = diff <= tolerance.get();
    Ok(match rank {
        Rank::Exact => book.number.as_deref() == Some(doc.number.as_str()) && diff == 0,
        Rank::Normalised => norm_equal && diff == 0,
        Rank::NumberTolerance => norm_equal && within,
        Rank::DateTolerance => book.date == doc.date && within,
    })
}

/// Result of matching, per portal document.
struct Assignment {
    matched: Vec<Option<(usize, MatchLevel)>>,
    ambiguous: Vec<bool>,
}

fn assign(docs: &[PortalDoc], books: &[BookDoc], tolerance: Paise) -> Result<Assignment, GstError> {
    let mut matched: Vec<Option<(usize, MatchLevel)>> = vec![None; docs.len()];
    let mut ambiguous = vec![false; docs.len()];
    let mut used = vec![false; books.len()];
    for rank in RANKS {
        let mut candidates: Vec<Vec<usize>> = Vec::with_capacity(docs.len());
        let mut claims = vec![0usize; books.len()];
        for (i, doc) in docs.iter().enumerate() {
            // A document that is ambiguous at an earlier level still counts its candidates here.
            // The books it fits are then reserved: another document cannot take one of them
            // uniquely at a later level. It is never assigned (see the loop below).
            let unmatched = matched.get(i).is_some_and(Option::is_none);
            let list = if unmatched {
                candidates_for(rank, doc, books, &used, tolerance)?
            } else {
                Vec::new()
            };
            for &j in &list {
                if let Some(c) = claims.get_mut(j) {
                    *c += 1;
                }
            }
            candidates.push(list);
        }
        for (i, list) in candidates.iter().enumerate() {
            if ambiguous.get(i).copied() == Some(true) {
                continue;
            }
            match list.as_slice() {
                [] => {}
                [j] if claims.get(*j).copied() == Some(1) => {
                    set(&mut matched, i, Some((*j, level_of(rank))));
                    set(&mut used, *j, true);
                }
                _ => set(&mut ambiguous, i, true),
            }
        }
    }
    Ok(Assignment { matched, ambiguous })
}

/// Indices of the unused book vouchers that fit `doc` at `rank`.
fn candidates_for(
    rank: Rank,
    doc: &PortalDoc,
    books: &[BookDoc],
    used: &[bool],
    tolerance: Paise,
) -> Result<Vec<usize>, GstError> {
    let mut list = Vec::new();
    for (j, book) in books.iter().enumerate() {
        let free = !used.get(j).copied().unwrap_or(true);
        if free && fits(rank, doc, book, tolerance)? {
            list.push(j);
        }
    }
    Ok(list)
}

/// Sets `items[index]` when the index exists.
fn set<T>(items: &mut [T], index: usize, value: T) {
    if let Some(slot) = items.get_mut(index) {
        *slot = value;
    }
}

/// The cancelled voucher of the same supplier and kind with the same normalised number.
fn cancelled_twin(doc: &PortalDoc, cancelled: &[BookDoc]) -> Option<VoucherId> {
    let norm = doc.norm.as_ref()?;
    cancelled
        .iter()
        .find(|b| {
            same_gstin(&doc.supplier, &b.supplier)
                && kind_pairs(doc.kind, b.kind)
                && b.norm.as_ref() == Some(norm)
        })
        .map(|b| b.id)
}

/// Everything the matcher worked out, for [`match_2b`] and [`reconcile_2b`].
struct Analysis {
    docs: Vec<PortalDoc>,
    active: Vec<BookDoc>,
    cancelled: Vec<BookDoc>,
    result: Assignment,
}

fn analyse(
    books: &Books,
    gstin: &Gstin,
    data: &Gstr2b,
    tolerance: Paise,
) -> Result<Analysis, GstError> {
    if !same_gstin(&data.gstin, gstin.as_str()) {
        return Err(GstError::Invalid {
            field: "gstin".to_owned(),
            reason: format!(
                "the statement is for {} but the reconciliation is for {gstin}",
                data.gstin
            ),
        });
    }
    if tolerance.is_negative() {
        return Err(GstError::Invalid {
            field: "tolerance".to_owned(),
            reason: "the tolerance cannot be negative".to_owned(),
        });
    }
    let docs = portal_documents(data)
        .into_iter()
        .map(|(ctin, d)| portal_doc(ctin, d))
        .collect::<Result<Vec<_>, _>>()?;
    let (active, cancelled) = book_docs(books)?;
    let result = assign(&docs, &active, tolerance)?;
    Ok(Analysis {
        docs,
        active,
        cancelled,
        result,
    })
}

fn lines_of(a: &Analysis) -> Vec<ReconLine> {
    let mut lines = Vec::with_capacity(a.docs.len());
    for (i, doc) in a.docs.iter().enumerate() {
        let hit = a.result.matched.get(i).copied().flatten();
        let line = if let Some((j, level)) = hit {
            ReconLine {
                portal_index: i,
                voucher: a.active.get(j).map(|b| b.id),
                level,
                suggested: suggestion_for(level),
                confirmed: false,
            }
        } else {
            let ambiguous = a.result.ambiguous.get(i).copied().unwrap_or(false);
            let twin = if ambiguous {
                None
            } else {
                cancelled_twin(doc, &a.cancelled)
            };
            ReconLine {
                portal_index: i,
                voucher: twin,
                level: MatchLevel::Unmatched,
                suggested: if twin.is_some() {
                    ImsAction::Reject
                } else {
                    ImsAction::Pending
                },
                confirmed: false,
            }
        };
        lines.push(line);
    }
    lines
}

/// Matches the portal's GSTR-2B documents against the books and suggests an IMS action for each.
///
/// Returns one [`ReconLine`] per portal document, in [`portal_documents`] order, all with
/// `confirmed == false`. `tolerance` is the largest tax difference, in paise, still reported as
/// [`MatchLevel::Tolerance`].
///
/// # Errors
/// [`GstError::Invalid`] when `gstin` is not the GSTIN the statement was downloaded for, or the
/// tolerance is negative; [`GstError::NotFound`] when a voucher refers to a voucher type or party
/// ledger that does not exist; [`GstError::Money`] on arithmetic overflow.
pub fn match_2b(
    books: &Books,
    gstin: &Gstin,
    data: &Gstr2b,
    tolerance: Paise,
) -> Result<Vec<ReconLine>, GstError> {
    Ok(lines_of(&analyse(books, gstin, data, tolerance)?))
}

/// How the portal document compares with the books, for the screen's "Engine status" column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReconState {
    /// Same voucher, same taxable value and tax, same date.
    Matched,
    /// A voucher with the same supplier and number, but the taxable value or the tax differs.
    ValueDiffers,
    /// A voucher with equal amounts, but the date differs.
    DateDiffers,
    /// No voucher of the books fits the document.
    NotInBooks,
    /// More than one voucher fits the document, so none is chosen.
    PossibleDuplicate,
    /// A voucher fits by number, but one side has no tax split, so the amounts cannot be compared.
    NotComparable,
}

/// The amounts and date of one side of a reconciliation row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocAmounts {
    /// The document date: the voucher's reference date (or voucher date), or the 2B date.
    pub date: Date,
    /// Taxable value, `None` when this side has no tax split.
    pub taxable: Option<Paise>,
    /// Total tax (IGST, CGST, SGST/UTGST and cess), `None` when this side has no tax split.
    pub tax: Option<Paise>,
    /// Taxable value plus tax, `None` when either is unknown.
    pub total: Option<Paise>,
}

/// One row of the 2B reconciliation: the matcher's line and the amounts it was made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconEntry {
    /// The matcher's line (level, suggestion, voucher).
    pub line: ReconLine,
    /// How the document compares with the books.
    pub state: ReconState,
    /// The books side, `None` when no voucher is paired.
    pub books: Option<DocAmounts>,
    /// The 2B side.
    pub portal: DocAmounts,
    /// 2B total less books total, `None` unless both are known.
    pub difference: Option<Paise>,
}

/// Rows for the reconciliation screen: [`match_2b`]'s lines with the amounts and a state each.
///
/// A document the matcher leaves unmatched is still paired with a voucher of the same supplier
/// that carries the same number (after normalisation), when exactly one such voucher is free:
/// that is a document whose value differs, not one that is missing from the books. Several such
/// vouchers make the document a possible duplicate and none is chosen. A pair is `Matched` only
/// when taxable value, tax and date are all equal; amounts are never compared when one side has
/// no tax split ([`ReconState::NotComparable`]).
///
/// The rows follow the portal documents. Vouchers of the books that no document names are not
/// listed.
///
/// # Errors
/// As [`match_2b`].
pub fn reconcile_2b(
    books: &Books,
    gstin: &Gstin,
    data: &Gstr2b,
    tolerance: Paise,
) -> Result<Vec<ReconEntry>, GstError> {
    let analysis = analyse(books, gstin, data, tolerance)?;
    let lines = lines_of(&analysis);
    let used: Vec<VoucherId> = lines
        .iter()
        .filter(|l| l.level != MatchLevel::Unmatched)
        .filter_map(|l| l.voucher)
        .collect();
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let doc = analysis
            .docs
            .get(line.portal_index)
            .ok_or_else(|| GstError::NotFound {
                what: format!("portal document {}", line.portal_index),
            })?;
        let portal = amounts(doc.date, doc.taxable, doc.tax)?;
        let ambiguous = analysis
            .result
            .ambiguous
            .get(line.portal_index)
            .copied()
            .unwrap_or(false);
        let (line, book) = if line.level != MatchLevel::Unmatched {
            let book = line
                .voucher
                .and_then(|id| analysis.active.iter().find(|b| b.id == id));
            (line, book)
        } else if ambiguous || line.voucher.is_some() {
            // Ambiguous, or the document names a cancelled voucher: nothing is paired.
            (line, None)
        } else {
            let free: Vec<&BookDoc> = analysis
                .active
                .iter()
                .filter(|b| {
                    !used.contains(&b.id)
                        && same_gstin(&doc.supplier, &b.supplier)
                        && kind_pairs(doc.kind, b.kind)
                        && doc.norm.is_some()
                        && doc.norm == b.norm
                })
                .collect();
            match free.as_slice() {
                [only] => (
                    ReconLine {
                        voucher: Some(only.id),
                        ..line
                    },
                    Some(*only),
                ),
                _ => (line, None),
            }
        };
        let (state, books_side) = match book {
            Some(b) => {
                let side = amounts(b.date, b.taxable, b.tax)?;
                (pair_state(&portal, &side), Some(side))
            }
            None if ambiguous || many_free(&analysis, doc, &used) => {
                (ReconState::PossibleDuplicate, None)
            }
            None => (ReconState::NotInBooks, None),
        };
        let difference = match (portal.total, books_side.and_then(|b| b.total)) {
            (Some(p), Some(b)) => Some(p.checked_sub(b)?),
            _ => None,
        };
        out.push(ReconEntry {
            line,
            state,
            books: books_side,
            portal,
            difference,
        });
    }
    Ok(out)
}

/// `true` when more than one free voucher carries the document's number.
fn many_free(analysis: &Analysis, doc: &PortalDoc, used: &[VoucherId]) -> bool {
    analysis
        .active
        .iter()
        .filter(|b| {
            !used.contains(&b.id)
                && same_gstin(&doc.supplier, &b.supplier)
                && kind_pairs(doc.kind, b.kind)
                && doc.norm.is_some()
                && doc.norm == b.norm
        })
        .count()
        > 1
}

fn amounts(date: Date, taxable: Option<Paise>, tax: Option<Paise>) -> Result<DocAmounts, GstError> {
    let total = match (taxable, tax) {
        (Some(a), Some(b)) => Some(a.checked_add(b)?),
        _ => None,
    };
    Ok(DocAmounts {
        date,
        taxable,
        tax,
        total,
    })
}

fn pair_state(portal: &DocAmounts, book: &DocAmounts) -> ReconState {
    match (portal.taxable, portal.tax, book.taxable, book.tax) {
        (Some(pt), Some(px), Some(bt), Some(bx)) => {
            if pt == bt && px == bx {
                if portal.date == book.date {
                    ReconState::Matched
                } else {
                    ReconState::DateDiffers
                }
            } else {
                ReconState::ValueDiffers
            }
        }
        _ => ReconState::NotComparable,
    }
}
