//! GSTR-1 table 5 (B2C large, B2CL) and table 7 (B2C small, B2CS) JSON.

use super::b2b::gstin_json;
use super::emit::{
    amt, date_str, dec, doc_number, items_json, kv, map_arr, obj, pos_str, require_some, tax_fields,
};
use super::{B2clGroup, B2csKind, B2csRow};
use crate::FormatError;

pub(super) fn b2cl_json(list: &[B2clGroup]) -> Result<String, FormatError> {
    map_arr(list, |i, group| {
        let path = format!("b2cl[{i}]");
        require_some(&format!("{path}.inv"), &group.invoices)?;
        let invs = map_arr(&group.invoices, |j, inv| {
            let p = format!("{path}.inv[{j}]");
            Ok(obj(&[
                kv("inum", &doc_number(&format!("{p}.inum"), &inv.num)?),
                kv("idt", &date_str(inv.date)),
                kv("val", &amt(inv.value)),
                kv("itms", &items_json(&format!("{p}.itms"), &inv.items)?),
            ]))
        })?;
        Ok(obj(&[kv("pos", &pos_str(group.pos)), kv("inv", &invs)]))
    })
}

pub(super) fn b2cs_json(list: &[B2csRow]) -> Result<String, FormatError> {
    map_arr(list, |_, row| {
        let mut f = vec![
            kv(
                "sply_ty",
                if row.intra_state {
                    "\"INTRA\""
                } else {
                    "\"INTER\""
                },
            ),
            kv("rt", &dec(row.line.rate.0)),
        ];
        match &row.kind {
            B2csKind::Other => f.push(kv("typ", "\"OE\"")),
            B2csKind::Ecommerce { etin } => {
                f.push(kv("typ", "\"E\""));
                f.push(kv("etin", &gstin_json(etin)?));
            }
        }
        f.push(kv("pos", &pos_str(row.pos)));
        f.push(kv("txval", &amt(row.line.taxable)));
        f.extend(tax_fields(row.line.tax));
        if !row.line.cess.is_zero() {
            f.push(kv("csamt", &amt(row.line.cess)));
        }
        Ok(obj(&f))
    })
}
