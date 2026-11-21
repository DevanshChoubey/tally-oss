//! GSTR-1 and GSTR-1A data shapes and JSON writer. **Card F1** implements this module.
//!
//! Frozen interface (plan card F1):
//!
//! ```text
//! pub struct Gstr1 { .. }  pub struct Gstr1a { .. }   // sections: b2b, b2cl, b2cs, cdnr, cdnur, exp, nil, hsn, doc_issue
//! pub fn gstr1_to_json(r: &Gstr1) -> Result<String, FormatError>;
//! pub fn gstr1a_to_json(r: &Gstr1a) -> Result<String, FormatError>;
//! ```
//!
//! Output rules:
//!
//! - Amounts are JSON numbers with exactly two decimals, written from [`Paise`] by integer
//!   division. No float is ever built. Rates and quantities are written from `Decimal` without
//!   trailing zeros.
//! - Keys appear in a fixed order; list entries appear in the order the builder supplied (the
//!   builder sorts, rule A7). Empty sections are left out.
//! - Dates are `dd-mm-yyyy`; place of supply is the two-digit state code; `fp` is `MMYYYY` of the
//!   last month of the period.
//!
//! Provenance: the GSTN does not publish a JSON schema for GSTR-1 (research 04-gst.md, row
//! "GSTR-1 / IFF upload"). The key names and code values here (`inum`, `idt`, `itms`, `sply_ty`,
//! `INTRB2B`, `SEWP`, and so on) are **recalled from the offline-tool layout, not yet checked
//! against an official sample**. The owner supplies a sample from the offline tool, and until then
//! the format tests carry that caveat. Invoice-number length (at most 16 characters) is CGST Rules
//! 2017 rule 46(b).

use ledgerly_core::{Date, Decimal, Gstin, Hsn, Paise, Period, Qty, Rate, StateCode};

use crate::FormatError;

/// How the tax on a line is split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemTax {
    /// Inter-state supply: integrated tax only.
    Igst(Paise),
    /// Intra-state supply: central and state (or union-territory) tax.
    CgstSgst {
        /// Central tax.
        cgst: Paise,
        /// State or union-territory tax.
        sgst: Paise,
    },
}

/// One rate-wise line of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineItem {
    /// Total GST rate in percent (for example 18).
    pub rate: Rate,
    /// Taxable value.
    pub taxable: Paise,
    /// Tax split.
    pub tax: ItemTax,
    /// Compensation cess; zero when none.
    pub cess: Paise,
}

/// Invoice type for B2B and credit/debit-note rows (`inv_typ`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum B2bInvoiceType {
    /// Regular (`R`).
    Regular,
    /// SEZ supplies with payment of tax (`SEWP`).
    SezWithPayment,
    /// SEZ supplies without payment of tax (`SEWOP`).
    SezWithoutPayment,
    /// Deemed export (`DE`).
    DeemedExport,
}

/// A B2B invoice (table 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2bInvoice {
    /// Invoice number, 1 to 16 characters.
    pub num: String,
    /// Invoice date.
    pub date: Date,
    /// Invoice value including tax.
    pub value: Paise,
    /// Place of supply.
    pub pos: StateCode,
    /// Tax payable on reverse charge.
    pub reverse_charge: bool,
    /// Invoice type.
    pub inv_type: B2bInvoiceType,
    /// Rate-wise lines; at least one.
    pub items: Vec<LineItem>,
}

/// B2B invoices for one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2bParty {
    /// Recipient GSTIN.
    pub ctin: Gstin,
    /// Invoices; at least one.
    pub invoices: Vec<B2bInvoice>,
}

/// A B2C large invoice (table 5): inter-state, unregistered recipient, above the threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2clInvoice {
    /// Invoice number, 1 to 16 characters.
    pub num: String,
    /// Invoice date.
    pub date: Date,
    /// Invoice value including tax.
    pub value: Paise,
    /// Rate-wise lines; at least one.
    pub items: Vec<LineItem>,
}

/// B2C large invoices for one place of supply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2clGroup {
    /// Place of supply.
    pub pos: StateCode,
    /// Invoices; at least one.
    pub invoices: Vec<B2clInvoice>,
}

/// Whether a B2CS row was supplied through an e-commerce operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum B2csKind {
    /// Other than e-commerce (`OE`).
    Other,
    /// Through an e-commerce operator (`E`), identified by its GSTIN.
    Ecommerce {
        /// E-commerce operator GSTIN.
        etin: Gstin,
    },
}

