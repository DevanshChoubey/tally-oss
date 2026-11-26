//! e-Invoice INV-01 JSON and local validation. **Card F4** implements this module.
//!
//! Frozen interface (plan card F4):
//!
//! ```text
//! pub struct EInvoice { .. }
//! pub fn einvoice_to_json(e: &EInvoice) -> Result<String, FormatError>;
//! pub fn validate_einvoice(e: &EInvoice) -> Result<(), Vec<FormatError>>;
//! ```
//!
//! The JSON follows the NIC INV-01 schema, version 1.1 (notified under N.60/2020-CT; reproduced by
//! ICAI, see `docs/research/04-gst.md` section 2.13). The field names, the key order and the party
//! length limits (`Addr2` 3 to 100; `Loc` 3 to 50 for the seller, 3 to 100 for the recipient) were
//! checked against the NIC e-Invoice sandbox schema page,
//! <https://einv-apisandbox.nic.in/version1.03/generate-irn.html>, read 2026-11-26. The other limits
//! are still as recalled from the schema: check them against that page before release.
//!
//! Validation is local and strict: amounts that must equal a sum are compared exactly, in paise,
//! because the IRP's rounding tolerance is unconfirmed (research 04, section 2.13). A strict check
//! can reject a document the IRP would accept; it never accepts a document with a wrong total.
//!
//! Item arithmetic is checked against the rounding-independent window: an amount that is an exact
//! decimal product (`Qty x UnitPrice` for `TotAmt`, `AssAmt x rate` for each tax head, with CGST
//! and SGST at half the GST rate on an intra-state supply) must equal that product rounded to
//! whole paise by any rule, that is its floor or its ceiling. These two rules are recalled from
//! the schema, not yet quoted from it (rule A5), so their tests are pending.
//!
//! Not modelled (a document needing them cannot be expressed, rather than being written wrongly):
//! export supply types (`EXPWP`, `EXPWOP`) and their `ExpDtls`/foreign-currency total, `DispDtls`,
//! `ShipDtls`, `PayDtls`, `RefDtls`, `AddlDocDtls`, `EwbDtls`, item batch and attribute details,
//! item-level other charges, non-ad-valorem cess, and `PreTaxVal`. Not checked (pending, no
//! derivable rule): the cess heads per item, the allowed GST rate list, the round-off limit, and
//! the earliest document date. `TotAmt` is not checked when `Qty` is absent.

use ledgerly_core::{Gstin, Hsn, MoneyError, Paise, StateCode};
use rust_decimal::Decimal;
use time::Date;

use crate::FormatError;

/// The schema version written to `Version`.
const SCHEMA_VERSION: &str = "1.1";

/// `TranDtls.SupTyp`: the kind of supply (domestic and SEZ/deemed-export kinds only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplyType {
    /// Business to business.
    B2b,
    /// Supply to a SEZ with payment of tax.
    Sezwp,
    /// Supply to a SEZ without payment of tax.
    Sezwop,
    /// Deemed export.
    Dexp,
}

impl SupplyType {
    /// The schema code.
    pub const fn code(self) -> &'static str {
        match self {
            SupplyType::B2b => "B2B",
            SupplyType::Sezwp => "SEZWP",
            SupplyType::Sezwop => "SEZWOP",
            SupplyType::Dexp => "DEXP",
        }
    }
}

/// `DocDtls.Typ`: the document type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    /// Tax invoice.
    Invoice,
    /// Credit note.
    CreditNote,
    /// Debit note.
    DebitNote,
}

impl DocType {
    /// The schema code.
    pub const fn code(self) -> &'static str {
        match self {
            DocType::Invoice => "INV",
            DocType::CreditNote => "CRN",
            DocType::DebitNote => "DBN",
        }
    }
}

