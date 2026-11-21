//! Local validation of an e-way bill: the checks behind `validate_ewb`, see the module docs of
//! `ewb`.

use ledgerly_core::Paise;
use rust_decimal::Decimal;

use super::{
    EwayBill, EwayItem, MAX_DISTANCE_KM, MAX_ITEMS, Party, SubSupplyType, SupplyType, TransMode,
    TransactionType, Transport, money,
};
use crate::FormatError;

/// `true` when `v` is a vehicle number shape the portal accepts: a state series such as
/// `KA01AB1234` (2 letters, 2 digits, 0 to 3 letters, 4 digits) or a Bharat series such as
/// `22BH1234AA` (2 digits, `BH`, 4 digits, 1 or 2 letters). Upper case only, no separators.
/// Temporary-registration numbers are not recognised and are reported as invalid.
pub fn is_valid_vehicle_no(v: &str) -> bool {
    let b = v.as_bytes();
    is_state_series(b) || is_bharat_series(b)
}

fn is_state_series(b: &[u8]) -> bool {
    let Some((state, rest)) = b.split_at_checked(2) else {
        return false;
    };
    let Some((rto, rest)) = rest.split_at_checked(2) else {
        return false;
    };
    if !state.iter().all(u8::is_ascii_uppercase) || !rto.iter().all(u8::is_ascii_digit) {
        return false;
    }
    let letters = rest.iter().take_while(|c| c.is_ascii_uppercase()).count();
    let Some((_, number)) = rest.split_at_checked(letters) else {
        return false;
    };
    letters <= 3 && number.len() == 4 && number.iter().all(u8::is_ascii_digit)
}

fn is_bharat_series(b: &[u8]) -> bool {
    let Some((year, rest)) = b.split_at_checked(2) else {
        return false;
    };
    let Some((bh, rest)) = rest.split_at_checked(2) else {
        return false;
    };
    let Some((number, series)) = rest.split_at_checked(4) else {
        return false;
    };
    year.iter().all(u8::is_ascii_digit)
        && bh == b"BH".as_slice()
        && number.iter().all(u8::is_ascii_digit)
        && (1..=2).contains(&series.len())
        && series.iter().all(u8::is_ascii_uppercase)
}

fn push(errs: &mut Vec<FormatError>, field: &str, reason: &str) {
    errs.push(FormatError::Field {
        field: field.to_owned(),
        reason: reason.to_owned(),
    });
}

fn check_text(errs: &mut Vec<FormatError>, field: &str, value: &str, max: usize, required: bool) {
    if required && value.trim().is_empty() {
        push(errs, field, "is required");
    }
    if value.chars().count() > max {
        push(errs, field, &format!("is longer than {max} characters"));
    }
}

