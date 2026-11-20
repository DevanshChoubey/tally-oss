//! Place of supply (IGST Act s.10, s.12). **Card G3** implements this module.
//!
//! Frozen interface (plan card G3):
//!
//! ```text
//! pub struct SupplyFacts { .. }   // defined by G3: supplier state, recipient registration/state/address, goods movement, kind
//! pub enum PlaceOfSupply { Intra { tax: IntraTax }, Inter, ZeroRatedExport { with_payment: bool }, ZeroRatedSez { with_payment: bool }, DeemedExport }
//! pub enum IntraTax { Sgst, Utgst }
//! pub fn place_of_supply(facts: &SupplyFacts) -> Outcome<PlaceOfSupply>;
//! ```
//!
//! # What is implemented
//!
//! - Goods, domestic: s.10(1)(a) (movement: where it terminates) and s.10(1)(c) (no movement:
//!   location of the goods at delivery). For an unregistered recipient the s.10(1)(ca) reading
//!   (address on the invoice) is unconfirmed in the research, so it is only used as a cross-check:
//!   if it disagrees with (a)/(c) the result is `Unsupported`.
//! - Services, domestic, default rule only: s.12(2)(a) (registered recipient: its location) and
//!   s.12(2)(b) (otherwise the address on record, else the supplier's location).
//! - Supply to an SEZ unit or developer: inter-State (s.7(5)(b)) and zero-rated (s.16(1)(b)).
//!   Supply by an SEZ supplier to a domestic recipient: inter-State (s.7(5)(b)).
//! - Export of goods to an overseas recipient: zero-rated (s.2(5), s.16(1)(a)).
//! - The supplier's choice between payment of tax and bond/LUT (s.16(3)) is never defaulted.
//!
//! # Not implemented (returns `Unsupported`)
//!
//! Goods under s.10(1)(b)/(d)/(e); services under s.12(3) to s.12(14); services to an overseas
//! recipient (s.13, and the s.2(6) export conditions); UIN holders; deemed exports; an SEZ
//! supplier dealing with an SEZ or overseas recipient; voucher-level overrides.

use ledgerly_core::{Outcome, StateCode};
use ledgerly_model::RegistrationType;

/// How goods move for a domestic supply of goods (IGST Act s.10(1)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoodsMovement {
    /// The supply involves movement of goods, by the supplier, the recipient or anyone else,
    /// and the movement terminates in `destination` (s.10(1)(a)).
    Moved {
        /// State where the movement terminates for delivery to the recipient.
        destination: StateCode,
    },
    /// The goods are not moved; they are at `goods_location` at the time of delivery (s.10(1)(c)).
    NotMoved {
        /// State where the goods are at the time of delivery.
        goods_location: StateCode,
    },
    /// A case with its own place-of-supply rule that is not implemented.
    Special(GoodsSpecialCase),
}

/// Goods cases with their own rule under s.10(1), not implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoodsSpecialCase {
    /// Bill-to-ship-to: delivery on the direction of a third person (s.10(1)(b)).
    BillToShipTo,
    /// Goods assembled or installed at site (s.10(1)(d)).
    AssemblyOrInstallation,
    /// Goods supplied on board a conveyance (s.10(1)(e)).
    OnBoardConveyance,
}

/// Services with their own place-of-supply rule under s.12(3) to s.12(14), not implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialService {
    /// Services relating to immovable property.
    ImmovableProperty,
    /// Restaurant and catering services.
    Restaurant,
    /// Training and performance appraisal.
    Training,
    /// Admission to events and ancillary services.
    Events,
    /// Transportation of goods.
    TransportOfGoods,
    /// Passenger transportation.
    PassengerTransport,
    /// Telecommunication services.
    Telecom,
    /// Banking and other financial services.
    BankingOrFinancial,
    /// Insurance services.
    Insurance,
    /// Advertisement services to the Government.
    Advertising,
    /// Any other service with a special rule.
    Other,
}

