//! Card M3: feature flag dependency rules.
//!
//! Provenance for every expected value: `derived` from spec section 5 (docs/specs/
//! 2026-11-20-ledgerly-design.md): "Feature flags are a typed struct with dependency rules".
//! The card text fixes two rules: batches without inventory is an error, and GST without a GSTIN
//! on the company is an error. Godowns do not require inventory: the round-2 default (godowns need
//! inventory, a judgment) was replaced in round 3 because the TallyPrime godowns page
//! (<https://help.tallysolutions.com/tally-prime/inventory/inventory-storage-using-godowns-locations-tally/>,
//! read 2026-11-21) states no such prerequisite. Those tests are `derived` from that page.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{CompanyId, Gstin, StateCode};
use ledgerly_model::features::FeatureError;
use ledgerly_model::{
    Address, Company, EntityKind, Features, Flag, GstRegistration, NegativeStockPolicy,
};
use time::macros::date;

fn company(with_gstin: bool) -> Company {
    let registrations = if with_gstin {
        vec![GstRegistration {
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: StateCode::new(29).unwrap(),
            effective_from: date!(2026 - 04 - 01),
            composition: false,
        }]
    } else {
        Vec::new()
    };
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations,
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Proprietorship,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

#[test]
fn batches_without_inventory_is_an_error() {
    // derived: card M3 "batches without inventory errors".
    let f = Features {
        batches: true,
        ..Features::default()
    };
    assert_eq!(
        f.validate(&company(false)),
        Err(FeatureError::MissingRequirement {
            flag: Flag::Batches,
            requires: Flag::Inventory
        })
    );
}

#[test]
fn batches_with_inventory_is_ok() {
    let f = Features {
        inventory: true,
        batches: true,
        ..Features::default()
    };
    assert_eq!(f.validate(&company(false)), Ok(()));
}

#[test]
fn godowns_without_inventory_is_ok() {
    // provenance: derived. Replaces the round-2 default ("godowns need inventory", pending): the
    // TallyPrime godowns page (https://help.tallysolutions.com/tally-prime/inventory/inventory-
    // storage-using-godowns-locations-tally/, read 2026-11-21) states no inventory prerequisite;
    // its only stated prerequisite is an F12 option for storage of materials. So the godowns flag
    // on its own is valid.
    let f = Features {
        godowns: true,
        ..Features::default()
    };
    assert_eq!(f.validate(&company(false)), Ok(()));
}

#[test]
fn gst_without_gstin_is_an_error() {
    // derived: card M3 "GST without a GSTIN on the company errors".
    let f = Features {
        gst: true,
        ..Features::default()
    };
    assert_eq!(
        f.validate(&company(false)),
        Err(FeatureError::GstWithoutGstin)
    );
}

#[test]
fn gst_with_gstin_is_ok() {
    let f = Features {
        gst: true,
        ..Features::default()
    };
    assert_eq!(f.validate(&company(true)), Ok(()));
}

#[test]
fn defaults_and_independent_flags_are_ok() {
    // derived: flags with no prerequisite never fail on their own.
    assert_eq!(Features::default().validate(&company(false)), Ok(()));
    let f = Features {
        cost_centres: true,
        bill_wise: true,
        bank_reconciliation: true,
        optional_vouchers: true,
        post_dated: true,
        ..Features::default()
    };
    assert_eq!(f.validate(&company(false)), Ok(()));
}

#[test]
fn requires_lists_direct_prerequisites() {
    // derived: the batches rule above (card text).
    assert_eq!(Features::requires(Flag::Batches), vec![Flag::Inventory]);
    assert_eq!(Features::requires(Flag::Inventory), Vec::<Flag>::new());
    assert_eq!(Features::requires(Flag::Gst), Vec::<Flag>::new());
}

#[test]
fn requires_godowns_lists_nothing() {
    // provenance: derived. Replaces the round-2 default (godowns require inventory, pending): the
    // godowns page named in the module doc states no prerequisite.
    assert_eq!(Features::requires(Flag::Godowns), Vec::<Flag>::new());
}

#[test]
fn first_violation_is_reported_in_flag_order() {
    // provenance: derived. With godowns and batches on and inventory off, the godowns rule no
    // longer fails, so the first violation in `Flag` declaration order is Batches, which requires
    // Inventory (card text, see `batches_without_inventory_is_an_error`). Replaces the round-2
    // expectation (Godowns reported first), which depended on the pending godowns rule.
    let f = Features {
        godowns: true,
        batches: true,
        ..Features::default()
    };
    assert_eq!(
        f.validate(&company(false)),
        Err(FeatureError::MissingRequirement {
            flag: Flag::Batches,
            requires: Flag::Inventory
        })
    );
}
