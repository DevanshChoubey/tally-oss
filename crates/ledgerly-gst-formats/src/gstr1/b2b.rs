//! GSTR-1 table 4 (B2B) and table 9A (amended B2B, B2BA) JSON.

use super::emit::{
    amt, date_str, doc_number, flag, items_json, kv, map_arr, obj, pos_str, require_some, text,
};
use super::{B2bInvoice, B2bInvoiceType, B2bParty, B2baParty};
use crate::FormatError;
use ledgerly_core::Gstin;

pub(super) fn inv_type_code(t: B2bInvoiceType) -> &'static str {
    match t {
        B2bInvoiceType::Regular => "R",
        B2bInvoiceType::SezWithPayment => "SEWP",
        B2bInvoiceType::SezWithoutPayment => "SEWOP",
        B2bInvoiceType::DeemedExport => "DE",
    }
}

pub(super) fn b2b_invoice_fields(path: &str, inv: &B2bInvoice) -> Result<Vec<String>, FormatError> {
    Ok(vec![
        kv("inum", &doc_number(&format!("{path}.inum"), &inv.num)?),
        kv("idt", &date_str(inv.date)),
        kv("val", &amt(inv.value)),
        kv("pos", &pos_str(inv.pos)),
        kv("rchrg", flag(inv.reverse_charge)),
        kv("inv_typ", &text(inv_type_code(inv.inv_type))?),
        kv("itms", &items_json(&format!("{path}.itms"), &inv.items)?),
    ])
}

pub(super) fn gstin_json(g: &Gstin) -> Result<String, FormatError> {
    text(g.as_str())
}

pub(super) fn b2b_json(list: &[B2bParty], root: &str) -> Result<String, FormatError> {
    map_arr(list, |i, party| {
        let path = format!("{root}[{i}]");
        require_some(&format!("{path}.inv"), &party.invoices)?;
        let invs = map_arr(&party.invoices, |j, inv| {
            Ok(obj(&b2b_invoice_fields(&format!("{path}.inv[{j}]"), inv)?))
        })?;
        Ok(obj(&[
            kv("ctin", &gstin_json(&party.ctin)?),
            kv("inv", &invs),
        ]))
    })
}

pub(super) fn b2ba_json(list: &[B2baParty]) -> Result<String, FormatError> {
    map_arr(list, |i, party| {
        let path = format!("b2ba[{i}]");
        require_some(&format!("{path}.inv"), &party.invoices)?;
        let invs = map_arr(&party.invoices, |j, a| {
            let p = format!("{path}.inv[{j}]");
            let mut f = vec![
                kv(
                    "oinum",
                    &doc_number(&format!("{p}.oinum"), &a.original_num)?,
                ),
                kv("oidt", &date_str(a.original_date)),
            ];
            f.extend(b2b_invoice_fields(&p, &a.amended)?);
            Ok(obj(&f))
        })?;
        Ok(obj(&[
            kv("ctin", &gstin_json(&party.ctin)?),
            kv("inv", &invs),
        ]))
    })
}