/// A B2C small summary row (table 7): consolidated by place of supply and rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2csRow {
    /// `true` for an intra-state supply (`INTRA`), `false` for inter-state (`INTER`).
    pub intra_state: bool,
    /// Other or e-commerce.
    pub kind: B2csKind,
    /// Place of supply.
    pub pos: StateCode,
    /// Rate, taxable value, tax and cess.
    pub line: LineItem,
}

/// Credit note or debit note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// Credit note (`C`).
    Credit,
    /// Debit note (`D`).
    Debit,
}

/// A credit or debit note to a registered recipient (table 9B, CDNR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnrNote {
    /// Credit or debit.
    pub kind: NoteKind,
    /// Note number, 1 to 16 characters.
    pub num: String,
    /// Note date.
    pub date: Date,
    /// Note value including tax.
    pub value: Paise,
    /// Place of supply.
    pub pos: StateCode,
    /// Tax payable on reverse charge.
    pub reverse_charge: bool,
    /// Invoice type of the original supply.
    pub inv_type: B2bInvoiceType,
    /// Rate-wise lines; at least one.
    pub items: Vec<LineItem>,
}

/// Credit and debit notes for one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnrParty {
    /// Recipient GSTIN.
    pub ctin: Gstin,
    /// Notes; at least one.
    pub notes: Vec<CdnrNote>,
}

/// What an unregistered-recipient note relates to (`typ`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdnurType {
    /// Against a B2C large invoice (`B2CL`); the place of supply is reported.
    B2cl {
        /// Place of supply.
        pos: StateCode,
    },
    /// Against an export with payment of tax (`EXPWP`).
    ExportWithPayment,
    /// Against an export without payment of tax (`EXPWOP`).
    ExportWithoutPayment,
}

/// A credit or debit note to an unregistered recipient (table 9B, CDNUR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnurNote {
    /// What the note relates to.
    pub ur_type: CdnurType,
    /// Credit or debit.
    pub kind: NoteKind,
    /// Note number, 1 to 16 characters.
    pub num: String,
    /// Note date.
    pub date: Date,
    /// Note value including tax.
    pub value: Paise,
    /// Rate-wise lines; at least one.
    pub items: Vec<LineItem>,
}

/// One rate-wise export line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpItem {
    /// Total GST rate in percent.
    pub rate: Rate,
    /// Taxable value.
    pub taxable: Paise,
    /// Integrated tax; zero for an export without payment of tax.
    pub igst: Paise,
    /// Compensation cess; zero when none.
    pub cess: Paise,
}

/// An export invoice (table 6A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpInvoice {
    /// Invoice number, 1 to 16 characters.
    pub num: String,
    /// Invoice date.
    pub date: Date,
    /// Invoice value including tax.
    pub value: Paise,
    /// Port code, when a shipping bill exists.
    pub port_code: Option<String>,
    /// Shipping bill number and date, when it exists.
    pub shipping_bill: Option<(String, Date)>,
    /// Rate-wise lines; at least one.
    pub items: Vec<ExpItem>,
}

/// Export invoices of one kind (`exp_typ`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpGroup {
    /// `true` for export with payment of tax (`WPAY`), `false` without (`WOPAY`).
    pub with_payment: bool,
    /// Invoices; at least one.
    pub invoices: Vec<ExpInvoice>,
}

/// Which nil-rated, exempt and non-GST supplies a row covers (`sply_ty`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NilSupplyKind {
    /// Inter-state supplies to registered persons (`INTRB2B`).
    InterB2b,
    /// Inter-state supplies to unregistered persons (`INTRB2C`).
    InterB2c,
    /// Intra-state supplies to registered persons (`INTRAB2B`).
    IntraB2b,
    /// Intra-state supplies to unregistered persons (`INTRAB2C`).
    IntraB2c,
}

/// A nil-rated, exempt and non-GST supply row (table 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NilRow {
    /// Supply kind.
    pub kind: NilSupplyKind,
    /// Nil-rated supplies.
    pub nil_rated: Paise,
    /// Exempted supplies.
    pub exempt: Paise,
    /// Non-GST supplies.
    pub non_gst: Paise,
}

