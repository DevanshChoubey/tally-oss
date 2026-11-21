//! Supply classification. **Card G6** implements this module.
//!
//! Frozen interface (plan card G6):
//!
//! ```text
//! pub enum SupplyKind { Taxable, Exempt, NilRated, NonGst, ExportWithPayment, ExportUnderLut, SezWithPayment, SezUnderLut, DeemedExport, Advance }
//! pub fn classify_line(v: &Voucher, line: usize, masters: &Masters, rules: &RuleSet) -> Result<Outcome<SupplyKind>, GstError>;
//! ```
//!
//! # Which line `line` is
//!
//! * Sales, credit-note, purchase and debit-note vouchers entered as **item invoices**: the
//!   position in `Voucher::inventory_lines`.
//! * The same vouchers entered as **accounting invoices**: the position in
//!   `Voucher::ledger_lines`; that ledger must carry a GST classification.
//! * **Receipt** vouchers: the position in `Voucher::ledger_lines` of the party line that carries
//!   the advance bill allocation.
//!
//! A position past the end of the list is a caller bug and a [`GstError::Invalid`].
//!
//! # What is decided
//!
//! The kind says how the line is reported (spec 6.3), not how much tax it bears; tax is G4's.
//!
//! * Domestic supply: the line's taxability on the time of supply (item history or ledger
//!   classification) gives `Taxable`, `Exempt`, `NilRated` or `NonGst`. A line that is `Taxable`
//!   with a rate of exactly 0% is `Unsupported`: whether it is nil-rated is the user's call
//!   (set the taxability to nil-rated).
//! * Recipient overseas (goods only, IGST Act s.2(5), s.16(1)(a)) or in an SEZ (s.16(1)(b)):
//!   zero-rated; the supplier's choice of paying IGST or supplying under bond or LUT
//!   (s.16(3)) is never defaulted and is read from [`SUPPLY_OPTION_KEY`].
//! * Deemed export (CGST Act s.147): a taxable supply to a regular registered recipient that the
//!   voucher marks with the option `deemed_export`. Whether a supply is one of the notified
//!   categories is the user's declaration; it is not derived.
//! * Advance: a receipt voucher line whose bill allocations are all advances.
//!
//! # The supplier's option
//!
//! The model has no typed field for the s.16(3) choice or for a deemed export, so it is read from
//! `Voucher::extra[SUPPLY_OPTION_KEY]` with the value `with_payment`, `under_lut` or
//! `deemed_export` (see `docs/interface-requests.md`, G6). An unknown value is `Unsupported`.
//! `under_lut` on a voucher that also posts an IGST amount is `Unsupported` (the two contradict:
//! no tax is charged under LUT, s.16(3)(a)).
//!
//! # Not decided (returns `Unsupported`)
//!
//! Cancelled vouchers and reverse charge; voucher kinds other than the five above; plain Dr/Cr
//! entry; UIN recipients; a customer ledger with no party details (not defaulted to a domestic
//! recipient; a voucher with no party at all is still read as a domestic cash sale); services to
//! an overseas recipient (s.13 and the s.2(6) conditions);
//! exempt or nil-rated supplies to an overseas or SEZ recipient; a supplier whose SEZ status
//! matters (the company's SEZ status is not modelled); inward supplies from a party that is not a
//! regular taxpayer; an option on an inward or advance line; payment before invoice without an
//! explicit supply date; advances from an overseas, SEZ or UIN party; payment vouchers.
//! Whether tax is due on an advance is not decided here: the kind only records that the line is
//! an advance, and the time-of-supply rules (card G12) decide the rest.
//!
//! The company's GST feature switch is not checked here (no company is passed in); G4 checks it.

use ledgerly_core::{Date, DrCr, Outcome};
use ledgerly_model::{
    BaseVoucherKind, BillRefKind, GstHistory, Masters, RegistrationType, StockItem, TaxLedgerKind,
    Taxability, Voucher, VoucherMode, VoucherStatus,
};

use crate::GstError;
use crate::pack::RuleSet;

/// The `Voucher::extra` key that carries the supplier's s.16(3) choice or a deemed-export
/// declaration. Values: `with_payment`, `under_lut`, `deemed_export`.
pub const SUPPLY_OPTION_KEY: &str = "gst_supply_option";