/// Kind of service for place-of-supply purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    /// A service covered only by the s.12(2) default rule.
    General,
    /// A service with a special rule; not implemented.
    Special(SpecialService),
}

/// What is supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplyCategory {
    /// Goods, with how they move.
    Goods(GoodsMovement),
    /// A service.
    Service(ServiceKind),
}

/// The recipient as far as place of supply is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recipient {
    /// Registration type.
    pub registration: RegistrationType,
    /// For a registered recipient (or SEZ unit): the State of its GSTIN. For an unregistered
    /// recipient: the State of the address on record, or `None` if there is no address.
    pub state: Option<StateCode>,
}

/// Facts the place-of-supply decision needs. The caller supplies them from the voucher and
/// masters; nothing is defaulted here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplyFacts {
    /// The supplier's location (State of its GSTIN).
    pub supplier_state: StateCode,
    /// The supplier is itself an SEZ unit or developer.
    pub supplier_is_sez: bool,
    /// The recipient.
    pub recipient: Recipient,
    /// What is supplied.
    pub category: SupplyCategory,
    /// For exports and supplies to an SEZ: `Some(true)` when tax is paid, `Some(false)` when
    /// supplied under bond or LUT (s.16(3)); `None` when not stated. Ignored for other supplies.
    pub with_payment: Option<bool>,
}

/// Which State tax accompanies CGST on an intra-State supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntraTax {
    /// State GST.
    Sgst,
    /// Union territory GST.
    Utgst,
}

/// The outcome of the place-of-supply decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceOfSupply {
    /// Same State: CGST plus SGST or UTGST.
    Intra {
        /// The State-level tax.
        tax: IntraTax,
    },
    /// Different States: IGST.
    Inter,
    /// Export, zero-rated.
    ZeroRatedExport {
        /// `true` when IGST is paid (and refunded), `false` under bond or LUT.
        with_payment: bool,
    },
    /// Supply to an SEZ unit or developer, zero-rated.
    ZeroRatedSez {
        /// `true` when IGST is paid (and refunded), `false` under bond or LUT.
        with_payment: bool,
    },
    /// Deemed export. Never produced yet: deemed exports are domestic supplies whose tax head
    /// follows the ordinary rule, and the classification is not implemented.
    DeemedExport,
}

fn unsupported(what: &str, reason: &str) -> Outcome<PlaceOfSupply> {
    Outcome::unsupported(format!("GST place of supply: {what}"), reason)
}

/// Decides the place of supply and the tax head (IGST Act s.7, s.8, s.10, s.12).
///
/// Returns `Unsupported` for every case not listed in the module documentation.
pub fn place_of_supply(facts: &SupplyFacts) -> Outcome<PlaceOfSupply> {
    match facts.recipient.registration {
        RegistrationType::Uin => unsupported(
            "UIN holder",
            "supplies to embassies and UN bodies are not modelled",
        ),
        RegistrationType::Overseas => export(facts),
        RegistrationType::Sez => sez_recipient(facts),
        RegistrationType::Regular | RegistrationType::Composition => {
            domestic(facts, Registered::Yes)
        }
        RegistrationType::Unregistered | RegistrationType::Consumer => {
            domestic(facts, Registered::No)
        }
    }
}

#[derive(Clone, Copy)]
enum Registered {
    Yes,
    No,
}

/// s.16(3): the payment choice must be stated.
fn payment_choice(facts: &SupplyFacts, what: &str) -> Result<bool, Outcome<PlaceOfSupply>> {
    facts.with_payment.ok_or_else(|| {
        unsupported(
            what,
            "the supplier chooses between paying IGST and supplying under bond or LUT (s.16(3)); \
             the voucher does not say which",
        )
    })
}

