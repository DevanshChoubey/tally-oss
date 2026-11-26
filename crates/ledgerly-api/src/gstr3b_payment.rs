//! GSTR-3B table 6.1, payment of tax: the output tax of a period set off against the input tax
//! credit the user accepted in IMS, giving the tax payable in cash per head (round s16, engine
//! item E-1).
//!
//! # What is worked out
//!
//! * The liability is table 3.1(a) of [`Api::gstr3b`], by head.
//! * The credit is the tax of the documents of the period's 2B file whose IMS action the user
//!   recorded as Accept. A document with no action is shown apart as "deemed accepted when 2B is
//!   generated" and is **not** used; a rejected or pending document is neither. That is a product
//!   policy: what the user claims in table 4 is the user's choice, and this shows the cash needed
//!   when only the credit accepted by the user is claimed. Sources for the portal's treatment of
//!   no action, pending and rejected records (read 2026-11-26; the GSTN FAQs themselves could not
//!   be read, so this is `pending observed`): Zoho Books Academy, "FAQs about the IMS in the GST
//!   portal" (zoho.com/in/books/academy/taxes-and-compliance/ims-in-gst.html), which says a record
//!   with no action is accepted and flows into GSTR-2B, a pending record appears in neither
//!   GSTR-2B nor GSTR-3B, and a rejected record falls in the rejected section.
//! * The set-off is [`ledgerly_gst::setoff::set_off`] (CGST Act s.49(5), s.49A, Rule 88A, s.170).
//!
//! # What is `Unsupported`
//!
//! * No 2B file for the period: the credit the user accepted cannot be known.
//! * A period before s.49A, and a period with reverse charge purchases (their tax is paid in
//!   cash, outside this working).
//! * Taxable supplies of the month above ₹50 lakh: Rule 86B (notification 94/2020-Central Tax,
//!   taxinformation.cbic.gov.in, read 2026-11-26) caps the credit at 99 per cent of the output
//!   tax unless one of its exceptions holds, and the exceptions are not checked.
//! * An accepted document that is a credit or debit note, is marked "ITC not available" in the
//!   file, has no tax split, does not agree with the books (not a matched row), or carries more
//!   credit than the books show as eligible: the credit would be a guess.
//!
//! The balance already in the electronic credit ledger from earlier periods and the electronic
//! cash ledger are not read: the working sets this period's accepted credit off against this
//! period's tax, and says so.

use ledgerly_core::{Gstin, Outcome, Paise, Period};
use ledgerly_gst::recon2b::{ImsAction, ReconState};
use ledgerly_gst::setoff::{SetOff, section_49a_in_force, set_off};
use ledgerly_gst_formats::gstr2b::{
    InwardDoc, InwardDocKind, ItcAvailability, TaxAmounts, parse_gstr2b,
};
use ledgerly_gst_formats::gstr3b::Gstr3bTaxAmounts;

use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;
use crate::gst::Gstr3bWorking;
use crate::recon::{last_import, same_gstin};

/// Taxable supplies of one month above which Rule 86B applies: ₹50,00,000, in paise.
const RULE_86B_TURNOVER: Paise = Paise::new(500_000_000);

/// GSTR-3B table 6.1 for one period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr3bPayment {
    /// The working the liability and the books' credit come from.
    pub working: Gstr3bWorking,
    /// Output tax of the period, table 3.1(a), by head.
    pub liability: Gstr3bTaxAmounts,
    /// Credit the books could claim, table 4(A)(5), by head.
    pub credit_in_books: Gstr3bTaxAmounts,
    /// Credit of the 2B documents the user accepted in IMS, by head. This is the credit used.
    pub credit_accepted: Gstr3bTaxAmounts,
    /// Credit of the 2B documents with no IMS action, by head: deemed accepted when 2B is
    /// generated. Shown apart; not used.
    pub credit_no_action: Gstr3bTaxAmounts,
    /// How many 2B documents have no IMS action.
    pub no_action_documents: usize,
    /// The set-off of [`Gstr3bPayment::credit_accepted`] against [`Gstr3bPayment::liability`].
    pub set_off: SetOff,
}