/// A supplier or recipient block (`SellerDtls` or `BuyerDtls`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EInvParty {
    /// `Gstin`.
    pub gstin: Gstin,
    /// `LglNm`, 3 to 100 characters.
    pub legal_name: String,
    /// `TrdNm`, 3 to 100 characters when present.
    pub trade_name: Option<String>,
    /// `Addr1`, 1 to 100 characters.
    pub address1: String,
    /// `Addr2`, 3 to 100 characters when present.
    pub address2: Option<String>,
    /// `Loc`, 3 to 50 characters for the seller and 3 to 100 for the recipient.
    pub location: String,
    /// `Pin`, a 6-digit code from 100000 to 999999.
    pub pin: u32,
    /// `Stcd`; must match the first two characters of the GSTIN.
    pub state: StateCode,
    /// `Ph`, 6 to 12 digits when present.
    pub phone: Option<String>,
    /// `Em`, 6 to 100 characters containing `@` when present.
    pub email: Option<String>,
}

/// One `ItemList` entry. Amounts are paise; `SlNo` is the position in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EInvItem {
    /// `PrdDesc`, 3 to 300 characters when present.
    pub description: Option<String>,
    /// `IsServc`; must agree with the code (a SAC starts with `99`).
    pub is_service: bool,
    /// `HsnCd`.
    pub hsn: Hsn,
    /// `Qty`, at most 3 decimal places, not negative.
    pub quantity: Option<Decimal>,
    /// `FreeQty`, at most 3 decimal places, not negative.
    pub free_quantity: Option<Decimal>,
    /// `Unit` (a UQC code), 3 to 8 characters when present.
    pub unit: Option<String>,
    /// `UnitPrice` in rupees, at most 3 decimal places, not negative.
    pub unit_price: Decimal,
    /// `TotAmt`: gross amount before discount.
    pub gross_amount: Paise,
    /// `Discount`.
    pub discount: Paise,
    /// `AssAmt`: taxable value; must equal `gross_amount - discount`.
    pub assessable: Paise,
    /// `GstRt` in percent, at most 3 decimal places.
    pub gst_rate: Decimal,
    /// `IgstAmt`.
    pub igst: Paise,
    /// `CgstAmt`.
    pub cgst: Paise,
    /// `SgstAmt`.
    pub sgst: Paise,
    /// `CesAmt` (ad valorem cess).
    pub cess: Paise,
    /// `StateCesAmt` (ad valorem state cess).
    pub state_cess: Paise,
    /// `TotItemVal`: taxable value plus every tax amount of the item.
    pub total: Paise,
}

/// The `ValDtls` block. Tax and value totals must equal the sums over the items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EInvTotals {
    /// `AssVal`: sum of item taxable values.
    pub assessable: Paise,
    /// `CgstVal`.
    pub cgst: Paise,
    /// `SgstVal`.
    pub sgst: Paise,
    /// `IgstVal`.
    pub igst: Paise,
    /// `CesVal`.
    pub cess: Paise,
    /// `StCesVal`.
    pub state_cess: Paise,
    /// `Discount`: document-level discount, separate from item discounts.
    pub discount: Paise,
    /// `OthChrg`: document-level other charges.
    pub other_charges: Paise,
    /// `RndOffAmt`: rounding adjustment, may be negative.
    pub round_off: Paise,
    /// `TotInvVal`: `assessable + taxes + other_charges - discount + round_off`.
    pub total: Paise,
}

/// An e-invoice document in INV-01 shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EInvoice {
    /// `TranDtls.SupTyp`.
    pub supply_type: SupplyType,
    /// `TranDtls.RegRev`: tax payable on reverse charge.
    pub reverse_charge: bool,
    /// `TranDtls.EcmGstin`: the e-commerce operator, when the supply goes through one.
    pub ecom_gstin: Option<Gstin>,
    /// `TranDtls.IgstOnIntra`: IGST charged on an intra-state supply.
    pub igst_on_intra: bool,
    /// `DocDtls.Typ`.
    pub doc_type: DocType,
    /// `DocDtls.No`: 1 to 16 characters, first `a-zA-Z1-9`, then `a-zA-Z0-9/-`.
    pub doc_no: String,
    /// `DocDtls.Dt`.
    pub doc_date: Date,
    /// `SellerDtls`.
    pub seller: EInvParty,
    /// `BuyerDtls`.
    pub buyer: EInvParty,
    /// `BuyerDtls.Pos`: place of supply.
    pub place_of_supply: StateCode,
    /// `ItemList`: 1 to 1000 items.
    pub items: Vec<EInvItem>,
    /// `ValDtls`.
    pub totals: EInvTotals,
}

