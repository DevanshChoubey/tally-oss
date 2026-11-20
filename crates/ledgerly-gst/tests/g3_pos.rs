//! Card G3 tests: place of supply (IGST Act 2017 s.7, s.10, s.12; UTGST Act 2017).
//!
//! Every expected value is `derived`: the statutory rule and the hand reasoning sit next to it.
//! State codes: 27 Maharashtra, 29 Karnataka, 4 Chandigarh (from the state table in
//! `ledgerly-core`, which the owner is verifying; the arithmetic of the rules does not depend on
//! which number a state has, only on whether two codes are equal).

#![allow(clippy::unwrap_used, reason = "tests may unwrap")]

use ledgerly_core::{Outcome, StateCode};
use ledgerly_gst::pos::{
    GoodsMovement, GoodsSpecialCase, IntraTax, PlaceOfSupply, Recipient, ServiceKind,
    SpecialService, SupplyCategory, SupplyFacts, place_of_supply,
};
use ledgerly_model::RegistrationType;

fn st(c: u8) -> StateCode {
    StateCode::new(c).unwrap()
}

fn recipient(reg: RegistrationType, state: Option<u8>) -> Recipient {
    Recipient {
        registration: reg,
        state: state.map(st),
    }
}

fn facts(supplier: u8, recipient: Recipient, category: SupplyCategory) -> SupplyFacts {
    SupplyFacts {
        supplier_state: st(supplier),
        supplier_is_sez: false,
        recipient,
        category,
        with_payment: None,
    }
}

fn goods_moved(dest: u8) -> SupplyCategory {
    SupplyCategory::Goods(GoodsMovement::Moved {
        destination: st(dest),
    })
}

fn is_unsupported(o: &Outcome<PlaceOfSupply>) -> bool {
    matches!(o, Outcome::Unsupported(_))
}

/// derived: IGST Act s.10(1)(a): movement of goods involved, place of supply is the location
/// where movement terminates for delivery to the recipient = Karnataka (29). Supplier location is
/// Maharashtra (27). IGST Act s.7(1)/s.8(1): supply is inter-State when supplier location and
/// place of supply are in different States: 27 != 29 so Inter.
#[test]
fn goods_moved_maharashtra_to_karnataka_is_inter() {
    let f = facts(
        27,
        recipient(RegistrationType::Regular, Some(29)),
        goods_moved(29),
    );
    assert_eq!(place_of_supply(&f), Outcome::Value(PlaceOfSupply::Inter));
}

/// derived: s.10(1)(a), movement terminates in 27, supplier in 27: same State, so intra-State
/// (IGST Act s.8(1)); Maharashtra has a legislature so the State tax is SGST (27 is not a UT
/// without legislature).
#[test]
fn goods_moved_within_maharashtra_is_intra_sgst() {
    let f = facts(
        27,
        recipient(RegistrationType::Regular, Some(27)),
        goods_moved(27),
    );
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::Intra {
            tax: IntraTax::Sgst
        })
    );
}

/// derived: IGST Act s.10(1)(c): goods not moved, place of supply is the location of the goods at
/// the time of delivery = 27. Supplier location 27. Same State: Intra, SGST.
#[test]
fn goods_not_moved_supplier_and_location_in_27_is_intra() {
    let f = facts(
        27,
        recipient(RegistrationType::Regular, Some(27)),
        SupplyCategory::Goods(GoodsMovement::NotMoved {
            goods_location: st(27),
        }),
    );
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::Intra {
            tax: IntraTax::Sgst
        })
    );
}

/// derived: s.10(1)(c): goods at 29 at delivery, supplier in 27: place of supply 29 != 27 so Inter.
#[test]
fn goods_not_moved_location_in_other_state_is_inter() {
    let f = facts(
        27,
        recipient(RegistrationType::Regular, Some(27)),
        SupplyCategory::Goods(GoodsMovement::NotMoved {
            goods_location: st(29),
        }),
    );
    assert_eq!(place_of_supply(&f), Outcome::Value(PlaceOfSupply::Inter));
}

/// derived: IGST Act s.12(2)(b)(ii): supply of services to a person other than a registered
/// person, where no address of the recipient is on record: place of supply is the location of the
/// supplier = 29. Supplier is in 29, so same State: Intra, SGST.
#[test]
fn b2c_service_no_address_is_location_of_supplier() {
    let f = facts(
        29,
        recipient(RegistrationType::Consumer, None),
        SupplyCategory::Service(ServiceKind::General),
    );
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::Intra {
            tax: IntraTax::Sgst
        })
    );
}