/// An HSN summary row (table 12), one per code, unit and rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HsnRow {
    /// HSN or SAC code.
    pub hsn: Hsn,
    /// Description; may be empty.
    pub description: String,
    /// Unit quantity code (`KGS`, `NOS`; `NA` for services).
    pub uqc: String,
    /// Total quantity.
    pub qty: Qty,
    /// Total value including tax.
    pub value: Paise,
    /// Total taxable value.
    pub taxable: Paise,
    /// Rate in percent.
    pub rate: Rate,
    /// Integrated tax.
    pub igst: Paise,
    /// Central tax.
    pub cgst: Paise,
    /// State or union-territory tax.
    pub sgst: Paise,
    /// Compensation cess.
    pub cess: Paise,
}

/// The kind of document a series covers (table 13, `doc_num`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    /// Invoices for outward supply (1).
    OutwardInvoice,
    /// Invoices for inward supply from an unregistered person (2).
    InwardFromUnregistered,
    /// Revised invoice (3).
    RevisedInvoice,
    /// Debit note (4).
    DebitNote,
    /// Credit note (5).
    CreditNote,
    /// Receipt voucher (6).
    ReceiptVoucher,
    /// Payment voucher (7).
    PaymentVoucher,
    /// Refund voucher (8).
    RefundVoucher,
    /// Delivery challan for job work (9).
    ChallanJobWork,
    /// Delivery challan for supply on approval (10).
    ChallanOnApproval,
    /// Delivery challan in case of liquid gas (11).
    ChallanLiquidGas,
    /// Delivery challan, other (12).
    ChallanOther,
}

/// One consecutive run of document numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocRange {
    /// First number in the run.
    pub from: String,
    /// Last number in the run.
    pub to: String,
    /// Total documents in the run, cancelled ones included.
    pub total: u32,
    /// Cancelled documents; at most `total`.
    pub cancelled: u32,
}

/// Document series of one type (table 13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocIssue {
    /// Document type.
    pub doc_type: DocType,
    /// Runs; at least one.
    pub ranges: Vec<DocRange>,
}

/// A GSTR-1 return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr1 {
    /// Supplier GSTIN.
    pub gstin: Gstin,
    /// Return period (one month, or a quarter under QRMP).
    pub period: Period,
    /// Aggregate turnover of the preceding financial year, when reported (`gt`).
    pub aggregate_turnover: Option<Paise>,
    /// Gross turnover of the current year up to the period, when reported (`cur_gt`).
    pub current_turnover: Option<Paise>,
    /// Table 4.
    pub b2b: Vec<B2bParty>,
    /// Table 5.
    pub b2cl: Vec<B2clGroup>,
    /// Table 7.
    pub b2cs: Vec<B2csRow>,
    /// Table 9B, registered recipients.
    pub cdnr: Vec<CdnrParty>,
    /// Table 9B, unregistered recipients.
    pub cdnur: Vec<CdnurNote>,
    /// Table 6A.
    pub exp: Vec<ExpGroup>,
    /// Table 8.
    pub nil: Vec<NilRow>,
    /// Table 12.
    pub hsn: Vec<HsnRow>,
    /// Table 13.
    pub doc_issue: Vec<DocIssue>,
}

/// An amended B2B invoice (table 9A, B2BA).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2baInvoice {
    /// Number of the original invoice.
    pub original_num: String,
    /// Date of the original invoice.
    pub original_date: Date,
    /// The invoice as amended.
    pub amended: B2bInvoice,
}

/// Amended B2B invoices for one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B2baParty {
    /// Recipient GSTIN.
    pub ctin: Gstin,
    /// Amended invoices; at least one.
    pub invoices: Vec<B2baInvoice>,
}

/// A GSTR-1A return (amendments made after GSTR-1 is filed and before GSTR-3B).
///
/// Only the B2B amendment section (`b2ba`) is written. The other amendment sections (B2CLA, B2CSA,
/// CDNRA, CDNURA, EXPA) are not modelled yet and have no field, so nothing can be dropped silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr1a {
    /// Supplier GSTIN.
    pub gstin: Gstin,
    /// Return period.
    pub period: Period,
    /// Amended B2B invoices.
    pub b2ba: Vec<B2baParty>,
}

// ---------------------------------------------------------------------------------------------
// JSON emission helpers. Everything is built as text so that amounts keep two decimals.