fn field_err(field: &str, reason: impl Into<String>) -> FormatError {
    FormatError::Field {
        field: field.to_owned(),
        reason: reason.into(),
    }
}

/// Rupees with exactly two decimals, for JSON numbers and messages.
fn rupees(p: Paise) -> String {
    let v = p.get();
    let abs = v.unsigned_abs();
    let sign = if v < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

fn check_len(errs: &mut Vec<FormatError>, field: &str, text: &str, min: usize, max: usize) {
    let n = text.chars().count();
    if !(min..=max).contains(&n) {
        errs.push(field_err(
            field,
            format!("must be {min} to {max} characters, found {n}"),
        ));
    }
}

fn check_opt_len(
    errs: &mut Vec<FormatError>,
    field: &str,
    text: Option<&str>,
    min: usize,
    max: usize,
) {
    if let Some(t) = text {
        check_len(errs, field, t, min, max);
    }
}

fn check_nonneg(errs: &mut Vec<FormatError>, field: &str, amount: Paise) {
    if amount.is_negative() {
        errs.push(field_err(
            field,
            format!("must not be negative, found {}", rupees(amount)),
        ));
    }
}

fn check_decimal(errs: &mut Vec<FormatError>, field: &str, d: Decimal, max_scale: u32) {
    if d.is_sign_negative() && !d.is_zero() {
        errs.push(field_err(field, format!("must not be negative, found {d}")));
    }
    if d.normalize().scale() > max_scale {
        errs.push(field_err(
            field,
            format!("allows at most {max_scale} decimal places, found {d}"),
        ));
    }
}

fn sum_by(items: &[EInvItem], f: impl Fn(&EInvItem) -> Paise) -> Result<Paise, MoneyError> {
    items
        .iter()
        .try_fold(Paise::ZERO, |acc, item| acc.checked_add(f(item)))
}

fn valid_doc_no(no: &str) -> bool {
    let mut chars = no.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || ('1'..='9').contains(&c));
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-')
}

fn validate_doc(e: &EInvoice, errs: &mut Vec<FormatError>) {
    let n = e.doc_no.chars().count();
    if n > 16 {
        errs.push(field_err(
            "DocDtls.No",
            format!("must be at most 16 characters, found {n}"),
        ));
    } else if !valid_doc_no(&e.doc_no) {
        errs.push(field_err(
            "DocDtls.No",
            "must start with a letter or a digit 1-9 and contain only letters, digits, '/' and '-'",
        ));
    }
    if !(1000..=9999).contains(&e.doc_date.year()) {
        errs.push(field_err("DocDtls.Dt", "the year must have four digits"));
    }
}

fn validate_party(section: &str, loc_max: usize, p: &EInvParty, errs: &mut Vec<FormatError>) {
    let name = |f: &str| format!("{section}.{f}");
    check_len(errs, &name("LglNm"), &p.legal_name, 3, 100);
    check_opt_len(errs, &name("TrdNm"), p.trade_name.as_deref(), 3, 100);
    check_len(errs, &name("Addr1"), &p.address1, 1, 100);
    check_opt_len(errs, &name("Addr2"), p.address2.as_deref(), 3, 100);
    check_len(errs, &name("Loc"), &p.location, 3, loc_max);
    if !(100_000..=999_999).contains(&p.pin) {
        errs.push(field_err(
            &name("Pin"),
            format!("must be a 6-digit code, found {}", p.pin),
        ));
    }
    if let Some(ph) = p.phone.as_deref() {
        check_len(errs, &name("Ph"), ph, 6, 12);
        if !ph.bytes().all(|b| b.is_ascii_digit()) {
            errs.push(field_err(&name("Ph"), "must contain only digits"));
        }
    }
    if let Some(em) = p.email.as_deref() {
        check_len(errs, &name("Em"), em, 6, 100);
        if !em.contains('@') {
            errs.push(field_err(&name("Em"), "must contain '@'"));
        }
    }
    if p.gstin.state() != Some(p.state) {
        errs.push(field_err(
            &name("Stcd"),
            format!(
                "state code {:02} does not match the GSTIN, which starts with {}",
                p.state.get(),
                p.gstin.as_str().get(0..2).unwrap_or_default()
            ),
        ));
    }
}

