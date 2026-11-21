//! GSTR-1A amendments. **Card G9** implements this module.
//!
//! Frozen interface (plan card G9):
//!
//! ```text
//! pub fn build_gstr1a(books: &Books, gstin: &Gstin, period: Period, filed: &Gstr1, pack: &RulePack) -> Result<Outcome<Gstr1a>, GstError>;
//! ```
//!
//! # What this does
//!
//! GSTR-1A lets a taxpayer correct outward supplies of a period after GSTR-1 is filed and before
//! GSTR-3B is filed (research 04-gst section on amendments, `[K]`). The filed GSTR-1 is a fixed
//! record: it is only read here, never edited. When the books are altered after filing, the
//! current return is rebuilt with [`build_gstr1`] and compared with `filed`. Every B2B invoice
//! that is in both and differs becomes one B2BA entry (Table 9A) that names the invoice as it
//! was filed (`original_num`, `original_date`) and carries the invoice as it now stands.
//!
//! # What is `Unsupported`
//!
//! The data shape [`Gstr1a`] holds B2BA entries only. Anything else the comparison finds cannot
//! be written down, and a partial amendment would be a wrong return, so the whole result is
//! `Unsupported` with the reason:
//!
//! * the current books do not give a complete GSTR-1 (a voucher is `Mismatch` or
//!   `NeedsCorrection`, or the rebuild itself is `Unsupported`);
//! * a B2B invoice is in the current books but not in the filed return (a new invoice), or in the
//!   filed return but not in the current books (deleted, cancelled, renumbered or moved to
//!   another recipient: B2BA identifies the original by its number, so these cannot be told
//!   apart from a deleted plus a new invoice);
//! * a section other than B2B differs (`b2cl`, `b2cs`, `cdnr`, `cdnur`, `exp`, `nil`): B2CLA,
//!   B2CSA, CDNRA, CDNURA and EXPA are not modelled;
//! * a recipient GSTIN and invoice number occurs twice in one return.
//!
//! Tables 12 (HSN) and 13 (documents issued) are not compared: GSTR-1A has no place for them in
//! this data shape, and an amended B2B invoice changes Table 12 only through the next GSTR-1.
//! Totals (`aggregate_turnover`, `current_turnover`) are not compared either.
//!
//! # Not checked here
//!
//! That GSTR-3B for the period is still unfiled, and that the period is one in which GSTR-1A
//! exists on the portal, is the caller's business: the books do not hold that portal state.
//!
//! # Errors
//!
//! `filed` for another GSTIN or another period than the one asked for is
//! [`GstError::Invalid`], as are the errors of [`build_gstr1`].

use std::collections::BTreeMap;

use ledgerly_core::{Gstin, Outcome, Period};
use ledgerly_gst_formats::gstr1::{B2bInvoice, B2baInvoice, B2baParty, Gstr1, Gstr1a};
use ledgerly_model::Books;

use crate::GstError;
use crate::gstr1::{Bucket, build_gstr1};
use crate::pack::RulePack;

/// A B2B invoice index: recipient and number to the invoice.
type Index<'a> = BTreeMap<(&'a Gstin, &'a str), &'a B2bInvoice>;

/// Builds GSTR-1A for `gstin` and `period`: the B2B invoices of `filed` that the books now state
/// differently, each as one B2BA entry. `filed` is not changed.
///
/// With no difference the result is a [`Gstr1a`] with an empty `b2ba`.
///
/// # Errors
/// * [`GstError::Invalid`] when `filed` belongs to another GSTIN or another period.
/// * Every error of [`build_gstr1`].
pub fn build_gstr1a(
    books: &Books,
    gstin: &Gstin,
    period: Period,
    filed: &Gstr1,
    pack: &RulePack,
) -> Result<Outcome<Gstr1a>, GstError> {
    if filed.gstin != *gstin {
        return Err(GstError::Invalid {
            field: "filed".to_owned(),
            reason: format!("the filed return is for {}, not for {gstin}", filed.gstin),
        });
    }
    if filed.period != period {
        return Err(GstError::Invalid {
            field: "filed".to_owned(),
            reason: format!(
                "the filed return is for {} to {}, not for {} to {}",
                filed.period.from(),
                filed.period.to(),
                period.from(),
                period.to()
            ),
        });
    }
    let (current, buckets) = match build_gstr1(books, gstin, period, pack)? {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => {
            return Ok(Outcome::unsupported(
                format!("GSTR-1A: {}", u.what),
                format!(
                    "the current GSTR-1 cannot be built, so it cannot be compared: {}",
                    u.reason
                ),
            ));
        }
    };
    let unready: Vec<_> = buckets
        .iter()
        .filter(|b| matches!(b.bucket, Bucket::Mismatch | Bucket::NeedsCorrection))
        .collect();
    if let Some(first) = unready.first() {
        return Ok(Outcome::unsupported(
            "GSTR-1A from books with unresolved vouchers",
            format!(
                "{} voucher(s) are left out of the current GSTR-1 until fixed (first: voucher {}: {}); \
                 an amendment computed without them could drop or misstate an invoice",
                unready.len(),
                first.voucher.0,
                first.reason.as_deref().unwrap_or("no reason given")
            ),
        ));
    }
    let other = differing_sections(filed, &current);
    if !other.is_empty() {
        return Ok(Outcome::unsupported(
            "GSTR-1A amendment outside Table 9A (B2BA)",
            format!(
                "the books now differ from the filed return in {}; only B2B amendments are modelled \
                 (B2CLA, B2CSA, CDNRA, CDNURA and EXPA are not)",
                other.join(", ")
            ),
        ));
    }
    Ok(diff_b2b(filed, &current).map(|b2ba| Gstr1a {
        gstin: filed.gstin.clone(),
        period: filed.period,
        b2ba,
    }))
}