fn export(facts: &SupplyFacts) -> Outcome<PlaceOfSupply> {
    if facts.supplier_is_sez {
        return unsupported(
            "export by an SEZ supplier",
            "a supplier in an SEZ exporting is not modelled",
        );
    }
    match facts.category {
        SupplyCategory::Service(_) => unsupported(
            "service to a recipient outside India",
            "s.13 applies and export status depends on the s.2(6) conditions, which are not modelled",
        ),
        SupplyCategory::Goods(_) => match payment_choice(facts, "export of goods") {
            Ok(with_payment) => Outcome::Value(PlaceOfSupply::ZeroRatedExport { with_payment }),
            Err(u) => u,
        },
    }
}

fn sez_recipient(facts: &SupplyFacts) -> Outcome<PlaceOfSupply> {
    if facts.supplier_is_sez {
        return unsupported(
            "SEZ supplier to SEZ recipient",
            "a supply between two SEZ parties is not modelled",
        );
    }
    match payment_choice(facts, "supply to an SEZ unit or developer") {
        Ok(with_payment) => Outcome::Value(PlaceOfSupply::ZeroRatedSez { with_payment }),
        Err(u) => u,
    }
}

/// The State that is the place of supply for a domestic supply, or why it cannot be decided.
fn domestic_place(
    facts: &SupplyFacts,
    registered: Registered,
) -> Result<StateCode, Outcome<PlaceOfSupply>> {
    match facts.category {
        SupplyCategory::Goods(movement) => {
            let place = match movement {
                GoodsMovement::Moved { destination } => destination,
                GoodsMovement::NotMoved { goods_location } => goods_location,
                GoodsMovement::Special(GoodsSpecialCase::BillToShipTo) => {
                    return Err(unsupported("s.10(1)(b) bill-to-ship-to", "not implemented"));
                }
                GoodsMovement::Special(GoodsSpecialCase::AssemblyOrInstallation) => {
                    return Err(unsupported(
                        "s.10(1)(d) assembly or installation",
                        "not implemented",
                    ));
                }
                GoodsMovement::Special(GoodsSpecialCase::OnBoardConveyance) => {
                    return Err(unsupported(
                        "s.10(1)(e) goods on board a conveyance",
                        "not implemented",
                    ));
                }
            };
            match registered {
                Registered::Yes => Ok(place),
                Registered::No => {
                    // s.10(1)(ca) is unconfirmed: accept only when it agrees with (a)/(c).
                    let by_address = facts.recipient.state.unwrap_or(facts.supplier_state);
                    if by_address == place {
                        Ok(place)
                    } else {
                        Err(unsupported(
                            "goods to an unregistered recipient, address State differs from delivery State",
                            "s.10(1)(ca) is unconfirmed and disagrees with s.10(1)(a)/(c)",
                        ))
                    }
                }
            }
        }
        SupplyCategory::Service(ServiceKind::Special(_)) => Err(unsupported(
            "service with a special rule under s.12(3) to s.12(14)",
            "only the s.12(2) default rule is implemented",
        )),
        SupplyCategory::Service(ServiceKind::General) => match registered {
            // s.12(2)(a)
            Registered::Yes => facts.recipient.state.ok_or_else(|| {
                unsupported(
                    "service to a registered recipient without a State",
                    "s.12(2)(a) needs the recipient's location",
                )
            }),
            // s.12(2)(b)(i) address on record, (ii) else the supplier's location
            Registered::No => Ok(facts.recipient.state.unwrap_or(facts.supplier_state)),
        },
    }
}

fn domestic(facts: &SupplyFacts, registered: Registered) -> Outcome<PlaceOfSupply> {
    let place = match domestic_place(facts, registered) {
        Ok(p) => p,
        Err(u) => return u,
    };
    if facts.supplier_is_sez {
        // s.7(5)(b)
        return Outcome::Value(PlaceOfSupply::Inter);
    }
    if place == facts.supplier_state {
        let tax = if facts.supplier_state.is_utgst_territory() {
            IntraTax::Utgst
        } else {
            IntraTax::Sgst
        };
        Outcome::Value(PlaceOfSupply::Intra { tax })
    } else {
        Outcome::Value(PlaceOfSupply::Inter)
    }
}