fn validate_item(idx: usize, it: &EInvItem, errs: &mut Vec<FormatError>) {
    let name = |f: &str| format!("ItemList[{idx}].{f}");
    check_opt_len(errs, &name("PrdDesc"), it.description.as_deref(), 3, 300);
    check_opt_len(errs, &name("Unit"), it.unit.as_deref(), 3, 8);
    if it.is_service != it.hsn.is_sac() {
        errs.push(field_err(
            &name("IsServc"),
            "is Y only for a SAC (a code starting with 99) and N only for an HSN",
        ));
    }
    if let Some(q) = it.quantity {
        check_decimal(errs, &name("Qty"), q, 3);
    }
    if let Some(q) = it.free_quantity {
        check_decimal(errs, &name("FreeQty"), q, 3);
    }
    check_decimal(errs, &name("UnitPrice"), it.unit_price, 3);
    check_decimal(errs, &name("GstRt"), it.gst_rate, 3);
    if it.gst_rate > Decimal::ONE_HUNDRED {
        errs.push(field_err(&name("GstRt"), "must not exceed 100"));
    }
    validate_item_amounts(idx, it, errs);
    validate_gross_amount(idx, it, errs);
}

/// Whether `found` (whole paise) is `exact` (paise, exact in decimal) rounded to whole paise by
/// any rounding rule: its floor or its ceiling.
fn within_paise_rounding(found: Paise, exact: Decimal) -> bool {
    let found = Decimal::from(found.get());
    found == exact.floor() || found == exact.ceil()
}

/// `TotAmt` is `Qty x UnitPrice` (rupees, stated in paise). The product is exact, so only a
/// whole-paise rounding of it is acceptable.
fn validate_gross_amount(idx: usize, it: &EInvItem, errs: &mut Vec<FormatError>) {
    let Some(qty) = it.quantity else {
        return;
    };
    let name = format!("ItemList[{idx}].TotAmt");
    let exact = qty
        .checked_mul(it.unit_price)
        .and_then(|v| v.checked_mul(Decimal::ONE_HUNDRED));
    match exact {
        Some(exact) if within_paise_rounding(it.gross_amount, exact) => {}
        Some(exact) => errs.push(field_err(
            &name,
            format!(
                "must be Qty x UnitPrice = {} rupees rounded to paise, found {}",
                (exact / Decimal::ONE_HUNDRED).round_dp(4),
                rupees(it.gross_amount)
            ),
        )),
        None => errs.push(field_err(
            &name,
            "cannot be checked: Qty x UnitPrice does not fit the arithmetic",
        )),
    }
}

/// Each item's CGST and SGST (intra-state, at half the GST rate) or IGST (otherwise) must be the
/// whole-paise rounding of `AssAmt x rate`. The heads that must be zero are left to
/// [`validate_tax_kind`]; cess is not checked here.
fn validate_item_tax(e: &EInvoice, idx: usize, it: &EInvItem, errs: &mut Vec<FormatError>) {
    let intra = e.place_of_supply == e.seller.state && !e.igst_on_intra;
    let divisor = if intra {
        Decimal::from(200)
    } else {
        Decimal::ONE_HUNDRED
    };
    let heads: [(&str, Paise, bool); 3] = [
        ("CgstAmt", it.cgst, intra),
        ("SgstAmt", it.sgst, intra),
        ("IgstAmt", it.igst, !intra),
    ];
    for (head, found, expected) in heads {
        if !expected {
            continue;
        }
        let name = format!("ItemList[{idx}].{head}");
        let exact = Decimal::from(it.assessable.get())
            .checked_mul(it.gst_rate)
            .and_then(|v| v.checked_div(divisor));
        match exact {
            Some(exact) if within_paise_rounding(found, exact) => {}
            Some(exact) => errs.push(field_err(
                &name,
                format!(
                    "must be AssAmt x rate = {} rupees rounded to paise, found {}",
                    (exact / Decimal::ONE_HUNDRED).round_dp(4),
                    rupees(found)
                ),
            )),
            None => errs.push(field_err(
                &name,
                "cannot be checked: AssAmt x rate does not fit the arithmetic",
            )),
        }
    }
}

