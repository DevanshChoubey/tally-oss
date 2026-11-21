//! e-Way bill JSON. **Card F5** implements this module.
//!
//! Frozen interface (plan card F5):
//!
//! ```text
//! pub struct EwayBill { .. }
//! pub fn ewb_to_json(e: &EwayBill) -> Result<String, FormatError>;
//! pub fn validate_ewb(e: &EwayBill) -> Result<(), Vec<FormatError>>;
//! ```
//!
//! # Output shape
//!
//! [`ewb_to_json`] writes the bulk-generation upload file of the e-way bill portal: an object
//! with `version` and `billLists`, one entry per bill. Amounts are JSON numbers written from
//! [`Paise`] with exactly two decimals; no float is involved. Rates and quantities are written
//! from [`Decimal`] with trailing zeros removed. Optional fields that are `None` are left out.
//!
//! # Provenance (rule A4)
//!
//! The field names, their order, the numeric codes (`supplyType`, `subSupplyType`, `docType`,
//! `transactionType`, `transMode`, `vehicleType`) and the limits (16-character document number,
//! 4000 km, 250 items, vehicle-number shapes) are **recalled** from the public NIC e-way bill
//! API/bulk-upload documentation, not yet quoted from it. They must be checked against the
//! official JSON schema and sample file before release; the exact-output test in
//! `tests/f5_ewb.rs` is marked accordingly. Nothing here computes a tax or a value: the writer
//! only checks that the bill is internally consistent and prints what it was given.
//!
//! Not checked here (needs data or a clock we do not have): the pincode-to-state mapping, the
//! 180-day age limit of the base document, the Rule 138 value threshold, and registration status
//! of the parties.
use rust_decimal::Decimal;
use time::Date;

use ledgerly_core::{Gstin, Hsn, Paise, StateCode};

use crate::FormatError;

/// `version` written to the bulk-generation upload file. Recalled, see the module docs.
pub const EWB_BULK_VERSION: &str = "1.0.0621";

/// Longest distance the portal accepts in `transDistance`, in kilometres. Recalled.
pub const MAX_DISTANCE_KM: u32 = 4000;

/// Most line items one e-way bill carries. Recalled.
pub const MAX_ITEMS: usize = 250;

/// Direction of the supply from the point of view of the user generating the bill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplyType {
    /// Outward supply (`O`).
    Outward,
    /// Inward supply (`I`).
    Inward,
}

/// Sub type of the supply (`subSupplyType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubSupplyType {
    /// Supply (1).
    Supply,
    /// Import (2). Inward only.
    Import,
    /// Export (3). Outward only.
    Export,
    /// Job work (4).
    JobWork,
    /// For own use (5).
    ForOwnUse,
    /// Job work returns (6).
    JobWorkReturns,
    /// Sales return (7).
    SalesReturn,
    /// Others (8); [`EwayBill::sub_supply_desc`] must then say what.
    Others,
    /// SKD or CKD or lots (9).
    SkdCkd,
    /// Line sales (10).
    LineSales,
    /// Recipient not known (11).
    RecipientNotKnown,
    /// Exhibition or fairs (12).
    ExhibitionOrFairs,
}

impl SubSupplyType {
    /// The numeric code written to `subSupplyType`.
    pub const fn code(self) -> u8 {
        match self {
            SubSupplyType::Supply => 1,
            SubSupplyType::Import => 2,
            SubSupplyType::Export => 3,
            SubSupplyType::JobWork => 4,
            SubSupplyType::ForOwnUse => 5,
            SubSupplyType::JobWorkReturns => 6,
            SubSupplyType::SalesReturn => 7,
            SubSupplyType::Others => 8,
            SubSupplyType::SkdCkd => 9,
            SubSupplyType::LineSales => 10,
            SubSupplyType::RecipientNotKnown => 11,
            SubSupplyType::ExhibitionOrFairs => 12,
        }
    }
}

/// Type of the base document (`docType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    /// Tax invoice (`INV`).
    Invoice,
    /// Bill of supply (`BIL`).
    BillOfSupply,
    /// Bill of entry (`BOE`).
    BillOfEntry,
    /// Delivery challan (`CHL`).
    DeliveryChallan,
    /// Credit note (`CNT`).
    CreditNote,
    /// Others (`OTH`).
    Others,
}

impl DocType {
    /// The code written to `docType`.
    pub const fn code(self) -> &'static str {
        match self {
            DocType::Invoice => "INV",
            DocType::BillOfSupply => "BIL",
            DocType::BillOfEntry => "BOE",
            DocType::DeliveryChallan => "CHL",
            DocType::CreditNote => "CNT",
            DocType::Others => "OTH",
        }
    }
}

/// Transaction type (`transactionType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    /// Regular (1): goods go from the billing address to the billing address.
    Regular,
    /// Bill to - ship to (2): delivery to a place other than the recipient's address.
    BillToShipTo,
    /// Bill from - dispatch from (3): dispatch from a place other than the supplier's address.
    BillFromDispatchFrom,
    /// Combination of 2 and 3 (4).
    Combination,
}

impl TransactionType {
    /// The numeric code written to `transactionType`.
    pub const fn code(self) -> u8 {
        match self {
            TransactionType::Regular => 1,
            TransactionType::BillToShipTo => 2,
            TransactionType::BillFromDispatchFrom => 3,
            TransactionType::Combination => 4,
        }
    }
}