/// Names of the non-B2B sections (other than Tables 12 and 13) whose content differs.
fn differing_sections(filed: &Gstr1, current: &Gstr1) -> Vec<&'static str> {
    let mut out = Vec::new();
    if filed.b2cl != current.b2cl {
        out.push("b2cl (Table 5)");
    }
    if filed.b2cs != current.b2cs {
        out.push("b2cs (Table 7)");
    }
    if filed.cdnr != current.cdnr {
        out.push("cdnr (Table 9B, registered)");
    }
    if filed.cdnur != current.cdnur {
        out.push("cdnur (Table 9B, unregistered)");
    }
    if filed.exp != current.exp {
        out.push("exp (Table 6A)");
    }
    if filed.nil != current.nil {
        out.push("nil (Table 8)");
    }
    out
}

/// Indexes the B2B invoices of a return; `Err` names a duplicate recipient and number.
fn index(r: &Gstr1) -> Result<Index<'_>, String> {
    let mut map = Index::new();
    for party in &r.b2b {
        for inv in &party.invoices {
            if map.insert((&party.ctin, inv.num.as_str()), inv).is_some() {
                return Err(format!(
                    "invoice {} to {} occurs twice in one return",
                    inv.num, party.ctin
                ));
            }
        }
    }
    Ok(map)
}

/// The B2BA entries that turn `filed` into `current`, grouped by recipient in GSTIN order and by
/// invoice number within a recipient.
fn diff_b2b(filed: &Gstr1, current: &Gstr1) -> Outcome<Vec<B2baParty>> {
    let (old, new) = match (index(filed), index(current)) {
        (Ok(o), Ok(n)) => (o, n),
        (Err(e), _) => return Outcome::unsupported("GSTR-1A: filed return", e),
        (_, Err(e)) => return Outcome::unsupported("GSTR-1A: current books", e),
    };
    let mut parties: Vec<B2baParty> = Vec::new();
    for (&(ctin, num), &filed_inv) in &old {
        let Some(&now) = new.get(&(ctin, num)) else {
            return Outcome::unsupported(
                "GSTR-1A for an invoice that is in the filed return but not in the books",
                format!(
                    "invoice {num} to {ctin} was filed but the books no longer hold it under that number and \
                     recipient (deleted, cancelled, renumbered or moved); B2BA cannot express that"
                ),
            );
        };
        if filed_inv == now {
            continue;
        }
        let entry = B2baInvoice {
            original_num: filed_inv.num.clone(),
            original_date: filed_inv.date,
            amended: now.clone(),
        };
        match parties.last_mut() {
            Some(p) if p.ctin == *ctin => p.invoices.push(entry),
            Some(_) | None => parties.push(B2baParty {
                ctin: ctin.clone(),
                invoices: vec![entry],
            }),
        }
    }
    for &(ctin, num) in new.keys() {
        if !old.contains_key(&(ctin, num)) {
            return Outcome::unsupported(
                "GSTR-1A for an invoice that is not in the filed return",
                format!(
                    "invoice {num} to {ctin} is in the books but was not filed; a new invoice goes in the \
                     next GSTR-1, and B2BA only amends filed invoices"
                ),
            );
        }
    }
    Outcome::Value(parties)
}