fn validate_item_amounts(idx: usize, it: &EInvItem, errs: &mut Vec<FormatError>) {
    let name = |f: &str| format!("ItemList[{idx}].{f}");
    let amounts = [
        ("TotAmt", it.gross_amount),
        ("Discount", it.discount),
        ("AssAmt", it.assessable),
        ("IgstAmt", it.igst),
        ("CgstAmt", it.cgst),
        ("SgstAmt", it.sgst),
        ("CesAmt", it.cess),
        ("StateCesAmt", it.state_cess),
        ("TotItemVal", it.total),
    ];
    for (f, p) in amounts {
        check_nonneg(errs, &name(f), p);
    }
    match it.gross_amount.checked_sub(it.discount) {
        Ok(expected) if expected != it.assessable => errs.push(field_err(
            &name("AssAmt"),
            format!(
                "must equal TotAmt - Discount = {}, found {}",
                rupees(expected),
                rupees(it.assessable)
            ),
        )),
        Ok(_) => {}
        Err(e) => errs.push(field_err(&name("AssAmt"), e.to_string())),
    }
    let expected_total = [it.igst, it.cgst, it.sgst, it.cess, it.state_cess]
        .into_iter()
        .try_fold(it.assessable, Paise::checked_add);
    match expected_total {
        Ok(expected) if expected != it.total => errs.push(field_err(
            &name("TotItemVal"),
            format!(
                "must equal AssAmt plus all tax amounts = {}, found {}",
                rupees(expected),
                rupees(it.total)
            ),
        )),
        Ok(_) => {}
        Err(e) => errs.push(field_err(&name("TotItemVal"), e.to_string())),
    }
}

/// Intra-state supplies carry CGST and SGST; inter-state supplies carry IGST (IGST Act s.7, s.8),
/// unless `IgstOnIntra` is set.
fn validate_tax_kind(e: &EInvoice, errs: &mut Vec<FormatError>) {
    let same_state = e.place_of_supply == e.seller.state;
    if e.igst_on_intra && !same_state {
        errs.push(field_err(
            "TranDtls.IgstOnIntra",
            "is Y only when the place of supply is in the supplier's state",
        ));
    }
    let intra = same_state && !e.igst_on_intra;
    for (idx, it) in e.items.iter().enumerate() {
        if intra && !it.igst.is_zero() {
            errs.push(field_err(
                &format!("ItemList[{idx}].IgstAmt"),
                "must be zero for an intra-state supply; use CgstAmt and SgstAmt",
            ));
        }
        if !intra && (!it.cgst.is_zero() || !it.sgst.is_zero()) {
            errs.push(field_err(
                &format!("ItemList[{idx}].CgstAmt"),
                "CgstAmt and SgstAmt must be zero for an inter-state supply or IGST on intra-state; use IgstAmt",
            ));
        }
    }
}

fn expected_invoice_total(t: &EInvTotals) -> Result<Paise, MoneyError> {
    [
        t.cgst,
        t.sgst,
        t.igst,
        t.cess,
        t.state_cess,
        t.other_charges,
    ]
    .into_iter()
    .try_fold(t.assessable, Paise::checked_add)?
    .checked_sub(t.discount)?
    .checked_add(t.round_off)
}

fn compare_total(
    errs: &mut Vec<FormatError>,
    field: &str,
    what: &str,
    expected: Result<Paise, MoneyError>,
    found: Paise,
) {
    match expected {
        Ok(exp) if exp != found => errs.push(field_err(
            field,
            format!(
                "must equal {what} = {}, found {}",
                rupees(exp),
                rupees(found)
            ),
        )),
        Ok(_) => {}
        Err(e) => errs.push(field_err(field, e.to_string())),
    }
}