fn add_heads(a: Gstr3bTaxAmounts, t: TaxAmounts) -> Result<Gstr3bTaxAmounts, ApiError> {
    let sum = |x: Paise, y: Paise| {
        x.checked_add(y)
            .map_err(|e| ApiError::from(ledgerly_gst::GstError::from(e)))
    };
    Ok(Gstr3bTaxAmounts {
        integrated: sum(a.integrated, t.igst)?,
        central: sum(a.central, t.cgst)?,
        state_ut: sum(a.state_ut, t.sgst)?,
        cess: sum(a.cess, t.cess)?,
    })
}

fn exceeds(accepted: Gstr3bTaxAmounts, books: Gstr3bTaxAmounts) -> bool {
    accepted.integrated > books.integrated
        || accepted.central > books.central
        || accepted.state_ut > books.state_ut
        || accepted.cess > books.cess
}

fn unsupported<T>(what: &str, reason: String) -> Result<Outcome<T>, ApiError> {
    Ok(Outcome::unsupported(what, reason))
}

/// Why an accepted document cannot be counted as credit, `None` when it can.
fn refusal(doc: &InwardDoc, state: ReconState) -> Option<String> {
    let name = &doc.number;
    match doc.kind {
        InwardDocKind::Invoice => {}
        InwardDocKind::CreditNote | InwardDocKind::DebitNote => {
            return Some(format!(
                "accepted note {name} is a credit or debit note; how it nets against credit is not worked out"
            ));
        }
    }
    match &doc.itc_available {
        Some(ItcAvailability::Available) => {}
        Some(ItcAvailability::Unavailable | ItcAvailability::Other(_)) | None => {
            return Some(format!(
                "accepted document {name} is not marked as carrying input tax credit in the 2B file"
            ));
        }
    }
    match state {
        ReconState::Matched => None,
        ReconState::ValueDiffers
        | ReconState::DateDiffers
        | ReconState::NotInBooks
        | ReconState::PossibleDuplicate
        | ReconState::NotComparable => Some(format!(
            "accepted document {name} does not agree with a purchase in the books, so its credit is not certain"
        )),
    }
}

/// The calendar month holding `day`.
fn month_of(day: ledgerly_core::Date) -> Option<Period> {
    let from = day.replace_day(1).ok()?;
    let last = time::util::days_in_month(from.month(), from.year());
    let to = from.replace_day(last).ok()?;
    Period::new(from, to).ok()
}

/// What Home shows of GST for one month: the cash payable and the IMS count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeGst {
    /// The tax payable in cash for the month (GSTR-3B 6.1, after the credit accepted in IMS).
    pub cash_payable: Outcome<Paise>,
    /// How many 2B records have no IMS action; `None` when there is no 2B file or the return
    /// was marked filed.
    pub ims_no_action: Outcome<Option<usize>>,
}

impl Api {
    /// What Home shows of GST for the calendar month of `as_of`, for the company's one regular
    /// registration. A company with none, or more than one, gets `Unsupported` cash and no
    /// count: Home does not pick a registration.
    ///
    /// # Errors
    /// As [`Api::gstr3b_payment`].
    pub fn home_gst(&self, as_of: ledgerly_core::Date) -> Result<HomeGst, ApiError> {
        let Outcome::Value(books) = self.cached_books()? else {
            return Ok(HomeGst {
                cash_payable: Outcome::unsupported("Needs GSTR-2B", "the books could not be read"),
                ims_no_action: Outcome::Value(None),
            });
        };
        let Some(gstin) = crate::nudges::single_regular_gstin(books) else {
            return Ok(HomeGst {
                cash_payable: Outcome::unsupported(
                    "GST cash payable",
                    "the company needs one regular GST registration",
                ),
                ims_no_action: Outcome::Value(None),
            });
        };
        let Some(period) = month_of(as_of) else {
            return Ok(HomeGst {
                cash_payable: Outcome::unsupported("GST cash payable", "the month is not a date"),
                ims_no_action: Outcome::Value(None),
            });
        };
        let cash_payable = match self.gstr3b_payment(&gstin, period)? {
            Outcome::Value(p) => match p.set_off.cash_total() {
                Ok(total) => Outcome::Value(total),
                Err(e) => Outcome::unsupported("GST cash payable", e.to_string()),
            },
            Outcome::Unsupported(u) => Outcome::Unsupported(u),
        };
        Ok(HomeGst {
            cash_payable,
            ims_no_action: self.ims_no_action_count(&gstin, period)?,
        })
    }

