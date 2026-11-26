//! Eligible input tax credit of a purchase draft.
//!
//! The figure comes from the same rule as GSTR-3B 4(A): `ledgerly_gst::itc::itc_eligibility` per
//! purchase line, with each head's tax summed unrounded over the lines and rounded once to the
//! paisa (see `ledgerly_gst::gstr3b`'s `add_purchase`). A draft cannot know the s.16(2)
//! conditions (the supplier's bill in 2B, receipt of the goods, payment within 180 days), so the
//! figure is the credit that is eligible before those are met.

use ledgerly_core::{
    Decimal, LedgerId, MoneyError, Outcome, Paise, RoundingRule, round_rupees_to_paise,
};
use ledgerly_gst::GstError;
use ledgerly_gst::compute::{GstBreakup, LineSource, LineTax};
use ledgerly_gst::itc::itc_eligibility;
use ledgerly_model::{BaseVoucherKind, Books, Taxability, Voucher};

use crate::gst::bundled_pack;
use crate::voucher_gst::EligibleItc;

/// Unrounded tax of the four heads over some lines.
#[derive(Default)]
struct Sums {
    cgst: Decimal,
    sgst: Decimal,
    utgst: Decimal,
    igst: Decimal,
}

impl Sums {
    fn add(&mut self, l: &LineTax) -> Result<(), MoneyError> {
        self.cgst = self.cgst.checked_add(l.cgst).ok_or(MoneyError::Overflow)?;
        self.sgst = self.sgst.checked_add(l.sgst).ok_or(MoneyError::Overflow)?;
        self.utgst = self
            .utgst
            .checked_add(l.utgst)
            .ok_or(MoneyError::Overflow)?;
        self.igst = self.igst.checked_add(l.igst).ok_or(MoneyError::Overflow)?;
        Ok(())
    }

    fn rounded(&self) -> Result<Paise, MoneyError> {
        let mut total = Paise::ZERO;
        for head in [self.cgst, self.sgst, self.utgst, self.igst] {
            total = total.checked_add(round_rupees_to_paise(head, RoundingRule::HalfUpToPaise)?)?;
        }
        Ok(total)
    }
}

/// The ledger a taxable line posts to.
fn line_ledger(v: &Voucher, source: LineSource) -> Option<LedgerId> {
    match source {
        LineSource::Inventory { index } => v.inventory_lines.get(index).and_then(|l| l.ledger),
        LineSource::Ledger { ledger } => Some(ledger),
    }
}

/// The credit of `v`, a purchase whose ledger lines are built; `None` when `v` is not a purchase.
pub(crate) fn eligible_itc(
    books: &Books,
    v: &Voucher,
    breakup: &GstBreakup,
) -> Result<Option<Outcome<EligibleItc>>, GstError> {
    let is_purchase = books
        .masters
        .voucher_types
        .get(&v.type_id)
        .is_some_and(|t| t.base == BaseVoucherKind::Purchase);
    if !is_purchase {
        return Ok(None);
    }
    if v.reverse_charge {
        return Ok(Some(Outcome::unsupported(
            "eligible credit of a reverse-charge purchase",
            "credit under reverse charge follows the payment of the tax, which a draft cannot know",
        )));
    }
    let pack = bundled_pack().map_err(|e| GstError::Pack {
        reason: e.to_string(),
    })?;
    let tos = v.supply_date.unwrap_or(v.date);
    let rules = match pack.for_time_of_supply(tos) {
        Outcome::Value(rules) => rules,
        Outcome::Unsupported(u) => return Ok(Some(Outcome::Unsupported(u))),
    };
    let (mut eligible, mut not_claimable) = (Sums::default(), Sums::default());
    for l in &breakup.lines {
        match l.taxability {
            Taxability::Taxable => {}
            Taxability::Exempt | Taxability::NilRated | Taxability::NonGst => continue,
        }
        let Some(ledger) = line_ledger(v, l.source) else {
            return Ok(Some(Outcome::unsupported(
                "eligible credit of a line without a purchase ledger",
                "every line needs the purchase ledger it posts to",
            )));
        };
        let mut found = v
            .ledger_lines
            .iter()
            .enumerate()
            .filter(|(_, x)| x.ledger == ledger)
            .map(|(i, _)| i);
        let index = match (l.source, found.next(), found.next()) {
            (_, Some(only), None) | (LineSource::Ledger { .. }, Some(only), Some(_)) => only,
            (_, None, _) | (LineSource::Inventory { .. }, Some(_), Some(_)) => {
                return Ok(Some(Outcome::unsupported(
                    "eligible credit of a line whose ledger is not on exactly one ledger line",
                    "the credit of this line cannot be attributed",
                )));
            }
        };
        let eligibility = match itc_eligibility(v, index, &books.company, &books.masters, &rules)? {
            Outcome::Value(e) => e,
            Outcome::Unsupported(u) => return Ok(Some(Outcome::Unsupported(u))),
        };
        if eligibility.is_claimable() {
            eligible.add(l)?;
        } else {
            not_claimable.add(l)?;
        }
    }
    Ok(Some(Outcome::Value(EligibleItc {
        eligible: eligible.rounded()?,
        not_claimable: not_claimable.rounded()?,
    })))
}
