//! A saved or drafted sales invoice as the IRP e-invoice JSON (INV-01, schema version 1.1).
//!
//! Every figure comes from the GST engine's per-line unrounded tax ([`compute_for`]); nothing is
//! computed here but the rounding of each item's heads to the paisa (half away from zero, as the
//! engine rounds a head total) and sums of those. A voucher whose item-wise heads do not add up to
//! the heads the voucher carries is `Unsupported` with the reason, never adjusted.
//!
//! Source of the shape: the NIC e-Invoice sandbox schema page,
//! <https://einv-apisandbox.nic.in/version1.03/generate-irn.html>, read 2026-11-26. No upload is
//! done (the product offers the same offline route, export then upload on the IRP bulk page:
//! <https://help.tallysolutions.com/generate-e-invoice-offline-tallyhelp/>, read 2026-11-26).

use ledgerly_core::format::format_inr;
use ledgerly_core::{
    Decimal, Outcome, Paise, RoundingRule, StateCode, VoucherId, round_rupees_to_paise,
};
use ledgerly_gst::compute::{GstBreakup, LineSource};
use ledgerly_gst::pack::load_pack;
use ledgerly_gst_formats::einvoice::{
    DocType, EInvItem, EInvParty, EInvTotals, EInvoice, SupplyType, einvoice_to_json,
    validate_einvoice,
};
use ledgerly_model::{Address, BaseVoucherKind, Books, RegistrationType, TaxLedgerKind, Voucher};

use crate::error::ApiError;
use crate::facade::Api;
use crate::gst::BUNDLED_RATE_PACK;
use crate::voucher_gst::compute_for;

impl Api {
    /// The e-invoice JSON of saved voucher `id`.
    ///
    /// Only a sales item invoice to a registered buyer maps (credit and debit notes, accounting
    /// invoices and unregistered buyers are `Unsupported` with the reason), and only when its
    /// supplier, buyer and items carry what the schema requires.
    ///
    /// # Errors
    /// [`ApiError::UnknownVoucher`] when the voucher is not in the books, plus the errors of the
    /// GST engine and the rule pack.
    pub fn einvoice_json(&self, id: VoucherId) -> Result<Outcome<String>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(b) => b,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let v = books
            .vouchers
            .get(&id)
            .ok_or(ApiError::UnknownVoucher { id: id.0 })?;
        let Some(number) = v.number.as_deref() else {
            return Ok(Outcome::unsupported(
                format!("e-invoice of voucher {}", id.0),
                "the voucher has no number yet",
            ));
        };
        Ok(map(books, v, number)?.map_json())
    }

    /// Whether `draft` (a sales invoice not yet saved) would map to e-invoice JSON, so the
    /// invoice screen can say so before the save. The document number is a stand-in.
    ///
    /// # Errors
    /// As [`Api::einvoice_json`].
    pub fn einvoice_check(&self, draft: &Voucher) -> Result<Outcome<()>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(b) => b,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        Ok(map(books, draft, "1")?.map_json().map(|_json| ()))
    }
}

/// The mapped document, or why there is none.
struct Mapped(Outcome<EInvoice>);

impl Mapped {
    fn map_json(self) -> Outcome<String> {
        match self.0 {
            Outcome::Unsupported(u) => Outcome::Unsupported(u),
            Outcome::Value(e) => match einvoice_to_json(&e) {
                Ok(json) => Outcome::Value(json),
                Err(err) => Outcome::unsupported("e-invoice", err.to_string()),
            },
        }
    }
}

fn no(reason: impl Into<String>) -> Mapped {
    Mapped(Outcome::unsupported("e-invoice", reason))
}

