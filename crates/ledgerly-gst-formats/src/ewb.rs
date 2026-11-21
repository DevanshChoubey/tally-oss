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
//! `tests/f5_ewb.rs` is marked accordingly. Nothing here computes a tax or a value to write: the
//! writer checks that the bill is internally consistent (its totals add up, and each header tax
//! head lies within rounding of the items' own tax, see `check_tax_heads`) and prints what it was
//! given. An HSN code with a leading zero is refused as [`FormatError::Unsupported`] until the
//! numeric `hsnCode` field is confirmed against the official schema, rather than written as a
//! different code.
//!
//! Not checked here (needs data or a clock we do not have): the pincode-to-state mapping, the
//! 180-day age limit of the base document, the Rule 138 value threshold, and registration status
//! of the parties.
use rust_decimal::Decimal;
use time::Date;

use ledgerly_core::{Gstin, Hsn, Paise, StateCode};


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
    /// HSN code. A code with a leading zero is refused by [`validate_ewb`] (see the module docs).
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

mod validate;
mod write;

pub use validate::{is_valid_vehicle_no, validate_ewb};
pub use write::ewb_to_json;

fn money(p: Paise) -> String {
    let v = p.get();
    let sign = if v < 0 { "-" } else { "" };
    let abs = v.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}
