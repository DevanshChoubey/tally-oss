//! GSTR-1 exports (table 6), nil-rated (table 8), HSN summary (table 12) and documents
//! issued (table 13) JSON.

use crate::FormatError;
use super::{DocIssue, DocType, ExpGroup, ExpInvoice, ExpItem, HsnRow, NilRow, NilSupplyKind};
use super::emit::{
    amt,
    arr,
    date_str,
    dec,
    doc_number,
    field_err,
    kv,
    map_arr,
    obj,
    require_some,
    text,
};

pub(super) fn exp_item_json(item: &ExpItem) -> String {
    let mut f = vec![
        kv("txval", &amt(item.taxable)),
        kv("rt", &dec(item.rate.0)),
        kv("iamt", &amt(item.igst)),
    ];
    if !item.cess.is_zero() {
        f.push(kv("csamt", &amt(item.cess)));
    }
    obj(&f)
}

pub(super) fn exp_invoice_json(path: &str, inv: &ExpInvoice) -> Result<String, FormatError> {
    require_some(&format!("{path}.itms"), &inv.items)?;
    let mut f = vec![
        kv("inum", &doc_number(&format!("{path}.inum"), &inv.num)?),
        kv("idt", &date_str(inv.date)),
        kv("val", &amt(inv.value)),
    ];
    if let Some(port) = &inv.port_code {
        f.push(kv("sbpcode", &text(port)?));
    }
    if let Some((number, date)) = &inv.shipping_bill {
        f.push(kv("sbnum", &text(number)?));
        f.push(kv("sbdt", &date_str(*date)));
    }
    let items: Vec<String> = inv.items.iter().map(exp_item_json).collect();
    f.push(kv("itms", &arr(&items)));
    Ok(obj(&f))
}

pub(super) fn exp_json(list: &[ExpGroup]) -> Result<String, FormatError> {
    map_arr(list, |i, group| {
        let path = format!("exp[{i}]");
        require_some(&format!("{path}.inv"), &group.invoices)?;
        let invs = map_arr(&group.invoices, |j, inv| {
            exp_invoice_json(&format!("{path}.inv[{j}]"), inv)
        })?;
        let ty = if group.with_payment {
            "\"WPAY\""
        } else {
            "\"WOPAY\""
        };
        Ok(obj(&[kv("exp_typ", ty), kv("inv", &invs)]))
    })
}

pub(super) fn nil_code(k: NilSupplyKind) -> &'static str {
    match k {
        NilSupplyKind::InterB2b => "INTRB2B",
        NilSupplyKind::InterB2c => "INTRB2C",
        NilSupplyKind::IntraB2b => "INTRAB2B",
        NilSupplyKind::IntraB2c => "INTRAB2C",
    }
}

pub(super) fn nil_json(list: &[NilRow]) -> Result<String, FormatError> {
    let rows = map_arr(list, |_, r| {
        Ok(obj(&[
            kv("sply_ty", &text(nil_code(r.kind))?),
            kv("expt_amt", &amt(r.exempt)),
            kv("nil_amt", &amt(r.nil_rated)),
            kv("ngsup_amt", &amt(r.non_gst)),
        ]))
    })?;
    Ok(obj(&[kv("inv", &rows)]))
}

pub(super) fn hsn_json(list: &[HsnRow]) -> Result<String, FormatError> {
    let rows = map_arr(list, |i, r| {
        if r.uqc.trim().is_empty() {
            return Err(field_err(
                &format!("hsn.data[{i}].uqc"),
                "a unit quantity code is required (use NA for services)",
            ));
        }
        Ok(obj(&[
            kv("num", &i.saturating_add(1).to_string()),
            kv("hsn_sc", &text(r.hsn.as_str())?),
            kv("desc", &text(&r.description)?),
            kv("uqc", &text(&r.uqc)?),
            kv("qty", &dec(r.qty.0)),
            kv("val", &amt(r.value)),
            kv("txval", &amt(r.taxable)),
            kv("iamt", &amt(r.igst)),
            kv("camt", &amt(r.cgst)),
            kv("samt", &amt(r.sgst)),
            kv("csamt", &amt(r.cess)),
            kv("rt", &dec(r.rate.0)),
        ]))
    })?;
    Ok(obj(&[kv("data", &rows)]))
}

pub(super) fn doc_type_number(t: DocType) -> u8 {
    match t {
        DocType::OutwardInvoice => 1,
        DocType::InwardFromUnregistered => 2,
        DocType::RevisedInvoice => 3,
        DocType::DebitNote => 4,
        DocType::CreditNote => 5,
        DocType::ReceiptVoucher => 6,
        DocType::PaymentVoucher => 7,
        DocType::RefundVoucher => 8,
        DocType::ChallanJobWork => 9,
        DocType::ChallanOnApproval => 10,
        DocType::ChallanLiquidGas => 11,
        DocType::ChallanOther => 12,
    }
}

pub(super) fn doc_issue_json(list: &[DocIssue]) -> Result<String, FormatError> {
    let det = map_arr(list, |i, d| {
        let path = format!("doc_issue.doc_det[{i}]");
        require_some(&format!("{path}.docs"), &d.ranges)?;
        let docs = map_arr(&d.ranges, |j, r| {
            let net = r.total.checked_sub(r.cancelled).ok_or_else(|| {
                field_err(
                    &format!("{path}.docs[{j}].cancel"),
                    "cancelled documents exceed the total issued",
                )
            })?;
            Ok(obj(&[
                kv("num", &j.saturating_add(1).to_string()),
                kv("from", &text(&r.from)?),
                kv("to", &text(&r.to)?),
                kv("totnum", &r.total.to_string()),
                kv("cancel", &r.cancelled.to_string()),
                kv("net_issue", &net.to_string()),
            ]))
        })?;
        Ok(obj(&[
            kv("doc_num", &doc_type_number(d.doc_type).to_string()),
            kv("docs", &docs),
        ]))
    })?;
    Ok(obj(&[kv("doc_det", &det)]))
}