/// The seller, the buyer and the place of supply, or why the invoice is not an e-invoice.
fn parties(books: &Books, v: &Voucher) -> Result<(EInvParty, EInvParty, StateCode), String> {
    let kind = books.masters.voucher_types.get(&v.type_id).map(|t| t.base);
    match kind {
        Some(BaseVoucherKind::Sales) => {}
        Some(BaseVoucherKind::CreditNote | BaseVoucherKind::DebitNote) => {
            return Err(
                "credit and debit notes are not mapped yet (only sales invoices, document type INV)"
                    .to_owned(),
            );
        }
        Some(_) | None => return Err("only a sales invoice is an e-invoice".to_owned()),
    }
    if v.inventory_lines.is_empty() {
        return Err(
            "an accounting invoice has no items to list (ItemList needs HSN codes)".to_owned(),
        );
    }
    if v.reverse_charge {
        return Err("reverse charge is not mapped".to_owned());
    }
    let buyer = v
        .party
        .and_then(|p| books.masters.ledgers.get(&p))
        .ok_or("the invoice has no buyer")?;
    let details = buyer
        .party
        .as_ref()
        .ok_or("the buyer is not a party ledger")?;
    let buyer_gstin = details
        .gstin
        .as_ref()
        .filter(|_| matches!(details.registration, RegistrationType::Regular))
        .ok_or("not applicable: the buyer is not a registered regular taxpayer (B2C)")?;
    let pos = v
        .place_of_supply
        .ok_or("the invoice has no place of supply")?;
    let reg = books
        .company
        .registrations
        .iter()
        .filter(|r| r.effective_from <= v.date)
        .max_by_key(|r| r.effective_from)
        .ok_or("the company has no GST registration in force on the invoice date")?;
    if reg.composition {
        return Err("a composition taxpayer issues no tax invoice".to_owned());
    }
    let seller_name = books
        .company
        .mailing_name
        .clone()
        .unwrap_or_else(|| books.company.name.clone());
    let seller = party_block(
        "the company",
        seller_name,
        reg.gstin.clone(),
        reg.state,
        &books.company.address,
    )?;
    let buyer_state = buyer_gstin
        .state()
        .ok_or("the buyer's GSTIN has no state code")?;
    let buyer_block = party_block(
        &format!("the buyer {}", buyer.name),
        buyer.name.clone(),
        buyer_gstin.clone(),
        buyer_state,
        &details.address,
    )?;
    Ok((seller, buyer_block, pos))
}

fn map(books: &Books, v: &Voucher, doc_no: &str) -> Result<Mapped, ApiError> {
    let (seller, buyer, place_of_supply) = match parties(books, v) {
        Ok(p) => p,
        Err(why) => return Ok(no(why)),
    };
    let pack = load_pack(BUNDLED_RATE_PACK)?;
    let breakup = match compute_for(books, v, &pack)? {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => return Ok(Mapped(Outcome::Unsupported(u))),
    };
    let items = match items_of(books, v, &breakup) {
        Ok(items) => items,
        Err(why) => return Ok(no(why)),
    };
    let sum = |f: fn(&EInvItem) -> Paise| {
        items
            .iter()
            .try_fold(Paise::ZERO, |acc, i| acc.checked_add(f(i)))
    };
    let (Ok(assessable), Ok(cgst), Ok(sgst), Ok(igst), Ok(items_total)) = (
        sum(|i| i.assessable),
        sum(|i| i.cgst),
        sum(|i| i.sgst),
        sum(|i| i.igst),
        sum(|i| i.total),
    ) else {
        return Ok(no("the item amounts overflow"));
    };
    if let Some(why) = disagrees_with_stored(v, cgst, sgst, igst) {
        return Ok(no(why));
    }
    let round_off = v.gst.as_ref().map_or(breakup.round_off, |g| g.round_off);
    let Ok(total) = items_total.checked_add(round_off) else {
        return Ok(no("the invoice total overflows"));
    };
    let e = EInvoice {
        supply_type: SupplyType::B2b,
        reverse_charge: false,
        ecom_gstin: None,
        igst_on_intra: false,
        doc_type: DocType::Invoice,
        doc_no: doc_no.to_owned(),
        doc_date: v.date,
        seller,
        buyer,
        place_of_supply,
        items,
        totals: EInvTotals {
            assessable,
            cgst,
            sgst,
            igst,
            cess: Paise::ZERO,
            state_cess: Paise::ZERO,
            discount: Paise::ZERO,
            other_charges: Paise::ZERO,
            round_off,
            total,
        },
    };
    match validate_einvoice(&e) {
        Ok(()) => Ok(Mapped(Outcome::Value(e))),
        Err(errs) => {
            let why: Vec<String> = errs.iter().map(ToString::to_string).collect();
            Ok(no(why.join("; ")))
        }
    }
}

/// One `SellerDtls` or `BuyerDtls` block. The address lines map as the first line to `Addr1`,
/// the last to `Loc` (the place) and any between to `Addr2`; fewer than two lines or no PIN
/// cannot be written to the schema and are refused, never invented.
fn party_block(
    who: &str,
    name: String,
    gstin: ledgerly_core::Gstin,
    state: ledgerly_core::StateCode,
    address: &Address,
) -> Result<EInvParty, String> {
    let lines: Vec<&str> = address
        .lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        return Err(format!("{who} has no address (Addr1 and Loc are required)"));
    };
    if lines.len() < 2 {
        return Err(format!(
            "{who} has one address line; the schema needs a street line and a place (Loc)"
        ));
    }
    let middle: Vec<&str> = lines
        .get(1..lines.len().saturating_sub(1))
        .unwrap_or_default()
        .to_vec();
    let Some(pin) = address
        .pincode
        .as_deref()
        .and_then(|p| p.trim().parse::<u32>().ok())
    else {
        return Err(format!("{who} has no PIN code (Pin is required)"));
    };
    Ok(EInvParty {
        gstin,
        legal_name: name,
        trade_name: None,
        address1: (*first).to_owned(),
        address2: if middle.is_empty() {
            None
        } else {
            Some(middle.join(", "))
        },
        location: (*last).to_owned(),
        pin,
        state,
        phone: None,
        email: None,
    })
}

