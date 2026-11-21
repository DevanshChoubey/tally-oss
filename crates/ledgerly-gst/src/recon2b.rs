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
            tax: voucher_tax(v)?,
        });
    }
    Ok((active, cancelled))
}

fn portal_doc(ctin: &str, d: &InwardDoc) -> Result<PortalDoc, GstError> {
    let tax = match d.tax_totals()? {
        Outcome::Value(t) => Some(t.checked_total_tax()?),
        Outcome::Unsupported(_) => None,
    };
    Ok(PortalDoc {
        supplier: ctin.to_owned(),
        kind: d.kind,
        number: d.number.clone(),
        norm: normalise(&d.number),
        date: d.date,
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
    let mut lines = Vec::with_capacity(docs.len());
    for (i, doc) in docs.iter().enumerate() {
        let hit = result.matched.get(i).copied().flatten();
        let line = if let Some((j, level)) = hit {
            ReconLine {
                portal_index: i,
                voucher: active.get(j).map(|b| b.id),
                level,
                suggested: suggestion_for(level),
                confirmed: false,
            }
        } else {
            let ambiguous = result.ambiguous.get(i).copied().unwrap_or(false);
            let twin = if ambiguous {
                None
            } else {
                cancelled_twin(doc, &cancelled)
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
    Ok(lines)
}