fn validate_totals(e: &EInvoice, errs: &mut Vec<FormatError>) {
    let t = &e.totals;
    let sum = "the sum of the items";
    compare_total(
        errs,
        "ValDtls.AssVal",
        sum,
        sum_by(&e.items, |i| i.assessable),
        t.assessable,
    );
    compare_total(
        errs,
        "ValDtls.CgstVal",
        sum,
        sum_by(&e.items, |i| i.cgst),
        t.cgst,
    );
    compare_total(
        errs,
        "ValDtls.SgstVal",
        sum,
        sum_by(&e.items, |i| i.sgst),
        t.sgst,
    );
    compare_total(
        errs,
        "ValDtls.IgstVal",
        sum,
        sum_by(&e.items, |i| i.igst),
        t.igst,
    );
    compare_total(
        errs,
        "ValDtls.CesVal",
        sum,
        sum_by(&e.items, |i| i.cess),
        t.cess,
    );
    compare_total(
        errs,
        "ValDtls.StCesVal",
        sum,
        sum_by(&e.items, |i| i.state_cess),
        t.state_cess,
    );
    compare_total(
        errs,
        "ValDtls.TotInvVal",
        "AssVal + taxes + OthChrg - Discount + RndOffAmt",
        expected_invoice_total(t),
        t.total,
    );
    let non_negative = [
        ("AssVal", t.assessable),
        ("CgstVal", t.cgst),
        ("SgstVal", t.sgst),
        ("IgstVal", t.igst),
        ("CesVal", t.cess),
        ("StCesVal", t.state_cess),
        ("Discount", t.discount),
        ("OthChrg", t.other_charges),
        ("TotInvVal", t.total),
    ];
    for (f, p) in non_negative {
        check_nonneg(errs, &format!("ValDtls.{f}"), p);
    }
}