/// Mode of transport (`transMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransMode {
    /// Road (1); needs a vehicle number.
    Road,
    /// Rail (2); needs a transport document number and date.
    Rail,
    /// Air (3); needs a transport document number and date.
    Air,
    /// Ship (4); needs a transport document number and date.
    Ship,
}

impl TransMode {
    /// The numeric code written to `transMode`.
    pub const fn code(self) -> u8 {
        match self {
            TransMode::Road => 1,
            TransMode::Rail => 2,
            TransMode::Air => 3,
            TransMode::Ship => 4,
        }
    }
}

/// Vehicle type (`vehicleType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleType {
    /// Regular vehicle (`R`).
    Regular,
    /// Over-dimensional cargo (`O`).
    OverDimensional,
}

impl VehicleType {
    /// The code written to `vehicleType`.
    pub const fn code(self) -> &'static str {
        match self {
            VehicleType::Regular => "R",
            VehicleType::OverDimensional => "O",
        }
    }
}

/// One end of the movement: the party, its address, and where the goods really go or start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    /// GSTIN of the party; `None` for an unregistered person, written as `URP`.
    pub gstin: Option<Gstin>,
    /// Trade name.
    pub trade_name: String,
    /// Address line 1 (required).
    pub addr1: String,
    /// Address line 2.
    pub addr2: Option<String>,
    /// Place (city or town), required.
    pub place: String,
    /// Six-digit pincode.
    pub pincode: u32,
    /// State code of the party's address.
    pub state_code: StateCode,
    /// State code where the goods actually start (from) or end (to). Equals
    /// [`Party::state_code`] for a [`TransactionType::Regular`] bill.
    pub actual_state_code: StateCode,
}

/// Part B: the transport details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transport {
    /// Transporter id: a GSTIN or a TRANSIN (15 characters).
    pub transporter_id: Option<String>,
    /// Transporter name.
    pub transporter_name: Option<String>,
    /// Mode of transport; `None` when only Part A is generated and a transporter will add Part B.
    pub mode: Option<TransMode>,
    /// Approximate distance in kilometres, `0..=4000`. `0` asks the portal to work it out.
    pub distance_km: u32,
    /// Transport document number (rail, air, ship; also allowed for road).
    pub doc_no: Option<String>,
    /// Transport document date.
    pub doc_date: Option<Date>,
    /// Vehicle number, upper case with no spaces or hyphens.
    pub vehicle_no: Option<String>,
    /// Vehicle type; required when a vehicle number is given.
    pub vehicle_type: Option<VehicleType>,
}

/// One line of the item list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EwayItem {
    /// Product name.
    pub product_name: String,
    /// Product description.
    pub product_desc: Option<String>,
    /// HSN code. Written as a number, so leading zeros are dropped (the portal field is numeric).
    pub hsn: Hsn,
    /// Quantity, greater than zero.
    pub quantity: Decimal,
    /// Unit quantity code such as `NOS` or `KGS` (up to three capital letters).
    pub qty_unit: String,
    /// Taxable value of the line.
    pub taxable_amount: Paise,
    /// CGST rate in percent.
    pub cgst_rate: Decimal,
    /// SGST/UTGST rate in percent.
    pub sgst_rate: Decimal,
    /// IGST rate in percent.
    pub igst_rate: Decimal,
    /// Cess rate in percent.
    pub cess_rate: Decimal,
    /// Non-advalorem cess amount.
    pub cess_non_advol: Paise,
}

/// An e-way bill, Part A and Part B, ready to write as portal JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EwayBill {
    /// GSTIN of the user who generates the bill (`userGstin`).
    pub user_gstin: Gstin,
    /// Outward or inward.
    pub supply_type: SupplyType,
    /// Sub type of the supply.
    pub sub_supply_type: SubSupplyType,
    /// Description when the sub type is [`SubSupplyType::Others`].
    pub sub_supply_desc: Option<String>,
    /// Type of the base document.
    pub doc_type: DocType,
    /// Base document number: 1 to 16 characters of `A-Z a-z 0-9 / -`.
    pub doc_no: String,
    /// Base document date.
    pub doc_date: Date,
    /// Supplier side.
    pub from: Party,
    /// Recipient side.
    pub to: Party,
    /// Transaction type.
    pub transaction_type: TransactionType,
    /// Other value (round-off, freight, TCS); negative for a deduction.
    pub other_value: Paise,
    /// Total taxable value; must equal the sum of the items.
    pub total_value: Paise,
    /// CGST amount.
    pub cgst_value: Paise,
    /// SGST/UTGST amount.
    pub sgst_value: Paise,
    /// IGST amount.
    pub igst_value: Paise,
    /// Cess amount.
    pub cess_value: Paise,
    /// Non-advalorem cess amount.
    pub cess_non_advol_value: Paise,
    /// Total invoice value; must equal total value + taxes + cesses + other value.
    pub tot_inv_value: Paise,
    /// Part B.
    pub transport: Transport,
    /// Item list, 1 to 250 lines.
    pub items: Vec<EwayItem>,
}

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
    check_transport(&e.transport, &mut errs);
    if errs.is_empty() { Ok(()) } else { Err(errs) }
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

fn money(p: Paise) -> String {
    let v = p.get();
    let sign = if v < 0 { "-" } else { "" };
    let abs = v.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
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
    let digits = i.hsn.as_str().trim_start_matches('0');
    o.raw("hsnCode", if digits.is_empty() { "0" } else { digits });
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
