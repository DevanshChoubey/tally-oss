//! The e-way bill bulk-generation JSON writer, see the module docs of `ewb`.

use ledgerly_core::Gstin;
use rust_decimal::Decimal;
use time::Date;

use super::{
    EWB_BULK_VERSION, EwayBill, EwayItem, Party, SupplyType, Transport, money, validate_ewb,
};
use crate::FormatError;

/// Writes the bill as the portal's bulk-generation JSON (compact, fixed key order).
///
/// The bill is validated first; a bill with any problem is not written.
///
/// # Errors
/// The first [`FormatError`] that [`validate_ewb`] finds.
pub fn ewb_to_json(e: &EwayBill) -> Result<String, FormatError> {
    validate_ewb(e).map_err(first_error)?;
    let bill = bill_object(e).finish();
    Ok(format!(
        "{{\"version\":{},\"billLists\":[{bill}]}}",
        json_text(EWB_BULK_VERSION)
    ))
}

fn first_error(errs: Vec<FormatError>) -> FormatError {
    errs.into_iter()
        .next()
        .unwrap_or_else(|| FormatError::Field {
            field: "ewb".to_owned(),
            reason: "validation failed".to_owned(),
        })
}

/// A JSON object written key by key, so numbers can be printed from exact types.
struct Obj {
    parts: Vec<String>,
}

impl Obj {
    fn new() -> Self {
        Obj { parts: Vec::new() }
    }

    fn raw(&mut self, key: &str, value: &str) {
        self.parts.push(format!("{}:{value}", json_text(key)));
    }

    fn text(&mut self, key: &str, value: &str) {
        self.raw(key, &json_text(value));
    }

    fn finish(self) -> String {
        format!("{{{}}}", self.parts.join(","))
    }
}

fn json_text(s: &str) -> String {
    serde_json::Value::String(s.to_owned()).to_string()
}

fn number(d: Decimal) -> String {
    d.normalize().to_string()
}

fn portal_date(d: Date) -> String {
    format!("{:02}/{:02}/{:04}", d.day(), u8::from(d.month()), d.year())
}

fn bill_object(e: &EwayBill) -> Obj {
    let mut o = Obj::new();
    o.text("userGstin", e.user_gstin.as_str());
    o.text(
        "supplyType",
        match e.supply_type {
            SupplyType::Outward => "O",
            SupplyType::Inward => "I",
        },
    );
    o.raw("subSupplyType", &e.sub_supply_type.code().to_string());
    if let Some(d) = &e.sub_supply_desc {
        o.text("subSupplyDesc", d);
    }
    o.text("docType", e.doc_type.code());
    o.text("docNo", &e.doc_no);
    o.text("docDate", &portal_date(e.doc_date));
    push_party(&mut o, "from", "From", &e.from);
    push_party(&mut o, "to", "To", &e.to);
    o.raw("transactionType", &e.transaction_type.code().to_string());
    o.raw("otherValue", &money(e.other_value));
    o.raw("totalValue", &money(e.total_value));
    o.raw("cgstValue", &money(e.cgst_value));
    o.raw("sgstValue", &money(e.sgst_value));
    o.raw("igstValue", &money(e.igst_value));
    o.raw("cessValue", &money(e.cess_value));
    o.raw("cessNonAdvolValue", &money(e.cess_non_advol_value));
    o.raw("totInvValue", &money(e.tot_inv_value));
    push_transport(&mut o, &e.transport);
    let items: Vec<String> = e.items.iter().map(|i| item_object(i).finish()).collect();
    o.raw("itemList", &format!("[{}]", items.join(",")));
    o
}

fn push_party(o: &mut Obj, lower: &str, upper: &str, p: &Party) {
    let gstin = p.gstin.as_ref().map_or("URP", Gstin::as_str);
    o.text(&format!("{lower}Gstin"), gstin);
    o.text(&format!("{lower}TrdName"), &p.trade_name);
    o.text(&format!("{lower}Addr1"), &p.addr1);
    if let Some(a) = &p.addr2 {
        o.text(&format!("{lower}Addr2"), a);
    }
    o.text(&format!("{lower}Place"), &p.place);
    o.raw(&format!("{lower}Pincode"), &p.pincode.to_string());
    o.raw(
        &format!("act{upper}StateCode"),
        &p.actual_state_code.get().to_string(),
    );
    o.raw(
        &format!("{lower}StateCode"),
        &p.state_code.get().to_string(),
    );
}

fn push_transport(o: &mut Obj, t: &Transport) {
    if let Some(v) = &t.transporter_id {
        o.text("transporterId", v);
    }
    if let Some(v) = &t.transporter_name {
        o.text("transporterName", v);
    }
    if let Some(v) = &t.doc_no {
        o.text("transDocNo", v);
    }
    if let Some(d) = t.doc_date {
        o.text("transDocDate", &portal_date(d));
    }
    if let Some(m) = t.mode {
        o.raw("transMode", &m.code().to_string());
    }
    o.text("transDistance", &t.distance_km.to_string());
    if let Some(v) = &t.vehicle_no {
        o.text("vehicleNo", v);
    }
    if let Some(v) = t.vehicle_type {
        o.text("vehicleType", v.code());
    }
}

fn item_object(i: &EwayItem) -> Obj {
    let mut o = Obj::new();
    o.text("productName", &i.product_name);
    if let Some(d) = &i.product_desc {
        o.text("productDesc", d);
    }
    o.raw("hsnCode", i.hsn.as_str());
    o.raw("quantity", &number(i.quantity));
    o.text("qtyUnit", &i.qty_unit);
    o.raw("taxableAmount", &money(i.taxable_amount));
    o.raw("sgstRate", &number(i.sgst_rate));
    o.raw("cgstRate", &number(i.cgst_rate));
    o.raw("igstRate", &number(i.igst_rate));
    o.raw("cessRate", &number(i.cess_rate));
    o.raw("cessNonAdvol", &money(i.cess_non_advol));
    o
}