/// How a supply line is classified for returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupplyKind {
    /// Taxable domestic supply.
    Taxable,
    /// Exempt supply.
    Exempt,
    /// Nil-rated supply.
    NilRated,
    /// Outside GST.
    NonGst,
    /// Export of goods with payment of IGST.
    ExportWithPayment,
    /// Export of goods under bond or letter of undertaking, without payment of IGST.
    ExportUnderLut,
    /// Supply to an SEZ unit or developer with payment of IGST.
    SezWithPayment,
    /// Supply to an SEZ unit or developer under bond or LUT, without payment of IGST.
    SezUnderLut,
    /// Deemed export (CGST Act s.147).
    DeemedExport,
    /// Advance received against a future supply.
    Advance,
}

/// What a voucher's supply option says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupplyOption {
    /// Nothing stated.
    NotStated,
    /// IGST is paid on a zero-rated supply.
    WithPayment,
    /// Supplied under bond or LUT.
    UnderLut,
    /// A deemed export.
    DeemedExport,
}

/// Whether the supplied thing is goods or a service, where that can be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nature {
    Goods,
    Service,
    Unknown,
}

/// The kind of voucher, as far as classification is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Sales or credit note: the company supplies.
    Outward,
    /// Purchase or debit note: the party supplies.
    Inward,
    /// Receipt voucher.
    Receipt,
}

/// The recipient of an outward supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recipient {
    /// A regular taxpayer.
    Regular,
    /// Any other domestic recipient, or none recorded (a cash sale).
    OtherDomestic,
    /// An SEZ unit or developer.
    Sez,
    /// A party outside India.
    Overseas,
}

/// What the line itself says.
struct LineFacts {
    taxability: Taxability,
    /// The rate on the master, when there is one.
    rate_is_zero: bool,
    nature: Nature,
}

/// Returns the value of an `Outcome`, or leaves the function with the `Unsupported`.
macro_rules! supported {
    ($e:expr) => {
        match $e {
            Outcome::Value(v) => v,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        }
    };
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "builds the Result<Outcome> every check returns, so callers can `return unsupported(..)`"
)]
fn unsupported<T>(
    what: impl Into<String>,
    reason: impl Into<String>,
) -> Result<Outcome<T>, GstError> {
    Ok(Outcome::unsupported(what, reason))
}

fn not_found(what: String) -> GstError {
    GstError::NotFound { what }
}

fn invalid(field: &str, reason: String) -> GstError {
    GstError::Invalid {
        field: field.to_owned(),
        reason,
    }
}

/// Classifies one line of a voucher (see the module docs for what `line` indexes and what is
/// `Unsupported`).
///
/// `rules` must be the rule set selected for the voucher's time of supply (`supply_date` when
/// set, otherwise the voucher date).
///
/// # Errors
/// * [`GstError::NotFound`] when the voucher refers to a voucher type, party, ledger or stock
///   item that is not in `masters`.
/// * [`GstError::Invalid`] when `line` is past the end of the voucher's lines, an item has several
///   GST detail entries from the same date, or `rules` was selected for another date than the
///   time of supply.
pub fn classify_line(
    v: &Voucher,
    line: usize,
    masters: &Masters,
    rules: &RuleSet,
) -> Result<Outcome<SupplyKind>, GstError> {
    let direction = supported!(check_scope(v, masters)?);
    let tos = match direction {
        Direction::Outward | Direction::Inward => supported!(time_of_supply(v)),
        Direction::Receipt => v.date,
    };
    if rules.date() != tos {
        return Err(invalid(
            "rules",
            format!(
                "the rule set is for {} but the time of supply is {tos}",
                rules.date()
            ),
        ));
    }
    let option = supported!(read_option(v));
    match direction {
        Direction::Receipt => classify_advance(v, line, masters, option),
        Direction::Outward => {
            let facts = supported!(line_facts(v, line, masters, tos)?);
            classify_outward(v, masters, &facts, option)
        }
        Direction::Inward => {
            let facts = supported!(line_facts(v, line, masters, tos)?);
            classify_inward(v, masters, &facts, option)
        }
    }
}