/// Validates an e-invoice locally against the INV-01 rules this module knows.
///
/// Every problem found is returned, each naming the schema field (`DocDtls.No`,
/// `ItemList[0].HsnCd`, `ValDtls.AssVal`, ...).
///
/// # Errors
/// A non-empty list of [`FormatError::Field`] values, one per problem.
pub fn validate_einvoice(e: &EInvoice) -> Result<(), Vec<FormatError>> {
    let mut errs = Vec::new();
    validate_doc(e, &mut errs);
    validate_party("SellerDtls", 50, &e.seller, &mut errs);
    validate_party("BuyerDtls", 100, &e.buyer, &mut errs);
    if e.items.is_empty() || e.items.len() > 1000 {
        errs.push(field_err(
            "ItemList",
            format!("must hold 1 to 1000 items, found {}", e.items.len()),
        ));
    }
    for (idx, it) in e.items.iter().enumerate() {
        validate_item(idx, it, &mut errs);
        validate_item_tax(e, idx, it, &mut errs);
    }
    validate_tax_kind(e, &mut errs);
    validate_totals(e, &mut errs);
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// A JSON object under construction; keys keep insertion order.
struct Obj(Vec<(&'static str, String)>);

impl Obj {
    fn new() -> Self {
        Obj(Vec::new())
    }

    fn raw(&mut self, key: &'static str, value: String) {
        self.0.push((key, value));
    }

    fn text(&mut self, key: &'static str, value: &str) {
        self.0.push((key, json_string(value)));
    }

    fn opt_text(&mut self, key: &'static str, value: Option<&str>) {
        if let Some(v) = value {
            self.text(key, v);
        }
    }

    fn amount(&mut self, key: &'static str, value: Paise) {
        self.raw(key, rupees(value));
    }

    fn amount_if_nonzero(&mut self, key: &'static str, value: Paise) {
        if !value.is_zero() {
            self.amount(key, value);
        }
    }

    fn decimal(&mut self, key: &'static str, value: Decimal) {
        self.raw(key, value.normalize().to_string());
    }

    fn yes_no(&mut self, key: &'static str, value: bool) {
        self.text(key, if value { "Y" } else { "N" });
    }

    fn render(self) -> String {
        let body: Vec<String> = self
            .0
            .into_iter()
            .map(|(k, v)| format!("{}:{v}", json_string(k)))
            .collect();
        format!("{{{}}}", body.join(","))
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let n = usize::try_from(u32::from(c)).unwrap_or_default();
                out.push_str("\\u00");
                for nibble in [(n >> 4) & 0xf, n & 0xf] {
                    out.push(HEX.get(nibble).map_or('0', |&b| char::from(b)));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn party_json(p: &EInvParty, pos: Option<StateCode>) -> String {
    let mut o = Obj::new();
    o.text("Gstin", p.gstin.as_str());
    o.text("LglNm", &p.legal_name);
    o.opt_text("TrdNm", p.trade_name.as_deref());
    if let Some(pos) = pos {
        o.text("Pos", &format!("{:02}", pos.get()));
    }
    o.text("Addr1", &p.address1);
    o.opt_text("Addr2", p.address2.as_deref());
    o.text("Loc", &p.location);
    o.raw("Pin", p.pin.to_string());
    o.text("Stcd", &format!("{:02}", p.state.get()));
    o.opt_text("Ph", p.phone.as_deref());
    o.opt_text("Em", p.email.as_deref());
    o.render()
}

fn item_json(idx: usize, it: &EInvItem) -> String {
    let mut o = Obj::new();
    o.text("SlNo", &idx.saturating_add(1).to_string());
    o.opt_text("PrdDesc", it.description.as_deref());
    o.yes_no("IsServc", it.is_service);
    o.text("HsnCd", it.hsn.as_str());
    if let Some(q) = it.quantity {
        o.decimal("Qty", q);
    }
    if let Some(q) = it.free_quantity {
        o.decimal("FreeQty", q);
    }
    o.opt_text("Unit", it.unit.as_deref());
    o.decimal("UnitPrice", it.unit_price);
    o.amount("TotAmt", it.gross_amount);
    o.amount_if_nonzero("Discount", it.discount);
    o.amount("AssAmt", it.assessable);
    o.decimal("GstRt", it.gst_rate);
    o.amount("IgstAmt", it.igst);
    o.amount("CgstAmt", it.cgst);
    o.amount("SgstAmt", it.sgst);
    o.amount_if_nonzero("CesAmt", it.cess);
    o.amount_if_nonzero("StateCesAmt", it.state_cess);
    o.amount("TotItemVal", it.total);
    o.render()
}

fn totals_json(t: &EInvTotals) -> String {
    let mut o = Obj::new();
    o.amount("AssVal", t.assessable);
    o.amount("CgstVal", t.cgst);
    o.amount("SgstVal", t.sgst);
    o.amount("IgstVal", t.igst);
    o.amount_if_nonzero("CesVal", t.cess);
    o.amount_if_nonzero("StCesVal", t.state_cess);
    o.amount_if_nonzero("Discount", t.discount);
    o.amount_if_nonzero("OthChrg", t.other_charges);
    o.amount("RndOffAmt", t.round_off);
    o.amount("TotInvVal", t.total);
    o.render()
}

fn date_dmy(d: Date) -> Result<String, FormatError> {
    let year = d.year();
    if !(1000..=9999).contains(&year) {
        return Err(field_err("DocDtls.Dt", "the year must have four digits"));
    }
    Ok(format!(
        "{:02}/{:02}/{year:04}",
        d.day(),
        u8::from(d.month())
    ))
}

/// Writes the e-invoice as INV-01 v1.1 JSON (compact, keys in schema order).
///
/// The document is validated first; amounts are written as exact rupee decimals with two places.
///
/// # Errors
/// The first [`FormatError`] from [`validate_einvoice`], naming the offending field.
pub fn einvoice_to_json(e: &EInvoice) -> Result<String, FormatError> {
    if let Err(errs) = validate_einvoice(e) {
        return Err(errs
            .into_iter()
            .next()
            .unwrap_or_else(|| field_err("EInvoice", "is not valid")));
    }
    let mut tran = Obj::new();
    tran.text("TaxSch", "GST");
    tran.text("SupTyp", e.supply_type.code());
    tran.yes_no("RegRev", e.reverse_charge);
    tran.opt_text("EcmGstin", e.ecom_gstin.as_ref().map(Gstin::as_str));
    tran.yes_no("IgstOnIntra", e.igst_on_intra);

    let mut doc = Obj::new();
    doc.text("Typ", e.doc_type.code());
    doc.text("No", &e.doc_no);
    doc.text("Dt", &date_dmy(e.doc_date)?);

    let items: Vec<String> = e
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| item_json(i, it))
        .collect();

    let mut root = Obj::new();
    root.text("Version", SCHEMA_VERSION);
    root.raw("TranDtls", tran.render());
    root.raw("DocDtls", doc.render());
    root.raw("SellerDtls", party_json(&e.seller, None));
    root.raw("BuyerDtls", party_json(&e.buyer, Some(e.place_of_supply)));
    root.raw("ItemList", format!("[{}]", items.join(",")));
    root.raw("ValDtls", totals_json(&e.totals));
    Ok(root.render())
}