/// Checks a bill against the portal's rules that can be decided locally.
///
/// Every problem found is returned, in field order, so the user can fix them in one pass.
///
/// # Errors
/// A list of [`FormatError::Field`], one per problem, each naming the portal field (for example
/// `transDistance` or `itemList[0].hsnCode`).
pub fn validate_ewb(e: &EwayBill) -> Result<(), Vec<FormatError>> {
    let mut errs = Vec::new();
    check_header(e, &mut errs);
    check_party("from", &e.from, &mut errs);
    check_party("to", &e.to, &mut errs);
    check_transaction(e, &mut errs);
    check_items(&e.items, &mut errs);
    check_amounts(e, &mut errs);
    check_tax_heads(e, &mut errs);
    check_transport(&e.transport, &mut errs);
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// Lowest and highest whole-paise tax the items can add up to for one tax head.
///
/// An item's tax is `taxable x rate / 100`, exact in decimal. Whatever rounding the portal
/// applies to a whole-paise amount, the result is its floor or its ceiling, so the header total
/// lies between the sum of the floors and the sum of the ceilings. `None` when the arithmetic
/// overflows, which the caller reports rather than skipping.
fn tax_bounds(items: &[EwayItem], rate: fn(&EwayItem) -> Decimal) -> Option<(Decimal, Decimal)> {
    let mut low = Decimal::ZERO;
    let mut high = Decimal::ZERO;
    for it in items {
        let exact = Decimal::from(it.taxable_amount.get())
            .checked_mul(rate(it))?
            .checked_div(Decimal::ONE_HUNDRED)?;
        low = low.checked_add(exact.floor())?;
        high = high.checked_add(exact.ceil())?;
    }
    Some((low, high))
}

/// A tax head: its JSON name, its stated value and the item rate it is checked against.
type TaxHead = (&'static str, Paise, fn(&EwayItem) -> Decimal);

/// Each header tax head (CGST, SGST, IGST, cess) must lie within rounding of the sum of the
/// items' tax for that head (CGST Act s.15 and the rate in percent: tax is value times rate).
///
/// Skipped when an item has a rate outside 0 to 100: that item is already reported, and the tax
/// it implies is not defined.
fn check_tax_heads(e: &EwayBill, errs: &mut Vec<FormatError>) {
    let rates_defined = e.items.iter().all(|it| {
        [it.cgst_rate, it.sgst_rate, it.igst_rate, it.cess_rate]
            .into_iter()
            .all(|r| !r.is_sign_negative() && r <= Decimal::ONE_HUNDRED)
    });
    if !rates_defined {
        return;
    }
    let heads: [TaxHead; 4] = [
        ("cgstValue", e.cgst_value, |it| it.cgst_rate),
        ("sgstValue", e.sgst_value, |it| it.sgst_rate),
        ("igstValue", e.igst_value, |it| it.igst_rate),
        ("cessValue", e.cess_value, |it| it.cess_rate),
    ];
    for (field, header, rate) in heads {
        let Some((low, high)) = tax_bounds(&e.items, rate) else {
            push(
                errs,
                field,
                "cannot be checked against the items: the tax does not fit the arithmetic",
            );
            continue;
        };
        let found = Decimal::from(header.get());
        if found < low || found > high {
            push(
                errs,
                field,
                &format!(
                    "is {} but the items' tax on this head comes to {} to {}",
                    money(header),
                    (low / Decimal::ONE_HUNDRED).round_dp(2),
                    (high / Decimal::ONE_HUNDRED).round_dp(2)
                ),
            );
        }
    }
}

fn check_header(e: &EwayBill, errs: &mut Vec<FormatError>) {
    let n = e.doc_no.chars().count();
    if n == 0 || n > 16 {
        push(errs, "docNo", "must be 1 to 16 characters");
    }
    if !e
        .doc_no
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-')
    {
        push(
            errs,
            "docNo",
            "may contain only letters, digits, '/' and '-'",
        );
    }
    if e.sub_supply_type == SubSupplyType::Others
        && e.sub_supply_desc
            .as_deref()
            .is_none_or(|d| d.trim().is_empty())
    {
        push(
            errs,
            "subSupplyDesc",
            "is required when subSupplyType is Others",
        );
    }
    if e.sub_supply_type == SubSupplyType::Export && e.supply_type == SupplyType::Inward {
        push(errs, "subSupplyType", "Export is an outward supply");
    }
    if e.sub_supply_type == SubSupplyType::Import && e.supply_type == SupplyType::Outward {
        push(errs, "subSupplyType", "Import is an inward supply");
    }
}

fn check_party(prefix: &str, p: &Party, errs: &mut Vec<FormatError>) {
    check_text(errs, &format!("{prefix}TrdName"), &p.trade_name, 100, false);
    check_text(errs, &format!("{prefix}Addr1"), &p.addr1, 120, true);
    check_text(
        errs,
        &format!("{prefix}Addr2"),
        p.addr2.as_deref().unwrap_or_default(),
        120,
        false,
    );
    check_text(errs, &format!("{prefix}Place"), &p.place, 50, true);
    if !(100_000..=999_999).contains(&p.pincode) {
        push(
            errs,
            &format!("{prefix}Pincode"),
            "must be a six-digit pincode",
        );
    }
}

fn check_transaction(e: &EwayBill, errs: &mut Vec<FormatError>) {
    if e.transaction_type != TransactionType::Regular {
        return;
    }
    if e.from.actual_state_code != e.from.state_code {
        push(
            errs,
            "actFromStateCode",
            "a Regular transaction dispatches from the billing state",
        );
    }
    if e.to.actual_state_code != e.to.state_code {
        push(
            errs,
            "actToStateCode",
            "a Regular transaction delivers to the billing state",
        );
    }
}

fn check_items(items: &[EwayItem], errs: &mut Vec<FormatError>) {
    if items.is_empty() {
        push(errs, "itemList", "needs at least one item");
    }
    if items.len() > MAX_ITEMS {
        push(
            errs,
            "itemList",
            &format!("has more than {MAX_ITEMS} items"),
        );
    }
    for (i, it) in items.iter().enumerate() {
        check_item(i, it, errs);
    }
}

fn check_item(i: usize, it: &EwayItem, errs: &mut Vec<FormatError>) {
    let f = |name: &str| format!("itemList[{i}].{name}");
    check_text(errs, &f("productName"), &it.product_name, 100, true);
    check_text(
        errs,
        &f("productDesc"),
        it.product_desc.as_deref().unwrap_or_default(),
        100,
        false,
    );
    if it.hsn.as_str().starts_with('0') {
        errs.push(FormatError::Unsupported {
            reason: format!(
                "itemList[{i}].hsnCode: HSN {} starts with 0; the portal field is written as a \
                 number and leading zeros are not yet confirmed against the official schema \
                 (card F5), so the code is not written as a different one",
                it.hsn.as_str()
            ),
        });
    }
    if it.quantity <= Decimal::ZERO {
        push(errs, &f("quantity"), "must be greater than zero");
    }
    let unit_ok =
        (1..=3).contains(&it.qty_unit.len()) && it.qty_unit.bytes().all(|b| b.is_ascii_uppercase());
    if !unit_ok {
        push(
            errs,
            &f("qtyUnit"),
            "must be a unit code of up to 3 capital letters",
        );
    }
    if it.taxable_amount.is_negative() {
        push(errs, &f("taxableAmount"), "must not be negative");
    }
    if it.cess_non_advol.is_negative() {
        push(errs, &f("cessNonAdvol"), "must not be negative");
    }
    let rates = [
        ("cgstRate", it.cgst_rate),
        ("sgstRate", it.sgst_rate),
        ("igstRate", it.igst_rate),
        ("cessRate", it.cess_rate),
    ];
    for (name, r) in rates {
        if r.is_sign_negative() || r > Decimal::ONE_HUNDRED {
            push(errs, &f(name), "must be between 0 and 100 percent");
        }
    }
    if !it.igst_rate.is_zero() && (!it.cgst_rate.is_zero() || !it.sgst_rate.is_zero()) {
        push(
            errs,
            &f("igstRate"),
            "IGST cannot be charged together with CGST/SGST",
        );
    }
}

fn check_amounts(e: &EwayBill, errs: &mut Vec<FormatError>) {
    let named = [
        ("totalValue", e.total_value),
        ("cgstValue", e.cgst_value),
        ("sgstValue", e.sgst_value),
        ("igstValue", e.igst_value),
        ("cessValue", e.cess_value),
        ("cessNonAdvolValue", e.cess_non_advol_value),
        ("totInvValue", e.tot_inv_value),
    ];
    for (name, v) in named {
        if v.is_negative() {
            push(errs, name, "must not be negative");
        }
    }
    if !e.igst_value.is_zero() && (!e.cgst_value.is_zero() || !e.sgst_value.is_zero()) {
        push(
            errs,
            "igstValue",
            "IGST cannot be charged together with CGST/SGST",
        );
    }
    match e
        .items
        .iter()
        .try_fold(Paise::ZERO, |a, i| a.checked_add(i.taxable_amount))
    {
        Err(m) => push(errs, "totalValue", &m.to_string()),
        Ok(sum) if sum != e.total_value => push(
            errs,
            "totalValue",
            &format!(
                "is {} but the items add up to {}",
                money(e.total_value),
                money(sum)
            ),
        ),
        Ok(_) => {}
    }
    let parts = [
        e.cgst_value,
        e.sgst_value,
        e.igst_value,
        e.cess_value,
        e.cess_non_advol_value,
        e.other_value,
    ];
    match parts
        .iter()
        .try_fold(e.total_value, |a, &p| a.checked_add(p))
    {
        Err(m) => push(errs, "totInvValue", &m.to_string()),
        Ok(sum) if sum != e.tot_inv_value => push(
            errs,
            "totInvValue",
            &format!(
                "is {} but total value, taxes, cesses and other value add up to {}",
                money(e.tot_inv_value),
                money(sum)
            ),
        ),
        Ok(_) => {}
    }
}

fn check_transport(t: &Transport, errs: &mut Vec<FormatError>) {
    if t.distance_km > MAX_DISTANCE_KM {
        push(
            errs,
            "transDistance",
            &format!(
                "is {} km; the portal accepts at most {MAX_DISTANCE_KM}",
                t.distance_km
            ),
        );
    }
    if let Some(id) = &t.transporter_id {
        let ok = id.len() == 15
            && id
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        if !ok {
            push(
                errs,
                "transporterId",
                "must be a 15-character GSTIN or TRANSIN",
            );
        }
    }
    if let Some(v) = &t.vehicle_no {
        if !is_valid_vehicle_no(v) {
            push(
                errs,
                "vehicleNo",
                "is not a recognised vehicle number (capital letters and digits, no spaces or hyphens)",
            );
        }
        if t.mode != Some(TransMode::Road) {
            push(errs, "vehicleNo", "only applies to transMode Road");
        }
        if t.vehicle_type.is_none() {
            push(
                errs,
                "vehicleType",
                "is required when a vehicle number is given",
            );
        }
    }
    check_mode(t, errs);
}

fn check_mode(t: &Transport, errs: &mut Vec<FormatError>) {
    match t.mode {
        None => {
            if t.transporter_id.is_none() {
                push(
                    errs,
                    "transporterId",
                    "is required when Part B (transMode) is not given",
                );
            }
        }
        Some(TransMode::Road) => {
            if t.vehicle_no.is_none() {
                push(errs, "vehicleNo", "is required when transMode is Road");
            }
        }
        Some(TransMode::Rail | TransMode::Air | TransMode::Ship) => {
            if t.doc_no.as_deref().is_none_or(|d| d.trim().is_empty()) {
                push(errs, "transDocNo", "is required for rail, air and ship");
            }
            if t.doc_date.is_none() {
                push(errs, "transDocDate", "is required for rail, air and ship");
            }
        }
    }
}