    /// GSTR-3B table 6.1 for `gstin` and `period`: the tax payable in cash after the credit the
    /// user accepted in IMS is set off (see the module docs for what is counted and refused).
    ///
    /// # Errors
    /// As [`Api::gstr3b`] and [`Api::gstr2b_reconciliation`].
    pub fn gstr3b_payment(
        &self,
        gstin: &Gstin,
        period: Period,
    ) -> Result<Outcome<Gstr3bPayment>, ApiError> {
        let working = match self.gstr3b(gstin, period)? {
            Outcome::Value(w) => w,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        if !section_49a_in_force(period.from()) {
            return unsupported(
                "GSTR-3B payment of tax",
                "the set-off order of CGST Act s.49A applies from February 2019 only".to_owned(),
            );
        }
        if !working.reverse_charge_purchases.is_empty() {
            return unsupported(
                "GSTR-3B payment of tax",
                "the period has purchases under reverse charge, whose tax is paid in cash outside this working".to_owned(),
            );
        }
        if working.outward_taxable_value > RULE_86B_TURNOVER {
            return unsupported(
                "GSTR-3B payment of tax",
                "taxable supplies of the month exceed 50 lakh rupees, so Rule 86B may cap the credit at 99 per cent of the tax; its exceptions are not checked".to_owned(),
            );
        }
        let records = read_all(self.store())?;
        let Some((_, json)) = last_import(&records, gstin, period) else {
            return unsupported(
                "Needs GSTR-2B",
                "no GSTR-2B file is imported for the period, so the credit accepted in IMS is not known".to_owned(),
            );
        };
        let rec = match self.gstr2b_reconciliation(gstin, period)? {
            Outcome::Value(r) => r,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let data = parse_gstr2b(json)?;
        let docs: Vec<(&str, &InwardDoc)> = ledgerly_gst::recon2b::portal_documents(&data);
        let mut accepted = Gstr3bTaxAmounts::default();
        let mut no_action = Gstr3bTaxAmounts::default();
        let mut no_action_documents = 0_usize;
        for row in &rec.rows {
            let Some((_, doc)) = docs.iter().find(|(ctin, d)| {
                same_gstin(ctin, &row.supplier_gstin) && d.number == row.doc_number
            }) else {
                continue;
            };
            match row.action {
                Some(ImsAction::Accept) => {
                    if let Some(reason) = refusal(doc, row.state) {
                        return unsupported("GSTR-3B payment of tax", reason);
                    }
                    let totals = match doc.tax_totals().map_err(ledgerly_gst::GstError::from)? {
                        Outcome::Value(t) => t,
                        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
                    };
                    accepted = add_heads(accepted, totals)?;
                }
                None => {
                    let totals = match doc.tax_totals().map_err(ledgerly_gst::GstError::from)? {
                        Outcome::Value(t) => t,
                        Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
                    };
                    no_action = add_heads(no_action, totals)?;
                    no_action_documents += 1;
                }
                Some(ImsAction::Reject | ImsAction::Pending) => {}
            }
        }
        if exceeds(accepted, working.itc_all_other) {
            return unsupported(
                "GSTR-3B payment of tax",
                "the credit accepted in IMS is more than the credit the books show as eligible, so some of it is blocked or ineligible".to_owned(),
            );
        }
        let liability = working.outward_tax;
        let result = set_off(liability, accepted)?;
        Ok(Outcome::Value(Gstr3bPayment {
            credit_in_books: working.itc_all_other,
            liability,
            credit_accepted: accepted,
            credit_no_action: no_action,
            no_action_documents,
            set_off: result,
            working,
        }))
    }

    /// How many documents of the period's 2B file have no IMS action, `None` when there is no
    /// file or GSTR-3B for the period was marked filed (then the actions are final).
    ///
    /// # Errors
    /// As [`Api::gstr2b_reconciliation`].
    pub fn ims_no_action_count(
        &self,
        gstin: &Gstin,
        period: Period,
    ) -> Result<Outcome<Option<usize>>, ApiError> {
        let rec = match self.gstr2b_reconciliation(gstin, period)? {
            Outcome::Value(r) => r,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        if rec.file_name.is_none() || rec.filed_on.is_some() {
            return Ok(Outcome::Value(None));
        }
        let count = rec.rows.iter().filter(|r| r.action.is_none()).count();
        Ok(Outcome::Value(Some(count)))
    }
}