fn field_err(field: &str, reason: &str) -> FormatError {
    FormatError::Field {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

fn text(value: &str) -> Result<String, FormatError> {
    serde_json::to_string(value).map_err(|e| field_err("string", &e.to_string()))
}

fn kv(key: &str, value: &str) -> String {
    format!("\"{key}\":{value}")
}

fn obj(fields: &[String]) -> String {
    format!("{{{}}}", fields.join(","))
}

fn arr(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

/// Rupees with exactly two decimals, from integer paise.
fn amt(p: Paise) -> String {
    let abs = p.get().unsigned_abs();
    let sign = if p.is_negative() { "-" } else { "" };
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

fn dec(d: Decimal) -> String {
    d.normalize().to_string()
}

fn date_str(d: Date) -> String {
    format!(
        "\"{:02}-{:02}-{:04}\"",
        d.day(),
        u8::from(d.month()),
        d.year()
    )
}

fn pos_str(s: StateCode) -> String {
    format!("\"{:02}\"", s.get())
}

fn flag(b: bool) -> &'static str {
    if b { "\"Y\"" } else { "\"N\"" }
}

fn fp(period: Period) -> String {
    let end = period.to();
    format!("\"{:02}{:04}\"", u8::from(end.month()), end.year())
}

fn map_arr<T>(
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
fn doc_number(path: &str, num: &str) -> Result<String, FormatError> {
    let n = num.chars().count();
    if n == 0 || n > 16 {
        return Err(field_err(
            path,
            &format!("a document number has 1 to 16 characters, found {n}"),
        ));
    }
    text(num)
}

fn require_some<T>(path: &str, items: &[T]) -> Result<(), FormatError> {
    if items.is_empty() {
        Err(field_err(path, "at least one entry is required"))
    } else {
        Ok(())
    }
}

fn tax_fields(tax: ItemTax) -> Vec<String> {
    match tax {
        ItemTax::Igst(i) => vec![kv("iamt", &amt(i))],
        ItemTax::CgstSgst { cgst, sgst } => {
            vec![kv("camt", &amt(cgst)), kv("samt", &amt(sgst))]
        }
    }
}

/// `rt`, `txval`, tax amounts and (when not zero) `csamt`.
fn line_fields(line: &LineItem) -> Vec<String> {
    let mut f = vec![kv("rt", &dec(line.rate.0)), kv("txval", &amt(line.taxable))];
    f.extend(tax_fields(line.tax));
    if !line.cess.is_zero() {
        f.push(kv("csamt", &amt(line.cess)));
    }
    f
}

fn items_json(path: &str, items: &[LineItem]) -> Result<String, FormatError> {
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

fn inv_type_code(t: B2bInvoiceType) -> &'static str {
    match t {
        B2bInvoiceType::Regular => "R",
        B2bInvoiceType::SezWithPayment => "SEWP",
        B2bInvoiceType::SezWithoutPayment => "SEWOP",
        B2bInvoiceType::DeemedExport => "DE",
    }
}

fn note_code(k: NoteKind) -> &'static str {
    match k {
        NoteKind::Credit => "C",
        NoteKind::Debit => "D",
    }
}

fn b2b_invoice_fields(path: &str, inv: &B2bInvoice) -> Result<Vec<String>, FormatError> {
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

fn gstin_json(g: &Gstin) -> Result<String, FormatError> {
    text(g.as_str())
}

fn b2b_json(list: &[B2bParty], root: &str) -> Result<String, FormatError> {
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

fn b2cl_json(list: &[B2clGroup]) -> Result<String, FormatError> {
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

fn b2cs_json(list: &[B2csRow]) -> Result<String, FormatError> {
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

fn cdnr_json(list: &[CdnrParty]) -> Result<String, FormatError> {
    map_arr(list, |i, party| {
        let path = format!("cdnr[{i}]");
        require_some(&format!("{path}.nt"), &party.notes)?;
        let notes = map_arr(&party.notes, |j, n| {
            let p = format!("{path}.nt[{j}]");
            Ok(obj(&[
                kv("ntty", &text(note_code(n.kind))?),
                kv("nt_num", &doc_number(&format!("{p}.nt_num"), &n.num)?),
                kv("nt_dt", &date_str(n.date)),
                kv("val", &amt(n.value)),
                kv("pos", &pos_str(n.pos)),
                kv("rchrg", flag(n.reverse_charge)),
                kv("inv_typ", &text(inv_type_code(n.inv_type))?),
                kv("itms", &items_json(&format!("{p}.itms"), &n.items)?),
            ]))
        })?;
        Ok(obj(&[
            kv("ctin", &gstin_json(&party.ctin)?),
            kv("nt", &notes),
        ]))
    })
}

fn cdnur_json(list: &[CdnurNote]) -> Result<String, FormatError> {
    map_arr(list, |i, n| {
        let p = format!("cdnur[{i}]");
        let mut f = Vec::new();
        match n.ur_type {
            CdnurType::B2cl { pos } => {
                f.push(kv("typ", "\"B2CL\""));
                f.push(kv("pos", &pos_str(pos)));
            }
            CdnurType::ExportWithPayment => f.push(kv("typ", "\"EXPWP\"")),
            CdnurType::ExportWithoutPayment => f.push(kv("typ", "\"EXPWOP\"")),
        }
        f.push(kv("ntty", &text(note_code(n.kind))?));
        f.push(kv("nt_num", &doc_number(&format!("{p}.nt_num"), &n.num)?));
        f.push(kv("nt_dt", &date_str(n.date)));
        f.push(kv("val", &amt(n.value)));
        f.push(kv("itms", &items_json(&format!("{p}.itms"), &n.items)?));
        Ok(obj(&f))
    })
}

fn exp_item_json(item: &ExpItem) -> String {
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

fn exp_invoice_json(path: &str, inv: &ExpInvoice) -> Result<String, FormatError> {
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

fn exp_json(list: &[ExpGroup]) -> Result<String, FormatError> {
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

fn nil_code(k: NilSupplyKind) -> &'static str {
    match k {
        NilSupplyKind::InterB2b => "INTRB2B",
        NilSupplyKind::InterB2c => "INTRB2C",
        NilSupplyKind::IntraB2b => "INTRAB2B",
        NilSupplyKind::IntraB2c => "INTRAB2C",
    }
}

fn nil_json(list: &[NilRow]) -> Result<String, FormatError> {
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

fn hsn_json(list: &[HsnRow]) -> Result<String, FormatError> {
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

fn doc_type_number(t: DocType) -> u8 {
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

fn doc_issue_json(list: &[DocIssue]) -> Result<String, FormatError> {
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

fn b2ba_json(list: &[B2baParty]) -> Result<String, FormatError> {
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

/// Writes a GSTR-1 return as JSON.
///
/// # Errors
/// [`FormatError::Field`] naming the JSON path when a document number is empty or longer than 16
/// characters, a table has no lines, a unit quantity code is empty, or cancelled documents exceed
/// the total issued.
pub fn gstr1_to_json(r: &Gstr1) -> Result<String, FormatError> {
    let mut f = vec![kv("gstin", &gstin_json(&r.gstin)?), kv("fp", &fp(r.period))];
    if let Some(gt) = r.aggregate_turnover {
        f.push(kv("gt", &amt(gt)));
    }
    if let Some(gt) = r.current_turnover {
        f.push(kv("cur_gt", &amt(gt)));
    }
    if !r.b2b.is_empty() {
        f.push(kv("b2b", &b2b_json(&r.b2b, "b2b")?));
    }
    if !r.b2cl.is_empty() {
        f.push(kv("b2cl", &b2cl_json(&r.b2cl)?));
    }
    if !r.b2cs.is_empty() {
        f.push(kv("b2cs", &b2cs_json(&r.b2cs)?));
    }
    if !r.cdnr.is_empty() {
        f.push(kv("cdnr", &cdnr_json(&r.cdnr)?));
    }
    if !r.cdnur.is_empty() {
        f.push(kv("cdnur", &cdnur_json(&r.cdnur)?));
    }
    if !r.exp.is_empty() {
        f.push(kv("exp", &exp_json(&r.exp)?));
    }
    if !r.nil.is_empty() {
        f.push(kv("nil", &nil_json(&r.nil)?));
    }
    if !r.hsn.is_empty() {
        f.push(kv("hsn", &hsn_json(&r.hsn)?));
    }
    if !r.doc_issue.is_empty() {
        f.push(kv("doc_issue", &doc_issue_json(&r.doc_issue)?));
    }
    Ok(obj(&f))
}

/// Writes a GSTR-1A return as JSON.
///
/// # Errors
/// [`FormatError::Field`] naming the JSON path, as for [`gstr1_to_json`].
pub fn gstr1a_to_json(r: &Gstr1a) -> Result<String, FormatError> {
    let mut f = vec![kv("gstin", &gstin_json(&r.gstin)?), kv("fp", &fp(r.period))];
    if !r.b2ba.is_empty() {
        f.push(kv("b2ba", &b2ba_json(&r.b2ba)?));
    }
    Ok(obj(&f))
}