fn check_scope(v: &Voucher, masters: &Masters) -> Result<Outcome<Direction>, GstError> {
    match v.status {
        VoucherStatus::Cancelled => {
            return unsupported(
                "classification of a cancelled voucher",
                "a cancelled voucher has no amounts and no supply",
            );
        }
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    if v.reverse_charge {
        return unsupported(
            "classification under reverse charge",
            "reverse charge supplies are not classified here",
        );
    }
    let voucher_type = masters
        .voucher_types
        .get(&v.type_id)
        .ok_or_else(|| not_found(format!("voucher type {}", v.type_id.0)))?;
    let direction = match voucher_type.base {
        BaseVoucherKind::Sales | BaseVoucherKind::CreditNote => Direction::Outward,
        BaseVoucherKind::Purchase | BaseVoucherKind::DebitNote => Direction::Inward,
        BaseVoucherKind::Receipt => return Ok(Outcome::Value(Direction::Receipt)),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Payment
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => {
            return unsupported(
                format!("classification of a {:?} voucher", voucher_type.base),
                "only sales, credit-note, purchase, debit-note and receipt vouchers are classified",
            );
        }
    };
    match v.mode {
        VoucherMode::AccountingInvoice | VoucherMode::ItemInvoice => Ok(Outcome::Value(direction)),
        VoucherMode::AsVoucher => unsupported(
            "classification of a voucher entered as plain Dr/Cr lines",
            "enter it as an invoice so that supply lines can be identified",
        ),
    }
}

/// The time of supply taken from the voucher: its `supply_date`, else its date. A payment dated
/// before the invoice can move the time of supply, which this module does not decide.
fn time_of_supply(v: &Voucher) -> Outcome<Date> {
    match (v.supply_date, v.payment_date) {
        (Some(d), Some(_) | None) => Outcome::Value(d),
        (None, Some(p)) if p < v.date => Outcome::unsupported(
            "time of supply with a payment dated before the invoice",
            "set the supply date explicitly; the time-of-supply rules are not applied here",
        ),
        (None, Some(_) | None) => Outcome::Value(v.date),
    }
}

fn read_option(v: &Voucher) -> Outcome<SupplyOption> {
    let Some(text) = v.extra.get(SUPPLY_OPTION_KEY) else {
        return Outcome::Value(SupplyOption::NotStated);
    };
    match text.trim() {
        "with_payment" => Outcome::Value(SupplyOption::WithPayment),
        "under_lut" => Outcome::Value(SupplyOption::UnderLut),
        "deemed_export" => Outcome::Value(SupplyOption::DeemedExport),
        other => Outcome::unsupported(
            format!("supply option `{other}`"),
            "the value must be with_payment, under_lut or deemed_export",
        ),
    }
}

fn line_facts(
    v: &Voucher,
    line: usize,
    masters: &Masters,
    tos: Date,
) -> Result<Outcome<LineFacts>, GstError> {
    match v.mode {
        VoucherMode::ItemInvoice => {
            let inv = v.inventory_lines.get(line).ok_or_else(|| {
                invalid(
                    "line",
                    format!(
                        "line {line} is past the {} inventory lines of the voucher",
                        v.inventory_lines.len()
                    ),
                )
            })?;
            let item = masters
                .stock_items
                .get(&inv.item)
                .ok_or_else(|| not_found(format!("stock item {}", inv.item.0)))?;
            let history = supported!(item_history(item, tos)?);
            let nature = match item.hsn.as_ref().map(ledgerly_core::Hsn::is_sac) {
                Some(true) => Nature::Service,
                Some(false) => Nature::Goods,
                None => Nature::Unknown,
            };
            Ok(Outcome::Value(LineFacts {
                taxability: history.taxability,
                rate_is_zero: history.rate_percent.is_some_and(|r| r.0.is_zero()),
                nature,
            }))
        }
        VoucherMode::AccountingInvoice => {
            let l = v.ledger_lines.get(line).ok_or_else(|| {
                invalid(
                    "line",
                    format!(
                        "line {line} is past the {} ledger lines of the voucher",
                        v.ledger_lines.len()
                    ),
                )
            })?;
            let ledger = masters
                .ledgers
                .get(&l.ledger)
                .ok_or_else(|| not_found(format!("ledger {}", l.ledger.0)))?;
            let Some(class) = &ledger.gst else {
                return unsupported(
                    "classification of a ledger line without a GST classification",
                    format!(
                        "ledger {} has no GST details; set its taxability or pick the sales or purchase ledger",
                        l.ledger.0
                    ),
                );
            };
            let nature = match class.hsn.as_ref().map(ledgerly_core::Hsn::is_sac) {
                Some(true) => Nature::Service,
                Some(false) => Nature::Goods,
                None => Nature::Unknown,
            };
            Ok(Outcome::Value(LineFacts {
                taxability: class.taxability,
                rate_is_zero: class.rate_percent.is_some_and(|r| r.0.is_zero()),
                nature,
            }))
        }
        VoucherMode::AsVoucher => unsupported(
            "classification of a voucher entered as plain Dr/Cr lines",
            "enter it as an invoice so that supply lines can be identified",
        ),
    }
}

/// The item's GST details in force on `tos`: the entry with the latest `effective_from` not after
/// it. Two entries with that same date are ambiguous master data and an error.
fn item_history(item: &StockItem, tos: Date) -> Result<Outcome<&GstHistory>, GstError> {
    let latest = item
        .gst_history
        .iter()
        .filter(|h| h.effective_from <= tos)
        .map(|h| h.effective_from)
        .max();
    let winners: Vec<&GstHistory> = item
        .gst_history
        .iter()
        .filter(|h| Some(h.effective_from) == latest)
        .collect();
    match winners.as_slice() {
        [] => unsupported(
            format!("GST details of item `{}` on {tos}", item.name),
            "the item has no GST details effective on the time of supply; add them to the item",
        ),
        [one] => Ok(Outcome::Value(*one)),
        [..] => Err(invalid(
            "stock item GST history",
            format!(
                "item `{}` has several GST detail entries effective from the same date; keep one",
                item.name
            ),
        )),
    }
}

/// The kind a domestic line has from its taxability alone.
fn by_taxability(facts: &LineFacts) -> Outcome<SupplyKind> {
    match facts.taxability {
        Taxability::Taxable if facts.rate_is_zero => Outcome::unsupported(
            "a taxable line with a 0% rate",
            "a 0% rate is nil-rated supply; set the taxability to nil-rated or give the rate",
        ),
        Taxability::Taxable => Outcome::Value(SupplyKind::Taxable),
        Taxability::Exempt => Outcome::Value(SupplyKind::Exempt),
        Taxability::NilRated => Outcome::Value(SupplyKind::NilRated),
        Taxability::NonGst => Outcome::Value(SupplyKind::NonGst),
    }
}

fn recipient_of(v: &Voucher, masters: &Masters) -> Result<Outcome<Recipient>, GstError> {
    let Some(party_id) = v.party else {
        return Ok(Outcome::Value(Recipient::OtherDomestic));
    };
    let party = masters
        .ledgers
        .get(&party_id)
        .ok_or_else(|| not_found(format!("party ledger {}", party_id.0)))?;
    // A party ledger without details is not known to be domestic: it could be an overseas or SEZ
    // recipient, which changes the kind of every line. It is not defaulted to a domestic recipient.
    let Some(details) = &party.party else {
        return unsupported(
            "classification of a supply to a ledger without party details",
            "the customer's registration is not on the party ledger, so the supply cannot be told apart as domestic, zero-rated or a deemed export; add the registration to the ledger",
        );
    };
    match details.registration {
        RegistrationType::Regular => Ok(Outcome::Value(Recipient::Regular)),
        RegistrationType::Composition
        | RegistrationType::Unregistered
        | RegistrationType::Consumer => Ok(Outcome::Value(Recipient::OtherDomestic)),
        RegistrationType::Sez => Ok(Outcome::Value(Recipient::Sez)),
        RegistrationType::Overseas => Ok(Outcome::Value(Recipient::Overseas)),
        RegistrationType::Uin => unsupported(
            "supply to a UIN holder",
            "supplies to embassies and UN bodies are not classified",
        ),
    }
}

fn classify_outward(
    v: &Voucher,
    masters: &Masters,
    facts: &LineFacts,
    option: SupplyOption,
) -> Result<Outcome<SupplyKind>, GstError> {
    let recipient = supported!(recipient_of(v, masters)?);
    match option {
        SupplyOption::DeemedExport => match recipient {
            Recipient::Regular => match facts.taxability {
                Taxability::Taxable if facts.rate_is_zero => Ok(by_taxability(facts)),
                Taxability::Taxable => Ok(Outcome::Value(SupplyKind::DeemedExport)),
                Taxability::Exempt | Taxability::NilRated | Taxability::NonGst => unsupported(
                    "deemed export of a line that is not taxable",
                    "only taxable supplies are classified as deemed exports",
                ),
            },
            Recipient::OtherDomestic | Recipient::Sez | Recipient::Overseas => unsupported(
                "deemed export to this recipient",
                "a deemed export is a supply to a regular registered recipient (CGST Act s.147)",
            ),
        },
        SupplyOption::WithPayment | SupplyOption::UnderLut => {
            let with_payment = option == SupplyOption::WithPayment;
            match recipient {
                Recipient::Overseas | Recipient::Sez => {
                    supported!(lut_consistent(v, masters, option)?);
                    Ok(zero_rated(facts, recipient, with_payment))
                }
                Recipient::Regular | Recipient::OtherDomestic => unsupported(
                    "payment or LUT option on a domestic supply",
                    "the s.16(3) choice applies only to exports and supplies to an SEZ",
                ),
            }
        }
        SupplyOption::NotStated => match recipient {
            Recipient::Overseas | Recipient::Sez => match facts.taxability {
                // outside GST whether or not the recipient is abroad: nothing to choose
                Taxability::NonGst => Ok(Outcome::Value(SupplyKind::NonGst)),
                Taxability::Taxable | Taxability::Exempt | Taxability::NilRated => unsupported(
                    "zero-rated supply without the payment or LUT choice",
                    "the supplier chooses between paying IGST and supplying under bond or LUT (IGST Act s.16(3)); the voucher does not say which",
                ),
            },
            Recipient::Regular | Recipient::OtherDomestic => Ok(by_taxability(facts)),
        },
    }
}

/// The kind of a zero-rated line, given a recipient that is overseas or in an SEZ.
fn zero_rated(facts: &LineFacts, recipient: Recipient, with_payment: bool) -> Outcome<SupplyKind> {
    match facts.taxability {
        Taxability::NonGst => return Outcome::Value(SupplyKind::NonGst),
        Taxability::Exempt | Taxability::NilRated => {
            return Outcome::unsupported(
                "exempt or nil-rated supply to an overseas or SEZ recipient",
                "whether such a supply is zero-rated or exempt is not decided; classify it by hand",
            );
        }
        Taxability::Taxable => {}
    }
    if facts.rate_is_zero {
        return Outcome::unsupported(
            "a taxable line with a 0% rate",
            "a 0% rate is nil-rated supply; set the taxability to nil-rated or give the rate",
        );
    }
    match recipient {
        Recipient::Overseas => match facts.nature {
            // s.2(5) export of goods; s.16(1)(a)
            Nature::Goods if with_payment => Outcome::Value(SupplyKind::ExportWithPayment),
            Nature::Goods => Outcome::Value(SupplyKind::ExportUnderLut),
            Nature::Service => Outcome::unsupported(
                "service to a recipient outside India",
                "s.13 applies and export status depends on the s.2(6) conditions, which are not modelled",
            ),
            Nature::Unknown => Outcome::unsupported(
                "supply to a recipient outside India of unknown nature",
                "set the HSN or SAC on the item or ledger so that goods and services can be told apart",
            ),
        },
        // s.16(1)(b): goods and services to an SEZ are both zero-rated
        Recipient::Sez if with_payment => Outcome::Value(SupplyKind::SezWithPayment),
        Recipient::Sez => Outcome::Value(SupplyKind::SezUnderLut),
        Recipient::Regular | Recipient::OtherDomestic => Outcome::unsupported(
            "zero-rating of a domestic supply",
            "only overseas and SEZ recipients are zero-rated",
        ),
    }
}

/// `under_lut` contradicts an IGST amount on the same voucher (IGST Act s.16(3)(a): no tax is
/// charged on a supply under bond or LUT).
fn lut_consistent(
    v: &Voucher,
    masters: &Masters,
    option: SupplyOption,
) -> Result<Outcome<()>, GstError> {
    match option {
        SupplyOption::WithPayment | SupplyOption::NotStated | SupplyOption::DeemedExport => {
            return Ok(Outcome::Value(()));
        }
        SupplyOption::UnderLut => {}
    }
    for l in &v.ledger_lines {
        let ledger = masters
            .ledgers
            .get(&l.ledger)
            .ok_or_else(|| not_found(format!("ledger {}", l.ledger.0)))?;
        let is_igst = ledger.tax.is_some_and(|t| t.kind == TaxLedgerKind::Igst);
        if is_igst && !l.amount.amount.is_zero() {
            return unsupported(
                "supply under LUT with IGST charged",
                "the voucher posts IGST but is marked under_lut; no tax is charged under bond or LUT (IGST Act s.16(3)(a)); fix one of them",
            );
        }
    }
    Ok(Outcome::Value(()))
}

fn classify_inward(
    v: &Voucher,
    masters: &Masters,
    facts: &LineFacts,
    option: SupplyOption,
) -> Result<Outcome<SupplyKind>, GstError> {
    match option {
        SupplyOption::NotStated => {}
        SupplyOption::WithPayment | SupplyOption::UnderLut | SupplyOption::DeemedExport => {
            return unsupported(
                "supply option on a purchase or debit note",
                "the option describes an outward supply; remove it",
            );
        }
    }
    let Some(party_id) = v.party else {
        return unsupported(
            "classification of a purchase without a party",
            "set the supplier as the party",
        );
    };
    let party = masters
        .ledgers
        .get(&party_id)
        .ok_or_else(|| not_found(format!("party ledger {}", party_id.0)))?;
    let Some(details) = &party.party else {
        return unsupported(
            "classification of a purchase from a ledger without party details",
            "the supplier's registration is not on the party ledger; add it",
        );
    };
    match details.registration {
        RegistrationType::Regular => Ok(by_taxability(facts)),
        RegistrationType::Composition
        | RegistrationType::Unregistered
        | RegistrationType::Consumer
        | RegistrationType::Sez
        | RegistrationType::Overseas
        | RegistrationType::Uin => unsupported(
            format!(
                "classification of a purchase from a {:?} supplier",
                details.registration
            ),
            "only purchases from regular taxpayers are classified; bills of supply, reverse charge, imports and SEZ supplies are not",
        ),
    }
}

fn classify_advance(
    v: &Voucher,
    line: usize,
    masters: &Masters,
    option: SupplyOption,
) -> Result<Outcome<SupplyKind>, GstError> {
    match option {
        SupplyOption::NotStated => {}
        SupplyOption::WithPayment | SupplyOption::UnderLut | SupplyOption::DeemedExport => {
            return unsupported(
                "supply option on a receipt",
                "the option describes a supply, not an advance; remove it",
            );
        }
    }
    let l = v.ledger_lines.get(line).ok_or_else(|| {
        invalid(
            "line",
            format!(
                "line {line} is past the {} ledger lines of the voucher",
                v.ledger_lines.len()
            ),
        )
    })?;
    let advances = l
        .bills
        .iter()
        .filter(|b| b.kind == BillRefKind::Advance)
        .count();
    if advances == 0 {
        return unsupported(
            "classification of a receipt line that is not an advance",
            "only a line with an advance bill allocation is classified as an advance",
        );
    }
    if advances != l.bills.len() {
        return unsupported(
            "a receipt line that is partly advance and partly against bills",
            "split the receipt so that the advance is on its own line",
        );
    }
    if l.amount.side != DrCr::Cr {
        return unsupported(
            "an advance on the debit side of a receipt",
            "an advance received credits the party",
        );
    }
    let ledger = masters
        .ledgers
        .get(&l.ledger)
        .ok_or_else(|| not_found(format!("ledger {}", l.ledger.0)))?;
    let Some(details) = &ledger.party else {
        return unsupported(
            "an advance on a ledger without party details",
            "the payer's registration is not on the ledger; add it",
        );
    };
    match details.registration {
        RegistrationType::Regular
        | RegistrationType::Composition
        | RegistrationType::Unregistered
        | RegistrationType::Consumer => Ok(Outcome::Value(SupplyKind::Advance)),
        RegistrationType::Sez | RegistrationType::Overseas | RegistrationType::Uin => unsupported(
            format!("an advance from a {:?} party", details.registration),
            "advances from SEZ, overseas and UIN parties are not classified",
        ),
    }
}
