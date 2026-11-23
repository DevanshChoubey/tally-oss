//! Wave 2 S-G: Alt+C in the ledger form creates a group for its Under field, on the fly, and
//! comes back to the ledger with the new group under it.
//!
//! TallyPrime does this: in the ledger screen, Alt+C on the Under field creates a group
//! (help.tallysolutions.com/ledgers-in-tallyprime/), and Alt+C is "To create another master on
//! the fly while creating masters and vouchers" (help.tallysolutions.com/keyboard-shortcuts-
//! tally-prime/).
//!
//! Every expected value is derived by hand and the derivation is written next to it.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::Api;
use ledgerly_app::screens::masters::{MasterDraft, MasterKind, MasterRequest, MastersScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, GroupId, Outcome, StateCode};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, NegativeStockPolicy, ReservedGroup,
};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Under Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            inventory: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn debtors() -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(ReservedGroup::SundryDebtors))
        .unwrap()
        .id
}

fn setup(name: &str) -> (Api, Books) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    (api, books)
}

fn app(books: Books, api: Option<Api>) -> AppCtx {
    AppCtx {
        books: Some(books),
        api: api.map(|a| ApiWorker::spawn(a).unwrap()),
        lock_date: None,
        ai_tier: None,
    }
}

fn frame(ctx: &egui::Context, screen: &mut MastersScreen, app: &mut AppCtx, input: RawInput) {
    let out = ctx.run_ui(input, |ui| {
        let _stay = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

fn key(key: Key, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

/// A ledger form named "Ramesh Kirana", with Alt+C pressed in it.
fn group_form_over_a_ledger(app: &mut AppCtx) -> (egui::Context, MastersScreen) {
    let ctx = egui::Context::default();
    let mut s = MastersScreen::creating_ledger("Ramesh Kirana");
    frame(&ctx, &mut s, app, RawInput::default());
    assert!(matches!(s.draft(), Some(MasterDraft::Ledger(_))));
    frame(&ctx, &mut s, app, key(Key::C, Modifiers::ALT));
    (ctx, s)
}

fn ledger_name(s: &MastersScreen) -> String {
    let Some(MasterDraft::Ledger(d)) = s.draft() else {
        panic!("the ledger form is not back: {:?}", s.draft());
    };
    d.name.clone()
}

#[test]
fn alt_c_in_the_ledger_form_opens_a_group_form() {
    // provenance: help.tallysolutions.com/ledgers-in-tallyprime/: Alt+C on the ledger's Under
    // field creates a group. Our ledger form's only master field is Under, so Alt+C anywhere in
    // it creates a group. Derived: the form showing is a blank group form named "Group" on the key
    // bar; the list's kind stays Ledger.
    let (_api, books) = setup("w2_under_opens.ledgerly");
    let mut app = app(books, None);
    let (_ctx, s) = group_form_over_a_ledger(&mut app);
    let Some(MasterDraft::Group(g)) = s.draft() else {
        panic!("Alt+C did not open a group form: {:?}", s.draft());
    };
    assert_eq!(g.name, "");
    assert_eq!(s.key_bar_name(), "Group");
    assert_eq!(s.kind(), MasterKind::Ledger);
}

#[test]
fn esc_in_the_group_form_goes_back_to_the_ledger_as_it_was() {
    // provenance: derived. Leaving the group form without saving goes back to the ledger form
    // with what was typed in it ("Ramesh Kirana"), not to the list: the ledger was not finished.
    let (_api, books) = setup("w2_under_esc.ledgerly");
    let mut app = app(books, None);
    let (ctx, mut s) = group_form_over_a_ledger(&mut app);
    frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert_eq!(ledger_name(&s), "Ramesh Kirana");
    assert!(s.is_form_open());
}

#[test]
fn saving_the_group_comes_back_to_the_ledger_with_the_group_under_it() {
    // provenance: help.tallysolutions.com/keyboard-shortcuts-tally-prime/: Alt+C creates
    // "another master on the fly while creating masters". Derived: the group is created by the
    // engine (through the worker), the ledger form comes back with "Ramesh Kirana", and its
    // Under is the id the engine gave the new group "Retail Debtors" (found in the store by
    // name). Catches the ledger form lost, or Under left empty.
    let (api, books) = setup("w2_under_save.ledgerly");
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("w2_under_save.ledgerly");
    let mut app = app(books, Some(api));
    let (ctx, mut s) = group_form_over_a_ledger(&mut app);
    let Some(MasterDraft::Group(g)) = s.draft_mut() else {
        panic!("no group form");
    };
    g.name = "Retail Debtors".to_owned();
    g.parent = Some(debtors());
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    assert_eq!(ledger_name(&s), "Ramesh Kirana");
    let mut under = None;
    for _ in 0..2000 {
        frame(&ctx, &mut s, &mut app, RawInput::default());
        if let Some(MasterDraft::Ledger(d)) = s.draft()
            && d.group.is_some()
        {
            under = d.group;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(app);
    let reopened = Api::open(&path).unwrap();
    let Ok(Outcome::Value(books)) = reopened.books() else {
        panic!("the books did not load");
    };
    let made = books
        .masters
        .groups
        .values()
        .find(|g| g.name == "Retail Debtors")
        .expect("the group was created");
    assert_eq!(under, Some(made.id));
    assert_eq!(made.parent, Some(debtors()));
}

#[test]
fn without_a_worker_the_group_waits_for_the_host_and_the_ledger_comes_back() {
    // provenance: derived. With no worker the screen hands its requests to the host, as every
    // other save does: the group goes out as a create request, and the ledger form comes back
    // with Under still empty until the engine names the group.
    let (_api, books) = setup("w2_under_host.ledgerly");
    let mut app = app(books, None);
    let (ctx, mut s) = group_form_over_a_ledger(&mut app);
    let Some(MasterDraft::Group(g)) = s.draft_mut() else {
        panic!("no group form");
    };
    g.name = "Retail Debtors".to_owned();
    g.parent = Some(debtors());
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    assert_eq!(ledger_name(&s), "Ramesh Kirana");
    let requests = s.take_requests();
    assert!(
        matches!(requests.as_slice(), [MasterRequest::Create(_)]),
        "{requests:?}"
    );
    let Some(MasterDraft::Ledger(d)) = s.draft() else {
        panic!("no ledger form");
    };
    assert_eq!(d.group, None);
}

#[test]
fn alt_c_in_a_stock_item_form_opens_nothing() {
    // provenance: the TallyPrime pages read name Alt+C on a ledger's Under field and a voucher's
    // ledger field only (design/PARITY-GAPS.md), so the stock item form keeps its own form.
    let (_api, books) = setup("w2_under_item.ledgerly");
    let mut app = app(books, None);
    let ctx = egui::Context::default();
    let mut s = MastersScreen::with_kind(MasterKind::StockItem);
    s.begin_create(app.books.as_ref().unwrap()).unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::C, Modifiers::ALT));
    assert!(matches!(s.draft(), Some(MasterDraft::StockItem(_))));
}