/// derived: IGST Act s.12(2)(b)(i): unregistered recipient with an address on record: place of
/// supply is the location per that address = 27. Supplier in 29. 27 != 29: Inter.
#[test]
fn b2c_service_with_address_is_location_of_address() {
    let f = facts(
        29,
        recipient(RegistrationType::Unregistered, Some(27)),
        SupplyCategory::Service(ServiceKind::General),
    );
    assert_eq!(place_of_supply(&f), Outcome::Value(PlaceOfSupply::Inter));
}

/// derived: IGST Act s.12(2)(a): registered recipient: place of supply is the location of the
/// recipient (GSTIN State) = 29. Supplier in 27. 27 != 29: Inter.
#[test]
fn b2b_service_is_location_of_registered_recipient() {
    let f = facts(
        27,
        recipient(RegistrationType::Regular, Some(29)),
        SupplyCategory::Service(ServiceKind::General),
    );
    assert_eq!(place_of_supply(&f), Outcome::Value(PlaceOfSupply::Inter));
}

/// derived: s.12(2)(a) needs the registered recipient's location; none given, so the rule cannot
/// be applied. Falling back to the supplier's State would be a guess (rule A1).
#[test]
fn b2b_service_without_recipient_state_is_unsupported() {
    let f = facts(
        27,
        recipient(RegistrationType::Regular, None),
        SupplyCategory::Service(ServiceKind::General),
    );
    assert!(is_unsupported(&place_of_supply(&f)));
}

/// derived: IGST Act s.2(5) export of goods (taking goods out of India to a place outside India)
/// and s.16(1)(a) (export is a zero-rated supply). Recipient is outside India. The caller states
/// whether it is under bond/LUT or with payment of tax (s.16(3)); here with payment.
#[test]
fn goods_export_is_zero_rated_export() {
    let mut f = facts(
        27,
        recipient(RegistrationType::Overseas, None),
        goods_moved(27),
    );
    f.with_payment = Some(true);
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::ZeroRatedExport { with_payment: true })
    );
    f.with_payment = Some(false);
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::ZeroRatedExport {
            with_payment: false
        })
    );
}

/// derived: s.16(3) leaves the choice between payment of IGST and supply under bond/LUT to the
/// supplier. If the facts do not say which, we must not pick one.
#[test]
fn export_without_payment_choice_is_unsupported() {
    let f = facts(
        27,
        recipient(RegistrationType::Overseas, None),
        goods_moved(27),
    );
    assert!(is_unsupported(&place_of_supply(&f)));
}

/// derived: s.13 (not s.12) governs services to a recipient outside India, and whether the
/// supply is an export depends on all the conditions of s.2(6), which the facts do not carry.
/// Not computed.
#[test]
fn service_to_overseas_recipient_is_unsupported() {
    let mut f = facts(
        27,
        recipient(RegistrationType::Overseas, None),
        SupplyCategory::Service(ServiceKind::General),
    );
    f.with_payment = Some(false);
    assert!(is_unsupported(&place_of_supply(&f)));
}

/// derived: IGST Act s.16(1)(b): supply of goods to an SEZ developer or SEZ unit is a zero-rated
/// supply; s.7(5)(b): it is an inter-State supply even when both are in the same State. Here
/// supplier and SEZ are both in 27 and the result is still `ZeroRatedSez`, not `Intra`.
#[test]
fn sez_unit_in_same_state_is_zero_rated_sez() {
    let mut f = facts(
        27,
        recipient(RegistrationType::Sez, Some(27)),
        goods_moved(27),
    );
    f.with_payment = Some(false);
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::ZeroRatedSez {
            with_payment: false
        })
    );
    f.with_payment = Some(true);
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::ZeroRatedSez { with_payment: true })
    );
}

/// derived: as above, s.16(3) payment choice is the supplier's; absent, unsupported.
#[test]
fn sez_without_payment_choice_is_unsupported() {
    let f = facts(
        27,
        recipient(RegistrationType::Sez, Some(27)),
        goods_moved(27),
    );
    assert!(is_unsupported(&place_of_supply(&f)));
}

/// derived: s.7(5)(b): a supply by an SEZ unit or developer is inter-State. A domestic
/// recipient in the same State as the SEZ supplier therefore gets Inter, not Intra.
#[test]
fn supply_by_sez_supplier_to_domestic_recipient_is_inter() {
    let mut f = facts(
        27,
        recipient(RegistrationType::Regular, Some(27)),
        goods_moved(27),
    );
    f.supplier_is_sez = true;
    assert_eq!(place_of_supply(&f), Outcome::Value(PlaceOfSupply::Inter));
}