/// The items, one per engine line, each head rounded to the paisa.
fn items_of(books: &Books, v: &Voucher, breakup: &GstBreakup) -> Result<Vec<EInvItem>, String> {
    let mut items = Vec::new();
    for line in &breakup.lines {
        let LineSource::Inventory { index } = line.source else {
            return Err("a line that is not an item line".to_owned());
        };
        let inv = v
            .inventory_lines
            .get(index)
            .ok_or("an item line is missing")?;
        let item = books
            .masters
            .stock_items
            .get(&inv.item)
            .ok_or("an item is not in the masters")?;
        let hsn = item
            .hsn
            .clone()
            .ok_or_else(|| format!("item {} has no HSN code", item.name))?;
        let unit = books
            .masters
            .units
            .get(&inv.unit)
            .and_then(|u| u.uqc.clone())
            .ok_or_else(|| format!("the unit of item {} has no UQC code", item.name))?;
        if !line.utgst.is_zero() {
            return Err("UTGST has no field in the schema".to_owned());
        }
        if !line.cess.is_zero() {
            return Err("cess is not mapped".to_owned());
        }
        let round = |d: Decimal| round_rupees_to_paise(d, RoundingRule::HalfUpToPaise);
        let gross = round(
            inv.qty
                .0
                .checked_mul(inv.rate.0)
                .ok_or("the line amount overflows")?,
        )
        .map_err(|e| e.to_string())?;
        let discount = gross.checked_sub(line.taxable).map_err(|e| e.to_string())?;
        if discount.get() < 0 {
            return Err(format!(
                "item {}: the taxable value is more than quantity times rate",
                item.name
            ));
        }
        let cgst = round(line.cgst).map_err(|e| e.to_string())?;
        let sgst = round(line.sgst).map_err(|e| e.to_string())?;
        let igst = round(line.igst).map_err(|e| e.to_string())?;
        let total = [cgst, sgst, igst]
            .into_iter()
            .try_fold(line.taxable, Paise::checked_add)
            .map_err(|e| e.to_string())?;
        items.push(EInvItem {
            description: Some(item.name.clone()),
            is_service: hsn.as_str().starts_with("99"),
            hsn,
            quantity: Some(inv.qty.0),
            free_quantity: None,
            unit: Some(unit),
            unit_price: inv.rate.0,
            gross_amount: gross,
            discount,
            assessable: line.taxable,
            gst_rate: line.rate_percent,
            igst,
            cgst,
            sgst,
            cess: Paise::ZERO,
            state_cess: Paise::ZERO,
            total,
        });
    }
    if items.is_empty() {
        return Err("the invoice has no taxable item".to_owned());
    }
    Ok(items)
}

/// When the voucher carries its heads and the item-wise rounded heads do not add up to them, the
/// reason. The invoice is then not mapped: the schema wants the items to add to the totals.
fn disagrees_with_stored(v: &Voucher, cgst: Paise, sgst: Paise, igst: Paise) -> Option<String> {
    let gst = v.gst.as_ref()?;
    let stored = |kinds: &[TaxLedgerKind]| {
        gst.lines
            .iter()
            .filter(|l| kinds.contains(&l.kind))
            .try_fold(Paise::ZERO, |acc, l| acc.checked_add(l.tax))
    };
    let pairs = [
        ("CGST", cgst, stored(&[TaxLedgerKind::Cgst])),
        ("SGST", sgst, stored(&[TaxLedgerKind::Sgst])),
        ("IGST", igst, stored(&[TaxLedgerKind::Igst])),
    ];
    pairs
        .into_iter()
        .find_map(|(head, items, total)| match total {
            Ok(total) if total == items => None,
            Ok(total) => Some(format!(
                "the items' {head} adds to {} but the voucher's {head} is {}; \
             the schema needs them equal",
                format_inr(items),
                format_inr(total)
            )),
            Err(_) => Some(format!("the voucher's {head} overflows")),
        })
}
