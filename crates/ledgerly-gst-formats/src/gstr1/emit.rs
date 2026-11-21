//! Shared JSON text helpers for the GSTR-1 writer. Everything is built as text so that
//! amounts keep two decimals.

use super::{ItemTax, LineItem};
use crate::FormatError;
use ledgerly_core::{Date, Decimal, Paise, Period, StateCode};

pub(super) fn field_err(field: &str, reason: &str) -> FormatError {
    FormatError::Field {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

pub(super) fn text(value: &str) -> Result<String, FormatError> {
    serde_json::to_string(value).map_err(|e| field_err("string", &e.to_string()))
}

pub(super) fn kv(key: &str, value: &str) -> String {
    format!("\"{key}\":{value}")
}

pub(super) fn obj(fields: &[String]) -> String {
    format!("{{{}}}", fields.join(","))
}

pub(super) fn arr(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

/// Rupees with exactly two decimals, from integer paise.
pub(super) fn amt(p: Paise) -> String {
    let abs = p.get().unsigned_abs();
    let sign = if p.is_negative() { "-" } else { "" };
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

pub(super) fn dec(d: Decimal) -> String {
    d.normalize().to_string()
}

pub(super) fn date_str(d: Date) -> String {
    format!(
        "\"{:02}-{:02}-{:04}\"",
        d.day(),
        u8::from(d.month()),
        d.year()
    )
}

pub(super) fn pos_str(s: StateCode) -> String {
    format!("\"{:02}\"", s.get())
}

pub(super) fn flag(b: bool) -> &'static str {
    if b { "\"Y\"" } else { "\"N\"" }
}

pub(super) fn fp(period: Period) -> String {
    let end = period.to();
    format!("\"{:02}{:04}\"", u8::from(end.month()), end.year())
}

pub(super) fn map_arr<T>(
    items: &[T],
    f: impl Fn(usize, &T) -> Result<String, FormatError>,
) -> Result<String, FormatError> {
    let parts = items
        .iter()
        .enumerate()
        .map(|(i, t)| f(i, t))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(arr(&parts))
}

/// A document number: 1 to 16 characters (CGST Rules 2017 rule 46(b)).
pub(super) fn doc_number(path: &str, num: &str) -> Result<String, FormatError> {
    let n = num.chars().count();
    if n == 0 || n > 16 {
        return Err(field_err(
            path,
            &format!("a document number has 1 to 16 characters, found {n}"),
        ));
    }
    text(num)
}

pub(super) fn require_some<T>(path: &str, items: &[T]) -> Result<(), FormatError> {
    if items.is_empty() {
        Err(field_err(path, "at least one entry is required"))
    } else {
        Ok(())
    }
}

pub(super) fn tax_fields(tax: ItemTax) -> Vec<String> {
    match tax {
        ItemTax::Igst(i) => vec![kv("iamt", &amt(i))],
        ItemTax::CgstSgst { cgst, sgst } => {
            vec![kv("camt", &amt(cgst)), kv("samt", &amt(sgst))]
        }
    }
}

/// `rt`, `txval`, tax amounts and (when not zero) `csamt`.
pub(super) fn line_fields(line: &LineItem) -> Vec<String> {
    let mut f = vec![kv("rt", &dec(line.rate.0)), kv("txval", &amt(line.taxable))];
    f.extend(tax_fields(line.tax));
    if !line.cess.is_zero() {
        f.push(kv("csamt", &amt(line.cess)));
    }
    f
}

pub(super) fn items_json(path: &str, items: &[LineItem]) -> Result<String, FormatError> {
    require_some(path, items)?;
    let parts: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, line)| {
            obj(&[
                kv("num", &i.saturating_add(1).to_string()),
                kv("itm_det", &obj(&line_fields(line))),
            ])
        })
        .collect();
    Ok(arr(&parts))
}
