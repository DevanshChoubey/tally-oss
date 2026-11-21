//! Bug B5 (docs/status/2026-11-22-ui-parity-report.md): the Home GST tile names a tab (GSTR-3B),
//! but the gateway dropped it and the GST screen opened on its default tab.
//!
//! Bug B1 (F8 and F9 on a company without inventory) has no test here: the item invoice screen
//! enters stock lines only and the tax API accepts only item invoices without ledger lines, so
//! there is no screen to route to yet. A test for it would stay red, and none is committed.
//!
//! The company is fictional. The GSTIN is generated with the check character of the GSTIN
//! algorithm, as `c4_gst.rs` does. No expected figure is involved: these tests check which tab
//! the screen opens on.
//!
//! Before the fix, the helper `gateway::gst_screen_for_target` did not exist, so this file did not
//! compile. Its precursor, `navigate::gst_screen`, always opened on the reconciliation tab
//! (`GstScreen::new`), so the GSTR-3B link opened the wrong tab.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

use ledgerly_api::Api;
use ledgerly_app::screens::gst::GstTab;
use ledgerly_app::screens::home::HomeTarget;
use ledgerly_app::shell::gateway::gst_screen_for_target;
use ledgerly_core::{Clock, CompanyId, Outcome, StateCode, gstin_check_character};
use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, GstRegistration, NegativeStockPolicy,
};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    let first14 = "29ABCDE1234F1Z";
    let check = gstin_check_character(first14).unwrap();
    let gstin = ledgerly_core::Gstin::parse(&format!("{first14}{check}")).unwrap();
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: vec![GstRegistration {
            gstin,
            state: StateCode::new(29).unwrap(),
            effective_from: date!(2026 - 04 - 01),
            composition: false,
        }],
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// The books of a new company file with one GST registration and no vouchers.
fn books(name: &str) -> Books {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    match api.books().unwrap() {
        Outcome::Value(books) => books,
        Outcome::Unsupported(u) => panic!("the books did not load: {}", u.reason),
    }
}

fn gst_link(tab: &str) -> HomeTarget {
    HomeTarget::GstReturns {
        tab: tab.to_owned(),
    }
}

#[test]
fn the_gst_tile_opens_gstr3b_on_the_tab_it_names() {
    // The Home GST tile (screens/home.rs, the KPI built with `tab: "gstr3b"`) must open GSTR-3B,
    // not the screen's default tab (2B and IMS reconciliation).
    let books = books("b1_gst_tile_gstr3b.db");
    let screen = gst_screen_for_target(&books, &gst_link("gstr3b")).unwrap();
    assert_eq!(screen.tab(), GstTab::Gstr3b);
}

#[test]
fn the_other_named_tabs_open_their_own_tab() {
    let books = books("b1_gst_named_tabs.db");
    let gstr1 = gst_screen_for_target(&books, &gst_link("gstr1")).unwrap();
    assert_eq!(gstr1.tab(), GstTab::Gstr1);
    let recon = gst_screen_for_target(&books, &gst_link("gstr2b-ims")).unwrap();
    assert_eq!(recon.tab(), GstTab::Reconciliation);
}

#[test]
fn the_list_link_opens_the_default_tab() {
    // An empty tab name (the G key and the palette) keeps the screen's default tab. The default is
    // GSTR-1, as the mock opens (design/mock-v2 SCREENS.gst default 'gstr1'); it was the 2B tab
    // before the monthly-period change (UI parity report, decision 7; see shell/navigate.rs).
    let books = books("b1_gst_list.db");
    let screen = gst_screen_for_target(&books, &gst_link("")).unwrap();
    assert_eq!(screen.tab(), GstTab::Gstr1);
}

#[test]
fn an_unknown_tab_name_opens_nothing() {
    // A tab name the screen does not have must not open a screen on a guessed tab.
    let books = books("b1_gst_unknown.db");
    let result = gst_screen_for_target(&books, &gst_link("gstr9"));
    assert!(matches!(
        result,
        Err(ledgerly_app::shell::navigate::NavError::Unsupported { .. })
    ));
}

#[test]
fn a_link_that_is_not_gst_returns_is_refused_here() {
    let books = books("b1_gst_not_gst.db");
    let result = gst_screen_for_target(&books, &HomeTarget::Masters);
    assert!(matches!(
        result,
        Err(ledgerly_app::shell::navigate::NavError::Unsupported { .. })
    ));
}