/// derived: s.7(5)(b) plus s.16: a supplier that is itself an SEZ supplying an SEZ or an
/// overseas recipient is a combination the facts cannot settle; not computed.
#[test]
fn sez_supplier_to_sez_or_overseas_is_unsupported() {
    let mut f = facts(
        27,
        recipient(RegistrationType::Sez, Some(27)),
        goods_moved(27),
    );
    f.supplier_is_sez = true;
    f.with_payment = Some(false);
    assert!(is_unsupported(&place_of_supply(&f)));
}

/// derived: UTGST Act 2017: UTGST (not SGST) is levied on intra-State supplies in a Union
/// territory without a legislature; Chandigarh is such a territory. Goods moved and delivered
/// within Chandigarh (4): s.10(1)(a) place = 4 = supplier State, intra, UTGST.
#[test]
fn intra_supply_in_chandigarh_is_utgst() {
    let f = facts(
        4,
        recipient(RegistrationType::Regular, Some(4)),
        goods_moved(4),
    );
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::Intra {
            tax: IntraTax::Utgst
        })
    );
}

/// derived: s.10(1)(b) bill-to-ship-to, s.10(1)(d) installation and s.10(1)(e) on board a
/// conveyance each name a different place; their inputs are not in the facts. Not computed.
#[test]
fn special_goods_cases_are_unsupported() {
    for case in [
        GoodsSpecialCase::BillToShipTo,
        GoodsSpecialCase::AssemblyOrInstallation,
        GoodsSpecialCase::OnBoardConveyance,
    ] {
        let f = facts(
            27,
            recipient(RegistrationType::Regular, Some(27)),
            SupplyCategory::Goods(GoodsMovement::Special(case)),
        );
        assert!(is_unsupported(&place_of_supply(&f)), "{case:?}");
    }
}

/// derived: s.12(3)-(14) give special places for these services; only the s.12(2) default is
/// implemented, so every special category is Unsupported (never the default rule).
#[test]
fn special_service_categories_are_unsupported() {
    for s in [
        SpecialService::ImmovableProperty,
        SpecialService::Restaurant,
        SpecialService::Training,
        SpecialService::Events,
        SpecialService::TransportOfGoods,
        SpecialService::PassengerTransport,
        SpecialService::Telecom,
        SpecialService::BankingOrFinancial,
        SpecialService::Insurance,
        SpecialService::Advertising,
        SpecialService::Other,
    ] {
        let f = facts(
            27,
            recipient(RegistrationType::Regular, Some(27)),
            SupplyCategory::Service(ServiceKind::Special(s)),
        );
        assert!(is_unsupported(&place_of_supply(&f)), "{s:?}");
    }
}

/// derived: s.10(1)(ca) (unconfirmed in research R4, so not relied on) could place a supply to an
/// unregistered person at the address on the invoice. Where the address State differs from the
/// State goods are delivered to, the two readings disagree and we do not pick: Unsupported.
#[test]
fn b2c_goods_with_conflicting_address_is_unsupported() {
    let f = facts(
        27,
        recipient(RegistrationType::Unregistered, Some(29)),
        goods_moved(27),
    );
    assert!(is_unsupported(&place_of_supply(&f)));
}

/// derived: where an unregistered recipient's address State equals the delivery State, both
/// s.10(1)(a) and s.10(1)(ca) give 29; supplier is 27: Inter.
#[test]
fn b2c_goods_with_agreeing_address_is_inter() {
    let f = facts(
        27,
        recipient(RegistrationType::Unregistered, Some(29)),
        goods_moved(29),
    );
    assert_eq!(place_of_supply(&f), Outcome::Value(PlaceOfSupply::Inter));
}

/// derived: walk-in retail sale: goods at 27 and not moved, no address. s.10(1)(c) gives 27; the
/// (ca) reading with no address gives the supplier location 27. Both agree: Intra.
#[test]
fn b2c_goods_no_address_not_moved_is_intra() {
    let f = facts(
        27,
        recipient(RegistrationType::Consumer, None),
        SupplyCategory::Goods(GoodsMovement::NotMoved {
            goods_location: st(27),
        }),
    );
    assert_eq!(
        place_of_supply(&f),
        Outcome::Value(PlaceOfSupply::Intra {
            tax: IntraTax::Sgst
        })
    );
}

/// derived: UIN holders (embassies, UN bodies) have refund-specific treatment that is not
/// modelled; not computed.
#[test]
fn uin_recipient_is_unsupported() {
    let f = facts(
        27,
        recipient(RegistrationType::Uin, Some(7)),
        goods_moved(7),
    );
    assert!(is_unsupported(&place_of_supply(&f)));
}
